//! The write verbs: one file per verb, the shared plumbing here.
//!
//! A write verb is a read plus a local write. Each takes an open
//! [`Store`] and the flight reference as typed; internally it folds
//! `read_all()`, the union of every writer, so a verb can name a flight
//! filed from another machine, resolves, guards, appends, and reads the
//! appended events back off the chain. Validation lives at write time
//! because a verb is the moment a typo is cheap to catch; the fold stays
//! tolerant of what got into the log anyway.
//!
//! Every verb returns an outcome: a serializable payload — the machine
//! envelope's `data`, emitted identically by the CLI's stdout and the
//! server's responses — plus the echo facts a human render needs, so
//! neither surface re-derives what the verb already knew. The refusals
//! live in [`Error`], one table beneath both surfaces. The view verbs
//! carry their noun as a module — `view::edit` beside `edit` — the way
//! the CLI spells `view <verb>`.

mod answer;
mod assign;
mod classify;
mod comment;
mod decompose;
mod edit;
mod error;
mod file;
mod hold;
mod link;
mod pass;
mod status;
pub mod view;

pub use answer::{Answer, Answered, answer};
pub use assign::{Assign, Assigned, assign};
pub use classify::{Fields, Parent, classify};
pub use comment::{Comment, Commented, comment};
pub use decompose::{Decompose, Decomposed, decompose};
pub use edit::{Edit, EditTarget, Edited, Overlay, edit};
pub use error::Error;
pub use file::{File, Filed, file};
pub use hold::{Held, Hold, hold};
pub use link::{Link, Linked, Unlink, Unlinked, link, unlink};
pub use pass::{Conclusion, conclusions, pass};
pub use status::{Move, Moved, cancel, done, status};
pub use view::{Delete, Deleted, Save, Saved, Views};

use crate::board::{Flight, Fold, display};
use crate::log::{self, Event, EventId, Store};

/// The event just appended, read back from this writer's chain so the
/// JSON payload is what the log holds — store-assigned time included —
/// not a reconstruction.
pub fn appended(store: &Store, id: &EventId) -> Result<Event, log::Error> {
    Ok(appended_all(store, std::slice::from_ref(id))?
        .into_iter()
        .next()
        .expect("the appended event is on the chain"))
}

/// The same, for a batch: one read of the chain, the events in the order
/// asked for rather than the chain's.
pub fn appended_all(store: &Store, ids: &[EventId]) -> Result<Vec<Event>, log::Error> {
    let chain = store.read()?;
    Ok(ids
        .iter()
        .map(|id| {
            chain
                .iter()
                .find(|event| &event.id == id)
                .cloned()
                .expect("the appended event is on the chain")
        })
        .collect())
}

/// The flight, refused when it is already closed — done or canceled.
/// The lifecycle verbs stop here; `comment`, `link`, and `edit` stay
/// permissive on purpose — a note on the record is fine, and a wrong
/// word in a closed record is exactly what `edit` is for.
pub fn ensure_active<'a>(fold: &'a Fold, id: &EventId) -> Result<&'a Flight, Error> {
    let flight = crate::board::flight(fold, id);
    if flight.closed() {
        return Err(Error::FlightDone {
            display: display(fold, id),
        });
    }
    Ok(flight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Kind;
    use ff_tower_testsupport::Repo;

    /// A store on a fresh fixture, the writer pinned so ids are stable.
    fn store() -> (Repo, Store) {
        let repo = Repo::new();
        repo.pin_writer("pi");
        let store = Store::open(repo.path()).expect("open");
        (repo, store)
    }

    fn filed(store: &Store, subject: &str) {
        store
            .append(vec![Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: String::new(),
                status: "triage".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            }])
            .expect("append");
    }

    /// One refusal, pinned whole: the id both surfaces match on, the
    /// message both print, and the exits both carry.
    fn pinned(err: &Error, id: &str, message: &str, exits: &[&str]) {
        assert_eq!(err.id(), id);
        assert_eq!(err.to_string(), message);
        assert_eq!(err.exits(), exits);
    }

    #[test]
    fn a_missing_message_is_the_needs_message_refusal_per_verb() {
        let (_repo, store) = store();
        filed(&store, "quiet");
        pinned(
            &hold(&store, "1", None).err().expect("no question"),
            "usage/needs-message",
            "no question given",
            &["ff tower hold <flight> -m <question>"],
        );
        pinned(
            &answer(&store, "1", None).err().expect("no answer"),
            "usage/needs-message",
            "no answer given",
            &["ff tower answer <flight> -m <answer>"],
        );
        pinned(
            &comment(&store, "1", None).err().expect("no note"),
            "usage/needs-message",
            "no note given",
            &["ff tower comment <flight> -m <note>"],
        );
    }

    #[test]
    fn an_empty_subject_or_procedure_refuses_before_the_registry() {
        let (_repo, store) = store();
        pinned(
            &file(&store, "   ", Fields::default(), None)
                .err()
                .expect("empty subject"),
            "usage/empty-subject",
            "the subject is empty",
            &[],
        );
        pinned(
            &file(&store, "a subject", Fields::default(), Some("  "))
                .err()
                .expect("empty name"),
            "usage/empty-procedure",
            "the procedure name is empty",
            &["ff tower procedures"],
        );
    }

    #[test]
    fn a_bad_status_or_lane_refuses_before_the_store() {
        let (_repo, store) = store();
        filed(&store, "standing");
        pinned(
            &status(&store, "1", "claimed", None)
                .err()
                .expect("not a status"),
            "usage/bad-status",
            "`claimed` is not a status — triage, waiting, ready, in_progress, held, done, or canceled",
            &[],
        );
        pinned(
            &assign(&store, "1", "you").err().expect("not a lane"),
            "usage/bad-assignee",
            "`you` is not a lane — me, agent, or none",
            &[],
        );
        pinned(
            &file(
                &store,
                "laned",
                Fields {
                    assignee: Some("pair".to_string()),
                    ..Fields::default()
                },
                None,
            )
            .err()
            .expect("not a lane"),
            "usage/bad-assignee",
            "`pair` is not a lane — me, agent, or none",
            &[],
        );
    }

    #[test]
    fn a_second_hold_is_refused_and_a_held_flight_refuses_a_move() {
        let (_repo, store) = store();
        filed(&store, "contested");
        hold(&store, "1", Some("which way?".to_string())).expect("the hold lands");
        pinned(
            &hold(&store, "1", Some("another?".to_string()))
                .err()
                .expect("held once already"),
            "hold/exists",
            "`#1` is already held: which way?",
            &["ff tower answer <flight> -m <answer>"],
        );
        pinned(
            &status(&store, "1", "ready", None)
                .err()
                .expect("the question stands"),
            "status/held",
            "`#1` is held on a question: which way?",
            &["ff tower answer <flight> -m <answer>"],
        );
        // The two exceptions: closing the flight abandons the question
        // deliberately.
        cancel(&store, "1", None).expect("cancel overrides the hold");
    }

    #[test]
    fn an_answer_to_nothing_refuses() {
        let (_repo, store) = store();
        filed(&store, "untouched");
        pinned(
            &answer(&store, "1", Some("to what?".to_string()))
                .err()
                .expect("no question"),
            "answer/not-held",
            "`#1` has no open question",
            &["ff tower"],
        );
    }

    #[test]
    fn a_closed_flight_refuses_the_lifecycle_in_both_wordings() {
        let (_repo, store) = store();
        filed(&store, "finished");
        done(&store, "1").expect("the finish lands");
        pinned(
            &done(&store, "1").err().expect("done twice"),
            "flight/done",
            "`#1` is already done",
            &["ff tower"],
        );
        pinned(
            &status(&store, "1", "ready", None)
                .err()
                .expect("the record is closed"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["ff tower"],
        );
        pinned(
            &assign(&store, "1", "agent")
                .err()
                .expect("the record is closed"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["ff tower"],
        );
        pinned(
            &cancel(&store, "1", None).err().expect("closed already"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["ff tower"],
        );
    }
}

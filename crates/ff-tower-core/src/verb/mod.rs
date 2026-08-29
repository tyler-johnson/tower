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
//! live in [`Error`], one table beneath both surfaces.

mod answer;
mod claim;
mod classify;
mod comment;
mod done;
mod error;
mod file;
mod hold;
mod requeue;
mod route;
mod take;

pub use answer::{Answer, Answered, answer};
pub use claim::{Claim, Claimed, claim};
pub use classify::{Parent, classify, stamp};
pub use comment::{Comment, Commented, comment};
pub use done::{Done, Finished, done};
pub use error::Error;
pub use file::{File, Filed, file};
pub use hold::{Held, Hold, hold};
pub use requeue::{Requeue, Requeued, requeue};
pub use route::{Route, Routed, route};
pub use take::{Take, Taken, take};

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

/// The flight, refused when it is already done. The lifecycle verbs stop
/// here; `comment`, `link`, and `edit` stay permissive on purpose — a
/// note on the record is fine, and a wrong word in a closed record is
/// exactly what `edit` is for.
pub fn ensure_active<'a>(fold: &'a Fold, id: &EventId) -> Result<&'a Flight, Error> {
    let flight = crate::board::flight(fold, id);
    if flight.done.is_some() {
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
                procedure: "open".to_string(),
                subject: subject.to_string(),
                body: String::new(),
                part: None,
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
            &file(&store, "   ", None, None)
                .err()
                .expect("empty subject"),
            "usage/empty-subject",
            "the subject is empty",
            &[],
        );
        pinned(
            &file(&store, "a subject", None, Some("  ".to_string()))
                .err()
                .expect("empty name"),
            "usage/empty-procedure",
            "`-p` names an empty procedure",
            &["ff tower procedures"],
        );
    }

    #[test]
    fn a_second_claim_take_or_hold_is_refused() {
        let (_repo, store) = store();
        filed(&store, "contested");
        claim(&store, "1").expect("the first claim lands");
        pinned(
            &claim(&store, "1").err().expect("claimed once already"),
            "claim/taken",
            "`#1` is already claimed by tests@tower.invalid",
            &["ff tower", "ff tower next"],
        );
        take(&store, "1").expect("the take overrides");
        pinned(
            &take(&store, "1").err().expect("taken once already"),
            "take/taken",
            "`#1` is already yours",
            &["ff tower requeue <flight>", "ff tower brief <flight>"],
        );
        hold(&store, "1", Some("which way?".to_string())).expect("the hold lands");
        pinned(
            &hold(&store, "1", Some("another?".to_string()))
                .err()
                .expect("held once already"),
            "hold/exists",
            "`#1` is already held: which way?",
            &["ff tower answer <flight> -m <answer>"],
        );
    }

    #[test]
    fn a_requeue_with_nothing_claimed_and_an_answer_to_nothing_refuse() {
        let (_repo, store) = store();
        filed(&store, "untouched");
        pinned(
            &requeue(&store, "1").err().expect("nothing claimed"),
            "requeue/unclaimed",
            "`#1` is not claimed — nothing to hand back",
            &["ff tower", "ff tower next"],
        );
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
    fn a_done_flight_refuses_the_lifecycle_in_both_wordings() {
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
            &claim(&store, "1").err().expect("the record is closed"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["ff tower"],
        );
    }
}

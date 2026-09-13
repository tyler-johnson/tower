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
mod callsign;
mod classify;
mod comment;
mod decompose;
mod edit;
mod error;
mod file;
mod hold;
mod link;
mod status;
pub mod view;

pub use answer::{Answer, Answered, answer};
pub use assign::{Assign, Assigned, assign};
pub use callsign::{Callsign, CallsignData, Left, Moved as Relaned, Took, callsign};
pub use classify::{Fields, Parent, classify};
pub use comment::{Comment, Commented, comment};
pub use decompose::{Decompose, Decomposed, decompose};
pub use edit::{Edit, EditTarget, Edited, Overlay, edit};
pub use error::Error;
pub use file::{File, Filed, file};
pub use hold::{Held, Hold, hold};
pub use link::{Link, Linked, Unlink, Unlinked, link, unlink};
pub use status::{Move, Moved, cancel, done, status};
pub use view::{Delete, Deleted, Save, Saved, Views};

use crate::board::{Flight, Fold, display};
use crate::log::{self, Event, EventId, Kind, Store, usable_callsign};

/// A lane word, validated: `none` is the absent lane spelled out, `me`
/// and `agent` pass, and any other word passes when it is a usable
/// callsign — one word, no spaces. Anything else refuses here, at the
/// boundary where a person is typing; the wire stays a free string.
/// A callsign needs nothing to be a lane: values are open everywhere in
/// tower. Public because `next <lane>` and `next --assignee` read the
/// same words from the CLI.
pub fn lane_word(word: &str) -> Result<Option<String>, Error> {
    match word {
        "none" => Ok(None),
        "me" | "agent" => Ok(Some(word.to_string())),
        other => usable_callsign(other)
            .map(Some)
            .ok_or_else(|| Error::BadAssignee {
                word: other.to_string(),
            }),
    }
}

/// The lane as the log stores it: `me` becomes the caller's callsign
/// when there is one, and stays the literal `me` when there is not.
/// Resolved at the append and never at parse, so a procedure rule
/// saying `assignee = "me"` still matches a `--assignee me` filing.
pub fn stored_lane(word: Option<String>, caller: Option<&str>) -> Option<String> {
    match (word.as_deref(), caller) {
        (Some("me"), Some(callsign)) => Some(callsign.to_string()),
        _ => word,
    }
}

/// [`stored_lane`] over a minted batch: every `Filed` in `kinds` with
/// the literal `me` takes the caller's callsign. `classify` is pure and
/// has no store, so the resolution happens here, after rule matching.
pub(crate) fn resolve_me(mut kinds: Vec<Kind>, caller: Option<&str>) -> Vec<Kind> {
    for kind in &mut kinds {
        if let Kind::Filed { assignee, .. } = kind
            && assignee.as_deref() == Some("me")
        {
            *assignee = stored_lane(assignee.take(), caller);
        }
    }
    kinds
}

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

/// Post-append board rows in response order: the parent, then the new parts.
pub(crate) fn minted_rows(
    store: &Store,
    parent: &EventId,
    parts: &[EventId],
) -> Result<Vec<crate::board::FlightView>, log::Error> {
    let fold = crate::board::fold(&store.read_all()?);
    let rows = crate::board::rows(fold, store.callsign());
    let mut by_id: std::collections::HashMap<_, _> = rows
        .flights
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect();
    Ok(std::iter::once(parent)
        .chain(parts)
        .map(|id| {
            by_id
                .remove(&id.to_string())
                .expect("the minted flight is on the board")
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
    use atc_testsupport::Repo;

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
                status: "backlog".to_string(),
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
            &["atc hold <flight> -m <question>"],
        );
        pinned(
            &answer(&store, "1", None).err().expect("no answer"),
            "usage/needs-message",
            "no answer given",
            &["atc answer <flight> -m <answer>"],
        );
        pinned(
            &comment(&store, "1", None, false).err().expect("no note"),
            "usage/needs-message",
            "no note given",
            &["atc comment <flight> -m <note>"],
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
            &["atc procedures"],
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
            "`claimed` is not a status — backlog, waiting, ready, in_progress, held, done, or canceled",
            &[],
        );
        pinned(
            &assign(&store, "1", "two words").err().expect("not a lane"),
            "usage/bad-assignee",
            "`two words` is not a lane — me, agent, none, or a callsign: one word, no spaces",
            &[],
        );
        pinned(
            &file(
                &store,
                "laned",
                Fields {
                    assignee: Some("two words".to_string()),
                    ..Fields::default()
                },
                None,
            )
            .err()
            .expect("not a lane"),
            "usage/bad-assignee",
            "`two words` is not a lane — me, agent, none, or a callsign: one word, no spaces",
            &[],
        );
    }

    /// The lane grammar: the three words, a callsign, and the refusals
    /// — a spaced word, a blank, and a word past the length.
    #[test]
    fn a_lane_word_is_a_lane_a_callsign_or_a_refusal() {
        assert_eq!(lane_word("none").expect("none"), None);
        assert_eq!(lane_word("me").expect("me").as_deref(), Some("me"));
        assert_eq!(lane_word("agent").expect("agent").as_deref(), Some("agent"));
        assert_eq!(
            lane_word("qwen-review").expect("a callsign").as_deref(),
            Some("qwen-review")
        );
        for bad in ["two words", "", "   ", "a\tb"] {
            assert_eq!(
                lane_word(bad).expect_err("refused").id(),
                "usage/bad-assignee",
                "{bad:?}"
            );
        }
        let long = "x".repeat(65);
        assert!(lane_word(&long).is_err());
    }

    /// `me` resolves at the append: the caller's callsign when there is
    /// one, the literal otherwise; every other word is untouched.
    #[test]
    fn me_stores_the_callers_callsign_when_there_is_one() {
        assert_eq!(
            stored_lane(Some("me".to_string()), Some("tyler")).as_deref(),
            Some("tyler")
        );
        assert_eq!(
            stored_lane(Some("me".to_string()), None).as_deref(),
            Some("me")
        );
        assert_eq!(
            stored_lane(Some("agent".to_string()), Some("tyler")).as_deref(),
            Some("agent")
        );
        assert_eq!(stored_lane(None, Some("tyler")), None);
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
            &["atc answer <flight> -m <answer>"],
        );
        pinned(
            &status(&store, "1", "ready", None)
                .err()
                .expect("the question stands"),
            "status/held",
            "`#1` is held on a question: which way?",
            &["atc answer <flight> -m <answer>"],
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
            &["atc"],
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
            &["atc"],
        );
        pinned(
            &status(&store, "1", "ready", None)
                .err()
                .expect("the record is closed"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["atc"],
        );
        pinned(
            &assign(&store, "1", "agent")
                .err()
                .expect("the record is closed"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["atc"],
        );
        pinned(
            &cancel(&store, "1", None).err().expect("closed already"),
            "flight/done",
            "`#1` is done — the log keeps its record",
            &["atc"],
        );
    }
}

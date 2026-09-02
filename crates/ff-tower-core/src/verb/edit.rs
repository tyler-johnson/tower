//! `edit <target> [-s <subject>] [-m <msg>] [field flags]` — reword a
//! flight's record, or a comment's text by its event id.
//!
//! An overlay event, not a rewrite: the fold applies it last-wins per
//! field and the log keeps every prior word. `labels` replaces the
//! label set wholesale — the set is one value. No `ensure_active` —
//! permissive like `comment` and `link`, because a wrong word in a
//! closed record is the motivating case. An empty message is accepted
//! for the same reason `comment` checks no emptiness: clearing a body
//! or a comment's text is a legitimate edit.

use serde::Serialize;

use crate::board::{self, FlightRef, Fold, display};
use crate::log::{Event, EventId, Kind, Store};

use super::{Error, appended};

/// The edit's payload, as the flags carried it — one field per flag,
/// `None` meaning unchanged. An empty `labels` means unchanged too: the
/// set cannot be cleared, on either surface.
pub struct Overlay {
    pub subject: Option<String>,
    pub message: Option<String>,
    pub priority: Option<String>,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    pub bay: Option<String>,
}

/// Where an edit lands: a flight's record, or one comment on it.
pub enum EditTarget {
    Flight(EventId),
    Comment { flight: EventId, comment: EventId },
}

/// The envelope's `data`: the overlay, as the log holds it.
#[derive(Serialize)]
pub struct Edited {
    pub edited: Event,
}

/// The outcome: the payload, where it landed, and the flight's display
/// either way — the comment's own name is its id on the target.
pub struct Edit {
    pub payload: Edited,
    pub target: EditTarget,
    pub display: String,
}

pub fn edit(store: &Store, target: &str, overlay: Overlay) -> Result<Edit, Error> {
    let labels = if overlay.labels.is_empty() {
        None
    } else {
        Some(overlay.labels)
    };
    if overlay.subject.is_none()
        && overlay.message.is_none()
        && overlay.priority.is_none()
        && labels.is_none()
        && overlay.skill.is_none()
        && overlay.bay.is_none()
    {
        return Err(Error::NeedsEdit);
    }
    // `file`'s discipline: the subject is trimmed, and trimmed to nothing
    // it would put a blank head on the board.
    let subject = overlay.subject.map(|subject| subject.trim().to_string());
    if subject.as_deref() == Some("") {
        return Err(Error::EmptySubject);
    }
    board::parse_ref(target)?;

    let fold = board::fold(&store.read_all()?);
    let target = resolve_edit_target(&fold, target)?;
    let fields_ride = subject.is_some()
        || overlay.priority.is_some()
        || labels.is_some()
        || overlay.skill.is_some()
        || overlay.bay.is_some();
    if fields_ride && matches!(target, EditTarget::Comment { .. }) {
        return Err(Error::SubjectOnComment);
    }

    let (edited, flight) = match &target {
        EditTarget::Flight(flight) => (flight.clone(), flight.clone()),
        EditTarget::Comment { flight, comment } => (comment.clone(), flight.clone()),
    };
    let ids = store.append(vec![Kind::Edited {
        target: edited,
        subject,
        body: overlay.message,
        priority: overlay.priority,
        labels,
        skill: overlay.skill,
        bay: overlay.bay,
    }])?;
    let id = ids.into_iter().next().expect("one edited event");

    Ok(Edit {
        payload: Edited {
            edited: appended(store, &id)?,
        },
        target,
        display: display(&fold, &flight),
    })
}

/// Resolve `edit`'s target: flights by any reference form, comments by
/// their full event id alone — the wire id is a comment's only name. A
/// sibling of `resolve` rather than a change to it, because every other
/// verb must keep refusing comment ids.
fn resolve_edit_target(fold: &Fold, text: &str) -> Result<EditTarget, Error> {
    match board::parse_ref(text)? {
        FlightRef::Full(id) => {
            if fold.flights.iter().any(|flight| flight.id == id) {
                return Ok(EditTarget::Flight(id));
            }
            for flight in &fold.flights {
                if flight.comments.iter().any(|comment| comment.id == id) {
                    return Ok(EditTarget::Comment {
                        flight: flight.id.clone(),
                        comment: id,
                    });
                }
            }
            Err(Error::EditTargetNotFound {
                text: text.to_string(),
            })
        }
        _ => Ok(EditTarget::Flight(board::resolve(fold, text)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff_tower_testsupport::Repo;

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

    fn nothing() -> Overlay {
        Overlay {
            subject: None,
            message: None,
            priority: None,
            labels: Vec::new(),
            skill: None,
            bay: None,
        }
    }

    fn pinned(err: &Error, id: &str, message: &str, exits: &[&str]) {
        assert_eq!(err.id(), id);
        assert_eq!(err.to_string(), message);
        assert_eq!(err.exits(), exits);
    }

    #[test]
    fn no_flags_and_a_blank_subject_refuse_before_the_store() {
        let (_repo, store) = store();
        filed(&store, "as filed");
        pinned(
            &edit(&store, "1", nothing())
                .err()
                .expect("nothing to change"),
            "usage/needs-edit",
            "nothing to change — `-s` rewords the subject, `-m` the body or comment text, and \
             `--priority`, `--label`, `--skill`, `--bay` reset a field",
            &[
                "ff tower edit <target> -s <subject>",
                "ff tower edit <target> -m <msg>",
            ],
        );
        pinned(
            &edit(
                &store,
                "1",
                Overlay {
                    subject: Some("   ".to_string()),
                    ..nothing()
                },
            )
            .err()
            .expect("a blank subject"),
            "usage/empty-subject",
            "the subject is empty",
            &[],
        );
    }

    #[test]
    fn a_field_on_a_comment_refuses_and_a_message_rewords_it() {
        let (_repo, store) = store();
        filed(&store, "commented on");
        let comment = super::super::comment(&store, "1", Some("a note".to_string()))
            .expect("the note lands")
            .payload
            .commented
            .id;
        pinned(
            &edit(
                &store,
                &comment.to_string(),
                Overlay {
                    priority: Some("high".to_string()),
                    ..nothing()
                },
            )
            .err()
            .expect("a comment carries no fields"),
            "usage/subject-on-comment",
            "a comment carries no fields — `-m` rewords its text",
            &["ff tower edit <target> -m <msg>"],
        );
        let outcome = edit(
            &store,
            &comment.to_string(),
            Overlay {
                message: Some("reworded".to_string()),
                ..nothing()
            },
        )
        .expect("the reword lands");
        assert!(matches!(
            &outcome.payload.edited.kind,
            Kind::Edited { target, .. } if *target == comment
        ));
        assert!(matches!(outcome.target, EditTarget::Comment { .. }));
        assert_eq!(outcome.display, "#1");
    }

    #[test]
    fn a_full_id_naming_nothing_is_neither_a_flight_nor_a_comment() {
        let (_repo, store) = store();
        filed(&store, "alone");
        pinned(
            &edit(
                &store,
                "pi.99",
                Overlay {
                    subject: Some("s".to_string()),
                    ..nothing()
                },
            )
            .err()
            .expect("names nothing"),
            "flight/not-found",
            "`pi.99` names neither a flight nor a comment",
            &["ff tower"],
        );
    }
}

//! The history: what happened to one flight, read off the log.
//!
//! The fold keeps last-wins marks, not events — a flight claimed and
//! requeued three times carries one `claim` and one `requeued_at` — so
//! the moments cannot come from [`Flight`](super::Flight). They come from
//! the events the fold was built out of, filtered to the ones that name
//! this flight, in log order.
//!
//! Deliberately thin. A moment says who did what, when, and nothing else:
//! the comment's text, the open question, the route's `because` already
//! sit flat on the [`Brief`](super::Brief), and a history that repeated
//! them would be a second copy of the record to keep in step with the
//! first.

use serde::{Deserialize, Serialize};

use crate::log::{Event, EventId, Kind};

/// One gesture on a flight's record.
#[derive(Debug, Serialize)]
pub struct Moment {
    /// The event's wire id.
    pub id: String,
    pub at: i64,
    /// The event's author, verbatim — never assumed to be the reader.
    pub by: String,
    /// The kind's own name; an unknown kind carries its own string
    /// through.
    pub what: String,
}

/// The loose read of an unknown kind's body: the four field names every
/// known kind uses to name a flight. Unknown fields are ignored, so a
/// newer tower's gesture is judged on what this one can recognize and
/// degrades to a labeled row rather than vanishing — which is what
/// [`Kind::Unknown`]'s own doc promises the fold. A body whose `flight`
/// is not a wire id at all fails this parse and is skipped: best-effort
/// is the whole contract here, and guessing would be worse.
#[derive(Deserialize)]
struct Names {
    #[serde(default)]
    flight: Option<EventId>,
    #[serde(default)]
    target: Option<EventId>,
    #[serde(default)]
    from: Option<EventId>,
    #[serde(default)]
    to: Option<EventId>,
}

/// Every event naming this flight, oldest first — the reading order
/// [`Brief::comments`](super::Brief::comments) already uses.
///
/// `Edited` lands on a comment's event id as well as the flight's own: a
/// reword is a gesture on the flight, and the log's order means a
/// comment's id is always known before an edit can target it.
pub fn history(events: &[Event], flight: &EventId) -> Vec<Moment> {
    let mut comments: Vec<&EventId> = Vec::new();
    let mut moments = Vec::new();
    for event in events {
        let names = match &event.kind {
            // A filing mints the flight: the event's own id is the name.
            Kind::Filed { .. } => &event.id == flight,
            Kind::Status { flight: on, .. }
            | Kind::Assigned { flight: on, .. }
            | Kind::Commented { flight: on, .. }
            | Kind::Held { flight: on, .. }
            | Kind::Answered { flight: on, .. } => on == flight,
            Kind::Edited { target, .. } => target == flight || comments.contains(&target),
            Kind::Linked { from, to } => from == flight || to == flight,
            Kind::Unknown { body, .. } => {
                serde_json::from_str::<Names>(body.get()).is_ok_and(|names| {
                    [names.flight, names.target, names.from, names.to]
                        .iter()
                        .flatten()
                        .any(|id| id == flight)
                })
            }
        };
        if let Kind::Commented { flight: on, .. } = &event.kind
            && on == flight
        {
            comments.push(&event.id);
        }
        if names {
            moments.push(Moment {
                id: event.id.to_string(),
                at: event.time,
                by: event.author.clone(),
                what: event.kind.name().to_string(),
            });
        }
    }
    moments
}

//! The fold: the log's events, partitioned into flights.
//!
//! Pure by construction — this file imports the log's types and nothing
//! else. No `crate::ff`, no `std::process`: the compiler is what keeps
//! principle 11 checkable rather than aspirational. Everything a flight
//! needs from the repository arrives later, in `enrich`, over data the
//! caller already fetched.

use std::collections::HashMap;

use crate::log::{Event, EventId, Kind};

/// One flight, assembled from its `filed` event and everything that named
/// it since.
#[derive(Debug, Clone)]
pub struct Flight {
    /// The filed event's id — the flight's identity, and the string that
    /// rides every fufu call as `--session`.
    pub id: EventId,
    pub procedure: String,
    pub subject: String,
    pub body: String,
    pub filed_by: String,
    pub filed_at: i64,
    /// Reading order — the union's order, which is the order a reader saw
    /// them arrive in.
    pub comments: Vec<Comment>,
    /// `Linked { from: this, to: X }` — this flight depends on X.
    pub depends_on: Vec<EventId>,
    /// The reverse edge, folded in here so a render never has to scan
    /// every other flight to answer "what waits on this one".
    pub blocks: Vec<EventId>,
}

/// A note on a flight's record.
#[derive(Debug, Clone)]
pub struct Comment {
    pub id: EventId,
    pub author: String,
    pub at: i64,
    pub text: String,
}

/// What the fold produced: every flight, and every event it could not
/// route.
#[derive(Debug)]
pub struct Fold {
    /// Filed order.
    pub flights: Vec<Flight>,
    /// Comments and links naming a flight never filed, and unknown kinds
    /// wholesale. Carried, never dropped, never an error — a fold that
    /// refused data it half-understood would flicker between board and
    /// error as a newer tower's events roll in.
    pub unrouted: Vec<Event>,
}

/// Fold the union into flights. Two passes, and that is load-bearing: the
/// union orders by `(time, writer, seq)` and wall clocks disagree across
/// machines, so a comment from a fast clock can sort *before* the filing
/// it names. Pass 1 mints every flight; pass 2 attaches to flights that
/// exist anywhere in the log, not merely earlier in it.
pub fn fold(events: &[Event]) -> Fold {
    let mut flights: Vec<Flight> = Vec::new();
    let mut by_id: HashMap<&EventId, usize> = HashMap::new();
    let mut unrouted: Vec<Event> = Vec::new();

    for event in events {
        if let Kind::Filed {
            procedure,
            subject,
            body,
        } = &event.kind
        {
            // A duplicate filed id is unreachable by construction — ids
            // are unique per writer, writers cannot collide — but a
            // hand-edited log must not panic: first filing wins.
            if by_id.contains_key(&event.id) {
                unrouted.push(event.clone());
                continue;
            }
            by_id.insert(&event.id, flights.len());
            flights.push(Flight {
                id: event.id.clone(),
                procedure: procedure.clone(),
                subject: subject.clone(),
                body: body.clone(),
                filed_by: event.author.clone(),
                filed_at: event.time,
                comments: Vec::new(),
                depends_on: Vec::new(),
                blocks: Vec::new(),
            });
        }
    }

    for event in events {
        match &event.kind {
            Kind::Filed { .. } => {}
            Kind::Commented { flight, text } => match by_id.get(flight) {
                Some(&at) => flights[at].comments.push(Comment {
                    id: event.id.clone(),
                    author: event.author.clone(),
                    at: event.time,
                    text: text.clone(),
                }),
                None => unrouted.push(event.clone()),
            },
            Kind::Linked { from, to } => match (by_id.get(from).copied(), by_id.get(to).copied()) {
                (Some(dependent), Some(dependency)) => {
                    flights[dependent].depends_on.push(to.clone());
                    flights[dependency].blocks.push(from.clone());
                }
                _ => unrouted.push(event.clone()),
            },
            Kind::Unknown { .. } => unrouted.push(event.clone()),
        }
    }

    Fold { flights, unrouted }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind,
        }
    }

    fn filed(id: &str, time: i64, subject: &str) -> Event {
        event(
            id,
            time,
            Kind::Filed {
                procedure: "open".to_string(),
                subject: subject.to_string(),
                body: String::new(),
            },
        )
    }

    fn commented(id: &str, time: i64, flight: &str) -> Event {
        event(
            id,
            time,
            Kind::Commented {
                flight: flight.parse().expect("id"),
                text: "a note".to_string(),
            },
        )
    }

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        event(
            id,
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
            },
        )
    }

    #[test]
    fn a_comment_attaches_to_its_flight() {
        let fold = fold(&[filed("pi.1", 10, "s"), commented("pi.2", 20, "pi.1")]);
        assert_eq!(fold.flights.len(), 1);
        assert_eq!(fold.flights[0].comments.len(), 1);
        assert_eq!(fold.flights[0].comments[0].text, "a note");
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_comment_delivered_ahead_of_its_filing_still_attaches() {
        // The union sorts by wall clock, and a fast clock can put the
        // comment first. One forward pass would orphan it; two must not.
        let fold = fold(&[commented("fast.1", 5, "pi.1"), filed("pi.1", 10, "s")]);
        assert_eq!(fold.flights[0].comments.len(), 1);
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_comment_for_a_flight_never_filed_is_unrouted() {
        let fold = fold(&[commented("pi.1", 10, "pi.99")]);
        assert!(fold.flights.is_empty());
        assert_eq!(fold.unrouted.len(), 1);
    }

    #[test]
    fn a_link_tracks_both_directions() {
        let fold = fold(&[
            filed("pi.1", 10, "dependency"),
            filed("pi.2", 20, "dependent"),
            linked("pi.3", 30, "pi.2", "pi.1"),
        ]);
        assert_eq!(fold.flights[1].depends_on, ["pi.1".parse().expect("id")]);
        assert_eq!(fold.flights[0].blocks, ["pi.2".parse().expect("id")]);
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_link_with_a_missing_endpoint_is_unrouted() {
        let fold = fold(&[filed("pi.1", 10, "s"), linked("pi.2", 20, "pi.1", "pi.99")]);
        assert!(fold.flights[0].depends_on.is_empty());
        assert!(fold.flights[0].blocks.is_empty());
        assert_eq!(fold.unrouted.len(), 1);
    }

    #[test]
    fn an_unknown_kind_is_carried_without_error() {
        let future = event(
            "pi.2",
            20,
            Kind::Unknown {
                kind: "claimed".to_string(),
                body: serde_json::value::to_raw_value(&serde_json::json!({"flight": "pi.1"}))
                    .expect("raw"),
            },
        );
        let fold = fold(&[filed("pi.1", 10, "s"), future]);
        assert_eq!(fold.flights.len(), 1);
        assert_eq!(fold.unrouted.len(), 1);
        assert!(matches!(&fold.unrouted[0].kind, Kind::Unknown { kind, .. } if kind == "claimed"));
    }

    #[test]
    fn an_empty_log_folds_to_an_empty_fold() {
        let fold = fold(&[]);
        assert!(fold.flights.is_empty());
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn the_fold_is_deterministic() {
        let events = [
            filed("pi.1", 10, "a"),
            filed("qi.1", 12, "b"),
            commented("pi.2", 20, "qi.1"),
            linked("pi.3", 30, "pi.1", "qi.1"),
            commented("pi.4", 40, "zz.9"),
        ];
        let once = fold(&events);
        let twice = fold(&events);
        assert_eq!(format!("{once:?}"), format!("{twice:?}"));
    }

    #[test]
    fn a_duplicate_filed_id_does_not_panic_and_the_first_wins() {
        let fold = fold(&[filed("pi.1", 10, "first"), filed("pi.1", 20, "second")]);
        assert_eq!(fold.flights.len(), 1);
        assert_eq!(fold.flights[0].subject, "first");
        assert_eq!(fold.unrouted.len(), 1);
    }
}

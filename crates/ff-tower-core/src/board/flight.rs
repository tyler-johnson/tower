//! The fold: the log's events, partitioned into flights, each carrying
//! its dense per-writer number — the human name's numeric half.
//!
//! Pure by construction — this file imports the log's types and nothing
//! else. No `crate::ff`, no `std::process`: the compiler is what keeps
//! principle 11 checkable rather than aspirational. Everything a flight
//! needs from the repository arrives later, in `enrich`, over data the
//! caller already fetched.

use std::collections::HashMap;

use crate::log::{Event, EventId, Kind, PartStamp};

/// One flight, assembled from its `filed` event and everything that named
/// it since.
#[derive(Debug, Clone)]
pub struct Flight {
    /// The filed event's id — the flight's identity, and the string that
    /// rides every fufu call as `--session`.
    pub id: EventId,
    /// The flight's human name: its 1-based rank among this writer's
    /// filed events. Dense where event seqs are sparse — every event
    /// kind consumes a seq, only filings consume a number. Derived here,
    /// never stored; the log is append-only and per-writer seqs are
    /// monotonic, so a new filing can never renumber an earlier one.
    pub number: u64,
    pub procedure: String,
    /// The procedure part this flight is, as the filing stamped it.
    /// Carried, never derived from: nothing in the fold keys on crew this
    /// slice, and the definition it came from is not read again.
    pub part: Option<PartStamp>,
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
    /// The last claim, when one stands — last wins, a reassignment.
    pub claim: Option<Mark>,
    /// The open question, when the flight is held. Cleared by `answered`.
    pub question: Option<Question>,
    /// Set once `done` lands; the partition drops the flight on it.
    pub done: Option<Mark>,
    /// The freshest answer's time — an answer counts as motion even
    /// though its text lives only in the log.
    pub answered_at: Option<i64>,
}

/// Who made a lifecycle mark, and when.
#[derive(Debug, Clone)]
pub struct Mark {
    pub by: String,
    pub at: i64,
}

/// The question a held flight is waiting on.
#[derive(Debug, Clone)]
pub struct Question {
    pub by: String,
    pub at: i64,
    pub text: String,
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
/// exist anywhere in the log, not merely earlier in it. A numbering
/// post-pass then ranks each writer's filings by seq — by seq and not
/// union order, so a clock step can reorder the union without ever
/// renumbering a flight.
pub fn fold(events: &[Event]) -> Fold {
    let mut flights: Vec<Flight> = Vec::new();
    let mut by_id: HashMap<&EventId, usize> = HashMap::new();
    let mut unrouted: Vec<Event> = Vec::new();

    for event in events {
        if let Kind::Filed {
            procedure,
            subject,
            body,
            part,
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
                number: 0,
                procedure: procedure.clone(),
                part: part.clone(),
                subject: subject.clone(),
                body: body.clone(),
                filed_by: event.author.clone(),
                filed_at: event.time,
                comments: Vec::new(),
                depends_on: Vec::new(),
                blocks: Vec::new(),
                claim: None,
                question: None,
                done: None,
                answered_at: None,
            });
        }
    }

    let mut per_writer: HashMap<String, Vec<usize>> = HashMap::new();
    for (at, flight) in flights.iter().enumerate() {
        per_writer
            .entry(flight.id.writer.clone())
            .or_default()
            .push(at);
    }
    for mut group in per_writer.into_values() {
        group.sort_by_key(|&at| flights[at].id.seq);
        for (rank, &at) in group.iter().enumerate() {
            flights[at].number = rank as u64 + 1;
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
            Kind::Claimed { flight } => match by_id.get(flight) {
                Some(&at) => {
                    flights[at].claim = Some(Mark {
                        by: event.author.clone(),
                        at: event.time,
                    })
                }
                None => unrouted.push(event.clone()),
            },
            Kind::Held { flight, question } => match by_id.get(flight) {
                Some(&at) => {
                    flights[at].question = Some(Question {
                        by: event.author.clone(),
                        at: event.time,
                        text: question.clone(),
                    })
                }
                None => unrouted.push(event.clone()),
            },
            // Log order, not causal order: cross-writer clock skew can
            // sort an `answered` before the `held` it answers, leaving
            // that question open until re-answered. Accepted — it heals
            // itself, and seq order makes it impossible single-writer.
            Kind::Answered { flight, .. } => match by_id.get(flight) {
                Some(&at) => {
                    let flight = &mut flights[at];
                    flight.question = None;
                    flight.answered_at = Some(
                        flight
                            .answered_at
                            .map_or(event.time, |at| at.max(event.time)),
                    );
                }
                None => unrouted.push(event.clone()),
            },
            Kind::Done { flight } => match by_id.get(flight) {
                Some(&at) => {
                    flights[at].done = Some(Mark {
                        by: event.author.clone(),
                        at: event.time,
                    })
                }
                None => unrouted.push(event.clone()),
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
                part: None,
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

    fn claimed(id: &str, time: i64, flight: &str) -> Event {
        event(
            id,
            time,
            Kind::Claimed {
                flight: flight.parse().expect("id"),
            },
        )
    }

    fn held(id: &str, time: i64, flight: &str, question: &str) -> Event {
        event(
            id,
            time,
            Kind::Held {
                flight: flight.parse().expect("id"),
                question: question.to_string(),
            },
        )
    }

    fn answered(id: &str, time: i64, flight: &str) -> Event {
        event(
            id,
            time,
            Kind::Answered {
                flight: flight.parse().expect("id"),
                answer: "an answer".to_string(),
            },
        )
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        event(
            id,
            time,
            Kind::Done {
                flight: flight.parse().expect("id"),
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
                kind: "promoted".to_string(),
                body: serde_json::value::to_raw_value(&serde_json::json!({"flight": "pi.1"}))
                    .expect("raw"),
            },
        );
        let fold = fold(&[filed("pi.1", 10, "s"), future]);
        assert_eq!(fold.flights.len(), 1);
        assert_eq!(fold.unrouted.len(), 1);
        assert!(matches!(&fold.unrouted[0].kind, Kind::Unknown { kind, .. } if kind == "promoted"));
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

    #[test]
    fn a_claim_records_its_author_and_time_and_the_last_claim_wins() {
        let once = fold(&[filed("pi.1", 10, "s"), claimed("pi.2", 20, "pi.1")]);
        let claim = once.flights[0].claim.as_ref().expect("claimed");
        assert_eq!(claim.by, "a@b.c");
        assert_eq!(claim.at, 20);

        let twice = fold(&[
            filed("pi.1", 10, "s"),
            claimed("pi.2", 20, "pi.1"),
            claimed("qi.1", 30, "pi.1"),
        ]);
        assert_eq!(twice.flights[0].claim.as_ref().expect("claimed").at, 30);
    }

    #[test]
    fn a_hold_sets_the_question() {
        let fold = fold(&[filed("pi.1", 10, "s"), held("pi.2", 20, "pi.1", "which?")]);
        let question = fold.flights[0].question.as_ref().expect("held");
        assert_eq!(question.text, "which?");
        assert_eq!(question.by, "a@b.c");
        assert_eq!(question.at, 20);
    }

    #[test]
    fn an_answer_clears_the_question_and_is_not_a_comment() {
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            held("pi.2", 20, "pi.1", "which?"),
            answered("pi.3", 30, "pi.1"),
        ]);
        assert!(fold.flights[0].question.is_none());
        assert_eq!(fold.flights[0].answered_at, Some(30));
        assert!(fold.flights[0].comments.is_empty());
    }

    #[test]
    fn an_answer_with_no_open_question_is_a_silent_no_op() {
        let fold = fold(&[filed("pi.1", 10, "s"), answered("pi.2", 20, "pi.1")]);
        assert!(fold.flights[0].question.is_none());
        assert_eq!(fold.flights[0].answered_at, Some(20));
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn cross_writer_skew_can_leave_a_question_open() {
        // The accepted wart, pinned: a fast clock sorts the `answered`
        // before the `held` it answers, so the log-order fold sees the
        // hold last and the question stands until re-answered. Impossible
        // single-writer — seq order is causal there.
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            answered("fast.1", 15, "pi.1"),
            held("pi.2", 20, "pi.1", "which?"),
        ]);
        assert!(fold.flights[0].question.is_some());
    }

    #[test]
    fn a_lifecycle_event_for_a_flight_never_filed_is_unrouted() {
        let fold = fold(&[
            claimed("pi.1", 10, "zz.9"),
            held("pi.2", 20, "zz.9", "q"),
            answered("pi.3", 30, "zz.9"),
            done("pi.4", 40, "zz.9"),
        ]);
        assert!(fold.flights.is_empty());
        assert_eq!(fold.unrouted.len(), 4);
    }

    #[test]
    fn numbers_count_filings_alone_and_stay_dense() {
        let fold = fold(&[
            filed("pi.1", 10, "first"),
            claimed("pi.2", 20, "pi.1"),
            commented("pi.3", 30, "pi.1"),
            filed("pi.4", 40, "second"),
        ]);
        let pairs: Vec<(u64, u64)> = fold
            .flights
            .iter()
            .map(|flight| (flight.id.seq, flight.number))
            .collect();
        assert_eq!(pairs, [(1, 1), (4, 2)]);
    }

    #[test]
    fn two_interleaved_writers_each_number_from_one() {
        let fold = fold(&[
            filed("pi.1", 10, "a"),
            filed("qi.1", 20, "b"),
            filed("pi.2", 30, "c"),
            filed("qi.2", 40, "d"),
        ]);
        let numbers: Vec<(String, u64)> = fold
            .flights
            .iter()
            .map(|flight| (flight.id.to_string(), flight.number))
            .collect();
        assert_eq!(
            numbers,
            [
                ("pi.1".to_string(), 1),
                ("qi.1".to_string(), 1),
                ("pi.2".to_string(), 2),
                ("qi.2".to_string(), 2),
            ]
        );
    }

    #[test]
    fn a_done_flight_keeps_its_number_and_the_next_filing_counts_on() {
        let fold = fold(&[
            filed("pi.1", 10, "finished"),
            done("pi.2", 20, "pi.1"),
            filed("pi.3", 30, "next"),
        ]);
        assert_eq!(fold.flights[0].number, 1);
        assert!(fold.flights[0].done.is_some());
        assert_eq!(fold.flights[1].number, 2);
    }

    #[test]
    fn done_records_its_author_and_time() {
        let fold = fold(&[filed("pi.1", 10, "s"), done("pi.2", 20, "pi.1")]);
        let mark = fold.flights[0].done.as_ref().expect("done");
        assert_eq!(mark.by, "a@b.c");
        assert_eq!(mark.at, 20);
    }
}

//! The fold: the log's events, partitioned into flights, each carrying
//! its dense per-writer number — the human name's numeric half.
//!
//! Pure by construction — this file imports the log's types and nothing
//! else. No `crate::ff`, no `std::process`: the compiler is what keeps
//! principle 11 checkable rather than aspirational. Everything a flight
//! needs from the repository arrives later, in `enrich`, over data the
//! caller already fetched.
//!
//! The fold stores the wire's free strings — status, assignee, priority
//! — and never rounds them into enums: the closed vocabularies live at
//! the verb and loader boundaries, so a value this binary has never
//! heard of survives the fold intact and simply matches nothing.

use std::collections::HashMap;

use crate::log::{Event, EventId, Kind};

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
    /// Provenance only: the procedure the filing was minted under, when
    /// there was one. Nothing derives from it.
    pub procedure: Option<String>,
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
    /// The stored status, last-wins: the filing seeds it, `status`
    /// events overwrite it, `held` and `answered` move it too.
    pub status: String,
    /// Who last moved the status, and when — `None` while the flight
    /// still stands where it was filed.
    pub status_mark: Option<Mark>,
    /// The stored lane, last-wins; `assigned` overwrites it, absent
    /// clears it.
    pub assignee: Option<String>,
    pub priority: String,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    pub bay: Option<String>,
    /// What finishing means, as the filing stamped it.
    pub done_kind: String,
    /// The branch the filing resolved for this flight, when a
    /// definition's `subject = "branch"` said so.
    pub branch_stamp: Option<String>,
    /// The open question, when the flight is held. Cleared by `answered`.
    pub question: Option<Question>,
    /// The last edit touching this flight's record — its own fields or a
    /// comment's text, either target type. A reword is a gesture on the
    /// flight, so it counts as motion.
    pub edited: Option<Mark>,
    /// The freshest answer's time — an answer counts as motion even
    /// though its text lives only in the log.
    pub answered_at: Option<i64>,
}

impl Flight {
    /// Off the board: done or canceled. Exact compares — an unknown
    /// status never rounds into closed.
    pub fn closed(&self) -> bool {
        self.status == "done" || self.status == "canceled"
    }

    /// Whether the pool admits this flight: Ready, in the agent lane.
    /// Exact string compares on the stored fields — unknown never rounds
    /// down. One method because `pick` and `brief` must agree on it, and
    /// two copies of a gate is where drift starts.
    pub fn pullable(&self) -> bool {
        self.status == "ready" && self.assignee.as_deref() == Some("agent")
    }
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

/// Fold the union into flights. Three passes, and that is load-bearing:
/// the union orders by `(time, writer, seq)` and wall clocks disagree
/// across machines, so a comment from a fast clock can sort *before* the
/// filing it names. Pass 1 mints every flight; pass 2 attaches to flights
/// that exist anywhere in the log, not merely earlier in it; pass 3
/// applies edits after every comment exists, because an edit can name a
/// comment and ride the same skew. A numbering post-pass then ranks each
/// writer's filings by seq — by seq and not union order, so a clock step
/// can reorder the union without ever renumbering a flight.
pub fn fold(events: &[Event]) -> Fold {
    let mut flights: Vec<Flight> = Vec::new();
    let mut by_id: HashMap<&EventId, usize> = HashMap::new();
    let mut unrouted: Vec<Event> = Vec::new();

    for event in events {
        if let Kind::Filed {
            procedure,
            subject,
            body,
            status,
            assignee,
            priority,
            labels,
            skill,
            bay,
            done,
            branch,
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
                subject: subject.clone(),
                body: body.clone(),
                filed_by: event.author.clone(),
                filed_at: event.time,
                comments: Vec::new(),
                depends_on: Vec::new(),
                blocks: Vec::new(),
                status: status.clone(),
                status_mark: None,
                assignee: assignee.clone(),
                priority: priority.clone(),
                labels: labels.clone(),
                skill: skill.clone(),
                bay: bay.clone(),
                done_kind: done.clone(),
                branch_stamp: branch.clone(),
                question: None,
                edited: None,
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

    let mut edits: Vec<&Event> = Vec::new();
    for event in events {
        match &event.kind {
            Kind::Filed { .. } => {}
            // Held for pass 3: pass 2 attaches comments in union order,
            // so clock skew could sort an edit before the comment it
            // names — the same skew pass 1 absorbs for flights.
            Kind::Edited { .. } => edits.push(event),
            Kind::Status { flight, status, .. } => match by_id.get(flight) {
                Some(&at) => {
                    let flight = &mut flights[at];
                    flight.status = status.clone();
                    flight.status_mark = Some(Mark {
                        by: event.author.clone(),
                        at: event.time,
                    });
                }
                None => unrouted.push(event.clone()),
            },
            Kind::Assigned { flight, assignee } => match by_id.get(flight) {
                Some(&at) => flights[at].assignee = assignee.clone(),
                None => unrouted.push(event.clone()),
            },
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
            // The hold is a status move as well as a question: the flight
            // reads `held` wherever status is read, and the question is
            // what the release answers.
            Kind::Held { flight, question } => match by_id.get(flight) {
                Some(&at) => {
                    let flight = &mut flights[at];
                    flight.question = Some(Question {
                        by: event.author.clone(),
                        at: event.time,
                        text: question.clone(),
                    });
                    flight.status = "held".to_string();
                    flight.status_mark = Some(Mark {
                        by: event.author.clone(),
                        at: event.time,
                    });
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
                    flight.status = "ready".to_string();
                    flight.status_mark = Some(Mark {
                        by: event.author.clone(),
                        at: event.time,
                    });
                    flight.answered_at = Some(
                        flight
                            .answered_at
                            .map_or(event.time, |at| at.max(event.time)),
                    );
                }
                None => unrouted.push(event.clone()),
            },
            Kind::Unknown { .. } => unrouted.push(event.clone()),
        }
    }

    // Pass 3: edits, in union order — last-wins is preserved, and every
    // comment already exists. Per-field overlay: concurrent partial edits
    // from two writers both land, and an absent field keeps the last
    // value standing. Labels replace wholesale — the set is one value. A
    // child's derived subject (`"{subject} · {id}"`, composed at file
    // time) never changes when a parent is reworded — each child is its
    // own flight.
    for event in edits {
        let Kind::Edited {
            target,
            subject,
            body,
            priority,
            labels,
            skill,
            bay,
        } = &event.kind
        else {
            unreachable!("pass 2 collected only edits");
        };
        let mark = Mark {
            by: event.author.clone(),
            at: event.time,
        };
        if let Some(&at) = by_id.get(target) {
            let flight = &mut flights[at];
            if let Some(subject) = subject {
                flight.subject = subject.clone();
            }
            if let Some(body) = body {
                flight.body = body.clone();
            }
            if let Some(priority) = priority {
                flight.priority = priority.clone();
            }
            if let Some(labels) = labels {
                flight.labels = labels.clone();
            }
            if let Some(skill) = skill {
                flight.skill = Some(skill.clone());
            }
            if let Some(bay) = bay {
                flight.bay = Some(bay.clone());
            }
            flight.edited = Some(mark);
        } else if let Some(at) = flights
            .iter()
            .position(|flight| flight.comments.iter().any(|comment| &comment.id == target))
        {
            // `subject` on a comment target is the tolerant-fold rule:
            // the verb refuses it, the fold shrugs. The mark still lands
            // — a comment reword is a gesture on the flight.
            let flight = &mut flights[at];
            if let Some(body) = body {
                let comment = flight
                    .comments
                    .iter_mut()
                    .find(|comment| &comment.id == target)
                    .expect("the position found this comment");
                comment.text = body.clone();
            }
            flight.edited = Some(mark);
        } else {
            unrouted.push(event.clone());
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

    /// A bare filing — Triage, no lane, default fields.
    fn filed(id: &str, time: i64, subject: &str) -> Event {
        event(
            id,
            time,
            Kind::Filed {
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
            },
        )
    }

    /// A filing born pullable — Ready, agent lane, the pool's norm.
    fn filed_agent(id: &str, time: i64, subject: &str) -> Event {
        event(
            id,
            time,
            Kind::Filed {
                procedure: Some("review".to_string()),
                subject: subject.to_string(),
                body: String::new(),
                status: "ready".to_string(),
                assignee: Some("agent".to_string()),
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: Some("review".to_string()),
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        )
    }

    fn status(id: &str, time: i64, flight: &str, to: &str) -> Event {
        event(
            id,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
            },
        )
    }

    fn assigned(id: &str, time: i64, flight: &str, lane: Option<&str>) -> Event {
        event(
            id,
            time,
            Kind::Assigned {
                flight: flight.parse().expect("id"),
                assignee: lane.map(str::to_string),
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
        status(id, time, flight, "done")
    }

    fn edited(
        id: &str,
        time: i64,
        target: &str,
        subject: Option<&str>,
        body: Option<&str>,
    ) -> Event {
        event(
            id,
            time,
            Kind::Edited {
                target: target.parse().expect("id"),
                subject: subject.map(str::to_string),
                body: body.map(str::to_string),
                priority: None,
                labels: None,
                skill: None,
                bay: None,
            },
        )
    }

    #[test]
    fn a_filing_seeds_every_stored_field() {
        let fold = fold(&[filed_agent("pi.1", 10, "s")]);
        let flight = &fold.flights[0];
        assert_eq!(flight.procedure.as_deref(), Some("review"));
        assert_eq!(flight.status, "ready");
        assert!(flight.status_mark.is_none(), "unmoved since filing");
        assert_eq!(flight.assignee.as_deref(), Some("agent"));
        assert_eq!(flight.priority, "none");
        assert!(flight.labels.is_empty());
        assert_eq!(flight.skill.as_deref(), Some("review"));
        assert_eq!(flight.done_kind, "asserted");
        assert!(flight.pullable());
        assert!(!flight.closed());
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
    fn a_status_move_overwrites_last_wins_with_its_mark() {
        let mut late = status("qi.1", 30, "pi.1", "ready");
        late.author = "mover@b.c".to_string();
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            status("pi.2", 20, "pi.1", "in_progress"),
            late,
        ]);
        let flight = &fold.flights[0];
        assert_eq!(flight.status, "ready");
        let mark = flight.status_mark.as_ref().expect("moved");
        assert_eq!(mark.by, "mover@b.c");
        assert_eq!(mark.at, 30);
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn an_unknown_status_is_stored_and_never_rounds_down() {
        let fold = fold(&[
            filed_agent("pi.1", 10, "s"),
            status("pi.2", 20, "pi.1", "parked"),
        ]);
        let flight = &fold.flights[0];
        assert_eq!(flight.status, "parked");
        assert!(!flight.pullable(), "unknown is not ready");
        assert!(!flight.closed(), "unknown is not closed either");
    }

    #[test]
    fn an_assignment_overwrites_and_absent_clears() {
        let laned = fold(&[
            filed_agent("pi.1", 10, "s"),
            assigned("pi.2", 20, "pi.1", Some("me")),
        ]);
        assert_eq!(laned.flights[0].assignee.as_deref(), Some("me"));
        assert!(!laned.flights[0].pullable(), "the lane left the pool");

        let cleared = fold(&[
            filed_agent("pi.1", 10, "s"),
            assigned("pi.2", 20, "pi.1", None),
        ]);
        assert!(cleared.flights[0].assignee.is_none());
    }

    #[test]
    fn a_hold_sets_the_question_and_the_status() {
        let fold = fold(&[filed("pi.1", 10, "s"), held("pi.2", 20, "pi.1", "which?")]);
        let flight = &fold.flights[0];
        let question = flight.question.as_ref().expect("held");
        assert_eq!(question.text, "which?");
        assert_eq!(question.by, "a@b.c");
        assert_eq!(question.at, 20);
        assert_eq!(flight.status, "held");
        assert_eq!(flight.status_mark.as_ref().expect("moved").at, 20);
    }

    #[test]
    fn an_answer_clears_the_question_and_releases_to_ready() {
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            held("pi.2", 20, "pi.1", "which?"),
            answered("pi.3", 30, "pi.1"),
        ]);
        let flight = &fold.flights[0];
        assert!(flight.question.is_none());
        assert_eq!(flight.status, "ready");
        assert_eq!(flight.answered_at, Some(30));
        assert!(flight.comments.is_empty());
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
        assert_eq!(fold.flights[0].status, "held");
    }

    #[test]
    fn a_lifecycle_event_for_a_flight_never_filed_is_unrouted() {
        let fold = fold(&[
            status("pi.1", 10, "zz.9", "ready"),
            assigned("pi.2", 15, "zz.9", Some("agent")),
            held("pi.3", 20, "zz.9", "q"),
            answered("pi.4", 30, "zz.9"),
        ]);
        assert!(fold.flights.is_empty());
        assert_eq!(fold.unrouted.len(), 4);
    }

    #[test]
    fn the_last_edit_wins_per_field() {
        let fold = fold(&[
            filed("pi.1", 10, "first"),
            edited("pi.2", 20, "pi.1", Some("second"), Some("a body")),
            edited("pi.3", 30, "pi.1", Some("third"), None),
        ]);
        let flight = &fold.flights[0];
        assert_eq!(flight.subject, "third");
        assert_eq!(
            flight.body, "a body",
            "an absent field leaves the last value standing"
        );
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_field_edit_overlays_and_labels_replace_wholesale() {
        let fields = event(
            "pi.2",
            20,
            Kind::Edited {
                target: "pi.1".parse().expect("id"),
                subject: None,
                body: None,
                priority: Some("high".to_string()),
                labels: Some(vec!["chore".to_string(), "web".to_string()]),
                skill: Some("review".to_string()),
                bay: Some("warm".to_string()),
            },
        );
        let relabel = event(
            "pi.3",
            30,
            Kind::Edited {
                target: "pi.1".parse().expect("id"),
                subject: None,
                body: None,
                priority: None,
                labels: Some(vec!["ops".to_string()]),
                skill: None,
                bay: None,
            },
        );
        let fold = fold(&[filed("pi.1", 10, "s"), fields, relabel]);
        let flight = &fold.flights[0];
        assert_eq!(flight.priority, "high", "an absent field stands");
        assert_eq!(flight.labels, ["ops"], "labels replace wholesale");
        assert_eq!(flight.skill.as_deref(), Some("review"));
        assert_eq!(flight.bay.as_deref(), Some("warm"));
        assert_eq!(flight.edited.as_ref().expect("marked").at, 30);
    }

    #[test]
    fn a_subject_edit_and_a_body_edit_from_two_writers_both_land() {
        // The overlay is per field, so concurrent partial edits do not
        // clobber each other.
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            edited("pi.2", 20, "pi.1", Some("new subject"), None),
            edited("qi.1", 30, "pi.1", None, Some("new body")),
        ]);
        let flight = &fold.flights[0];
        assert_eq!(flight.subject, "new subject");
        assert_eq!(flight.body, "new body");
    }

    #[test]
    fn an_edit_delivered_ahead_of_its_filing_still_attaches() {
        let fold = fold(&[
            edited("fast.1", 5, "pi.1", Some("reworded"), None),
            filed("pi.1", 10, "s"),
        ]);
        assert_eq!(fold.flights[0].subject, "reworded");
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_comment_edit_rewords_the_text_and_preserves_the_rest() {
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            commented("pi.2", 20, "pi.1"),
            edited("pi.3", 30, "pi.2", None, Some("the corrected note")),
        ]);
        let comment = &fold.flights[0].comments[0];
        assert_eq!(comment.text, "the corrected note");
        assert_eq!(comment.id, "pi.2".parse().expect("id"));
        assert_eq!(comment.author, "a@b.c");
        assert_eq!(comment.at, 20);
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_comment_edit_delivered_ahead_of_its_comment_still_attaches() {
        // The pass-3 payoff: pass 2 attaches comments in union order, so
        // an edit riding a fast clock can sort before the comment it
        // names and must still land.
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            edited("fast.1", 15, "pi.2", None, Some("reworded")),
            commented("pi.2", 20, "pi.1"),
        ]);
        assert_eq!(fold.flights[0].comments[0].text, "reworded");
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn an_edit_naming_nothing_is_unrouted() {
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            edited("pi.2", 20, "zz.9", Some("reworded"), None),
        ]);
        assert_eq!(fold.flights[0].subject, "s");
        assert_eq!(fold.unrouted.len(), 1);
    }

    #[test]
    fn a_subject_on_a_comment_target_is_ignored() {
        // The tolerant-fold rule: the verb refuses this shape, the fold
        // shrugs — but the mark still lands.
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            commented("pi.2", 20, "pi.1"),
            edited("pi.3", 30, "pi.2", Some("not a subject"), None),
        ]);
        let flight = &fold.flights[0];
        assert_eq!(flight.subject, "s");
        assert_eq!(flight.comments[0].text, "a note");
        assert_eq!(flight.edited.as_ref().expect("marked").at, 30);
    }

    #[test]
    fn the_edited_mark_carries_the_last_editor_comment_edits_included() {
        let mut late = edited("qi.1", 40, "pi.2", None, Some("reworded"));
        late.author = "editor@b.c".to_string();
        let fold = fold(&[
            filed("pi.1", 10, "s"),
            commented("pi.2", 20, "pi.1"),
            edited("pi.3", 30, "pi.1", Some("new subject"), None),
            late,
        ]);
        let mark = fold.flights[0].edited.as_ref().expect("edited");
        assert_eq!(mark.by, "editor@b.c");
        assert_eq!(mark.at, 40);
    }

    #[test]
    fn numbers_count_filings_alone_and_stay_dense() {
        let fold = fold(&[
            filed("pi.1", 10, "first"),
            status("pi.2", 20, "pi.1", "in_progress"),
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
    fn a_closed_flight_keeps_its_number_and_the_next_filing_counts_on() {
        let fold = fold(&[
            filed("pi.1", 10, "finished"),
            done("pi.2", 20, "pi.1"),
            filed("pi.3", 30, "next"),
        ]);
        assert_eq!(fold.flights[0].number, 1);
        assert!(fold.flights[0].closed());
        assert_eq!(fold.flights[1].number, 2);
    }

    #[test]
    fn done_and_canceled_both_close_and_carry_the_mover() {
        let fold = fold(&[
            filed("pi.1", 10, "a"),
            filed("pi.2", 20, "b"),
            done("pi.3", 30, "pi.1"),
            status("pi.4", 40, "pi.2", "canceled"),
        ]);
        assert!(fold.flights[0].closed());
        assert_eq!(fold.flights[0].status, "done");
        assert_eq!(fold.flights[0].status_mark.as_ref().expect("mark").at, 30);
        assert!(fold.flights[1].closed());
        assert_eq!(fold.flights[1].status, "canceled");
    }

    #[test]
    fn a_pull_and_a_release_round_trip_through_status_moves() {
        // What `next` writes and what a hand move undoes: in_progress
        // takes the flight out of the pool, ready puts it back.
        let pulled = fold(&[
            filed_agent("pi.1", 10, "s"),
            status("pi.2", 20, "pi.1", "in_progress"),
        ]);
        assert!(!pulled.flights[0].pullable());
        assert_eq!(pulled.flights[0].status, "in_progress");

        let released = fold(&[
            filed_agent("pi.1", 10, "s"),
            status("pi.2", 20, "pi.1", "in_progress"),
            status("pi.3", 30, "pi.1", "ready"),
        ]);
        assert!(released.flights[0].pullable());
    }
}

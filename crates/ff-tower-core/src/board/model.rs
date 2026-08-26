//! The board: the fold's flights, enriched with the repository's answer to
//! "where is each one" and partitioned into the inbox's sections.
//!
//! `enrich` is pure again — it runs over a [`Reads`] the caller already
//! gathered, so section classification is unit-testable with hand-built
//! rows and no repository.

use std::collections::HashMap;

use serde::Serialize;

use crate::ff::{BranchInfo, OpEntry};
use crate::log::Event;

use super::flight::{Flight, Fold};
use super::reads::Reads;

/// The inbox's four sections, plus what the fold could not route.
///
/// A done flight appears in none of them — sections and JSON both; the
/// log keeps its record, and a `--json` consumer deliberately has no
/// "done" list to read.
#[derive(Debug, Serialize)]
pub struct Board {
    pub waiting_on_you: Vec<FlightView>,
    pub in_the_air: Vec<FlightView>,
    pub holding: Vec<FlightView>,
    pub open: Vec<FlightView>,
    pub unrouted: Vec<Event>,
}

/// One flight, as a render sees it.
#[derive(Debug, Serialize)]
pub struct FlightView {
    pub id: String,
    pub procedure: String,
    pub subject: String,
    pub filed_by: String,
    /// Raw epoch; relative age is the render's concern.
    pub filed_at: i64,
    pub comments: usize,
    pub depends_on: Vec<String>,
    pub blocks: Vec<String>,
    /// `@detached` is a real literal value here, carried as fufu emitted
    /// it; a render decides how to print it.
    pub branch: Option<String>,
    /// That branch's tip, when the branch resolves to a row in the index.
    pub tip: Option<String>,
    pub last_motion: Option<i64>,
    pub held: bool,
    pub resolving: bool,
    /// The branch is the one this render's own worktree sits on.
    pub current: bool,
    /// The standing claim's author, when one stands.
    pub claimed_by: Option<String>,
    /// The open question tower's own `hold` attached — distinct from
    /// `held`/`resolving`, which stay fufu's branch verdicts.
    pub question: Option<String>,
    pub asked_at: Option<i64>,
}

/// Partition the fold's flights using already-fetched reads.
///
/// Per flight, in order: done drops it before any section; an unanswered
/// question is *waiting on you*; a branch fufu holds (`held` or
/// `resolving`) is *holding*; an op row or a standing claim is *in the
/// air*; the rest is *open*. Enrichment runs before routing, so a waiting
/// flight keeps its branch and tip — a warm bay is the point of holding.
/// A branch of `None`, `@detached`, or a name absent from the index
/// (deleted, or landed) cannot be held by definition: the flight stays in
/// the air. `last_motion` is the max of the freshest op row's time, the
/// claim, the question, and the freshest answer.
pub fn enrich(fold: Fold, reads: &Reads) -> Board {
    // Freshest op row per session tag.
    let mut freshest: HashMap<&str, &OpEntry> = HashMap::new();
    for op in &reads.ops {
        let Some(session) = op.session.as_deref() else {
            continue;
        };
        match freshest.get(session) {
            Some(seen) if seen.time >= op.time => {}
            _ => {
                freshest.insert(session, op);
            }
        }
    }

    let branches: HashMap<&str, &BranchInfo> = reads
        .branches
        .named
        .iter()
        .chain(reads.branches.anonymous.iter())
        .map(|branch| (branch.name.as_str(), branch))
        .collect();

    let mut waiting_on_you = Vec::new();
    let mut in_the_air = Vec::new();
    let mut holding = Vec::new();
    let mut open = Vec::new();
    for flight in fold.flights {
        if flight.done.is_some() {
            continue;
        }
        let op = freshest.get(flight.id.to_string().as_str()).copied();
        let row = op.and_then(|op| {
            op.branch
                .as_deref()
                .filter(|name| *name != "@detached")
                .and_then(|name| branches.get(name).copied())
        });
        let last_motion = [
            op.map(|op| op.time),
            flight.claim.as_ref().map(|claim| claim.at),
            flight.question.as_ref().map(|question| question.at),
            flight.answered_at,
        ]
        .into_iter()
        .flatten()
        .max();
        let claimed = flight.claim.is_some();
        let view = view(
            flight,
            op.and_then(|op| op.branch.clone()),
            row.and_then(|row| row.tip.clone()),
            last_motion,
            row.is_some_and(|row| row.held),
            row.is_some_and(|row| row.resolving),
            reads.current_branch.as_deref(),
        );
        if view.question.is_some() {
            waiting_on_you.push(view);
        } else if view.held || view.resolving {
            holding.push(view);
        } else if op.is_some() || claimed {
            in_the_air.push(view);
        } else {
            open.push(view);
        }
    }

    // Waiting sorts oldest-asked first — the longest-blocked agent gets
    // the top row. The rest are this slice's placeholders: motion
    // freshest-first in the air and holding, filed oldest-first in open.
    // DESIGN's "priority then readiness then age" waits on data that does
    // not exist yet.
    waiting_on_you.sort_by_key(|view| view.asked_at);
    in_the_air.sort_by_key(|view| std::cmp::Reverse(view.last_motion));
    holding.sort_by_key(|view| std::cmp::Reverse(view.last_motion));
    open.sort_by_key(|view| view.filed_at);

    Board {
        waiting_on_you,
        in_the_air,
        holding,
        open,
        unrouted: fold.unrouted,
    }
}

fn view(
    flight: Flight,
    branch: Option<String>,
    tip: Option<String>,
    last_motion: Option<i64>,
    held: bool,
    resolving: bool,
    current_branch: Option<&str>,
) -> FlightView {
    let current = match (branch.as_deref(), current_branch) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };
    let (question, asked_at) = match flight.question {
        Some(question) => (Some(question.text), Some(question.at)),
        None => (None, None),
    };
    FlightView {
        id: flight.id.to_string(),
        procedure: flight.procedure,
        subject: flight.subject,
        filed_by: flight.filed_by,
        filed_at: flight.filed_at,
        comments: flight.comments.len(),
        depends_on: flight.depends_on.iter().map(ToString::to_string).collect(),
        blocks: flight.blocks.iter().map(ToString::to_string).collect(),
        branch,
        tip,
        last_motion,
        held,
        resolving,
        current,
        claimed_by: flight.claim.map(|claim| claim.by),
        question,
        asked_at,
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::ff::BranchList;
    use crate::log::{Event, EventId, Kind};

    fn filed(id: &str, time: i64) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: "open".to_string(),
                subject: format!("subject of {time}"),
                body: String::new(),
            },
        }
    }

    fn lifecycle(id: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind,
        }
    }

    fn claimed(id: &str, time: i64, flight: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Claimed {
                flight: flight.parse().expect("id"),
            },
        )
    }

    fn held(id: &str, time: i64, flight: &str, question: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Held {
                flight: flight.parse().expect("id"),
                question: question.to_string(),
            },
        )
    }

    fn answered(id: &str, time: i64, flight: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Answered {
                flight: flight.parse().expect("id"),
                answer: "an answer".to_string(),
            },
        )
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Done {
                flight: flight.parse().expect("id"),
            },
        )
    }

    fn op(session: &str, branch: Option<&str>, time: i64) -> OpEntry {
        OpEntry {
            branch: branch.map(str::to_string),
            session: Some(session.to_string()),
            time,
        }
    }

    fn branch(name: &str, held: bool, resolving: bool) -> BranchInfo {
        BranchInfo {
            name: name.to_string(),
            tip: Some("3c8f91686a9e35a10ae8ebb6f0d6f9bbbfdd6940".to_string()),
            held,
            resolving,
        }
    }

    fn reads(ops: Vec<OpEntry>, named: Vec<BranchInfo>, current: Option<&str>) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named,
                anonymous: Vec::new(),
            },
            current_branch: current.map(str::to_string),
        }
    }

    #[test]
    fn a_tagged_flight_is_in_the_air() {
        let board = enrich(
            fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", false, false)],
                Some("main"),
            ),
        );
        assert_eq!(board.in_the_air.len(), 1);
        let view = &board.in_the_air[0];
        assert_eq!(view.id, "pi.1");
        assert_eq!(view.branch.as_deref(), Some("work"));
        assert_eq!(view.tip.as_deref().map(|t| &t[..8]), Some("3c8f9168"));
        assert_eq!(view.last_motion, Some(50));
        assert!(!view.current);
        assert!(board.holding.is_empty() && board.open.is_empty());
    }

    #[test]
    fn a_held_branch_puts_its_flight_in_holding() {
        let board = enrich(
            fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", true, false)],
                None,
            ),
        );
        assert!(board.in_the_air.is_empty());
        assert_eq!(board.holding.len(), 1);
        assert!(board.holding[0].held);
        assert!(!board.holding[0].resolving);
    }

    #[test]
    fn a_resolving_branch_puts_its_flight_in_holding() {
        let board = enrich(
            fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", false, true)],
                None,
            ),
        );
        assert_eq!(board.holding.len(), 1);
        assert!(board.holding[0].resolving);
    }

    #[test]
    fn an_untagged_flight_is_open() {
        let board = enrich(
            fold(&[filed("pi.1", 10)]),
            &reads(Vec::new(), Vec::new(), Some("main")),
        );
        assert_eq!(board.open.len(), 1);
        let view = &board.open[0];
        assert!(view.branch.is_none() && view.tip.is_none() && view.last_motion.is_none());
        assert!(!view.held && !view.resolving && !view.current);
    }

    #[test]
    fn a_detached_flight_stays_in_the_air_and_is_never_held() {
        // Even with a held branch in the index — `@detached` names nothing
        // and must not accidentally resolve to a row.
        let board = enrich(
            fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("@detached"), 50)],
                vec![branch("@detached", true, true)],
                None,
            ),
        );
        assert_eq!(board.in_the_air.len(), 1);
        let view = &board.in_the_air[0];
        assert_eq!(view.branch.as_deref(), Some("@detached"));
        assert!(view.tip.is_none());
        assert!(!view.held && !view.resolving);
    }

    #[test]
    fn the_current_branch_marks_its_flight() {
        let board = enrich(
            fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("main"), 50)],
                vec![branch("main", false, false)],
                Some("main"),
            ),
        );
        assert!(board.in_the_air[0].current);
    }

    #[test]
    fn in_the_air_is_sorted_freshest_first_and_the_freshest_row_wins_per_tag() {
        let board = enrich(
            fold(&[filed("pi.1", 10), filed("pi.2", 20)]),
            &reads(
                vec![
                    op("pi.1", Some("old"), 40),
                    op("pi.1", Some("new"), 60),
                    op("pi.2", Some("other"), 50),
                ],
                vec![branch("new", false, false), branch("other", false, false)],
                None,
            ),
        );
        let ids: Vec<&str> = board.in_the_air.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["pi.1", "pi.2"], "freshest motion first");
        assert_eq!(
            board.in_the_air[0].branch.as_deref(),
            Some("new"),
            "the freshest op row's branch wins"
        );
    }

    #[test]
    fn open_is_sorted_oldest_filing_first() {
        let board = enrich(
            fold(&[filed("pi.2", 20), filed("pi.1", 10)]),
            &reads(Vec::new(), Vec::new(), None),
        );
        let times: Vec<i64> = board.open.iter().map(|v| v.filed_at).collect();
        assert_eq!(times, [10, 20]);
    }

    #[test]
    fn a_question_beats_a_held_branch_and_keeps_the_bay() {
        // The routing order: waiting on you wins over holding even when
        // fufu holds the branch, and the enrichment survives the move —
        // a warm bay is the point of holding.
        let board = enrich(
            fold(&[filed("pi.1", 10), held("pi.2", 60, "pi.1", "which?")]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", true, false)],
                None,
            ),
        );
        assert!(board.holding.is_empty() && board.in_the_air.is_empty());
        assert_eq!(board.waiting_on_you.len(), 1);
        let view = &board.waiting_on_you[0];
        assert_eq!(view.question.as_deref(), Some("which?"));
        assert_eq!(view.asked_at, Some(60));
        assert_eq!(view.branch.as_deref(), Some("work"));
        assert!(view.tip.is_some());
        assert!(view.held);
    }

    #[test]
    fn a_done_flight_appears_nowhere() {
        let board = enrich(
            fold(&[filed("pi.1", 10), done("pi.2", 60, "pi.1")]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", false, false)],
                None,
            ),
        );
        assert!(board.waiting_on_you.is_empty());
        assert!(board.in_the_air.is_empty());
        assert!(board.holding.is_empty());
        assert!(board.open.is_empty());
    }

    #[test]
    fn a_claim_with_no_op_row_is_in_the_air_on_the_claim_alone() {
        let board = enrich(
            fold(&[filed("pi.1", 10), claimed("pi.2", 40, "pi.1")]),
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(board.in_the_air.len(), 1);
        let view = &board.in_the_air[0];
        assert_eq!(view.claimed_by.as_deref(), Some("a@b.c"));
        assert!(view.branch.is_none());
        assert_eq!(view.last_motion, Some(40));
    }

    #[test]
    fn a_claim_and_an_op_row_take_the_freshest_time_as_motion() {
        let board = enrich(
            fold(&[filed("pi.1", 10), claimed("pi.2", 40, "pi.1")]),
            &reads(
                vec![op("pi.1", Some("work"), 70)],
                vec![branch("work", false, false)],
                None,
            ),
        );
        assert_eq!(board.in_the_air[0].last_motion, Some(70));
    }

    #[test]
    fn an_answer_releases_the_flight_with_the_answer_as_motion() {
        let board = enrich(
            fold(&[
                filed("pi.1", 10),
                held("pi.2", 60, "pi.1", "which?"),
                answered("pi.3", 80, "pi.1"),
            ]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", false, false)],
                None,
            ),
        );
        assert!(board.waiting_on_you.is_empty());
        assert_eq!(board.in_the_air.len(), 1);
        let view = &board.in_the_air[0];
        assert!(view.question.is_none());
        assert_eq!(view.last_motion, Some(80));
    }

    #[test]
    fn waiting_on_you_is_sorted_oldest_asked_first() {
        let board = enrich(
            fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                held("pi.3", 70, "pi.2", "later"),
                held("pi.4", 60, "pi.1", "sooner"),
            ]),
            &reads(Vec::new(), Vec::new(), None),
        );
        let asked: Vec<Option<i64>> = board.waiting_on_you.iter().map(|v| v.asked_at).collect();
        assert_eq!(asked, [Some(60), Some(70)]);
    }
}

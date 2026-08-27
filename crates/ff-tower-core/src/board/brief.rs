//! The brief: one flight's full record over the same reads as the board.
//!
//! Pure like `flight.rs` and `pick.rs` — no `crate::ff` spawns, no
//! `std::process`; the brief runs over a [`Fold`] and a [`Reads`] the
//! caller already fetched, and deliberately takes no verdicts: the brief
//! is the read half of the handoff, and probes stay the board's and
//! `next`'s surfaces. A done flight briefs like any other — the log keeps
//! the record, and reading it is never a lifecycle move.

use serde::Serialize;

use crate::log::{EventId, PartStamp};

use super::flight::Fold;
use super::reads::Reads;

/// One flight, in full: the fold's record plus the reads' facts, flat in
/// wire form like `FlightView`. Absent facts are `None`/empty, never
/// missing keys.
#[derive(Debug, Serialize)]
pub struct Brief {
    pub id: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    pub procedure: String,
    /// The procedure part this flight is, as the filing stamped it. The
    /// brief is the read surface for one flight, so this is where crew
    /// and skill are meant to be read.
    pub part: Option<PartStamp>,
    pub subject: String,
    pub body: String,
    pub filed_by: String,
    pub filed_at: i64,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<i64>,
    /// The last route, flat like the claim. `because` carries the stored
    /// explanation, `None` when the route said nothing.
    pub routed_by: Option<String>,
    pub routed_at: Option<i64>,
    pub because: Option<String>,
    pub question: Option<String>,
    pub asked_by: Option<String>,
    pub asked_at: Option<i64>,
    pub done_by: Option<String>,
    pub done_at: Option<i64>,
    /// `@detached` is a real literal value here, carried as fufu emitted
    /// it; a render decides how to print it.
    pub branch: Option<String>,
    pub tip: Option<String>,
    pub held: bool,
    pub resolving: bool,
    pub current: bool,
    /// The board's formula — done deliberately excluded, `done_at` is its
    /// own field.
    pub last_motion: Option<i64>,
    pub depends_on: Vec<LinkView>,
    pub blocks: Vec<LinkView>,
    /// Reading order, the fold's order.
    pub comments: Vec<CommentView>,
}

/// One linked flight, carrying enough that a reader judges readiness
/// without a second call.
#[derive(Debug, Serialize)]
pub struct LinkView {
    pub flight: String,
    pub subject: String,
    pub done: bool,
}

/// A note on the record, as the brief carries it.
#[derive(Debug, Serialize)]
pub struct CommentView {
    pub author: String,
    pub at: i64,
    pub text: String,
}

/// The brief for one flight, or `None` when no such flight is filed.
///
/// Enrichment is `enrich`'s per-flight derivation, reused: branch from the
/// freshest op row (carried literal, `@detached` included), tip and holds
/// from the branch row — `@detached` and a name absent from the index
/// cannot be held — `current` against the reader's own branch, and
/// `last_motion` by the same max-of formula.
pub fn brief(fold: &Fold, reads: &Reads, id: &EventId) -> Option<Brief> {
    let flight = fold.flights.iter().find(|flight| &flight.id == id)?;
    let freshest = reads.freshest();
    let branches = reads.branch_index();

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
    let branch = op.and_then(|op| op.branch.clone());
    let current = match (branch.as_deref(), reads.current_branch.as_deref()) {
        (Some(mine), Some(here)) => mine == here,
        _ => false,
    };

    Some(Brief {
        id: flight.id.to_string(),
        number: flight.number,
        procedure: flight.procedure.clone(),
        part: flight.part.clone(),
        subject: flight.subject.clone(),
        body: flight.body.clone(),
        filed_by: flight.filed_by.clone(),
        filed_at: flight.filed_at,
        claimed_by: flight.claim.as_ref().map(|claim| claim.by.clone()),
        claimed_at: flight.claim.as_ref().map(|claim| claim.at),
        routed_by: flight.route.as_ref().map(|route| route.by.clone()),
        routed_at: flight.route.as_ref().map(|route| route.at),
        because: flight
            .route
            .as_ref()
            .map(|route| route.because.clone())
            .filter(|because| !because.is_empty()),
        question: flight.question.as_ref().map(|q| q.text.clone()),
        asked_by: flight.question.as_ref().map(|q| q.by.clone()),
        asked_at: flight.question.as_ref().map(|q| q.at),
        done_by: flight.done.as_ref().map(|mark| mark.by.clone()),
        done_at: flight.done.as_ref().map(|mark| mark.at),
        branch,
        tip: row.and_then(|row| row.tip.clone()),
        held: row.is_some_and(|row| row.held),
        resolving: row.is_some_and(|row| row.resolving),
        current,
        last_motion,
        depends_on: links(fold, &flight.depends_on),
        blocks: links(fold, &flight.blocks),
        comments: flight
            .comments
            .iter()
            .map(|comment| CommentView {
                author: comment.author.clone(),
                at: comment.at,
                text: comment.text.clone(),
            })
            .collect(),
    })
}

/// Link rows, resolved inside the fold. Infallible — the fold routes a
/// link with a missing endpoint to `unrouted`, so every carried id names a
/// filed flight.
fn links(fold: &Fold, ids: &[EventId]) -> Vec<LinkView> {
    ids.iter()
        .map(|id| {
            let other = fold
                .flights
                .iter()
                .find(|flight| &flight.id == id)
                .expect("the fold's links resolve");
            LinkView {
                flight: other.id.to_string(),
                subject: other.subject.clone(),
                done: other.done.is_some(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry};
    use crate::log::{Event, EventId, Kind, PartStamp};

    fn filed(id: &str, time: i64, subject: &str, body: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "filer@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: "open".to_string(),
                subject: subject.to_string(),
                body: body.to_string(),
                part: None,
            },
        }
    }

    fn lifecycle(id: &str, author: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: author.to_string(),
            time,
            id,
            kind,
        }
    }

    fn commented(id: &str, author: &str, time: i64, flight: &str, text: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Commented {
                flight: flight.parse().expect("id"),
                text: text.to_string(),
            },
        )
    }

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        lifecycle(
            id,
            "a@b.c",
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
            },
        )
    }

    fn claimed(id: &str, author: &str, time: i64, flight: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Claimed {
                flight: flight.parse().expect("id"),
            },
        )
    }

    fn held(id: &str, author: &str, time: i64, flight: &str, question: &str) -> Event {
        lifecycle(
            id,
            author,
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
            "a@b.c",
            time,
            Kind::Answered {
                flight: flight.parse().expect("id"),
                answer: "an answer".to_string(),
            },
        )
    }

    fn done(id: &str, author: &str, time: i64, flight: &str) -> Event {
        lifecycle(
            id,
            author,
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
            worktrees: Vec::new(),
        }
    }

    fn id(text: &str) -> EventId {
        text.parse().expect("id")
    }

    #[test]
    fn the_filing_is_carried_whole() {
        let brief = brief(
            &fold(&[filed("pi.1", 10, "the subject", "the body\ntwo lines")]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.id, "pi.1");
        assert_eq!(brief.number, 1);
        assert_eq!(brief.procedure, "open");
        assert_eq!(brief.subject, "the subject");
        assert_eq!(brief.body, "the body\ntwo lines");
        assert_eq!(brief.filed_by, "filer@b.c");
        assert_eq!(brief.filed_at, 10);
        assert!(brief.part.is_none(), "a plain filing is no part");
    }

    #[test]
    fn a_part_stamp_reaches_the_brief() {
        // The brief is the read surface for one flight, so this is where
        // crew and skill are meant to be read — the board's note line is
        // urgency-ordered, and crew is not urgency.
        let mut event = filed("pi.1", 10, "the retry test · pass", "");
        let Kind::Filed { part, .. } = &mut event.kind else {
            unreachable!("filed");
        };
        *part = Some(PartStamp {
            id: "pass".to_string(),
            crew: "agent".to_string(),
            skill: Some("review".to_string()),
            done: "asserted".to_string(),
            bay: None,
        });

        let brief = brief(
            &fold(&[event]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        let part = brief.part.expect("the stamp reaches the brief");
        assert_eq!(part.id, "pass");
        assert_eq!(part.crew, "agent");
        assert_eq!(part.skill.as_deref(), Some("review"));
        assert_eq!(part.done, "asserted");
    }

    #[test]
    fn comments_arrive_in_reading_order_with_author_and_time() {
        let brief = brief(
            &fold(&[
                filed("pi.1", 10, "s", ""),
                commented("pi.2", "one@b.c", 20, "pi.1", "first"),
                commented("pi.3", "two@b.c", 30, "pi.1", "second"),
            ]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.comments.len(), 2);
        assert_eq!(brief.comments[0].author, "one@b.c");
        assert_eq!(brief.comments[0].at, 20);
        assert_eq!(brief.comments[0].text, "first");
        assert_eq!(brief.comments[1].text, "second");
    }

    #[test]
    fn links_carry_both_directions_with_subjects_and_done_flags() {
        let events = [
            filed("pi.1", 10, "the dependent", ""),
            filed("pi.2", 20, "the dependency", ""),
            linked("pi.3", 30, "pi.1", "pi.2"),
            done("pi.4", "a@b.c", 40, "pi.2"),
        ];
        let fold = fold(&events);
        let empty = reads(Vec::new(), Vec::new(), None);

        let one = brief(&fold, &empty, &id("pi.1")).expect("filed");
        assert_eq!(one.depends_on.len(), 1);
        assert_eq!(one.depends_on[0].flight, "pi.2");
        assert_eq!(one.depends_on[0].subject, "the dependency");
        assert!(one.depends_on[0].done);
        assert!(one.blocks.is_empty());

        let two = brief(&fold, &empty, &id("pi.2")).expect("filed");
        assert_eq!(two.blocks.len(), 1);
        assert_eq!(two.blocks[0].flight, "pi.1");
        assert_eq!(two.blocks[0].subject, "the dependent");
        assert!(!two.blocks[0].done);
        assert!(two.depends_on.is_empty());
    }

    #[test]
    fn the_reads_facts_land_on_the_brief() {
        let brief = brief(
            &fold(&[filed("pi.1", 10, "s", "")]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", true, true)],
                Some("work"),
            ),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.branch.as_deref(), Some("work"));
        assert_eq!(brief.tip.as_deref().map(|t| &t[..8]), Some("3c8f9168"));
        assert!(brief.held);
        assert!(brief.resolving);
        assert!(brief.current);
        assert_eq!(brief.last_motion, Some(50));
    }

    #[test]
    fn a_detached_flight_carries_the_sentinel_with_no_tip_and_is_never_held() {
        // Even with a held `@detached` row in the index — the sentinel
        // names nothing and must not accidentally resolve.
        let brief = brief(
            &fold(&[filed("pi.1", 10, "s", "")]),
            &reads(
                vec![op("pi.1", Some("@detached"), 50)],
                vec![branch("@detached", true, true)],
                None,
            ),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.branch.as_deref(), Some("@detached"));
        assert!(brief.tip.is_none());
        assert!(!brief.held && !brief.resolving);
    }

    #[test]
    fn the_open_question_carries_who_and_when() {
        let brief = brief(
            &fold(&[
                filed("pi.1", 10, "s", ""),
                held("pi.2", "asker@b.c", 60, "pi.1", "which?"),
            ]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.question.as_deref(), Some("which?"));
        assert_eq!(brief.asked_by.as_deref(), Some("asker@b.c"));
        assert_eq!(brief.asked_at, Some(60));
    }

    #[test]
    fn the_claim_carries_who_and_when() {
        let brief = brief(
            &fold(&[
                filed("pi.1", 10, "s", ""),
                claimed("pi.2", "crew@b.c", 40, "pi.1"),
            ]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.claimed_by.as_deref(), Some("crew@b.c"));
        assert_eq!(brief.claimed_at, Some(40));
    }

    #[test]
    fn a_route_carries_who_when_and_why() {
        let route = |id: &str, time, because: &str| {
            lifecycle(
                id,
                "router@b.c",
                time,
                Kind::Routed {
                    flight: "pi.1".parse().expect("id"),
                    procedure: "chore".to_string(),
                    part: None,
                    because: because.to_string(),
                },
            )
        };
        let explained = brief(
            &fold(&[filed("pi.1", 10, "s", ""), route("pi.2", 20, "it is a chore")]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(explained.procedure, "chore");
        assert_eq!(explained.routed_by.as_deref(), Some("router@b.c"));
        assert_eq!(explained.routed_at, Some(20));
        assert_eq!(explained.because.as_deref(), Some("it is a chore"));

        // An unsaid `-m` stores an empty string; the brief carries `None`,
        // the absent-facts rule.
        let unsaid = brief(
            &fold(&[filed("pi.1", 10, "s", ""), route("pi.2", 20, "")]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert!(unsaid.because.is_none());
    }

    #[test]
    fn a_done_flight_briefs_with_its_mark() {
        let brief = brief(
            &fold(&[
                filed("pi.1", 10, "s", "the body"),
                done("pi.2", "closer@b.c", 90, "pi.1"),
            ]),
            &reads(Vec::new(), Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.done_by.as_deref(), Some("closer@b.c"));
        assert_eq!(brief.done_at, Some(90));
        assert_eq!(brief.body, "the body");
        // Done is not motion — `done_at` is its own field.
        assert!(brief.last_motion.is_none());
    }

    #[test]
    fn last_motion_is_the_max_of_op_claim_question_and_answer() {
        let brief = brief(
            &fold(&[
                filed("pi.1", 10, "s", ""),
                claimed("pi.2", "a@b.c", 40, "pi.1"),
                held("pi.3", "a@b.c", 60, "pi.1", "which?"),
                answered("pi.4", 80, "pi.1"),
            ]),
            &reads(vec![op("pi.1", Some("work"), 50)], Vec::new(), None),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.last_motion, Some(80));
    }

    #[test]
    fn an_unfiled_id_is_none() {
        assert!(
            brief(
                &fold(&[filed("pi.1", 10, "s", "")]),
                &reads(Vec::new(), Vec::new(), None),
                &id("pi.99"),
            )
            .is_none()
        );
    }
}

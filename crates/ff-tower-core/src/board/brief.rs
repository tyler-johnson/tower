//! The brief: one flight's full record over the same reads as the board,
//! plus its standing on `next`'s walk and what it beat.
//!
//! Pure like `flight.rs` and `pick.rs` — no `crate::ff` spawns, no
//! `std::process`; the brief runs over a [`Fold`], the events it was
//! folded from, a [`Reads`], and a [`Verdicts`] the caller already
//! fetched. The events ride along for the history alone: every other
//! field is the fold's, and both call sites hold the slice already. Laziness belongs to the
//! caller: [`wants_verdicts`] says when probes can change the answer, and
//! when it says no, [`Verdicts::default()`] briefs byte-identically to
//! the probed run. A closed flight briefs like any other — the log keeps
//! the record, and reading it is never a lifecycle move.
//!
//! The standing is the walk narrowed to one flight. The walk here is
//! `pick(fold, reads, verdicts, usize::MAX)`: `want` only gates the
//! walk's break, so the full walk is canonical — a flight's outcome is
//! byte-identical to any `next -n k` whose walk reached it, and every
//! pool candidate lands in exactly one of `picked` and `passed`.
//! Attribution inherits the walk's own limits: the gate check stops at
//! the first hit, so `beat` is under-inclusive by design — `next`'s
//! surface, not a full conflict matrix — and same-branch flights are one
//! tree, so a flight can take a beat row its branchmate would otherwise
//! have taken. The lane gate runs before readiness, so a me-laned
//! flight with unclosed dependencies is `yours`, never `waiting`.
//!
//! Standing precedence is `enrich`'s partition, not pick's one boolean:
//! closed, then the open question, then fufu's branch hold, then In
//! Progress, then the lane — `!pullable()` is *yours* — and only a pool
//! candidate takes the walk's outcome. A brief that said "in progress"
//! where the board shows *holding* would fail the one-glance test.

use serde::Serialize;

use crate::log::{Event, EventId};

use super::flight::{Flight, Fold};
use super::history::{Moment, history};
use super::pick::{Passed, Skip, pick};
use super::reads::{Reads, Verdicts, branch_pairs};

/// One flight, in full: the fold's record plus the reads' facts, flat in
/// wire form like `FlightView`. Absent facts are `None`/empty, never
/// missing keys — with one carved-out exception: the flattened
/// [`Standing`] tag's payload keys (`on`, `with`, `paths`) ride only the
/// variant that carries them, `Passed`'s own flatten precedent.
#[derive(Debug, Serialize)]
pub struct Brief {
    pub id: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    /// Provenance only: the procedure the filing was minted under, or
    /// the pass routed it under.
    pub procedure: Option<String>,
    pub subject: String,
    pub body: String,
    pub filed_by: String,
    pub filed_at: i64,
    /// The stored status, verbatim — the brief is the read surface for
    /// one flight, so this is where the fields are meant to be read.
    pub status: String,
    /// Who last moved it, and when — `None` while the flight still
    /// stands where it was filed.
    pub status_by: Option<String>,
    pub status_at: Option<i64>,
    pub assignee: Option<String>,
    pub priority: String,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    pub bay: Option<String>,
    /// The last edit touching the record — the flight's own fields or a
    /// comment's text — flat like the status mark.
    pub edited_by: Option<String>,
    pub edited_at: Option<i64>,
    pub question: Option<String>,
    pub asked_by: Option<String>,
    pub asked_at: Option<i64>,
    /// `@detached` is a real literal value here, carried as fufu emitted
    /// it; a render decides how to print it.
    pub branch: Option<String>,
    pub tip: Option<String>,
    pub held: bool,
    pub resolving: bool,
    pub current: bool,
    /// The board's formula.
    pub last_motion: Option<i64>,
    pub depends_on: Vec<LinkView>,
    pub blocks: Vec<LinkView>,
    /// Reading order, the fold's order.
    pub comments: Vec<CommentView>,
    /// What happened to this flight, oldest first — the log's own
    /// gestures, which the last-wins fold cannot reconstruct.
    pub history: Vec<Moment>,
    /// The arbitrated verdict, flat on the envelope beside the raw facts
    /// it arbitrates — the reader gets `"standing": "collides"` beside
    /// `with` and `paths`, no inner nesting.
    #[serde(flatten)]
    pub standing: Standing,
    /// The full walk's passed rows whose reason names this flight. A
    /// flying flight's list is what its branch is blocking right now; a
    /// passed candidate's is always empty — only gate entries and
    /// admitted candidates are ever named. Waiting rows name
    /// dependencies, never competitors, so they never land here.
    pub beat: Vec<Passed>,
}

/// Where one flight stands, in `enrich`'s precedence, flattened onto the
/// brief. The mark variants are units — their facts (the status fields,
/// the question fields, `held`/`resolving`, `assignee`) already sit flat
/// on [`Brief`], and a payload here would emit the same keys twice. Only
/// the walk variants carry what the brief has no other field for,
/// mirroring [`Skip`] flat — the walk's outcome, owned because `pick()`
/// hands its rows over whole.
#[derive(Debug, Serialize)]
#[serde(tag = "standing", rename_all = "kebab-case")]
pub enum Standing {
    /// Off the board — done or canceled; the log keeps the record.
    Done,
    /// Held on tower's own question — waiting on you.
    Question,
    /// fufu's branch verdict — derived, not authored.
    Held,
    /// The stored In Progress — someone already flies it; the status
    /// mark beside it says who.
    InProgress,
    /// Not in the pool by the stored fields alone: not Ready, or not in
    /// the agent lane. Unknown never rounds down.
    Yours,
    /// In the pool and admitted by the full walk.
    Ready,
    /// Declared dependencies not yet closed — all of them.
    Waiting { on: Vec<String> },
    /// A collide against a flying flight or an earlier-admitted
    /// candidate; the first hit wins.
    Collides { with: String, paths: Vec<String> },
    /// A pairing fufu could not judge — unknown never rounds down.
    NoVerdict { with: String },
}

/// One linked flight, carrying enough that a reader judges readiness
/// without a second call.
#[derive(Debug, Serialize)]
pub struct LinkView {
    pub flight: String,
    pub subject: String,
    pub status: String,
    pub closed: bool,
}

/// A note on the record, as the brief carries it.
#[derive(Debug, Serialize)]
pub struct CommentView {
    /// The wire id — a comment's only name, and what `edit` takes.
    pub id: String,
    pub author: String,
    pub at: i64,
    pub text: String,
}

/// The brief for one flight, or `None` when no such flight is filed.
///
/// `events` is the slice `fold` was built from — the history's only
/// source, since the fold keeps marks rather than gestures.
///
/// Enrichment is `enrich`'s per-flight derivation, reused: branch from the
/// freshest op row (carried literal, `@detached` included), tip and holds
/// from the branch row — `@detached` and a name absent from the index
/// cannot be held — `current` against the reader's own branch, and
/// `last_motion` by the same max-of formula.
pub fn brief(
    fold: &Fold,
    events: &[Event],
    reads: &Reads,
    verdicts: &Verdicts,
    id: &EventId,
) -> Option<Brief> {
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
        flight.status_mark.as_ref().map(|mark| mark.at),
        flight.question.as_ref().map(|question| question.at),
        flight.answered_at,
        flight.edited.as_ref().map(|mark| mark.at),
    ]
    .into_iter()
    .flatten()
    .max();
    let branch = op.and_then(|op| op.branch.clone());
    let current = match (branch.as_deref(), reads.current_branch.as_deref()) {
        (Some(mine), Some(here)) => mine == here,
        _ => false,
    };
    let (standing, beat) = standing_and_beat(fold, reads, verdicts, flight);

    Some(Brief {
        id: flight.id.to_string(),
        number: flight.number,
        procedure: flight.procedure.clone(),
        subject: flight.subject.clone(),
        body: flight.body.clone(),
        filed_by: flight.filed_by.clone(),
        filed_at: flight.filed_at,
        status: flight.status.clone(),
        status_by: flight.status_mark.as_ref().map(|mark| mark.by.clone()),
        status_at: flight.status_mark.as_ref().map(|mark| mark.at),
        assignee: flight.assignee.clone(),
        priority: flight.priority.clone(),
        labels: flight.labels.clone(),
        skill: flight.skill.clone(),
        bay: flight.bay.clone(),
        edited_by: flight.edited.as_ref().map(|mark| mark.by.clone()),
        edited_at: flight.edited.as_ref().map(|mark| mark.at),
        question: flight.question.as_ref().map(|q| q.text.clone()),
        asked_by: flight.question.as_ref().map(|q| q.by.clone()),
        asked_at: flight.question.as_ref().map(|q| q.at),
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
                id: comment.id.to_string(),
                author: comment.author.clone(),
                at: comment.at,
                text: comment.text.clone(),
            })
            .collect(),
        history: history(events, id),
        standing,
        beat,
    })
}

/// Whether probes can change this flight's brief.
///
/// Probe iff the flight is live, its freshest-op branch resolves in the
/// branch index, and the board has a branch pair to ask about. When this
/// says no, `brief` with [`Verdicts::default()`] is byte-identical to the
/// probed run: a closed flight is not in the walk at all, so its beat is
/// empty by construction; a branchless or index-absent branch cannot be
/// gated and is never named by a passed row, because `branch_pairs`
/// excludes it from every pair; and with no pairs the probe itself is a
/// zero-spawn no-op. The bar sits exactly here on purpose:
/// `verdicts.between() == None` reads as *clear* in the walk, so a looser
/// predicate would not save a spawn — it would silently change the
/// answer.
pub fn wants_verdicts(fold: &Fold, reads: &Reads, id: &EventId) -> bool {
    let Some(flight) = fold.flights.iter().find(|flight| &flight.id == id) else {
        return false;
    };
    if flight.closed() {
        return false;
    }
    let index = reads.branch_index();
    let resolves = reads
        .freshest()
        .get(flight.id.to_string().as_str())
        .and_then(|op| op.branch.as_deref())
        .is_some_and(|name| name != "@detached" && index.contains_key(name));
    resolves && !branch_pairs(fold, reads).is_empty()
}

/// The full walk, once — the flight's own outcome when it is a pool
/// candidate, and the beat rows either way. A closed flight is not in
/// the walk at all, so its beat is empty by construction.
fn standing_and_beat(
    fold: &Fold,
    reads: &Reads,
    verdicts: &Verdicts,
    flight: &Flight,
) -> (Standing, Vec<Passed>) {
    let id = flight.id.to_string();
    let picks = pick(fold, reads, verdicts, usize::MAX);
    let mut own = None;
    let mut beat = Vec::new();
    for row in picks.passed {
        if row.flight == id {
            own = Some(row.reason);
        } else if names(&row.reason, &id) {
            beat.push(row);
        }
    }

    // enrich's precedence, over the same per-flight derivation as pick's:
    // branch from the freshest op row, holds from the branch row.
    let freshest = reads.freshest();
    let branches = reads.branch_index();
    let row = freshest
        .get(id.as_str())
        .and_then(|op| op.branch.as_deref())
        .filter(|name| *name != "@detached")
        .and_then(|name| branches.get(name).copied());

    let standing = if flight.closed() {
        Standing::Done
    } else if flight.question.is_some() {
        Standing::Question
    } else if row.is_some_and(|row| row.held || row.resolving) {
        Standing::Held
    } else if flight.status == "in_progress" {
        Standing::InProgress
    } else if !flight.pullable() {
        Standing::Yours
    } else if picks.picked.iter().any(|pick| pick.flight == id) {
        Standing::Ready
    } else {
        // The full walk never breaks, so a candidate it did not admit has
        // a passed row.
        match own.expect("a pool candidate lands in picked or passed") {
            Skip::Waiting { on } => Standing::Waiting { on },
            Skip::Collides { with, paths } => Standing::Collides { with, paths },
            Skip::NoVerdict { with } => Standing::NoVerdict { with },
        }
    };
    (standing, beat)
}

/// Whether a passed row's reason names this flight as the competitor it
/// lost to. Waiting names dependencies, never competitors.
fn names(reason: &Skip, id: &str) -> bool {
    match reason {
        Skip::Waiting { .. } => false,
        Skip::Collides { with, .. } | Skip::NoVerdict { with } => with == id,
    }
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
                status: other.status.clone(),
                closed: other.closed(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::super::reads::BranchPairing;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry, Pairing, UnknownReason};
    use crate::log::{Event, EventId, Kind};

    /// A filing with the given status and lane stored — the pool gate's
    /// two fields, everything else defaulted.
    fn stored(id: &str, time: i64, status: &str, assignee: Option<&str>) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "filer@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: Some("review".to_string()),
                subject: format!("subject of {time}"),
                body: String::new(),
                status: status.to_string(),
                assignee: assignee.map(str::to_string),
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        }
    }

    /// A bare filing with subject and body.
    fn filed(id: &str, time: i64, subject: &str, body: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "filer@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: body.to_string(),
                status: "triage".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        }
    }

    /// The pool's norm: Ready, agent lane.
    fn agent(id: &str, time: i64) -> Event {
        stored(id, time, "ready", Some("agent"))
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

    fn edited(
        id: &str,
        author: &str,
        time: i64,
        target: &str,
        subject: Option<&str>,
        body: Option<&str>,
    ) -> Event {
        lifecycle(
            id,
            author,
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

    fn moved(id: &str, author: &str, time: i64, flight: &str, to: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
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
        moved(id, author, time, flight, "done")
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
            orphans: Vec::new(),
        }
    }

    fn collide(a: &str, b: &str, paths: &[&str]) -> BranchPairing {
        BranchPairing {
            a: a.to_string(),
            b: b.to_string(),
            pairing: Pairing::Collide {
                paths: paths.iter().map(|p| p.to_string()).collect(),
            },
        }
    }

    fn unknown(a: &str, b: &str) -> BranchPairing {
        BranchPairing {
            a: a.to_string(),
            b: b.to_string(),
            pairing: Pairing::Unknown {
                reason: UnknownReason::Other,
            },
        }
    }

    fn id(text: &str) -> EventId {
        text.parse().expect("id")
    }

    /// `brief` over one slice of events — the fold and the history from
    /// the same log, which is the only honest way to pair them.
    fn brief_of(
        events: &[Event],
        reads: &Reads,
        verdicts: &Verdicts,
        id: &EventId,
    ) -> Option<Brief> {
        brief(&fold(events), events, reads, verdicts, id)
    }

    #[test]
    fn the_filing_is_carried_whole() {
        let brief = brief_of(
            &[filed("pi.1", 10, "the subject", "the body\ntwo lines")],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.id, "pi.1");
        assert_eq!(brief.number, 1);
        assert!(brief.procedure.is_none(), "a bare filing has no procedure");
        assert_eq!(brief.subject, "the subject");
        assert_eq!(brief.body, "the body\ntwo lines");
        assert_eq!(brief.filed_by, "filer@b.c");
        assert_eq!(brief.filed_at, 10);
        assert_eq!(brief.status, "triage");
        assert!(brief.status_by.is_none() && brief.status_at.is_none());
        assert!(brief.assignee.is_none());
        assert_eq!(brief.priority, "none");
        assert!(brief.labels.is_empty());
        assert!(brief.skill.is_none());
    }

    #[test]
    fn the_stored_fields_reach_the_brief() {
        // The brief is the read surface for one flight, so this is where
        // the fields are meant to be read.
        let mut event = filed("pi.1", 10, "the retry test · pass", "");
        let Kind::Filed {
            status,
            assignee,
            priority,
            labels,
            skill,
            ..
        } = &mut event.kind
        else {
            unreachable!("filed");
        };
        *status = "ready".to_string();
        *assignee = Some("agent".to_string());
        *priority = "high".to_string();
        *labels = vec!["chore".to_string()];
        *skill = Some("review".to_string());

        let brief = brief_of(
            &[event],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.status, "ready");
        assert_eq!(brief.assignee.as_deref(), Some("agent"));
        assert_eq!(brief.priority, "high");
        assert_eq!(brief.labels, ["chore"]);
        assert_eq!(brief.skill.as_deref(), Some("review"));
    }

    #[test]
    fn comments_arrive_in_reading_order_with_author_and_time() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                commented("pi.2", "one@b.c", 20, "pi.1", "first"),
                commented("pi.3", "two@b.c", 30, "pi.1", "second"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.comments.len(), 2);
        assert_eq!(brief.comments[0].id, "pi.2");
        assert_eq!(brief.comments[0].author, "one@b.c");
        assert_eq!(brief.comments[0].at, 20);
        assert_eq!(brief.comments[0].text, "first");
        assert_eq!(brief.comments[1].id, "pi.3");
        assert_eq!(brief.comments[1].text, "second");
    }

    #[test]
    fn the_edited_mark_lands_flat_and_counts_as_motion() {
        let plain = brief_of(
            &[filed("pi.1", 10, "s", "")],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert!(plain.edited_by.is_none());
        assert!(plain.edited_at.is_none());

        let reworded = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                edited("pi.2", "editor@b.c", 30, "pi.1", Some("reworded"), None),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(reworded.subject, "reworded");
        assert_eq!(reworded.edited_by.as_deref(), Some("editor@b.c"));
        assert_eq!(reworded.edited_at, Some(30));
        assert_eq!(reworded.last_motion, Some(30), "an edit is motion");
    }

    #[test]
    fn links_carry_both_directions_with_subjects_and_statuses() {
        let events = [
            filed("pi.1", 10, "the dependent", ""),
            filed("pi.2", 20, "the dependency", ""),
            linked("pi.3", 30, "pi.1", "pi.2"),
            done("pi.4", "a@b.c", 40, "pi.2"),
        ];
        let empty = reads(Vec::new(), Vec::new(), None);

        let one = brief_of(&events, &empty, &Verdicts::default(), &id("pi.1")).expect("filed");
        assert_eq!(one.depends_on.len(), 1);
        assert_eq!(one.depends_on[0].flight, "pi.2");
        assert_eq!(one.depends_on[0].subject, "the dependency");
        assert_eq!(one.depends_on[0].status, "done");
        assert!(one.depends_on[0].closed);
        assert!(one.blocks.is_empty());

        let two = brief_of(&events, &empty, &Verdicts::default(), &id("pi.2")).expect("filed");
        assert_eq!(two.blocks.len(), 1);
        assert_eq!(two.blocks[0].flight, "pi.1");
        assert_eq!(two.blocks[0].subject, "the dependent");
        assert_eq!(two.blocks[0].status, "triage");
        assert!(!two.blocks[0].closed);
        assert!(two.depends_on.is_empty());
    }

    #[test]
    fn the_reads_facts_land_on_the_brief() {
        let brief = brief_of(
            &[filed("pi.1", 10, "s", "")],
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", true, true)],
                Some("work"),
            ),
            &Verdicts::default(),
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
        let brief = brief_of(
            &[filed("pi.1", 10, "s", "")],
            &reads(
                vec![op("pi.1", Some("@detached"), 50)],
                vec![branch("@detached", true, true)],
                None,
            ),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.branch.as_deref(), Some("@detached"));
        assert!(brief.tip.is_none());
        assert!(!brief.held && !brief.resolving);
    }

    #[test]
    fn the_open_question_carries_who_and_when() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                held("pi.2", "asker@b.c", 60, "pi.1", "which?"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.question.as_deref(), Some("which?"));
        assert_eq!(brief.asked_by.as_deref(), Some("asker@b.c"));
        assert_eq!(brief.asked_at, Some(60));
        assert_eq!(brief.status, "held");
        assert!(matches!(brief.standing, Standing::Question));
    }

    #[test]
    fn a_status_move_carries_who_and_when() {
        let brief = brief_of(
            &[
                agent("pi.1", 10),
                moved("pi.2", "crew@b.c", 40, "pi.1", "in_progress"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.status, "in_progress");
        assert_eq!(brief.status_by.as_deref(), Some("crew@b.c"));
        assert_eq!(brief.status_at, Some(40));
        assert!(matches!(brief.standing, Standing::InProgress));
    }

    #[test]
    fn a_closed_flight_briefs_with_its_mark() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", "the body"),
                done("pi.2", "closer@b.c", 90, "pi.1"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.status, "done");
        assert_eq!(brief.status_by.as_deref(), Some("closer@b.c"));
        assert_eq!(brief.status_at, Some(90));
        assert_eq!(brief.body, "the body");
        assert!(matches!(brief.standing, Standing::Done));
        assert!(brief.beat.is_empty(), "not in the walk at all");
    }

    #[test]
    fn last_motion_is_the_max_of_op_status_question_and_answer() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                moved("pi.2", "a@b.c", 40, "pi.1", "in_progress"),
                held("pi.3", "a@b.c", 60, "pi.1", "which?"),
                answered("pi.4", 80, "pi.1"),
            ],
            &reads(vec![op("pi.1", Some("work"), 50)], Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.last_motion, Some(80));
    }

    #[test]
    fn an_unfiled_id_is_none() {
        assert!(
            brief_of(
                &[filed("pi.1", 10, "s", "")],
                &reads(Vec::new(), Vec::new(), None),
                &Verdicts::default(),
                &id("pi.99"),
            )
            .is_none()
        );
    }

    #[test]
    fn question_outranks_held_outranks_in_progress() {
        // One flight carrying all three: the question wins.
        let all = brief_of(
            &[
                agent("pi.1", 10),
                moved("pi.2", "a@b.c", 20, "pi.1", "in_progress"),
                held("pi.3", "a@b.c", 30, "pi.1", "which?"),
            ],
            &reads(
                vec![op("pi.1", Some("work"), 40)],
                vec![branch("work", true, false)],
                None,
            ),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(all.standing, Standing::Question));

        // fufu-held and in progress, no question: enrich's order says
        // held.
        let fufu_held = brief_of(
            &[
                agent("pi.1", 10),
                moved("pi.2", "a@b.c", 20, "pi.1", "in_progress"),
            ],
            &reads(
                vec![op("pi.1", Some("work"), 40)],
                vec![branch("work", false, true)],
                None,
            ),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(fufu_held.standing, Standing::Held));
        assert!(fufu_held.resolving, "the flat fact carries the detail");
    }

    #[test]
    fn a_me_laned_flight_is_yours_never_waiting() {
        // The lane gate runs before readiness: unclosed dependencies and
        // all, the stored fields are the answer.
        let brief = brief_of(
            &[
                stored("pi.1", 10, "ready", Some("me")),
                agent("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::Yours));
        assert_eq!(
            brief.assignee.as_deref(),
            Some("me"),
            "the lane reads off the flat field, not the standing"
        );
    }

    #[test]
    fn a_triage_flight_is_yours_with_no_lane() {
        let brief = brief_of(
            &[filed("pi.1", 10, "s", "")],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::Yours));
        assert!(brief.assignee.is_none());
    }

    #[test]
    fn a_ready_flight_lists_only_the_rows_that_name_it() {
        // Three candidates, two collide pairs. pi.2 loses to pi.1 on the
        // first gate hit; pi.3 clears pi.1 and admits — the b/c pair
        // never fires because a passed row is not on the gate. pi.1's
        // beat is the naming row alone.
        let brief = brief_of(
            &[agent("pi.1", 10), agent("pi.2", 20), agent("pi.3", 30)],
            &reads(
                vec![
                    op("pi.1", Some("a"), 40),
                    op("pi.2", Some("b"), 50),
                    op("pi.3", Some("c"), 60),
                ],
                vec![
                    branch("a", false, false),
                    branch("b", false, false),
                    branch("c", false, false),
                ],
                None,
            ),
            &Verdicts {
                pairs: vec![
                    collide("a", "b", &["shared.txt"]),
                    collide("b", "c", &["other.txt"]),
                ],
            },
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::Ready));
        assert_eq!(brief.beat.len(), 1);
        assert_eq!(brief.beat[0].flight, "pi.2");
        match &brief.beat[0].reason {
            Skip::Collides { with, paths } => {
                assert_eq!(with, "pi.1");
                assert_eq!(paths, &["shared.txt"]);
            }
            other => panic!("expected a collides row, got {other:?}"),
        }
    }

    #[test]
    fn a_flying_flights_beat_is_what_its_branch_blocks() {
        let brief = brief_of(
            &[
                agent("pi.1", 10),
                agent("pi.2", 20),
                moved("pi.3", "a@b.c", 30, "pi.1", "in_progress"),
            ],
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
                None,
            ),
            &Verdicts {
                pairs: vec![unknown("left", "right")],
            },
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::InProgress));
        assert_eq!(brief.beat.len(), 1);
        assert_eq!(brief.beat[0].flight, "pi.2");
        assert!(matches!(
            &brief.beat[0].reason,
            Skip::NoVerdict { with } if with == "pi.1"
        ));
    }

    #[test]
    fn a_passed_flights_beat_is_empty() {
        // Only gate entries and admitted candidates are ever named, so
        // the loser blocks nothing.
        let brief = brief_of(
            &[agent("pi.1", 10), agent("pi.2", 20)],
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
                None,
            ),
            &Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
            &id("pi.2"),
        )
        .expect("filed");
        match &brief.standing {
            Standing::Collides { with, paths } => {
                assert_eq!(with, "pi.1");
                assert_eq!(paths, &["shared.txt"]);
            }
            other => panic!("expected collides, got {other:?}"),
        }
        assert!(brief.beat.is_empty());
    }

    #[test]
    fn the_walk_ignores_want_and_reaches_late_candidates() {
        // `next`'s default want is 1; the full walk still reaches the
        // third candidate.
        let brief = brief_of(
            &[agent("pi.1", 10), agent("pi.2", 20), agent("pi.3", 30)],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.3"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::Ready));
    }

    #[test]
    fn waiting_deps_brief_as_waiting_and_never_enter_beat() {
        let events = [
            agent("pi.1", 10),
            agent("pi.2", 20),
            linked("pi.3", 30, "pi.1", "pi.2"),
        ];
        let empty = reads(Vec::new(), Vec::new(), None);

        let dependent =
            brief_of(&events, &empty, &Verdicts::default(), &id("pi.1")).expect("filed");
        match &dependent.standing {
            Standing::Waiting { on } => assert_eq!(on, &["pi.2"]),
            other => panic!("expected waiting, got {other:?}"),
        }

        // The waiting row names pi.2 as a dependency, not a competitor —
        // its beat stays empty.
        let dependency =
            brief_of(&events, &empty, &Verdicts::default(), &id("pi.2")).expect("filed");
        assert!(matches!(dependency.standing, Standing::Ready));
        assert!(dependency.beat.is_empty());
    }

    #[test]
    fn the_slimmed_standing_carries_no_duplicate_payload() {
        // The mark variants flatten to the tag alone: `status_by` appears
        // once, from the brief's own field, never a second time from the
        // standing.
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                done("pi.2", "closer@b.c", 90, "pi.1"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        let json = serde_json::to_value(&brief).expect("serializes");
        assert_eq!(json["standing"], serde_json::json!("done"));
        assert_eq!(json["status_by"], serde_json::json!("closer@b.c"));
        let text = serde_json::to_string(&brief).expect("serializes");
        assert_eq!(text.matches("\"status_by\"").count(), 1);
    }

    #[test]
    fn wants_verdicts_asks_for_probes_only_when_they_can_matter() {
        // Live, resolving branch, a second live branch to pair with: yes.
        let two_branches = [agent("pi.1", 10), agent("pi.2", 20)];
        let paired = reads(
            vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
            vec![branch("left", false, false), branch("right", false, false)],
            None,
        );
        assert!(wants_verdicts(&fold(&two_branches), &paired, &id("pi.1")));
        assert!(wants_verdicts(&fold(&two_branches), &paired, &id("pi.2")));

        // Closed: no — not in the walk at all, whatever the board holds.
        let with_done = [
            agent("pi.1", 10),
            agent("pi.2", 20),
            agent("pi.3", 30),
            done("pi.4", "a@b.c", 90, "pi.3"),
        ];
        let three = reads(
            vec![
                op("pi.1", Some("left"), 40),
                op("pi.2", Some("right"), 50),
                op("pi.3", Some("gone"), 60),
            ],
            vec![
                branch("left", false, false),
                branch("right", false, false),
                branch("gone", false, false),
            ],
            None,
        );
        assert!(!wants_verdicts(&fold(&with_done), &three, &id("pi.3")));

        // Branchless: no — cannot be gated, and never named by a row.
        let one_branchless = [agent("pi.1", 10), agent("pi.2", 20), agent("pi.3", 30)];
        let branchless = reads(
            vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
            vec![branch("left", false, false), branch("right", false, false)],
            None,
        );
        assert!(!wants_verdicts(
            &fold(&one_branchless),
            &branchless,
            &id("pi.3")
        ));

        // `@detached` and an index-absent branch: no — the existing
        // cannot-be-held idiom.
        let sentinel = reads(
            vec![
                op("pi.1", Some("left"), 40),
                op("pi.2", Some("right"), 50),
                op("pi.3", Some("@detached"), 60),
            ],
            vec![branch("left", false, false), branch("right", false, false)],
            None,
        );
        assert!(!wants_verdicts(
            &fold(&one_branchless),
            &sentinel,
            &id("pi.3")
        ));
        let landed = reads(
            vec![
                op("pi.1", Some("left"), 40),
                op("pi.2", Some("right"), 50),
                op("pi.3", Some("landed"), 60),
            ],
            vec![branch("left", false, false), branch("right", false, false)],
            None,
        );
        assert!(!wants_verdicts(
            &fold(&one_branchless),
            &landed,
            &id("pi.3")
        ));

        // A solo branch makes no pair: no.
        let solo = reads(
            vec![op("pi.1", Some("left"), 40)],
            vec![branch("left", false, false)],
            None,
        );
        assert!(!wants_verdicts(
            &fold(&[agent("pi.1", 10)]),
            &solo,
            &id("pi.1")
        ));

        // Unfiled: no.
        assert!(!wants_verdicts(&fold(&two_branches), &paired, &id("pi.99")));
    }

    #[test]
    fn a_skipped_probe_briefs_byte_identically() {
        // The invariance pin behind the laziness: where `wants_verdicts`
        // says no, verdicts cannot reach the envelope. A closed flight on
        // a board whose live branches genuinely collide is the strongest
        // case — the pairs exist, and the answer must not care.
        let events = [
            agent("pi.1", 10),
            agent("pi.2", 20),
            agent("pi.3", 30),
            done("pi.4", "a@b.c", 90, "pi.3"),
        ];
        let board = reads(
            vec![
                op("pi.1", Some("left"), 40),
                op("pi.2", Some("right"), 50),
                op("pi.3", Some("gone"), 60),
            ],
            vec![
                branch("left", false, false),
                branch("right", false, false),
                branch("gone", false, false),
            ],
            None,
        );
        let fold = fold(&events);
        assert!(!wants_verdicts(&fold, &board, &id("pi.3")));

        let probed = Verdicts {
            pairs: vec![
                collide("left", "right", &["shared.txt"]),
                collide("left", "gone", &["shared.txt"]),
                collide("right", "gone", &["shared.txt"]),
            ],
        };
        let lazy = brief_of(&events, &board, &Verdicts::default(), &id("pi.3")).expect("filed");
        let full = brief_of(&events, &board, &probed, &id("pi.3")).expect("filed");
        assert_eq!(
            serde_json::to_string(&lazy).expect("serializes"),
            serde_json::to_string(&full).expect("serializes"),
        );
    }
}

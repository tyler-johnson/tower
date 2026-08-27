//! The explanation: one flight's standing on the board, and what it beat.
//!
//! Pure like `brief.rs` and `pick.rs` — no `crate::ff` spawns, no
//! `std::process`; the fold runs over a [`Fold`], a [`Reads`], and a
//! [`Verdicts`] the caller already fetched. `next`'s passed rows are the
//! explained ranking for the whole walk; this narrows it to one flight
//! and adds the clauses `next` has no room for — the standing that keeps
//! a flight out of the pool, and the passed rows attributable to it.
//!
//! The walk here is `pick(fold, reads, verdicts, usize::MAX)`: `want`
//! only gates the walk's break, so the full walk is canonical — a
//! flight's outcome is byte-identical to any `next -n k` whose walk
//! reached it, and every pool candidate lands in exactly one of `picked`
//! and `passed`. Attribution inherits the walk's own limits: the gate
//! check stops at the first hit, so `beat` is under-inclusive by design —
//! `next`'s surface, not a full conflict matrix — and same-branch flights
//! are one tree, so a flight can take a beat row its branchmate would
//! otherwise have taken. The crew gate runs before readiness, so a
//! you-crewed flight with undone dependencies is `yours`, never
//! `waiting`.
//!
//! Standing precedence is `enrich`'s partition, not pick's one boolean:
//! done, then the open question, then fufu's branch hold, then the claim,
//! then the crew stamp, and only a pool candidate takes the walk's
//! outcome. An explain that said "claimed" where the board shows
//! *holding* would fail the one-glance test.

use serde::Serialize;

use crate::log::{EventId, PartStamp};

use super::flight::Fold;
use super::pick::{Passed, Skip, pick};
use super::reads::{Reads, Verdicts};

/// One flight, explained: why it is here, why this procedure, and what it
/// beat.
#[derive(Debug, Serialize)]
pub struct Explanation {
    pub id: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    pub subject: String,
    pub procedure: String,
    /// The procedure part this flight is, as the filing stamped it.
    pub part: Option<PartStamp>,
    /// The last route, flat like the brief's. `because` carries the
    /// stored explanation, `None` when the route said nothing.
    pub routed_by: Option<String>,
    pub routed_at: Option<i64>,
    pub because: Option<String>,
    /// Flat on the envelope, `Passed`'s own precedent — the reader gets
    /// `"standing": "collides"` beside `with` and `paths`, no inner
    /// nesting.
    #[serde(flatten)]
    pub standing: Standing,
    /// The full walk's passed rows whose reason names this flight. A
    /// flying flight's list is what its branch is blocking right now; a
    /// passed candidate's is always empty — only gate entries and
    /// admitted candidates are ever named. Waiting rows name
    /// dependencies, never competitors, so they never land here.
    pub beat: Vec<Passed>,
}

/// Where one flight stands, in `enrich`'s precedence. The last three
/// mirror [`Skip`] flat — the walk's outcome, owned because `pick()`
/// hands its rows over whole.
#[derive(Debug, Serialize)]
#[serde(tag = "standing", rename_all = "kebab-case")]
pub enum Standing {
    /// Off the board; the log keeps the record.
    Done { by: String, at: i64 },
    /// Held on tower's own question — waiting on you.
    Question { by: String, at: i64, text: String },
    /// fufu's branch verdict — derived, not authored, so there is no by
    /// or at to name.
    Held { branch: String, resolving: bool },
    /// A standing claim — someone already flies it.
    Claimed { by: String, at: i64 },
    /// Unclaimed, and the crew stamp is what keeps it out of the pool:
    /// `None` is no part stamp at all — a parent, or a plain filing —
    /// and `Some` carries the crew verbatim, `you` or a crew this binary
    /// has never heard of. Unknown never rounds down.
    Yours { crew: Option<String> },
    /// In the pool and admitted by the full walk.
    Ready,
    /// Declared dependencies not yet done — all of them.
    Waiting { on: Vec<String> },
    /// A collide against a flying flight or an earlier-admitted
    /// candidate; the first hit wins.
    Collides { with: String, paths: Vec<String> },
    /// A pairing fufu could not judge — unknown never rounds down.
    NoVerdict { with: String },
}

/// The explanation for one flight, or `None` when no such flight is
/// filed.
pub fn explain(
    fold: &Fold,
    reads: &Reads,
    verdicts: &Verdicts,
    id: &EventId,
) -> Option<Explanation> {
    let flight = fold.flights.iter().find(|flight| &flight.id == id)?;
    let id = id.to_string();

    // The full walk, once — the flight's own outcome when it is a pool
    // candidate, and the beat rows either way. A done flight is not in
    // the walk at all, so its beat is empty by construction.
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
    let branch = freshest
        .get(id.as_str())
        .and_then(|op| op.branch.as_deref())
        .filter(|name| *name != "@detached");
    let row = branch.and_then(|name| branches.get(name).copied());
    let agent_crewed = flight
        .part
        .as_ref()
        .is_some_and(|part| part.crew == "agent");

    let standing = if let Some(mark) = &flight.done {
        Standing::Done {
            by: mark.by.clone(),
            at: mark.at,
        }
    } else if let Some(question) = &flight.question {
        Standing::Question {
            by: question.by.clone(),
            at: question.at,
            text: question.text.clone(),
        }
    } else if row.is_some_and(|row| row.held || row.resolving) {
        Standing::Held {
            branch: branch.expect("a held row has a branch").to_string(),
            resolving: row.is_some_and(|row| row.resolving),
        }
    } else if let Some(claim) = &flight.claim {
        Standing::Claimed {
            by: claim.by.clone(),
            at: claim.at,
        }
    } else if !agent_crewed {
        Standing::Yours {
            crew: flight.part.as_ref().map(|part| part.crew.clone()),
        }
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

    Some(Explanation {
        id,
        number: flight.number,
        subject: flight.subject.clone(),
        procedure: flight.procedure.clone(),
        part: flight.part.clone(),
        routed_by: flight.route.as_ref().map(|route| route.by.clone()),
        routed_at: flight.route.as_ref().map(|route| route.at),
        because: flight
            .route
            .as_ref()
            .map(|route| route.because.clone())
            .filter(|because| !because.is_empty()),
        standing,
        beat,
    })
}

/// Whether a passed row's reason names this flight as the competitor it
/// lost to. Waiting names dependencies, never competitors.
fn names(reason: &Skip, id: &str) -> bool {
    match reason {
        Skip::Waiting { .. } => false,
        Skip::Collides { with, .. } | Skip::NoVerdict { with } => with == id,
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::super::reads::BranchPairing;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry, Pairing, UnknownReason};
    use crate::log::{Event, Kind, PartStamp};

    fn crewed(id: &str, time: i64, crew: Option<&str>) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: "review".to_string(),
                subject: format!("subject of {time}"),
                body: String::new(),
                part: crew.map(|crew| PartStamp {
                    id: "pass".to_string(),
                    crew: crew.to_string(),
                    skill: None,
                    done: "asserted".to_string(),
                    bay: None,
                }),
            },
        }
    }

    fn filed(id: &str, time: i64) -> Event {
        crewed(id, time, Some("agent"))
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

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
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

    fn reads(ops: Vec<OpEntry>, named: Vec<BranchInfo>) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named,
                anonymous: Vec::new(),
            },
            current_branch: None,
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

    #[test]
    fn question_outranks_held_outranks_claimed() {
        // One flight carrying all three: the question wins.
        let all = explain(
            &fold(&[
                filed("pi.1", 10),
                claimed("pi.2", 20, "pi.1"),
                held("pi.3", 30, "pi.1", "which?"),
            ]),
            &reads(
                vec![op("pi.1", Some("work"), 40)],
                vec![branch("work", true, false)],
            ),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        match all.standing {
            Standing::Question { by, at, text } => {
                assert_eq!(by, "a@b.c");
                assert_eq!(at, 30);
                assert_eq!(text, "which?");
            }
            other => panic!("expected question, got {other:?}"),
        }

        // Held and claimed, no question: enrich's order says held.
        let held = explain(
            &fold(&[filed("pi.1", 10), claimed("pi.2", 20, "pi.1")]),
            &reads(
                vec![op("pi.1", Some("work"), 40)],
                vec![branch("work", false, true)],
            ),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        match held.standing {
            Standing::Held { branch, resolving } => {
                assert_eq!(branch, "work");
                assert!(resolving);
            }
            other => panic!("expected held, got {other:?}"),
        }
    }

    #[test]
    fn a_done_flight_explains_with_its_mark() {
        let explained = explain(
            &fold(&[filed("pi.1", 10), done("pi.2", 90, "pi.1")]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        match explained.standing {
            Standing::Done { by, at } => {
                assert_eq!(by, "a@b.c");
                assert_eq!(at, 90);
            }
            other => panic!("expected done, got {other:?}"),
        }
        assert!(explained.beat.is_empty(), "not in the walk at all");
    }

    #[test]
    fn a_you_crewed_flight_is_yours_never_waiting() {
        // The crew gate runs before readiness: undone dependencies and
        // all, the stamp is the answer.
        let explained = explain(
            &fold(&[
                crewed("pi.1", 10, Some("you")),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        match explained.standing {
            Standing::Yours { crew } => assert_eq!(crew.as_deref(), Some("you")),
            other => panic!("expected yours, got {other:?}"),
        }
    }

    #[test]
    fn a_stampless_parent_is_yours_with_no_crew() {
        let explained = explain(
            &fold(&[crewed("pi.1", 10, None)]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            &id("pi.1"),
        )
        .expect("filed");
        match explained.standing {
            Standing::Yours { crew } => assert!(crew.is_none()),
            other => panic!("expected yours, got {other:?}"),
        }
    }

    #[test]
    fn a_ready_flight_lists_only_the_rows_that_name_it() {
        // Three candidates, two collide pairs. pi.2 loses to pi.1 on the
        // first gate hit; pi.3 clears pi.1 and admits — the b/c pair
        // never fires because a passed row is not on the gate. pi.1's
        // beat is the naming row alone.
        let explained = explain(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
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
        assert!(matches!(explained.standing, Standing::Ready));
        assert_eq!(explained.beat.len(), 1);
        assert_eq!(explained.beat[0].flight, "pi.2");
        match &explained.beat[0].reason {
            Skip::Collides { with, paths } => {
                assert_eq!(with, "pi.1");
                assert_eq!(paths, &["shared.txt"]);
            }
            other => panic!("expected a collides row, got {other:?}"),
        }
    }

    #[test]
    fn a_claimed_flights_beat_is_what_its_branch_blocks() {
        let explained = explain(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                claimed("pi.3", 30, "pi.1"),
            ]),
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
            ),
            &Verdicts {
                pairs: vec![unknown("left", "right")],
            },
            &id("pi.1"),
        )
        .expect("filed");
        match &explained.standing {
            Standing::Claimed { by, at } => {
                assert_eq!(by, "a@b.c");
                assert_eq!(*at, 30);
            }
            other => panic!("expected claimed, got {other:?}"),
        }
        assert_eq!(explained.beat.len(), 1);
        assert_eq!(explained.beat[0].flight, "pi.2");
        assert!(matches!(
            &explained.beat[0].reason,
            Skip::NoVerdict { with } if with == "pi.1"
        ));
    }

    #[test]
    fn a_passed_flights_beat_is_empty() {
        // Only gate entries and admitted candidates are ever named, so
        // the loser blocks nothing.
        let explained = explain(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20)]),
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
            ),
            &Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
            &id("pi.2"),
        )
        .expect("filed");
        match &explained.standing {
            Standing::Collides { with, paths } => {
                assert_eq!(with, "pi.1");
                assert_eq!(paths, &["shared.txt"]);
            }
            other => panic!("expected collides, got {other:?}"),
        }
        assert!(explained.beat.is_empty());
    }

    #[test]
    fn the_walk_ignores_want_and_reaches_late_candidates() {
        // `next`'s default want is 1; the full walk still reaches the
        // third candidate.
        let explained = explain(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            &id("pi.3"),
        )
        .expect("filed");
        assert!(matches!(explained.standing, Standing::Ready));
    }

    #[test]
    fn waiting_deps_explain_as_waiting_and_never_enter_beat() {
        let events = [
            filed("pi.1", 10),
            filed("pi.2", 20),
            linked("pi.3", 30, "pi.1", "pi.2"),
        ];
        let empty = reads(Vec::new(), Vec::new());

        let dependent =
            explain(&fold(&events), &empty, &Verdicts::default(), &id("pi.1")).expect("filed");
        match &dependent.standing {
            Standing::Waiting { on } => assert_eq!(on, &["pi.2"]),
            other => panic!("expected waiting, got {other:?}"),
        }

        // The waiting row names pi.2 as a dependency, not a competitor —
        // its beat stays empty.
        let dependency =
            explain(&fold(&events), &empty, &Verdicts::default(), &id("pi.2")).expect("filed");
        assert!(matches!(dependency.standing, Standing::Ready));
        assert!(dependency.beat.is_empty());
    }

    #[test]
    fn an_unfiled_id_is_none() {
        assert!(
            explain(
                &fold(&[filed("pi.1", 10)]),
                &reads(Vec::new(), Vec::new()),
                &Verdicts::default(),
                &id("pi.99"),
            )
            .is_none()
        );
    }
}

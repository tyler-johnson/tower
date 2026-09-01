//! The pick: `next`'s fold over the same probe output as the board.
//!
//! Pure like `flight.rs` — no `crate::ff` spawns, no `std::process`; the
//! walk runs over a [`Fold`], a [`Reads`], and a [`Verdicts`] the caller
//! already fetched, so admission is unit-testable with hand-built rows.
//!
//! The pool is every Ready flight in the agent lane: the derived status
//! and the stored assignee, read off the fold — never the registry
//! (principle 11). The gate is `Flight::pullable`, the one the brief
//! reads too: exact string compares, so an unknown status or lane never
//! rounds down into the pool. Ready is derived, so a pool candidate has
//! no live dependency by construction — a dependent sits in Waiting
//! until its last dependency closes, done or canceled, and never
//! reaches the walk. An open question or a fufu hold takes a flight out
//! on top of it. Ready flights *not* in the agent lane are counted in
//! `yours`, the number behind `next`'s exit 3; the flights themselves
//! are silent here because the board is their surface, not this one's.
//!
//! Every live flight *not* in the pool keeps its branch on the gate —
//! waiting and holding flights included, because a warm bay holds real
//! work that will land. Admission is greedy: candidates walk in filed
//! order, and one joins the pick when its branch is clear against the
//! gate and every candidate already admitted. Unknown excludes — a
//! pairing fufu could not judge never rounds down to clear.

use serde::Serialize;

use crate::ff::Pairing;

use super::flight::Fold;
use super::reads::{Reads, Verdicts};

/// What the walk produced: the admitted set, and every candidate it
/// examined and skipped. Candidates past the point the walk stopped get
/// no row — the passed list explains the pick, never the whole board.
#[derive(Debug, Serialize)]
pub struct Picks {
    pub picked: Vec<Pick>,
    pub passed: Vec<Passed>,
    /// Ready, unquestioned, not fufu-held — excluded from the pool by
    /// the lane alone. Work that exists and needs you.
    pub yours: usize,
}

/// One admitted flight, in wire form.
#[derive(Debug, Serialize)]
pub struct Pick {
    pub flight: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    pub subject: String,
    pub branch: Option<String>,
}

/// One examined-and-skipped flight, with the machine-matchable reason.
#[derive(Debug, Serialize)]
pub struct Passed {
    pub flight: String,
    #[serde(flatten)]
    pub reason: Skip,
}

/// Why a candidate lost. Readiness is not a reason: a flight with a
/// live dependency is Waiting, and Waiting is not in the pool.
#[derive(Debug, Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum Skip {
    /// A collide against a flying flight or an already-picked candidate;
    /// the first hit wins.
    Collides { with: String, paths: Vec<String> },
    /// A pairing fufu could not judge — unknown never rounds down.
    NoVerdict { with: String },
}

/// Walk the candidates in filed order and admit up to `want` of them.
pub fn pick(fold: &Fold, reads: &Reads, verdicts: &Verdicts, want: usize) -> Picks {
    let freshest = reads.freshest();
    let index = reads.branch_index();

    // Per live flight: its branch (non-`@detached`, from the freshest op
    // row) and whether it is in the pool — Ready, agent lane, no open
    // question, and its branch row not held or resolving. A branch of
    // `None`, `@detached`, or a name absent from the index cannot be
    // held, the existing idiom.
    let mut candidates = Vec::new();
    let mut gate: Vec<(String, String)> = Vec::new();
    let mut yours = 0;
    for flight in &fold.flights {
        if flight.closed() {
            continue;
        }
        let id = flight.id.to_string();
        let branch = freshest
            .get(id.as_str())
            .and_then(|op| op.branch.as_deref())
            .filter(|name| *name != "@detached")
            .map(str::to_string);
        let fufu_held = branch
            .as_deref()
            .and_then(|name| index.get(name))
            .is_some_and(|row| row.held || row.resolving);
        let unheld = flight.question.is_none() && !fufu_held;
        if unheld && flight.pullable() {
            candidates.push((flight, id, branch));
        } else {
            if unheld && flight.status == "ready" {
                yours += 1;
            }
            if let Some(branch) = branch {
                gate.push((id, branch));
            }
        }
    }

    let mut picked: Vec<Pick> = Vec::new();
    let mut passed = Vec::new();
    for (flight, id, branch) in candidates {
        if picked.len() == want {
            break;
        }
        // Admission, only when the candidate has a tree to conflict:
        // clear against the gate and everything already admitted, in
        // that order. Same branch is one tree — no conflict.
        let skip = branch.as_deref().and_then(|mine| {
            gate.iter()
                .map(|(other, theirs)| (other, theirs))
                .chain(
                    picked
                        .iter()
                        .filter_map(|pick| Some((&pick.flight, pick.branch.as_ref()?))),
                )
                .find_map(|(other, theirs)| {
                    if theirs == mine {
                        return None;
                    }
                    match verdicts.between(mine, theirs) {
                        Some(Pairing::Collide { paths }) => Some(Skip::Collides {
                            with: other.clone(),
                            paths: paths.clone(),
                        }),
                        Some(Pairing::Unknown { .. }) => Some(Skip::NoVerdict {
                            with: other.clone(),
                        }),
                        Some(Pairing::Clear) | None => None,
                    }
                })
        });
        match skip {
            Some(reason) => passed.push(Passed { flight: id, reason }),
            None => picked.push(Pick {
                flight: id,
                number: flight.number,
                subject: flight.subject.clone(),
                branch,
            }),
        }
    }

    Picks {
        picked,
        passed,
        yours,
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::super::reads::BranchPairing;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry, UnknownReason};
    use crate::log::{Event, EventId, Kind};

    /// A filing with the given status and lane stored — the shape the
    /// pool gate is about.
    fn stored(id: &str, time: i64, status: &str, assignee: Option<&str>) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
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

    /// The pool's norm: Ready, agent lane.
    fn filed(id: &str, time: i64) -> Event {
        stored(id, time, "ready", Some("agent"))
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

    fn moved(id: &str, time: i64, flight: &str, to: &str) -> Event {
        lifecycle(
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
        lifecycle(
            id,
            time,
            Kind::Assigned {
                flight: flight.parse().expect("id"),
                assignee: lane.map(str::to_string),
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
        moved(id, time, flight, "done")
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

    fn reasons(picks: &Picks) -> Vec<(&str, &Skip)> {
        picks
            .passed
            .iter()
            .map(|passed| (passed.flight.as_str(), &passed.reason))
            .collect()
    }

    #[test]
    fn an_unclosed_dependency_keeps_the_dependent_out_of_the_pool() {
        // The fold derives the dependent Waiting, so it is neither
        // picked nor passed — it was never a candidate.
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            2,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.2");
        assert_eq!(picks.picked[0].number, 2);
        assert!(picks.passed.is_empty(), "waiting is not a walk outcome");
        assert_eq!(picks.yours, 0, "waiting is not yours either");
    }

    #[test]
    fn a_done_dependency_admits_the_dependent() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
                done("pi.4", 40, "pi.2"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
        assert!(picks.passed.is_empty());
    }

    #[test]
    fn a_canceled_dependency_releases_its_dependent() {
        // Closed is closed: a canceled part still shows on the parent's
        // brief, and the dependent is Ready for whoever reconsiders it.
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
                moved("pi.4", 40, "pi.2", "canceled"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
        assert!(picks.passed.is_empty());
    }

    #[test]
    fn pulled_questioned_and_fufu_held_flights_are_neither_picked_nor_passed() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                filed("pi.3", 30),
                moved("pi.4", 40, "pi.1", "in_progress"),
                held("pi.5", 50, "pi.2", "which?"),
            ]),
            &reads(
                vec![op("pi.3", Some("work"), 60)],
                vec![branch("work", true, false)],
            ),
            &Verdicts::default(),
            3,
        );
        assert!(picks.picked.is_empty());
        assert!(picks.passed.is_empty());
    }

    #[test]
    fn a_collide_against_a_flying_flight_passes_naming_it_and_its_paths() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                moved("pi.3", 30, "pi.1", "in_progress"),
            ]),
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
            ),
            &Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
            1,
        );
        assert!(picks.picked.is_empty());
        match reasons(&picks).as_slice() {
            [("pi.2", Skip::Collides { with, paths })] => {
                assert_eq!(with, "pi.1");
                assert_eq!(paths, &["shared.txt"]);
            }
            other => panic!("expected one collides row, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_pairing_against_a_flying_flight_is_no_verdict() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                moved("pi.3", 30, "pi.1", "in_progress"),
            ]),
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
            ),
            &Verdicts {
                pairs: vec![unknown("left", "right")],
            },
            1,
        );
        assert!(picks.picked.is_empty());
        match reasons(&picks).as_slice() {
            [("pi.2", Skip::NoVerdict { with })] => assert_eq!(with, "pi.1"),
            other => panic!("expected one no-verdict row, got {other:?}"),
        }
    }

    #[test]
    fn two_colliding_candidates_admit_the_first_and_pass_the_second_naming_it() {
        let picks = pick(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20)]),
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
            ),
            &Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
            2,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.1");
        assert_eq!(picks.picked[0].branch.as_deref(), Some("left"));
        match reasons(&picks).as_slice() {
            [("pi.2", Skip::Collides { with, .. })] => assert_eq!(with, "pi.1"),
            other => panic!("expected one collides row, got {other:?}"),
        }
    }

    #[test]
    fn branchless_candidates_all_admit_in_filed_order() {
        let picks = pick(
            &fold(&[filed("pi.2", 20), filed("pi.1", 10), filed("pi.3", 30)]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            3,
        );
        let ids: Vec<&str> = picks.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(
            ids,
            ["pi.2", "pi.1", "pi.3"],
            "fold order, which is filed order"
        );
        assert!(picks.picked.iter().all(|p| p.branch.is_none()));
        assert!(picks.passed.is_empty());
    }

    #[test]
    fn the_same_branch_as_a_flying_flight_is_one_tree_and_admits() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                moved("pi.3", 30, "pi.1", "in_progress"),
            ]),
            &reads(
                vec![op("pi.1", Some("work"), 40), op("pi.2", Some("work"), 50)],
                vec![branch("work", false, false)],
            ),
            // A same-name verdict row would be a caller bug; even present,
            // it must not fire.
            &Verdicts {
                pairs: vec![collide("work", "work", &["shared.txt"])],
            },
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.2");
        assert!(picks.passed.is_empty());
    }

    #[test]
    fn the_walk_stops_at_want_and_later_candidates_get_no_row() {
        let picks = pick(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            1,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.1");
        assert!(picks.passed.is_empty(), "unexamined, so unlisted");
    }

    #[test]
    fn only_ready_agent_flights_enter_the_pool_and_ready_rest_are_yours() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", None),
                stored("pi.2", 20, "ready", Some("me")),
                stored("pi.3", 30, "ready", Some("pair")),
                stored("pi.4", 40, "ready", Some("agent")),
                stored("pi.5", 50, "triage", Some("agent")),
                stored("pi.6", 60, "ready", Some("agent")),
                linked("pi.7", 70, "pi.6", "pi.5"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            6,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.4");
        assert!(picks.passed.is_empty(), "excluded silently, never passed");
        assert_eq!(
            picks.yours, 3,
            "Ready off the agent lane counts; Triage and Waiting do not"
        );
    }

    #[test]
    fn an_unknown_status_or_lane_never_rounds_into_the_pool() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "parked", Some("agent")),
                stored("pi.2", 20, "ready", Some("pair")),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            2,
        );
        assert!(picks.picked.is_empty());
        assert_eq!(picks.yours, 1, "the unknown lane's Ready flight is yours");
    }

    #[test]
    fn a_me_laned_flights_branch_still_collides_an_agent_candidate_off() {
        let picks = pick(
            &fold(&[stored("pi.1", 10, "ready", Some("me")), filed("pi.2", 20)]),
            &reads(
                vec![op("pi.1", Some("left"), 40), op("pi.2", Some("right"), 50)],
                vec![branch("left", false, false), branch("right", false, false)],
            ),
            &Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
            1,
        );
        assert!(picks.picked.is_empty());
        match reasons(&picks).as_slice() {
            [("pi.2", Skip::Collides { with, .. })] => assert_eq!(with, "pi.1"),
            other => panic!("expected one collides row, got {other:?}"),
        }
        assert_eq!(picks.yours, 1);
    }

    #[test]
    fn questioned_and_pulled_flights_are_not_yours() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                stored("pi.2", 20, "ready", None),
                moved("pi.3", 30, "pi.1", "in_progress"),
                held("pi.4", 40, "pi.2", "which?"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            2,
        );
        assert!(picks.picked.is_empty());
        assert_eq!(picks.yours, 0, "a pull or a question already has an owner");
    }

    #[test]
    fn a_reassignment_moves_a_flight_across_the_gate() {
        // Both directions: the agent lane opened by hand, and closed by
        // hand — the stored field is the whole gate.
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                assigned("pi.2", 20, "pi.1", Some("agent")),
                filed("pi.3", 30),
                assigned("pi.4", 40, "pi.3", Some("me")),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            2,
        );
        let ids: Vec<&str> = picks.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(ids, ["pi.1"]);
        assert_eq!(picks.yours, 1);
    }

    #[test]
    fn a_release_back_to_ready_rejoins_the_pool() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                moved("pi.2", 20, "pi.1", "in_progress"),
                moved("pi.3", 30, "pi.1", "ready"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn an_empty_fold_picks_nothing() {
        let picks = pick(
            &fold(&[]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            1,
        );
        assert!(picks.picked.is_empty());
        assert!(picks.passed.is_empty());
    }
}

//! The pick: `next`'s fold over the same probe output as the board.
//!
//! Pure like `flight.rs` — no `crate::ff` spawns, no `std::process`; the
//! walk runs over a [`Fold`], a [`Reads`], and a [`Verdicts`] the caller
//! already fetched, so admission is unit-testable with hand-built rows.
//!
//! The pool is any unclaimed live flight: a standing claim, an open
//! question, or a fufu hold takes a flight out, and an op row alone does
//! not. Every live flight *not* in the pool keeps its branch on the gate —
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

/// Why a candidate lost.
#[derive(Debug, Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum Skip {
    /// Declared dependencies not yet done — all of them.
    Waiting { on: Vec<String> },
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
    // row) and whether it is in the pool — no claim, no open question,
    // and its branch row not held or resolving. A branch of `None`,
    // `@detached`, or a name absent from the index cannot be held, the
    // existing idiom.
    let mut candidates = Vec::new();
    let mut gate: Vec<(String, String)> = Vec::new();
    for flight in &fold.flights {
        if flight.done.is_some() {
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
        if flight.claim.is_none() && flight.question.is_none() && !fufu_held {
            candidates.push((flight, id, branch));
        } else if let Some(branch) = branch {
            gate.push((id, branch));
        }
    }

    let mut picked: Vec<Pick> = Vec::new();
    let mut passed = Vec::new();
    for (flight, id, branch) in candidates {
        if picked.len() == want {
            break;
        }
        // Readiness first: every declared dependency must be done. Dep
        // ids always resolve — the fold routes unresolvable links to
        // `unrouted` — so a missing lookup is simply not-done.
        let waiting: Vec<String> = flight
            .depends_on
            .iter()
            .filter(|dep| {
                fold.flights
                    .iter()
                    .find(|other| &other.id == *dep)
                    .is_none_or(|other| other.done.is_none())
            })
            .map(ToString::to_string)
            .collect();
        if !waiting.is_empty() {
            passed.push(Passed {
                flight: id,
                reason: Skip::Waiting { on: waiting },
            });
            continue;
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

    Picks { picked, passed }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::super::reads::BranchPairing;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry, UnknownReason};
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
    fn an_undone_dependency_passes_with_waiting_naming_it() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ]),
            &reads(Vec::new(), Vec::new()),
            &Verdicts::default(),
            1,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.2");
        assert_eq!(picks.picked[0].number, 2);
        match reasons(&picks).as_slice() {
            [("pi.1", Skip::Waiting { on })] => assert_eq!(on, &["pi.2"]),
            other => panic!("expected one waiting row, got {other:?}"),
        }
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
    fn claimed_questioned_and_fufu_held_flights_are_neither_picked_nor_passed() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                filed("pi.3", 30),
                claimed("pi.4", 40, "pi.1"),
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
    fn a_collide_against_a_flying_claim_passes_naming_it_and_its_paths() {
        let picks = pick(
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
                claimed("pi.3", 30, "pi.1"),
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
                claimed("pi.3", 30, "pi.1"),
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

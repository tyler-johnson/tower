//! The bay fold: the pool, derived from the survey and the board's own
//! flight-to-branch derivation — never entered, never registered.
//!
//! Pure like `pick.rs` — no `crate::ff` spawns, no `std::process`; the
//! fold runs over a [`Fold`] and a [`Reads`] the caller already fetched.
//! Occupancy joins each worktree row's branch against the live flights'
//! derived branches, exactly `enrich`'s derivation: live is `done`
//! unset, the branch is the freshest session op's, `@detached` excluded.
//! A done flight frees its bay by derivation, not bookkeeping.

use serde::Serialize;

use super::flight::{Flight, Fold};
use super::pick::Pick;
use super::reads::Reads;

/// One bay, as the pool render and the release gate see it.
#[derive(Debug, Clone, Serialize)]
pub struct BayView {
    /// fufu's worktree id — `main` for the main worktree.
    pub id: String,
    pub path: String,
    /// `None` is a detached HEAD — a detached bay is free by definition.
    pub branch: Option<String>,
    /// The live flight whose derived branch is this bay's; `None` = free.
    pub flight: Option<String>,
    /// The occupant's subject, for the render.
    pub subject: Option<String>,
    /// True on the row the invoking worktree answered for.
    pub current: bool,
}

/// The pool as one payload — what a `bay list` envelope carries as
/// `data`. A named struct rather than an ad-hoc map so the key and the
/// field order are the same on every emitting surface.
#[derive(Serialize)]
pub struct Pool {
    pub bays: Vec<BayView>,
}

/// Every worktree with a path, in survey order — main is listed, because
/// the survey returns it, work genuinely flies on main's tree in the solo
/// norm, and hiding where a flight sits would lie; its `current` flag and
/// id distinguish it. Two live flights on one branch: the first filed
/// takes the slot, deterministically — the release gate only needs
/// `is_some()`. A row without a path (a bare repository's main) is
/// skipped.
pub fn bays(fold: &Fold, reads: &Reads) -> Vec<BayView> {
    let freshest = reads.freshest();

    // The live flight-to-branch assignments, in filed order — first
    // filed wins a shared branch.
    let assignments: Vec<(&Flight, String)> = fold
        .flights
        .iter()
        .filter(|flight| flight.done.is_none())
        .filter_map(|flight| {
            let branch = freshest
                .get(flight.id.to_string().as_str())?
                .branch
                .as_deref()?;
            if branch == "@detached" {
                return None;
            }
            Some((flight, branch.to_string()))
        })
        .collect();

    reads
        .worktrees
        .iter()
        .filter_map(|row| {
            let path = row.path.clone()?;
            let occupant = row.branch.as_deref().and_then(|branch| {
                assignments
                    .iter()
                    .find(|(_, theirs)| theirs == branch)
                    .map(|(flight, _)| flight)
            });
            Some(BayView {
                id: row.id.clone(),
                path,
                branch: row.branch.clone(),
                flight: occupant.map(|flight| flight.id.to_string()),
                subject: occupant.map(|flight| flight.subject.clone()),
                current: row.current,
            })
        })
        .collect()
}

/// One pick and the bay it flies in — `next`'s half of the assignment.
#[derive(Debug)]
pub struct Berth {
    /// The wire id of the picked flight, [`super::Pick::flight`]'s.
    pub flight: String,
    /// The bay the flight takes; `None` when the pool was full.
    pub bay: Option<BayView>,
    /// Its part stamped `bay = "warm"` and nothing was free — the one
    /// case that earns a new slot. False whenever a bay was taken.
    pub wants_warm: bool,
}

/// Join each pick to a bay, in pick order.
///
/// Pure, like [`bays`] it folds over: the assignment is a walk, not a
/// spawn, and warming is the caller's to do with what `wants_warm` says.
/// A pick prefers a bay already standing on its derived branch — a
/// requeued flight resumes where it left off rather than being moved —
/// and otherwise takes the first free bay in survey order, main included,
/// because in the solo norm main *is* the bay. A bay is available to a
/// pick when no live flight sits in it or the flight sitting in it is
/// this one; either way it is consumed for the rest of the walk, so two
/// picks in one `-n 2` can never be handed the same tree.
pub fn assign(fold: &Fold, reads: &Reads, picked: &[Pick]) -> Vec<Berth> {
    let pool = bays(fold, reads);
    let mut taken = vec![false; pool.len()];

    picked
        .iter()
        .map(|pick| {
            let free = |view: &BayView| {
                view.flight.is_none() || view.flight.as_deref() == Some(pick.flight.as_str())
            };
            let available = |at: &usize| !taken[*at] && free(&pool[*at]);
            let slot = (0..pool.len())
                .find(|at| {
                    available(at) && pool[*at].branch.is_some() && pool[*at].branch == pick.branch
                })
                .or_else(|| (0..pool.len()).find(available));
            if let Some(at) = slot {
                taken[at] = true;
            }
            let wants_warm = slot.is_none()
                && fold
                    .flights
                    .iter()
                    .find(|flight| flight.id.to_string() == pick.flight)
                    .and_then(|flight| flight.part.as_ref())
                    .and_then(|part| part.bay.as_deref())
                    == Some("warm");
            Berth {
                flight: pick.flight.clone(),
                bay: slot.map(|at| pool[at].clone()),
                wants_warm,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::ff::{BranchList, OpEntry, WorktreeInfo};
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
                part: None,
            },
        }
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Done {
                flight: flight.parse().expect("id"),
            },
        }
    }

    fn op(session: &str, branch: Option<&str>, time: i64) -> OpEntry {
        OpEntry {
            branch: branch.map(str::to_string),
            session: Some(session.to_string()),
            time,
        }
    }

    fn worktree(id: &str, path: Option<&str>, branch: Option<&str>, current: bool) -> WorktreeInfo {
        WorktreeInfo {
            id: id.to_string(),
            path: path.map(str::to_string),
            branch: branch.map(str::to_string),
            current,
        }
    }

    fn reads(ops: Vec<OpEntry>, worktrees: Vec<WorktreeInfo>) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named: Vec::new(),
                anonymous: Vec::new(),
            },
            current_branch: None,
            worktrees,
            orphans: Vec::new(),
        }
    }

    #[test]
    fn a_live_flights_branch_occupies_its_bay() {
        let views = bays(
            &fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![
                    worktree("main", Some("/repo"), Some("main"), true),
                    worktree("bay1", Some("/bay1"), Some("work"), false),
                ],
            ),
        );
        assert_eq!(views.len(), 2);
        let bay = &views[1];
        assert_eq!(bay.id, "bay1");
        assert_eq!(bay.flight.as_deref(), Some("pi.1"));
        assert_eq!(bay.subject.as_deref(), Some("subject of 10"));
        assert!(views[0].flight.is_none(), "main's branch has no flight");
    }

    #[test]
    fn a_done_flights_bay_is_free() {
        let views = bays(
            &fold(&[filed("pi.1", 10), done("pi.2", 60, "pi.1")]),
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![worktree("bay1", Some("/bay1"), Some("work"), false)],
            ),
        );
        assert!(views[0].flight.is_none() && views[0].subject.is_none());
    }

    #[test]
    fn a_detached_bay_is_free() {
        // Even with a flight whose freshest row says `@detached` — the
        // sentinel names no branch and must never join.
        let views = bays(
            &fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("@detached"), 50)],
                vec![worktree("bay1", Some("/bay1"), None, false)],
            ),
        );
        assert!(views[0].branch.is_none());
        assert!(views[0].flight.is_none());
    }

    #[test]
    fn an_unflown_branch_is_free() {
        let views = bays(
            &fold(&[filed("pi.1", 10)]),
            &reads(
                Vec::new(),
                vec![worktree("bay1", Some("/bay1"), Some("idle"), false)],
            ),
        );
        assert!(views[0].flight.is_none());
    }

    #[test]
    fn main_is_listed_with_its_current_mark() {
        let views = bays(
            &fold(&[]),
            &reads(
                Vec::new(),
                vec![worktree("main", Some("/repo"), Some("main"), true)],
            ),
        );
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id, "main");
        assert!(views[0].current);
    }

    #[test]
    fn two_flights_on_one_branch_seat_the_first_filed() {
        let views = bays(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20)]),
            &reads(
                vec![op("pi.1", Some("work"), 50), op("pi.2", Some("work"), 60)],
                vec![worktree("bay1", Some("/bay1"), Some("work"), false)],
            ),
        );
        assert_eq!(views[0].flight.as_deref(), Some("pi.1"));
    }

    /// A filing whose part carries the given `bay` stamp — the only
    /// thing `assign` reads off the fold beyond the occupancy join.
    fn stamped(id: &str, time: i64, bay: Option<&str>) -> Event {
        let mut event = filed(id, time);
        let Kind::Filed { part, .. } = &mut event.kind else {
            unreachable!("filed");
        };
        *part = Some(crate::log::PartStamp {
            id: "pass".to_string(),
            crew: "agent".to_string(),
            skill: None,
            done: "asserted".to_string(),
            bay: bay.map(str::to_string),
            branch: None,
        });
        event
    }

    fn want(flight: &str, branch: Option<&str>) -> Pick {
        Pick {
            flight: flight.to_string(),
            number: 1,
            subject: "s".to_string(),
            branch: branch.map(str::to_string),
        }
    }

    #[test]
    fn a_pick_resumes_the_bay_already_on_its_branch() {
        // The requeue-and-resume case: the flight's own bay reads as
        // occupied by the flight itself, and it is still the right one.
        let fold = fold(&[filed("pi.1", 10)]);
        let reads = reads(
            vec![op("pi.1", Some("work"), 50)],
            vec![
                worktree("main", Some("/repo"), Some("main"), true),
                worktree("bay1", Some("/bay1"), Some("work"), false),
            ],
        );
        let berths = assign(&fold, &reads, &[want("pi.1", Some("work"))]);
        let bay = berths[0].bay.as_ref().expect("a bay");
        assert_eq!(bay.id, "bay1");
        assert!(!berths[0].wants_warm);
    }

    #[test]
    fn two_picks_never_share_one_bay_and_the_pool_runs_out() {
        let fold = fold(&[]);
        let reads = reads(
            Vec::new(),
            vec![
                worktree("main", Some("/repo"), Some("main"), true),
                worktree("bay1", Some("/bay1"), Some("idle"), false),
            ],
        );
        let berths = assign(
            &fold,
            &reads,
            &[want("pi.1", None), want("pi.2", None), want("pi.3", None)],
        );
        let taken: Vec<Option<&str>> = berths
            .iter()
            .map(|berth| berth.bay.as_ref().map(|bay| bay.id.as_str()))
            .collect();
        // Main is assignable and first in survey order — in the solo
        // norm it *is* the bay.
        assert_eq!(taken, [Some("main"), Some("bay1"), None]);
    }

    #[test]
    fn warming_is_asked_for_by_the_stamp_alone_and_only_when_nothing_is_free() {
        let fold = fold(&[stamped("pi.1", 10, Some("warm")), stamped("pi.2", 20, None)]);
        let reads = reads(
            Vec::new(),
            vec![worktree("main", Some("/repo"), Some("main"), true)],
        );

        // One bay for two picks: the stamped flight asks for a slot, the
        // unstamped one claims anyway with no bay.
        let berths = assign(&fold, &reads, &[want("pi.2", None), want("pi.1", None)]);
        assert!(berths[0].bay.is_some() && !berths[0].wants_warm);
        assert!(berths[1].bay.is_none() && berths[1].wants_warm);

        // The stamp alone never warms — a bay was free, so it took it.
        let berths = assign(&fold, &reads, &[want("pi.1", None)]);
        assert!(berths[0].bay.is_some());
        assert!(!berths[0].wants_warm, "a free bay outranks the stamp");
    }

    #[test]
    fn survey_order_is_kept_and_a_pathless_row_is_skipped() {
        let views = bays(
            &fold(&[]),
            &reads(
                Vec::new(),
                vec![
                    worktree("main", None, None, true),
                    worktree("bay2", Some("/bay2"), Some("b"), false),
                    worktree("bay1", Some("/bay1"), Some("a"), false),
                ],
            ),
        );
        let ids: Vec<&str> = views.iter().map(|view| view.id.as_str()).collect();
        assert_eq!(ids, ["bay2", "bay1"], "no re-sort, bare main skipped");
    }
}

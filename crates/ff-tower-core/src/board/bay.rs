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
use super::reads::Reads;

/// One bay, as the pool render and the release gate see it.
#[derive(Debug, Serialize)]
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

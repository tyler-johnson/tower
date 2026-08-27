//! The doctor fold: stale bays and drift, as rows.
//!
//! Principle 5 is the verb's whole license: doctor observes and
//! complains, never enforces. Nothing here fixes, prunes, or retries —
//! every row is a report, and the hint inside it is a command for the
//! user to run, not one tower runs.
//!
//! Pure like `bay.rs` — no `crate::ff` spawns, no `std::process`; the
//! fold runs over facts the caller already fetched: the [`Fold`] and
//! [`Reads`] the board runs on, the seam's own answer from `ff version`,
//! and the directory-existence checks, which stay the CLI's so this
//! never touches a filesystem.
//!
//! A bare orphan chain is info, not a finding: every `bay release`
//! leaves one by design — fufu guarantees the chain outlives the bay —
//! so only actionable rows drive the exit. The leftover-branch check is
//! orphan-derived only: the orphan row records its branch, so the check
//! is exact, never a `bay-<n>` naming heuristic.

use std::collections::HashSet;

use serde::Serialize;

use crate::ff::CONTRACT;

use super::flight::Fold;
use super::reads::Reads;

/// What `ff version` said about the seam, decided by the caller before
/// any bay-facing read: a drifted contract fails every gather spawn, so
/// doctor is the verb that reports the break instead of dying of it.
#[derive(Debug)]
pub enum SeamHealth {
    /// The call answered: the seam speaks tower's contract.
    Ok { version: String },
    /// The envelope named a contract tower does not read.
    Drift { found: u32 },
    /// No `ff` to spawn at all.
    Missing,
}

/// How loud one row is. Only `Warn` counts as a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Info,
    Warn,
}

/// One check's report.
#[derive(Debug, Serialize)]
pub struct DoctorRow {
    pub level: Level,
    /// The machine-matchable half — `bay/orphan-chain` and its kin.
    pub check: String,
    pub message: String,
}

/// The whole report: rows in check order, and the warn count that drives
/// the exit — 0 healthy, 1 findings, fufu's own doctor precedent.
#[derive(Debug, Serialize)]
pub struct Doctor {
    pub rows: Vec<DoctorRow>,
    pub findings: usize,
}

/// The four checks, over facts the caller fetched. `gone` is the survey
/// rows whose directory the CLI found missing, as `(id, path)` pairs. On
/// a broken seam the caller passes an empty fold and reads — only the
/// seam row emerges, and absence of a row asserts nothing.
pub fn doctor(fold: &Fold, reads: &Reads, seam: &SeamHealth, gone: &[(String, String)]) -> Doctor {
    let mut rows = Vec::new();

    match seam {
        SeamHealth::Ok { version } => rows.push(row(
            Level::Ok,
            "ff/version",
            format!("ff {version} · contract {CONTRACT}"),
        )),
        SeamHealth::Drift { found } => rows.push(row(
            Level::Warn,
            "ff/contract",
            format!(
                "ff speaks contract {found}; tower reads {CONTRACT} — upgrade whichever is behind (bay checks skipped)"
            ),
        )),
        SeamHealth::Missing => rows.push(row(
            Level::Warn,
            "ff/not-installed",
            "`ff` is not on PATH — tower runs on fufu (bay checks skipped)".to_string(),
        )),
    }

    // The half-gone shape: admin entry alive, directory deleted by hand.
    for (id, path) in gone {
        rows.push(row(
            Level::Warn,
            "bay/missing-directory",
            format!("bay {id}: `{path}` is gone from disk — `ff worktree remove {id}` finishes the teardown"),
        ));
    }

    for orphan in &reads.orphans {
        let message = match orphan.tip.as_deref() {
            Some(tip) => format!(
                "released {}: the chain remains — `ff restore --at-op {tip}` reaches its work",
                orphan.id
            ),
            None => format!("released {}: the chain remains", orphan.id),
        };
        rows.push(row(Level::Info, "bay/orphan-chain", message));
    }

    // A leftover branch: an orphan's recorded branch that still exists,
    // is checked out by no surveyed worktree, and is no live flight's
    // derived branch — `enrich`'s derivation: the freshest session op,
    // `@detached` excluded. Deduplicated per branch across orphans.
    let index = reads.branch_index();
    let freshest = reads.freshest();
    let flying: HashSet<&str> = fold
        .flights
        .iter()
        .filter(|flight| flight.done.is_none())
        .filter_map(|flight| {
            freshest
                .get(flight.id.to_string().as_str())?
                .branch
                .as_deref()
        })
        .filter(|branch| *branch != "@detached")
        .collect();
    let checked_out: HashSet<&str> = reads
        .worktrees
        .iter()
        .filter_map(|worktree| worktree.branch.as_deref())
        .collect();

    let mut seen = HashSet::new();
    for orphan in &reads.orphans {
        let Some(branch) = orphan.branch.as_deref() else {
            continue;
        };
        if !seen.insert(branch)
            || !index.contains_key(branch)
            || checked_out.contains(branch)
            || flying.contains(branch)
        {
            continue;
        }
        rows.push(row(
            Level::Warn,
            "bay/leftover-branch",
            format!(
                "branch {branch} outlived its bay — nothing flies it, and the log keeps any flight that did"
            ),
        ));
    }

    let findings = rows.iter().filter(|row| row.level == Level::Warn).count();
    Doctor { rows, findings }
}

fn row(level: Level, check: &str, message: String) -> DoctorRow {
    DoctorRow {
        level,
        check: check.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry, OrphanInfo, WorktreeInfo};
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

    fn branch(name: &str) -> BranchInfo {
        BranchInfo {
            name: name.to_string(),
            tip: Some("3c8f91686a9e35a10ae8ebb6f0d6f9bbbfdd6940".to_string()),
            held: false,
            resolving: false,
        }
    }

    fn worktree(id: &str, branch: Option<&str>) -> WorktreeInfo {
        WorktreeInfo {
            id: id.to_string(),
            path: Some(format!("/{id}")),
            branch: branch.map(str::to_string),
            current: id == "main",
        }
    }

    fn orphan(id: &str, branch: Option<&str>) -> OrphanInfo {
        OrphanInfo {
            id: id.to_string(),
            tip: Some("xkvxvkxprnvxklvnyutuvqnsqynwmwpmkvlvtmxv".to_string()),
            branch: branch.map(str::to_string),
            time: Some(60),
        }
    }

    fn reads(
        ops: Vec<OpEntry>,
        named: Vec<BranchInfo>,
        worktrees: Vec<WorktreeInfo>,
        orphans: Vec<OrphanInfo>,
    ) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named,
                anonymous: Vec::new(),
            },
            current_branch: None,
            worktrees,
            orphans,
        }
    }

    fn healthy() -> SeamHealth {
        SeamHealth::Ok {
            version: "0.9.0".to_string(),
        }
    }

    fn checks(report: &Doctor) -> Vec<(&str, Level)> {
        report
            .rows
            .iter()
            .map(|row| (row.check.as_str(), row.level))
            .collect()
    }

    #[test]
    fn a_healthy_seam_is_one_ok_row_and_zero_findings() {
        let report = doctor(
            &fold(&[]),
            &reads(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            &healthy(),
            &[],
        );
        assert_eq!(checks(&report), [("ff/version", Level::Ok)]);
        assert_eq!(report.findings, 0);
        assert!(report.rows[0].message.contains("0.9.0"));
        assert!(report.rows[0].message.contains("contract 1"));
    }

    #[test]
    fn drift_and_not_installed_are_findings() {
        let empty = reads(Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let report = doctor(&fold(&[]), &empty, &SeamHealth::Drift { found: 99 }, &[]);
        assert_eq!(checks(&report), [("ff/contract", Level::Warn)]);
        assert_eq!(report.findings, 1);
        let message = &report.rows[0].message;
        assert!(message.contains("99") && message.contains('1'), "{message}");
        assert!(message.contains("skipped"), "{message}");

        let report = doctor(&fold(&[]), &empty, &SeamHealth::Missing, &[]);
        assert_eq!(checks(&report), [("ff/not-installed", Level::Warn)]);
        assert_eq!(report.findings, 1);
    }

    #[test]
    fn a_gone_directory_is_a_finding() {
        let report = doctor(
            &fold(&[]),
            &reads(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            &healthy(),
            &[("bay1".to_string(), "/pool/bay-1".to_string())],
        );
        assert_eq!(
            checks(&report),
            [
                ("ff/version", Level::Ok),
                ("bay/missing-directory", Level::Warn)
            ]
        );
        assert_eq!(report.findings, 1);
        let message = &report.rows[1].message;
        assert!(message.contains("ff worktree remove bay1"), "{message}");
        assert!(message.contains("/pool/bay-1"), "{message}");
    }

    #[test]
    fn an_orphan_alone_is_info_and_zero_findings() {
        // The branch is gone from the index — the release's leftovers
        // were already cleaned up, so only the chain remains, by design.
        let report = doctor(
            &fold(&[]),
            &reads(
                Vec::new(),
                Vec::new(),
                vec![worktree("main", Some("main"))],
                vec![orphan("bay1", Some("feather"))],
            ),
            &healthy(),
            &[],
        );
        assert_eq!(
            checks(&report),
            [("ff/version", Level::Ok), ("bay/orphan-chain", Level::Info)]
        );
        assert_eq!(report.findings, 0);
        assert!(
            report.rows[1].message.contains("ff restore --at-op xkvx"),
            "the tip is the way back to the work: {}",
            report.rows[1].message
        );
    }

    #[test]
    fn an_orphan_without_a_tip_still_reports() {
        let mut lone = orphan("bay1", None);
        lone.tip = None;
        let report = doctor(
            &fold(&[]),
            &reads(Vec::new(), Vec::new(), Vec::new(), vec![lone]),
            &healthy(),
            &[],
        );
        assert_eq!(report.rows[1].check, "bay/orphan-chain");
        assert!(!report.rows[1].message.contains("--at-op"));
    }

    #[test]
    fn an_orphans_surviving_branch_is_a_leftover_finding() {
        let report = doctor(
            &fold(&[]),
            &reads(
                Vec::new(),
                vec![branch("feather")],
                vec![worktree("main", Some("main"))],
                vec![orphan("bay1", Some("feather"))],
            ),
            &healthy(),
            &[],
        );
        assert_eq!(
            checks(&report),
            [
                ("ff/version", Level::Ok),
                ("bay/orphan-chain", Level::Info),
                ("bay/leftover-branch", Level::Warn),
            ]
        );
        assert_eq!(report.findings, 1);
        assert!(report.rows[2].message.contains("feather"));
    }

    #[test]
    fn a_branch_checked_out_in_a_live_worktree_is_not_a_leftover() {
        let report = doctor(
            &fold(&[]),
            &reads(
                Vec::new(),
                vec![branch("feather")],
                vec![
                    worktree("main", Some("main")),
                    worktree("bay2", Some("feather")),
                ],
                vec![orphan("bay1", Some("feather"))],
            ),
            &healthy(),
            &[],
        );
        assert_eq!(
            checks(&report),
            [("ff/version", Level::Ok), ("bay/orphan-chain", Level::Info)]
        );
        assert_eq!(report.findings, 0);
    }

    #[test]
    fn a_live_flights_derived_branch_is_not_a_leftover() {
        // The flight moved back onto the branch after the bay released —
        // work flies it, so the branch is not stale.
        let report = doctor(
            &fold(&[filed("pi.1", 10)]),
            &reads(
                vec![op("pi.1", Some("feather"), 50)],
                vec![branch("feather")],
                vec![worktree("main", Some("main"))],
                vec![orphan("bay1", Some("feather"))],
            ),
            &healthy(),
            &[],
        );
        assert_eq!(report.findings, 0);
        assert!(!checks(&report).contains(&("bay/leftover-branch", Level::Warn)));
    }

    #[test]
    fn a_done_flights_branch_does_not_shield() {
        let report = doctor(
            &fold(&[filed("pi.1", 10), done("pi.2", 70, "pi.1")]),
            &reads(
                vec![op("pi.1", Some("feather"), 50)],
                vec![branch("feather")],
                vec![worktree("main", Some("main"))],
                vec![orphan("bay1", Some("feather"))],
            ),
            &healthy(),
            &[],
        );
        assert_eq!(report.findings, 1);
        assert!(checks(&report).contains(&("bay/leftover-branch", Level::Warn)));
    }

    #[test]
    fn two_orphans_naming_one_branch_yield_one_row() {
        let report = doctor(
            &fold(&[]),
            &reads(
                Vec::new(),
                vec![branch("feather")],
                vec![worktree("main", Some("main"))],
                vec![
                    orphan("bay1", Some("feather")),
                    orphan("bay2", Some("feather")),
                ],
            ),
            &healthy(),
            &[],
        );
        let leftovers = report
            .rows
            .iter()
            .filter(|row| row.check == "bay/leftover-branch")
            .count();
        assert_eq!(leftovers, 1, "deduplicated per branch across orphans");
        assert_eq!(report.findings, 1);
    }
}

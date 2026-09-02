//! The doctor fold: stale bays, drift, and events off the board, as rows.
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
//!
//! The same rule sorts the log rows. The board says only how many events
//! are off it, because a count is all a board has room for; the causes
//! are four, they are not guessable from the count, and each has its own
//! answer — so doctor is where a person finds out which one they have.
//!
//! A retired kind is info, and the board does not count it at all. It is
//! tower's own former vocabulary: no fetch places it, no upgrade reads
//! it, and the flights it once moved carry their standing from the
//! events that replaced it. A permanent warning about history nobody can
//! change is a warning a person learns to scroll past, and the next one
//! that matters goes with it.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::ff::CONTRACT;
use crate::log::{EventId, Kind};

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

/// The checks, over facts the caller fetched. `gone` is the survey
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

    rows.extend(log_rows(fold));

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
        .filter(|flight| !flight.closed())
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

/// The unrouted events, sorted by cause, one row per cause. Every row
/// says what the events are and what places them, because "unrouted" on
/// its own is a word for a person to go looking with.
///
/// Ordered by what can be done about it: a chain to fetch, an upgrade to
/// run, then the two shapes no command fixes.
fn log_rows(fold: &Fold) -> Vec<DoctorRow> {
    let filed: HashSet<&EventId> = fold.flights.iter().map(|flight| &flight.id).collect();
    let known: HashSet<&str> = fold
        .flights
        .iter()
        .map(|flight| flight.id.writer.as_str())
        .collect();

    // Keyed for a stable row order out of an unordered log.
    let mut absent_chain: BTreeMap<&str, usize> = BTreeMap::new();
    let mut absent_filing: BTreeMap<&str, usize> = BTreeMap::new();
    let mut ahead: BTreeMap<&str, usize> = BTreeMap::new();
    let mut retired: BTreeMap<&str, usize> = BTreeMap::new();
    let mut refiled: Vec<String> = Vec::new();

    for event in &fold.retired {
        *retired.entry(event.kind.name()).or_default() += 1;
    }

    for event in &fold.unrouted {
        match &event.kind {
            Kind::Filed { .. } => refiled.push(event.id.to_string()),
            Kind::Unknown { kind, .. } => {
                *ahead.entry(kind.as_str()).or_default() += 1;
            }
            // Counted per event and per writer, not per id: a link with
            // both endpoints missing is one event against one writer when
            // they share one, and one against each when they do not.
            kind => {
                let mut counted: Vec<&str> = Vec::new();
                for id in named(kind) {
                    let writer = id.writer.as_str();
                    if filed.contains(id) || counted.contains(&writer) {
                        continue;
                    }
                    counted.push(writer);
                    let table = if known.contains(writer) {
                        &mut absent_filing
                    } else {
                        &mut absent_chain
                    };
                    *table.entry(writer).or_default() += 1;
                }
            }
        }
    }

    let mut rows = Vec::new();
    for (writer, count) in absent_chain {
        let (noun, verb) = agree(count, "names", "name");
        rows.push(row(
            Level::Warn,
            "log/absent-chain",
            format!(
                "{count} {noun} {verb} flights filed by `{writer}`, whose chain this repository does not have — fetch `refs/tower/log/*/{writer}` and they land on the board"
            ),
        ));
    }
    for (writer, count) in absent_filing {
        let (noun, verb) = agree(count, "names", "name");
        rows.push(row(
            Level::Warn,
            "log/absent-filing",
            format!(
                "{count} {noun} {verb} flights `{writer}` never filed, though its chain is here — the filing they answer is gone, which takes a hand-edited log"
            ),
        ));
    }
    for (kind, count) in ahead {
        let (noun, verb) = agree(count, "is", "are");
        rows.push(row(
            Level::Warn,
            "log/newer-tower",
            format!(
                "{count} {noun} {verb} `{kind}`, a kind this tower does not read — a newer tower wrote them, and `ff tower update` reads them"
            ),
        ));
    }
    for (kind, count) in retired {
        let (noun, verb) = agree(count, "is", "are");
        rows.push(row(
            Level::Info,
            "log/retired-kind",
            format!(
                "{count} {noun} {verb} `{kind}`, a kind tower has retired — they stay in the log, and the flights they moved carry their standing from later events"
            ),
        ));
    }
    for id in refiled {
        rows.push(row(
            Level::Warn,
            "log/refiled",
            format!(
                "{id} files a flight already filed — the first filing stands and this one is carried unread, which takes a hand-edited log"
            ),
        ));
    }
    rows
}

/// The flights an unrouted event names. `Filed` and `Unknown` name none —
/// what is wrong with those two is the event itself.
fn named(kind: &Kind) -> Vec<&EventId> {
    match kind {
        Kind::Status { flight, .. }
        | Kind::Assigned { flight, .. }
        | Kind::Commented { flight, .. }
        | Kind::Held { flight, .. }
        | Kind::Answered { flight, .. }
        | Kind::Routed { flight, .. } => vec![flight],
        Kind::Edited { target, .. } => vec![target],
        Kind::Linked { from, to } | Kind::Unlinked { from, to } => vec![from, to],
        // A view event names a view, never a flight: the fold routes it
        // by the view's id, so an unrouted one names a view never minted.
        Kind::ViewSaved {
            view: Some(view), ..
        }
        | Kind::ViewDeleted { view } => vec![view],
        Kind::Filed { .. } | Kind::ViewSaved { view: None, .. } | Kind::Unknown { .. } => {
            Vec::new()
        }
    }
}

/// Doctor counts in digits rather than words, so its nouns and verbs
/// agree by hand — the verb in whichever shape the row needs it.
fn agree(
    count: usize,
    singular: &'static str,
    plural: &'static str,
) -> (&'static str, &'static str) {
    if count == 1 {
        ("event", singular)
    } else {
        ("events", plural)
    }
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
                procedure: None,
                subject: format!("subject of {time}"),
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
        }
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Status {
                flight: flight.parse().expect("id"),
                status: "done".to_string(),
                reason: None,
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

    fn unknown(id: &str, time: i64, kind: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Unknown {
                kind: kind.to_string(),
                body: serde_json::value::RawValue::from_string("{}".to_string()).expect("body"),
            },
        }
    }

    fn commented(id: &str, time: i64, flight: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Commented {
                flight: flight.parse().expect("id"),
                text: "a note".to_string(),
            },
        }
    }

    /// The report over a log alone — no bays, a healthy seam.
    fn over(events: &[Event]) -> Doctor {
        doctor(
            &fold(events),
            &reads(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
            &healthy(),
            &[],
        )
    }

    #[test]
    fn a_retired_kind_is_info_and_no_finding() {
        // tower's own history: nothing routes it, and no command will, so
        // a row that drove the exit would be a chore with no end.
        let report = over(&[filed("pi.1", 10), unknown("pi.2", 20, "claimed")]);
        assert_eq!(
            checks(&report),
            [("ff/version", Level::Ok), ("log/retired-kind", Level::Info)]
        );
        assert_eq!(report.findings, 0);
        let message = &report.rows[1].message;
        assert!(message.contains("1 event"), "{message}");
        assert!(message.contains("claimed"), "{message}");
        assert!(message.contains("retired"), "{message}");
    }

    #[test]
    fn a_kind_from_ahead_is_a_finding_naming_the_upgrade() {
        let report = over(&[filed("pi.1", 10), unknown("pi.2", 20, "promoted")]);
        assert_eq!(
            checks(&report),
            [("ff/version", Level::Ok), ("log/newer-tower", Level::Warn)]
        );
        assert_eq!(report.findings, 1);
        let message = &report.rows[1].message;
        assert!(message.contains("promoted"), "{message}");
        assert!(message.contains("ff tower update"), "{message}");
    }

    #[test]
    fn an_absent_chain_names_the_ref_to_fetch() {
        let report = over(&[filed("pi.1", 10), commented("pi.2", 20, "qi.1")]);
        assert_eq!(
            checks(&report),
            [("ff/version", Level::Ok), ("log/absent-chain", Level::Warn)]
        );
        assert_eq!(report.findings, 1);
        let message = &report.rows[1].message;
        assert!(message.contains("refs/tower/log/*/qi"), "{message}");
    }

    #[test]
    fn a_missing_filing_on_a_chain_that_is_here_reads_as_hand_editing() {
        // `pi` filed pi.1, so the chain is present — a move naming pi.9
        // is a filing removed, not a chain unfetched.
        let report = over(&[filed("pi.1", 10), done("pi.3", 30, "pi.9")]);
        assert_eq!(
            checks(&report),
            [
                ("ff/version", Level::Ok),
                ("log/absent-filing", Level::Warn)
            ]
        );
        let message = &report.rows[1].message;
        assert!(message.contains("hand-edited"), "{message}");
    }

    #[test]
    fn one_writer_s_absent_flights_are_one_row() {
        let report = over(&[
            filed("pi.1", 10),
            commented("pi.2", 20, "qi.1"),
            commented("pi.3", 30, "qi.2"),
            commented("pi.4", 40, "zi.1"),
        ]);
        let absent: Vec<&str> = report
            .rows
            .iter()
            .filter(|row| row.check == "log/absent-chain")
            .map(|row| row.message.as_str())
            .collect();
        assert_eq!(absent.len(), 2, "one row per writer, not per event");
        assert!(absent[0].contains("2 events") && absent[0].contains("qi"));
        assert!(absent[1].contains("1 event") && absent[1].contains("zi"));
    }

    #[test]
    fn a_second_filing_of_one_id_is_a_finding() {
        let report = over(&[filed("pi.1", 10), filed("pi.1", 20)]);
        assert_eq!(
            checks(&report),
            [("ff/version", Level::Ok), ("log/refiled", Level::Warn)]
        );
        assert_eq!(report.findings, 1);
        assert!(report.rows[1].message.contains("pi.1"));
    }
}

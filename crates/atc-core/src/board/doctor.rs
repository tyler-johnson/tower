//! The doctor fold: the seam and the events off the board, as rows.
//!
//! Principle 5 is the verb's whole license: doctor observes and
//! complains, never enforces. Nothing here fixes, prunes, or retries —
//! every row is a report, and the hint inside it is a command for the
//! user to run, not one tower runs.
//!
//! Pure — no `crate::ff` spawns, no `std::process`; the fold runs over
//! facts the caller already fetched: the [`Fold`] the board runs on and
//! the seam's own answer from `ff version`. The registries and the
//! update cache are the CLI's rows, appended after this fold, because
//! they read files rather than the log.
//!
//! The board says only how many events are off it, because a count is
//! all a board has room for; the causes are four, they are not
//! guessable from the count, and each has its own answer — so doctor is
//! where a person finds out which one they have. Only actionable rows
//! drive the exit.
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

/// What `ff version` said about the seam, decided by the caller before
/// anything else. A drifted contract fails every spawn, so doctor is the
/// verb that reports the break instead of dying of it; a missing `ff` is
/// information, because the board folds tower's own log without one.
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
    /// The machine-matchable half — `log/refiled` and its kin.
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

/// The checks, over facts the caller fetched. One seam row first, at
/// the level the seam earned, then the log's.
pub fn doctor(fold: &Fold, seam: &SeamHealth) -> Doctor {
    let mut rows = Vec::new();

    let (level, message) = match seam {
        SeamHealth::Ok { version } => (Level::Ok, format!("ff {version} · contract {CONTRACT}")),
        SeamHealth::Drift { found } => (
            Level::Warn,
            format!(
                "ff speaks contract {found}; tower reads {CONTRACT} — upgrade whichever is behind"
            ),
        ),
        SeamHealth::Missing => (
            Level::Info,
            "no `ff` on PATH — fufu is optional; the board runs without it".to_string(),
        ),
    };
    rows.push(row(level, "ff", message));

    rows.extend(log_rows(fold));

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
                "{count} {noun} {verb} `{kind}`, a kind this tower does not read — a newer tower wrote them, and `atc update` reads them"
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
    use crate::log::{Event, EventId, Kind};

    fn filed(id: &str, time: i64) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            session: None,
            id,
            kind: Kind::Filed {
                procedure: None,
                subject: format!("subject of {time}"),
                body: String::new(),
                status: "backlog".to_string(),
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
            session: None,
            id,
            kind: Kind::Status {
                flight: flight.parse().expect("id"),
                status: "done".to_string(),
                reason: None,
            },
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
        let report = doctor(&fold(&[]), &healthy());
        assert_eq!(checks(&report), [("ff", Level::Ok)]);
        assert_eq!(report.findings, 0);
        assert!(report.rows[0].message.contains("0.9.0"));
        assert!(report.rows[0].message.contains("contract 1"));
    }

    #[test]
    fn drift_is_a_finding_and_absence_is_not() {
        let report = doctor(&fold(&[]), &SeamHealth::Drift { found: 99 });
        assert_eq!(checks(&report), [("ff", Level::Warn)]);
        assert_eq!(report.findings, 1);
        let message = &report.rows[0].message;
        assert!(message.contains("99") && message.contains('1'), "{message}");

        let report = doctor(&fold(&[]), &SeamHealth::Missing);
        assert_eq!(checks(&report), [("ff", Level::Info)]);
        assert_eq!(report.findings, 0);
        assert!(report.rows[0].message.contains("fufu is optional"));
    }

    fn unknown(id: &str, time: i64, kind: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            session: None,
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
            session: None,
            id,
            kind: Kind::Commented {
                flight: flight.parse().expect("id"),
                text: "a note".to_string(),
            },
        }
    }

    /// The report over a log alone, a healthy seam.
    fn over(events: &[Event]) -> Doctor {
        doctor(&fold(events), &healthy())
    }

    #[test]
    fn a_retired_kind_is_info_and_no_finding() {
        // tower's own history: nothing routes it, and no command will, so
        // a row that drove the exit would be a chore with no end.
        let report = over(&[filed("pi.1", 10), unknown("pi.2", 20, "claimed")]);
        assert_eq!(
            checks(&report),
            [("ff", Level::Ok), ("log/retired-kind", Level::Info)]
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
            [("ff", Level::Ok), ("log/newer-tower", Level::Warn)]
        );
        assert_eq!(report.findings, 1);
        let message = &report.rows[1].message;
        assert!(message.contains("promoted"), "{message}");
        assert!(message.contains("atc update"), "{message}");
    }

    #[test]
    fn an_absent_chain_names_the_ref_to_fetch() {
        let report = over(&[filed("pi.1", 10), commented("pi.2", 20, "qi.1")]);
        assert_eq!(
            checks(&report),
            [("ff", Level::Ok), ("log/absent-chain", Level::Warn)]
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
            [("ff", Level::Ok), ("log/absent-filing", Level::Warn)]
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
            [("ff", Level::Ok), ("log/refiled", Level::Warn)]
        );
        assert_eq!(report.findings, 1);
        assert!(report.rows[1].message.contains("pi.1"));
    }
}

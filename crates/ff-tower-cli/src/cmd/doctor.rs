//! `ff tower doctor` — stale bays and drift: observe and complain,
//! never enforce.
//!
//! Seam-first: `ff version` runs before any bay-facing read, because a
//! drifted contract fails every gather spawn — doctor is the verb that
//! reports the broken seam instead of dying of it. On a healthy seam the
//! pipeline is the board's — store, fold, gather — with the doctor fold
//! in place of `enrich`, plus the one impure check that stays out of the
//! fold: which surveyed directories still exist on disk.
//!
//! The exit is fufu's doctor precedent: 0 healthy, 1 findings, an
//! outcome riding the success path with a full envelope.

use std::path::Path;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, DoctorRow, Level, Reads, SeamHealth};
use ff_tower_core::ff::{self, BranchList};
use ff_tower_core::log::Store;
use ff_tower_core::procedure::Assignee;
use ff_tower_core::{procedure, skill};

pub fn run(json: bool) -> Result<i32, CliError> {
    let ff = super::ff()?;
    let seam = match ff.version() {
        Ok(version) => SeamHealth::Ok {
            version: version.version,
        },
        Err(ff::Error::Contract { found, .. }) => SeamHealth::Drift { found },
        Err(ff::Error::NotInstalled { .. }) => SeamHealth::Missing,
        Err(err) => return Err(err.into()),
    };

    let mut report = match &seam {
        SeamHealth::Ok { .. } => {
            let store = Store::open(ff.repo())?;
            let fold = board::fold(&store.read_all()?);
            let reads = board::gather(&ff)?;
            let gone: Vec<(String, String)> = reads
                .worktrees
                .iter()
                .filter_map(|row| {
                    let path = row.path.as_deref()?;
                    (!std::path::Path::new(path).exists())
                        .then(|| (row.id.clone(), path.to_string()))
                })
                .collect();
            board::doctor(&fold, &reads, &seam, &gone)
        }
        // A broken seam: no store, no gather — the seam row alone, and
        // absence of a bay row asserts nothing.
        _ => board::doctor(&board::fold(&[]), &no_reads(), &seam, &[]),
    };
    // The registries ride both arms too: they read files, never the
    // seam, so a drifted contract cannot hide an unresolved skill.
    let root = Store::open(ff.repo())
        .ok()
        .and_then(|store| store.main_worktree());
    for row in skill_rows(root.as_deref()) {
        if row.level == Level::Warn {
            report.findings += 1;
        }
        report.rows.push(row);
    }
    // The update row rides both arms: the passive lane's cache answers
    // without a seam, and its own row is why doctor suppresses the
    // generic notice. Info level — never a finding.
    report.rows.push(update_row());

    if json {
        println!("{}", machine::emit("doctor", &report));
    } else {
        let colored = render::colored();
        for row in &report.rows {
            match row.level {
                Level::Ok => println!(
                    "{}",
                    render::paint_dim(&format!("ok    {}", row.message), colored)
                ),
                Level::Info => println!(
                    "{}",
                    render::paint_dim(&format!("·     {}", row.message), colored)
                ),
                Level::Warn => println!("{}  {}", render::paint_warn("WARN", colored), row.message),
            }
        }
        if report.findings == 0 {
            println!("{}", render::paint_dim("healthy", colored));
        } else {
            let noun = if report.findings == 1 {
                "finding"
            } else {
                "findings"
            };
            println!("{} {noun}", report.findings);
        }
    }
    Ok(if report.findings == 0 { 0 } else { 1 })
}

/// Principle 5's half of the skill seam: a procedure naming a skill
/// nothing installs still loads and flies — nothing validates the link
/// at load — so doctor is where the unresolved name surfaces. A layer
/// that refuses to load is itself a Warn row naming the path rather
/// than a dead doctor.
fn skill_rows(root: Option<&Path>) -> Vec<DoctorRow> {
    let mut rows = Vec::new();
    let skills = match skill::registry(root) {
        Ok(registry) => registry,
        Err(err) => {
            rows.push(DoctorRow {
                level: Level::Warn,
                check: "skill/invalid".to_string(),
                message: err.to_string(),
            });
            return rows;
        }
    };
    let procedures = match procedure::registry(root) {
        Ok(registry) => registry,
        Err(err) => {
            rows.push(DoctorRow {
                level: Level::Warn,
                check: "procedure/invalid".to_string(),
                message: err.to_string(),
            });
            return rows;
        }
    };
    for definition in procedures.definitions() {
        for flight in &definition.flights {
            if flight.assignee != Assignee::Agent {
                continue;
            }
            let Some(name) = flight.skill.as_deref() else {
                continue;
            };
            if skills.get(name).is_none() {
                rows.push(DoctorRow {
                    level: Level::Warn,
                    check: "skill/unresolved".to_string(),
                    message: format!(
                        "procedure {} flies flight {} with skill `{name}`, which nothing installs — installed: {}",
                        definition.name,
                        flight.id,
                        skills.names().join(", ")
                    ),
                });
            }
        }
    }
    rows
}

/// The passive lane's cache, read and reported — the cache's two readers
/// are this row and `ff tower version`'s "available" line.
fn update_row() -> DoctorRow {
    use crate::selfupdate::notify::CheckStatus;
    let message = match crate::selfupdate::notify::check_status(env!("CARGO_PKG_VERSION")) {
        CheckStatus::Unofficial => "source build — updates via cargo install".to_string(),
        CheckStatus::NoCheckYet => "no check yet".to_string(),
        CheckStatus::Available(tag) => format!("{tag} available — `ff tower update`"),
        CheckStatus::UpToDate => format!("up to date (v{})", env!("CARGO_PKG_VERSION")),
    };
    DoctorRow {
        level: Level::Info,
        check: "tower/update".to_string(),
        message,
    }
}

/// Empty reads for the broken-seam path — nothing was gathered.
fn no_reads() -> Reads {
    Reads {
        ops: Vec::new(),
        branches: BranchList {
            named: Vec::new(),
            anonymous: Vec::new(),
        },
        current_branch: None,
        worktrees: Vec::new(),
        orphans: Vec::new(),
    }
}

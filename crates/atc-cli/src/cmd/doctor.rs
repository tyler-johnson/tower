//! `atc doctor` — the seam, the log, and the registries: observe
//! and complain, never enforce.
//!
//! Seam-first: `ff version` runs before anything else, because a
//! drifted contract fails every spawn — doctor is the verb that reports
//! the broken seam instead of dying of it. On a healthy seam the
//! pipeline is the board's — store, fold — with the doctor fold in
//! place of `enrich`.
//!
//! The exit is fufu's doctor precedent: 0 healthy, 1 findings, an
//! outcome riding the success path with a full envelope.

use std::path::Path;

use crate::error::CliError;
use crate::{machine, render};
use atc_core::board::{self, DoctorRow, Level, SeamHealth};
use atc_core::config::{self, Config};
use atc_core::ff;
use atc_core::log::Store;
use atc_core::model::Status;
use atc_core::procedure::Assignee;
use atc_core::{procedure, skill};

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
            board::doctor(&fold, &seam)
        }
        // A broken seam: no store — the seam row alone, and absence of
        // a log row asserts nothing.
        _ => board::doctor(&board::fold(&[]), &seam),
    };
    // The registries ride both arms too: they read files, never the
    // seam, so a drifted contract cannot hide an unresolved skill or a
    // shape that never comes back to a person.
    let root = Store::open(ff.repo())
        .ok()
        .and_then(|store| store.main_worktree());
    for row in registry_rows(root.as_deref()) {
        if row.level == Level::Warn {
            report.findings += 1;
        }
        report.rows.push(row);
    }
    if let Some(row) = stale_default_status(ff.repo()) {
        report.findings += 1;
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

/// The registry rows: principle 5's half of the skill seam, and
/// DESIGN.md's *Procedures* warning, a procedure should end with you.
///
/// A procedure naming a skill nothing installs still loads and flies —
/// nothing validates the link at load — so doctor is where the
/// unresolved name surfaces. The human end is the same shape of
/// complaint: the loader takes a definition whose terminal flights are
/// all agent-assigned, and doctor names it, by name and by flight. A
/// layer that refuses to load is itself a Warn row naming the path
/// rather than a dead doctor.
fn registry_rows(root: Option<&Path>) -> Vec<DoctorRow> {
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
        if let Some(terminal) = definition.no_human_end() {
            let noun = if terminal.len() == 1 {
                "flight"
            } else {
                "flights"
            };
            rows.push(DoctorRow {
                level: Level::Warn,
                check: "procedure/no-human-end".to_string(),
                message: format!(
                    "procedure {} ends on agent {noun} {} — a procedure should end with you",
                    definition.name,
                    terminal.join(", ")
                ),
            });
        }
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
                        "procedure {} flies flight {} with skill `{name}`, which nothing installs — {}",
                        definition.name,
                        flight.id,
                        installed_note(&skills)
                    ),
                });
            }
        }
    }
    rows
}

/// The setting a filing reads falls back to Ready without a word when
/// the set value is not one a flight can be filed with — so a
/// `defaultFileStatus = triage` left in git config from before the
/// rename would change every filing silently. Doctor is where it says so.
fn stale_default_status(repo: &Path) -> Option<DoctorRow> {
    let config = Config::open(repo).ok()?;
    let setting = config::lookup("defaultFileStatus").ok()?;
    let value = config.read(setting).value?;
    if Status::fileable(&value).is_some() {
        return None;
    }
    Some(DoctorRow {
        level: Level::Warn,
        check: "config/default-file-status".to_string(),
        message: format!(
            "tower.defaultFileStatus is `{value}`, not a word a flight can be filed with — filings land ready; atc config defaultFileStatus backlog"
        ),
    })
}

/// The tail of an unresolved-skill row. The engine ships empty, so no
/// skills at all is the ordinary case, and `installed: ` with nothing
/// after it would read like a bug rather than an answer.
fn installed_note(skills: &skill::Registry) -> String {
    if skills.is_empty() {
        "the skill shelf is empty".to_string()
    } else {
        format!("installed: {}", skills.names().join(", "))
    }
}

/// The passive lane's cache, read and reported — the cache's two readers
/// are this row and `atc version`'s "available" line.
fn update_row() -> DoctorRow {
    use crate::selfupdate::notify::CheckStatus;
    let message = match crate::selfupdate::notify::check_status(env!("CARGO_PKG_VERSION")) {
        CheckStatus::Unofficial => "source build — updates via cargo install".to_string(),
        CheckStatus::NoCheckYet => "no check yet".to_string(),
        CheckStatus::Available(tag) => format!("{tag} available — `atc update`"),
        CheckStatus::UpToDate => format!("up to date (v{})", env!("CARGO_PKG_VERSION")),
    };
    DoctorRow {
        level: Level::Info,
        check: "tower/update".to_string(),
        message,
    }
}

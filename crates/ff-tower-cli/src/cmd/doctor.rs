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

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Level, Reads, SeamHealth};
use ff_tower_core::ff::{self, BranchList};
use ff_tower_core::log::Store;

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

    let report = match &seam {
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

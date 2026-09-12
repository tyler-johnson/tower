//! `atc session [--mint]` — every session on this machine with a lease,
//! or a fresh id.
//!
//! Leases are per machine, so the listing opens no store: it reads
//! `lease::all()`, marks this process's own row, and takes the window
//! from the repository's config when there is one — `leaseWindow` —
//! and the compiled default otherwise. A read, not a sweep: a stale
//! lease is listed as stale and left where it is. `--mint` prints a
//! UUIDv7 and touches nothing, so the shells' rc lines can run it
//! inside `$(…)` at every interactive start with no repository in
//! sight. Neither form renews a lease.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::config::{self, Config};
use atc_core::lease;

pub fn run(json: bool, mint: bool) -> Result<(), CliError> {
    if mint {
        let session = lease::mint();
        if json {
            println!(
                "{}",
                machine::emit("session", &serde_json::json!({ "session": session }))
            );
        } else {
            println!("{session}");
        }
        return Ok(());
    }
    list(json)
}

/// One session's row: the lease body, its state against the window,
/// and whether it is this process's own.
struct Row {
    lease: lease::Lease,
    state: lease::State,
    this: bool,
}

fn list(json: bool) -> Result<(), CliError> {
    let window = std::env::current_dir()
        .ok()
        .and_then(|cwd| Config::open(&cwd).ok())
        .map(|config| config::lease_window(&config))
        .unwrap_or(lease::DEFAULT_WINDOW);
    let own = lease::session_key("");
    let rows: Vec<Row> = lease::all()
        .into_iter()
        .map(|(session, lease, mtime)| Row {
            state: lease::state(mtime, lease.pid(), window),
            this: own.as_deref() == Some(session.as_str()),
            lease,
        })
        .collect();

    if json {
        let sessions: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "session": row.lease.session,
                    "client": row.lease.client,
                    "callsign": row.lease.callsign,
                    "lease": row.state,
                    "pid": row.lease.pid,
                    "this": row.this,
                })
            })
            .collect();
        println!(
            "{}",
            machine::emit("session", &serde_json::json!({ "sessions": sessions }))
        );
        return Ok(());
    }

    let colored = render::colored();
    if rows.is_empty() {
        println!("no sessions on this machine");
        return Ok(());
    }
    let cells: Vec<[String; 5]> = rows
        .iter()
        .map(|row| {
            let dash = || "-".to_string();
            let lease = format!(
                "{} {}",
                if row.state.fresh { "fresh" } else { "stale" },
                lease::span(row.state.age)
            );
            let pid = match (row.lease.pid, row.state.pid_alive) {
                (Some(pid), Some(true)) => format!("{pid} alive"),
                (Some(pid), _) => format!("{pid} dead"),
                (None, _) => dash(),
            };
            [
                row.lease.session.clone(),
                row.lease.client.clone().unwrap_or_else(dash),
                row.lease.callsign.clone().unwrap_or_else(dash),
                lease,
                pid,
            ]
        })
        .collect();
    let header = ["session", "client", "callsign", "lease", "pid"];
    let widths: Vec<usize> = (0..5)
        .map(|column| {
            cells
                .iter()
                .map(|row| row[column].chars().count())
                .chain(std::iter::once(header[column].len()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: &[String]| -> String {
        cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:width$}"))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!(
        "{}",
        render::paint_dim(line(&header.map(str::to_string)).trim_end(), colored)
    );
    for (row, cells) in rows.iter().zip(&cells) {
        let mut text = line(cells);
        if row.this {
            text.push_str(&render::paint_dim("  this session", colored));
        }
        println!("{}", text.trim_end());
    }
    Ok(())
}

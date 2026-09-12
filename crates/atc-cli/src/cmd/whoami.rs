//! `atc whoami` — the identity read: who this process is, on every
//! axis the store resolves.
//!
//! The writer chain, the author from git, the client detected from its
//! mark; the session and which variable it came from — `launcher`,
//! `claude`, `codex`, `opencode`, `shell`, or the login name; the
//! callsign and its source — `env`, `session`, `client`, `login`; the
//! lease fresh or stale and by how much, and the pid when the session's
//! row hands one down. Bare `atc callsign` prints the same text under
//! its own `cmd`, so an agent that types the one it remembers gets the
//! answer; the render lives here and that verb calls it.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::lease;
use atc_core::log::Store;

pub fn run(json: bool) -> Result<(), CliError> {
    let store = super::store()?;
    print(json, &store, "whoami")
}

/// The render, under the envelope `cmd` the caller names. `--json`
/// carries `writer`, `author`, `client`, `session`, `session_source`,
/// `callsign`, `callsign_source`, `lease`, and `pid`.
pub fn print(json: bool, store: &Store, cmd: &str) -> Result<(), CliError> {
    let identity = store.identity();
    if json {
        println!(
            "{}",
            machine::emit(
                cmd,
                &serde_json::json!({
                    "writer": store.writer(),
                    "author": store.author(),
                    "client": identity.client,
                    "session": identity.session,
                    "session_source": identity.session_source,
                    "callsign": identity.callsign,
                    "callsign_source": identity.callsign_source,
                    "lease": identity.lease,
                    "pid": identity.pid.map(|pid| pid.pid),
                })
            )
        );
        return Ok(());
    }
    let colored = render::colored();
    let mut line = match (&identity.callsign, identity.callsign_source) {
        (Some(word), Some(source)) => format!("callsign {word} — {source}"),
        _ => "callsign none".to_string(),
    };
    match (&identity.session, identity.session_var()) {
        (Some(session), Some(var)) => line.push_str(&format!(" ({var} {session})")),
        (Some(session), None) => line.push_str(&format!(" (login {session})")),
        (None, _) => line.push_str(&render::paint_dim(" · no session", colored)),
    }
    println!("{line}");
    if let Some(state) = &identity.lease {
        let mut lease = format!(
            "lease {}, renewed {} ago",
            if state.fresh { "fresh" } else { "stale" },
            lease::span(state.age)
        );
        match (identity.pid, state.pid_alive) {
            (Some(pid), _) => lease.push_str(&format!(", pid {} alive", pid.pid)),
            (None, Some(true)) => lease.push_str(", pid alive"),
            (None, Some(false)) => lease.push_str(", pid dead"),
            (None, None) => lease.push_str(", no pid"),
        }
        println!("{lease}");
    }
    // The provenance stamps: the writer is `none` before the first
    // append mints one.
    let mut who = format!(
        "writer {} · author {}",
        store.writer().unwrap_or("none"),
        store.author()
    );
    if let Some(client) = identity.client {
        who.push_str(&format!(" · client {client}"));
    }
    println!("{who}");
    Ok(())
}

//! `atc callsign [<name>] [--force]` — name this session's pilot, or
//! say who you are.
//!
//! The verb's body lives in core's `verb::callsign`; this file is the
//! argument shape and the two renders. Bare is the identity read — the
//! callsign and where it came from, the session beside it, the lease's
//! state — shaped the way `atc whoami` will print it, so an agent that
//! types the one it remembers gets the answer. Named, it writes the
//! lease and reports what followed: the word it took from a stale
//! holder, the flights re-laned from the old word, and the ones left
//! there because someone else laned them.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::lease;
use atc_core::log::Store;
use atc_core::verb;

pub fn run(json: bool, name: Option<&str>, force: bool) -> Result<(), CliError> {
    let store = super::store()?;
    let Some(name) = name else {
        return whoami(json, &store);
    };
    let outcome = verb::callsign(&store, name, force)?;

    if json {
        println!("{}", machine::emit("callsign", &outcome.payload));
        return Ok(());
    }
    let colored = render::colored();
    let data = &outcome.payload;
    let mut line = format!("callsign {}", data.callsign);
    if data.renewed {
        line.push_str(&render::paint_dim(" (renewed)", colored));
    } else if let Some(previous) = &data.previous {
        let from = match outcome.previous_source {
            Some("session") => "this session",
            Some("client") => "the client",
            Some("login") => "the login name",
            _ => "before",
        };
        line.push_str(&render::paint_dim(
            &format!(" (was {previous}, {from})"),
            colored,
        ));
    }
    println!("{line}");
    if let Some(took) = &data.took {
        println!(
            "took {} from session {}, {}",
            data.callsign, took.session, took.detail
        );
    }
    for (row, name) in data.moved.iter().zip(&outcome.moved_display) {
        println!(
            "re-laned {} to {}: {}",
            render::paint_id(name, colored),
            data.callsign,
            row.subject
        );
    }
    if let Some(previous) = &data.previous
        && !data.left.is_empty()
    {
        for (row, name) in data.left.iter().zip(&outcome.left_display) {
            println!(
                "left in {previous}, assigned by {}: {} {}",
                row.by,
                render::paint_id(name, colored),
                row.subject
            );
        }
    }
    println!("{}", super::tail(colored));
    Ok(())
}

/// The identity read: who this process is, on every axis the store
/// resolves. `--json` carries the field set `atc whoami` names.
fn whoami(json: bool, store: &Store) -> Result<(), CliError> {
    let identity = store.identity();
    if json {
        println!(
            "{}",
            machine::emit(
                "callsign",
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
    Ok(())
}

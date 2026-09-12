//! `atc callsign [<name>] [--force]` — name this session's pilot, or
//! say who you are.
//!
//! The verb's body lives in core's `verb::callsign`; this file is the
//! argument shape and the write's render. Bare is the identity read —
//! what `atc whoami` prints, from `cmd/whoami.rs`, under this verb's
//! own `cmd` — so an agent that types the one it remembers gets the
//! answer. Named, it writes the lease and reports what followed: the
//! word it took from a stale holder, the flights re-laned from the old
//! word, and the ones left there because someone else laned them.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::verb;

pub fn run(json: bool, name: Option<&str>, force: bool) -> Result<(), CliError> {
    let store = super::store()?;
    let Some(name) = name else {
        return super::whoami::print(json, &store, "callsign");
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

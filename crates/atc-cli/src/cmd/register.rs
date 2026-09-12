//! `atc register [<callsign>] [--kind <kind>] [-m <msg>] [-d]` — the
//! roster: who flies here.
//!
//! Arity decides, `config`'s precedent: bare lists the roster, a
//! callsign with `--kind` registers or rewrites one, and `-d` retires
//! one. The verb's body lives in core's `verb::register`; this file is
//! the argument split and the render. A callsign given with neither
//! `--kind` nor `-d` is a coded refusal rather than clap's usage text,
//! so a `--json` caller gets an envelope.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::board::{self, Pilot};
use atc_core::verb;

pub fn run(
    json: bool,
    callsign: Option<&str>,
    kind: Option<&str>,
    message: Option<String>,
    retire: bool,
) -> Result<(), CliError> {
    let store = super::store()?;
    let colored = render::colored();

    let Some(callsign) = callsign else {
        let fold = board::fold(&store.read_all()?);
        if json {
            let roster: Vec<serde_json::Value> = fold
                .roster
                .iter()
                .map(|pilot| {
                    serde_json::json!({
                        "callsign": pilot.callsign,
                        "kind": pilot.kind,
                        "description": pilot.description,
                        "registered_at": pilot.registered_at,
                        "by": pilot.by,
                        "last_seen": pilot.last_seen,
                    })
                })
                .collect();
            println!(
                "{}",
                machine::emit("register", &serde_json::json!({ "roster": roster }))
            );
        } else {
            print!("{}", list(&fold.roster, board::now(), colored));
        }
        return Ok(());
    };

    if retire {
        let outcome = verb::retire(&store, callsign)?;
        if json {
            println!("{}", machine::emit("register", &outcome.payload));
        } else {
            println!("retired {}", outcome.payload.callsign);
            println!("{}", super::tail(colored));
        }
        return Ok(());
    }

    let Some(kind) = kind else {
        return Err(verb::Error::NeedsKind.into());
    };
    let outcome = verb::register(&store, callsign, kind, message.unwrap_or_default())?;
    if json {
        println!("{}", machine::emit("register", &outcome.payload));
    } else {
        let registered = &outcome.payload;
        let mut line = format!("registered {} · {}", registered.callsign, registered.kind);
        if !registered.description.is_empty() {
            line.push_str(&format!(": {}", registered.description));
        }
        println!("{line}");
        println!("{}", super::tail(colored));
    }
    Ok(())
}

/// The roster, one pilot per line: callsign, kind, when the callsign
/// was last seen on any event, and the description. Columns padded
/// across the list. Empty says how to add one.
fn list(roster: &[Pilot], now: i64, colored: bool) -> String {
    if roster.is_empty() {
        return format!(
            "{}\n",
            render::paint_dim(
                "no callsigns registered · atc register <callsign> --kind person|agent -m \"…\"",
                colored
            )
        );
    }
    let seen = |pilot: &Pilot| match pilot.last_seen {
        Some(at) => format!("seen {}", render::age(now, at)),
        None => "never".to_string(),
    };
    let name_width = roster
        .iter()
        .map(|pilot| pilot.callsign.chars().count())
        .max()
        .unwrap_or(0);
    let kind_width = roster
        .iter()
        .map(|pilot| pilot.kind.chars().count())
        .max()
        .unwrap_or(0);
    let seen_width = roster
        .iter()
        .map(|pilot| seen(pilot).chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for pilot in roster {
        let name = format!("{:<name_width$}", pilot.callsign);
        let mut line = format!(
            "{}  {}",
            render::paint_id(&name, colored),
            render::paint_dim(
                &format!("{:<kind_width$}  {:<seen_width$}", pilot.kind, seen(pilot)),
                colored
            )
        );
        if !pilot.description.is_empty() {
            line.push_str("  ");
            line.push_str(&pilot.description);
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

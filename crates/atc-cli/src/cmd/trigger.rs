//! `atc trigger [source]` — what a wired client runs on every event it
//! offers, and the verb a person runs to read the notice.
//!
//! Bare, it is a verb like any other: the notice for the repository you
//! are in, the ordinary refusal outside one, `--json` with the counts
//! beside the text. Named for a source, it is the command `atc hook`
//! wrote into that client's config, and it keeps fufu's client-source
//! doctrine: read the payload off stdin, do what the event calls for,
//! and on any failure exit 0 with nothing said — a hook's stderr is
//! noise in someone else's terminal, and a hook that fails loudly gets
//! uninstalled. A source it does not know is silent too.
//!
//! The dispatch is the source's event table. A boundary — a fresh
//! session, a resume, a clear, a compaction — renews the session's lease
//! and prints the notice wrapped the way the client reads it; activity
//! renews the lease and says nothing; the end releases it. A payload
//! with no `hook_event_name` is a boundary, so a stored `atc briefing
//! <client>` entry keeps doing what it did. The lease path opens no
//! store: a `touch` or an unlink, a few milliseconds, since it runs on
//! every tool call.
//!
//! The `shell` source is the four shells' rc lines: no payload — a
//! shell has none to hand down, and reading a piped interactive bash's
//! stdin would eat the rest of its script — so bare is activity, the
//! prompt's heartbeat, and `--end` is the end, the release at exit. It
//! has no boundary: a prompt has no context to inject a notice into.
//!
//! `atc briefing [client]` stays as an alias that prints the notice and
//! touches no lease, for the configs that still spell it; a source an
//! adapter that went once wrote — `cursor`, `gemini` — answers both
//! verbs forever, from `retired.rs`.

use crate::error::CliError;
use crate::integ::settings::Class;
use crate::integ::{self, briefing};
use crate::machine;
use atc_core::lease;

pub fn run(json: bool, source: Option<&str>, end: bool) -> Result<(), CliError> {
    let Some(source) = source else {
        return bare(json, "trigger");
    };
    // Machine surface: a name tower did not write is nothing to say,
    // not a refusal — the hook's stderr is someone else's terminal.
    let Some(integration) = integ::by_source(source) else {
        return Ok(());
    };
    let payload = if integration.carries_payload() {
        briefing::read_payload()
    } else {
        briefing::Payload::default()
    };
    let session = lease::session_key(&payload.session_id);
    // `--end` names the event where the source has no payload to; a
    // client source given it classes to nothing and stays silent.
    let name = if end {
        "end"
    } else {
        payload.hook_event_name.as_str()
    };
    match integration.class_of(Some(name)) {
        Some(Class::Boundary) => {
            if let Some(session) = &session {
                // A lease that would not write is not the notice's
                // problem: the boundary still delivers.
                let _ = lease::renew(session);
            }
            notice(json, integration, &payload)
        }
        Some(Class::Activity) => {
            if let Some(session) = &session {
                let _ = lease::renew(session);
            }
            Ok(())
        }
        Some(Class::End) => {
            if let Some(session) = &session {
                let _ = lease::release(session);
            }
            Ok(())
        }
        None => Ok(()),
    }
}

/// `atc briefing [client]`: the notice alone, as it always printed. The
/// client form refuses a name it does not know, because a person typing
/// this by hand deserves the answer; a retired one — `cursor`, `gemini`
/// — is a stored spelling and prints what it printed.
pub fn run_briefing(json: bool, client: Option<&str>) -> Result<(), CliError> {
    let Some(slug) = client else {
        return bare(json, "briefing");
    };
    let integration = integ::by_slug(slug)
        .or_else(|| integ::retired::by_source(slug))
        .ok_or_else(|| integ::verbs::unknown_slug(slug))?;
    let payload = briefing::read_payload();
    notice(json, integration, &payload)
}

/// The notice for the repository you are in, as a person reads it.
fn bare(json: bool, cmd: &str) -> Result<(), CliError> {
    let counts = briefing::counts(&super::repo()?)?;
    let text = briefing::text(counts.ready, counts.filed, &counts.on);
    if json {
        println!("{}", envelope(cmd, &text, &counts));
    } else {
        println!("{text}");
    }
    Ok(())
}

/// The notice at a boundary, in the client's envelope. No repository, a
/// store that will not open: the session is not one tower has anything
/// to say to. Exit 0, say nothing.
fn notice(
    json: bool,
    integration: &dyn integ::Integration,
    payload: &briefing::Payload,
) -> Result<(), CliError> {
    let Ok(counts) = briefing::counts(&payload.cwd()) else {
        return Ok(());
    };
    let text = briefing::text(counts.ready, counts.filed, &counts.on);
    if json {
        println!("{}", envelope("trigger", &text, &counts));
    } else {
        println!("{}", integration.envelope(&text));
    }
    Ok(())
}

fn envelope(cmd: &str, text: &str, counts: &briefing::Counts) -> String {
    machine::emit(
        cmd,
        &serde_json::json!({
            "text": text,
            "ready": counts.ready,
            "filed": counts.filed,
            "on": counts.on,
            "callsign": counts.callsign,
        }),
    )
}

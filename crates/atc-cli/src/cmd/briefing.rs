//! `atc briefing [client]` — the notice a wired client puts in front of
//! an agent, and the verb a person runs to read it.
//!
//! Bare, it is a verb like any other: the text for the repository you
//! are in, the ordinary refusal outside one, `--json` with the counts
//! beside the text. Named for a client, it is the command `atc hook`
//! wrote into that client's config, and it keeps fufu's client-source
//! doctrine: read the session's directory off stdin, wrap the text the
//! way the client reads it, and on any failure exit 0 with nothing said —
//! a hook's stderr is noise in someone else's terminal, and a hook that
//! fails loudly gets uninstalled.
//!
//! The pipeline is the fold alone — no gather, no spawn beyond the store
//! — because the question is the board's: how much is ready, and what
//! this callsign is already on.

use crate::error::CliError;
use crate::integ::{self, briefing};
use crate::machine;

pub fn run(json: bool, client: Option<&str>) -> Result<(), CliError> {
    let Some(slug) = client else {
        let counts = briefing::counts(&super::repo()?)?;
        let text = briefing::text(counts.ready, counts.filed, &counts.on);
        if json {
            println!("{}", envelope(&text, &counts));
        } else {
            println!("{text}");
        }
        return Ok(());
    };

    // Only tower writes this command into a client, so a name it does not
    // know is a refusal and not a silence: nothing legitimate spells it.
    let integration = integ::by_slug(slug).ok_or_else(|| integ::verbs::unknown_slug(slug))?;
    let payload = briefing::read_payload();
    // No repository, a store that will not open: the session is not one
    // tower has anything to say to. Exit 0, say nothing.
    let Ok(counts) = briefing::counts(&payload.cwd()) else {
        return Ok(());
    };
    let text = briefing::text(counts.ready, counts.filed, &counts.on);
    if json {
        println!("{}", envelope(&text, &counts));
    } else {
        println!("{}", integration.envelope(&text));
    }
    Ok(())
}

fn envelope(text: &str, counts: &briefing::Counts) -> String {
    machine::emit(
        "briefing",
        &serde_json::json!({
            "text": text,
            "ready": counts.ready,
            "filed": counts.filed,
            "on": counts.on,
            "callsign": counts.callsign,
        }),
    )
}

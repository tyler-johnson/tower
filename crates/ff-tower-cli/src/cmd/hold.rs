//! `ff tower hold <flight> -m <question>` — stop with a question attached.
//!
//! The one verb whose success is exit 3: the envelope is a full data
//! envelope with the held event in it, and only the code says the flight
//! stopped with a question — fufu's held-is-an-outcome precedent. The 3
//! itself lives in `main.rs`; this file returns `Ok(())` like any verb.
//! The body lives in core's `verb::hold`, where the server mounts it too
//! — and answers 200, HTTP having no exit-code channel.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::hold(&store, flight, message)?;

    if json {
        println!("{}", machine::emit("hold", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "held {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.question
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

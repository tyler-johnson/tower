//! `ff tower done <flight>` — finish a flight: off the board, on the
//! record.
//!
//! The verb's body lives in core's `verb::done`, where the server mounts
//! it too. What stays here is argument handling and the human render
//! around one core call.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let outcome = verb::done(&store, flight)?;

    if json {
        println!("{}", machine::emit("done", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "done {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

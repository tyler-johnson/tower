//! `ff tower claim <flight>` — claim one specific flight, out of order.
//!
//! The verb's body lives in core's `verb::claim`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let outcome = verb::claim(&store, flight)?;

    if json {
        println!("{}", machine::emit("claim", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "claimed {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

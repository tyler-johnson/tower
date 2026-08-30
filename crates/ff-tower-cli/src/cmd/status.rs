//! `ff tower status <flight> <status>` — move a flight.
//!
//! The verb's body lives in core's `verb::status`, where the server
//! mounts it too; this file is the argument order and the echo. `done`
//! and `cancel` are the same core append under their own cmd names.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, status: &str) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::status(&store, flight, status, None)?;

    if json {
        println!("{}", machine::emit("status", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "moved {} to {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.status.replace('_', " "),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

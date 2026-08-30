//! `ff tower cancel <flight> [-m <why>]` — off the board without the
//! finish.
//!
//! The verb's body lives in core's `verb::cancel`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::cancel(&store, flight, message)?;

    if json {
        println!("{}", machine::emit("cancel", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "canceled {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

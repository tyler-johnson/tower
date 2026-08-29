//! `ff tower answer <flight> -m <answer>` — answer the question, release
//! the hold.
//!
//! The verb's body lives in core's `verb::answer`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::answer(&store, flight, message)?;

    if json {
        println!("{}", machine::emit("answer", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "answered {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.answer
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

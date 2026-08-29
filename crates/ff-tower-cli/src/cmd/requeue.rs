//! `ff tower requeue <flight>` — hand the flight back to the pool.
//!
//! The verb's body lives in core's `verb::requeue`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let outcome = verb::requeue(&store, flight)?;

    if json {
        println!("{}", machine::emit("requeue", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "requeued {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

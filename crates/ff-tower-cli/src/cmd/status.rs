//! `ff tower status <flight> <status>` — move a flight.
//!
//! The verb's body lives in core's `verb::status`, where the server
//! mounts it too; this file is the argument order and the echo. `done`
//! and `cancel` are the same core append under their own cmd names.
//! The echo says where the flight landed, not what was typed: `ready`
//! on a flight with live dependencies lands it Waiting, and the line
//! says on how many.

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
        let landed = if outcome.landed == outcome.status {
            outcome.status.replace('_', " ")
        } else if outcome.landed == "waiting" {
            let flights = if outcome.waiting_on == 1 {
                "one flight".to_string()
            } else {
                format!("{} flights", super::count(outcome.waiting_on))
            };
            format!("waiting on {flights}")
        } else {
            outcome.landed.replace('_', " ")
        };
        println!(
            "moved {} to {landed}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

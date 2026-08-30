//! `ff tower assign <flight> <lane>` — whose queue this is in.
//!
//! The verb's body lives in core's `verb::assign`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, lane: &str) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::assign(&store, flight, lane)?;

    if json {
        println!("{}", machine::emit("assign", &outcome.payload));
    } else {
        let colored = render::colored();
        let name = render::paint_id(&outcome.display, colored);
        if outcome.lane == "none" {
            println!("cleared the lane on {name}: {}", outcome.subject);
        } else {
            println!("assigned {name} to {}: {}", outcome.lane, outcome.subject);
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

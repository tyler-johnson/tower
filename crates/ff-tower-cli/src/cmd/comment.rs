//! `ff tower comment <flight> -m <note>` — a note on a flight's record.
//!
//! The verb's body lives in core's `verb::comment`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::comment(&store, flight, message)?;

    if json {
        println!("{}", machine::emit("comment", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "commented on {}",
            render::paint_id(&outcome.display, colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

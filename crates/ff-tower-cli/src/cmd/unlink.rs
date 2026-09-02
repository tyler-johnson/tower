//! `ff tower unlink <a> <b>` — take back a declared dependency.
//!
//! The verb's body lives in core's `verb::unlink`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, a: &str, b: &str) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::unlink(&store, a, b)?;

    if json {
        println!("{}", machine::emit("unlink", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "unlinked {}: no longer depends on {}",
            render::paint_id(&outcome.from, colored),
            render::paint_id(&outcome.to, colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

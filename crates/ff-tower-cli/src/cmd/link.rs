//! `ff tower link <a> <b>` — declare that `a` depends on `b`.
//!
//! The verb's body lives in core's `verb::link`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, a: &str, b: &str) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::link(&store, a, b)?;

    if json {
        println!("{}", machine::emit("link", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "linked {}: depends on {}",
            render::paint_id(&outcome.from, colored),
            render::paint_id(&outcome.to, colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

//! The board verb — bare `ff tower`, and its `board` alias.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, ClosedWindow};

pub fn run(json: bool, closed: ClosedWindow) -> Result<(), CliError> {
    let store = super::store()?;
    let events = store.read_all()?;
    let now = board::now();
    let board = board::assemble(&events, now, closed);
    if json {
        println!("{}", machine::emit("board", &board));
    } else {
        print!("{}", render::board(&board, now, render::colored()));
    }
    Ok(())
}

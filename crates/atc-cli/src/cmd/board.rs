//! The board verb — bare `atc`, and its `board` alias.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::board::{self, ClosedWindow};

pub fn run(json: bool, closed: ClosedWindow) -> Result<(), CliError> {
    let store = super::store()?;
    store.touch(atc_core::log::sync::Touch::Ordinary)?;
    let now = board::now();
    let board = board::enrich(store.current()?, now, closed, store.callsign());
    if json {
        println!("{}", machine::emit("board", &board));
    } else {
        print!("{}", render::board(&board, now, render::colored()));
    }
    Ok(())
}

//! The board verb — bare `ff tower`, and its `board` alias.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, ClosedWindow};
use ff_tower_core::config::{self, Config};
use ff_tower_core::log::Store;

pub fn run(json: bool, closed: ClosedWindow) -> Result<(), CliError> {
    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let events = store.read_all()?;
    // The audit's threshold, read once and handed to the fold: the board
    // module reads no config, and a repository whose config will not open
    // gets the compiled default rather than a refusal.
    let stale_after = Config::open(ff.repo())
        .as_ref()
        .map(config::stale_flight_threshold)
        .unwrap_or(config::DEFAULT_STALE_FLIGHT);
    let now = board::now();
    let board = board::assemble(&ff, &events, now, stale_after, closed)?;
    if json {
        println!("{}", machine::emit("board", &board));
    } else {
        print!(
            "{}",
            render::board(&board, now, stale_after, render::colored())
        );
    }
    Ok(())
}

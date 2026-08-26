//! The board verb — bare `ff tower`, and its `board` alias.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Store;

pub fn run(json: bool) -> Result<(), CliError> {
    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let events = store.read_all()?;
    let board = board::assemble(&ff, &events)?;
    if json {
        println!("{}", machine::emit("board", &board));
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        print!("{}", render::board(&board, now, render::colored()));
    }
    Ok(())
}

//! `ff-tower` — the board, and the verbs around it.
//!
//! One binary under one name, and the name is fufu's dispatch name: `ff
//! tower <verb>` finds `ff-tower` on PATH the way git finds a subcommand.
//! There is no bare `tower` binary. The verb is reached through fufu
//! because the dependency is real — tower has nothing to say about a
//! machine with no fufu on it — and one spelling means no question about
//! which one is canonical.
//!
//! Exit codes: a board render is never a yes/no question, so success is 0,
//! an empty board included. Seam or store errors print `ff-tower: {err}`
//! to stderr and exit 1. Clap keeps its default 2 for usage; 3 has no
//! meaning for a render.

mod cli;
mod error;
mod machine;
mod render;

use clap::Parser;

use cli::{Cli, Command};
use error::CliError;
use ff_tower_core::board;
use ff_tower_core::ff::Ff;
use ff_tower_core::log::Store;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(&cli) {
        eprintln!("ff-tower: {err}");
        std::process::exit(1);
    }
}

fn run(cli: &Cli) -> Result<(), CliError> {
    match cli.command {
        None | Some(Command::Board) => board_verb(cli.json),
    }
}

fn board_verb(json: bool) -> Result<(), CliError> {
    let mut ff = Ff::here()?;
    // The test seam: environment carries addressing, argv carries verbs —
    // the seam's own discipline — and an env var cannot leak into an
    // interactive shell the way a hidden flag one autocomplete away could.
    if let Some(program) = std::env::var_os("TOWER_FF")
        && !program.is_empty()
    {
        ff = ff.program(program);
    }
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

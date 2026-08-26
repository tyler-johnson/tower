//! The command line. Bare `ff tower` is the board; `board` is an alias for
//! the bare form, so the muscle-memory spelling and the explicit one agree
//! by construction.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ff-tower", about = "tower: the board over fufu", version)]
pub struct Cli {
    /// Emit tower's machine envelope instead of the human render.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// The board — what is filed, what is moving, what is stuck.
    Board,
}

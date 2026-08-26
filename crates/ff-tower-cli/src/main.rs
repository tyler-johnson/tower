//! `ff-tower` — the board, and the verbs around it.
//!
//! One binary under one name, and the name is fufu's dispatch name: `ff
//! tower <verb>` finds `ff-tower` on PATH the way git finds a subcommand.
//! There is no bare `tower` binary. The verb is reached through fufu
//! because the dependency is real — tower has nothing to say about a
//! machine with no fufu on it — and one spelling means no question about
//! which one is canonical.
//!
//! Exit codes follow the error id's namespace, fufu's derivation:
//! `usage/*` exits 2, `held/*` exits 3 (reserved for the hold verb),
//! anything else 1. Success is 0, an empty board included — a board
//! render is never a yes/no question. Clap keeps its default 2 for a
//! command line it refused itself.

mod cli;
mod cmd;
mod error;
mod machine;
mod render;

use clap::Parser;

use cli::{Cli, Command};
use error::CliError;

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(&cli) {
        report(cli.json, verb(&cli.command), &err);
    }
}

/// The wire name of the verb being run — the envelope's `cmd`, success or
/// failure alike.
fn verb(command: &Option<Command>) -> &'static str {
    match command {
        None | Some(Command::Board) => "board",
        Some(Command::File { .. }) => "file",
        Some(Command::Comment { .. }) => "comment",
        Some(Command::Link { .. }) => "link",
    }
}

fn run(cli: &Cli) -> Result<(), CliError> {
    match &cli.command {
        None | Some(Command::Board) => cmd::board::run(cli.json),
        Some(Command::File {
            subject,
            message,
            procedure,
        }) => cmd::file::run(cli.json, subject, message.clone(), procedure.clone()),
        Some(Command::Comment { flight, message }) => {
            cmd::comment::run(cli.json, flight, message.clone())
        }
        Some(Command::Link { a, b }) => cmd::link::run(cli.json, a, b),
    }
}

/// Render one failure and exit — the single failure path, fufu's
/// `report()`. `--json` gets the error envelope on stdout, so a machine
/// caller always has an envelope to parse; a human gets `ff-tower: {err}`
/// on stderr with the exits as a `try:` block. Both exit by the id's
/// namespace.
fn report(json: bool, cmd: &str, err: &CliError) -> ! {
    if json {
        println!("{}", machine::emit_error(cmd, err));
    } else {
        eprintln!("ff-tower: {err}");
        let exits = err.exits();
        if !exits.is_empty() {
            eprintln!("  try:");
            for hint in exits {
                eprintln!("    {hint}");
            }
        }
    }
    std::process::exit(err.exit_code())
}

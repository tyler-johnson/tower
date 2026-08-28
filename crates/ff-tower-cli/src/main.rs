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
//! `usage/*` exits 2, anything else 1. The 3s are not among them — an
//! outcome, not an error, so they ride the success path below with a full
//! data envelope, and the `held/*` error namespace stays unused: `hold`'s
//! 3 says the flight stopped with a question, and `next`'s says the pool
//! is empty while unclassified or you-crewed work waits. `doctor`'s 1 is
//! the same kind of outcome as `next`'s: findings, reported in a full
//! data envelope, fufu's doctor precedent.
//! Success is otherwise 0, an empty board included — a board render is
//! never a yes/no question. Clap keeps its default 2 for a command line
//! it refused itself.

mod cli;
mod cmd;
mod error;
mod explain;
mod machine;
mod render;
mod selfupdate;

use clap::Parser;

use cli::{BayAction, Cli, Command};
use error::CliError;
use ff_tower_core::ff::Ff;

fn main() {
    let cli = Cli::parse();
    // `-V` is the version flag every other tool has; here it is lowercase.
    // Answered rather than parsed, so the person who typed the habit is
    // told the spelling instead of that the flag does not exist.
    if cli.version_shouted {
        report(
            cli.json,
            "version",
            &CliError::coded(
                "usage/bad-flags",
                "-V is not tower's spelling: the version flag is lowercase, and it is the \
                 verb — the same answer either way",
                vec!["ff tower -v".into(), "ff tower version".into()],
            ),
        );
    }
    // `-v` is the version verb spelled as a flag, and a verb does not ride
    // another verb: both on one line is two commands, not a flag and its
    // verb.
    if cli.version && cli.command.is_some() {
        report(
            cli.json,
            "version",
            &CliError::coded(
                "usage/bad-flags",
                "-v is the version verb spelled as a flag; it does not ride another verb",
                vec!["ff tower -v".into(), "ff tower version".into()],
            ),
        );
    }
    // The lanes are decided from the command line, before anything runs:
    // bare `ff tower` is the board and rides the board's, and bare
    // `ff tower -v` is the version verb spelled as a flag, so it takes
    // that verb's lane instead.
    let lanes = match (&cli.command, cli.version) {
        (None, true) => Command::Version.lanes(),
        (None, false) => Command::Board.lanes(),
        (Some(cmd), _) => cmd.lanes(),
    };

    // Preflight: the passive lane's background check. A lane must never
    // change what a command does, so the repository is best-effort — no
    // repo, no lane, never an error.
    let repo = if lanes.update {
        Ff::here().ok().map(|ff| ff.repo().to_path_buf())
    } else {
        None
    };
    if let Some(repo) = &repo {
        selfupdate::notify::maybe_spawn_check(repo);
    }

    match run(&cli) {
        // Safe to exit: stdout is line-buffered and every verb ends its
        // output with a newline, so nothing is left unflushed.
        Ok(code) => {
            // The trailer rides every success exit path — `hold`'s 3 and
            // `next`/`doctor`'s codes included — while errors leave
            // through `report()` and never reach it, fufu's rule. The
            // notice is stderr and tty-gated, so `--json` pipes never
            // see it.
            if let Some(repo) = &repo
                && let Some(notice) =
                    selfupdate::notify::pending(repo, env!("CARGO_PKG_VERSION"), lanes.notice)
            {
                eprintln!("{notice}");
                selfupdate::notify::mark_notified();
            }
            if code != 0 {
                std::process::exit(code);
            }
        }
        Err(err) => report(cli.json, verb(&cli.command, cli.version), &err),
    }
}

/// The wire name of the verb being run — the envelope's `cmd`, success or
/// failure alike. The version flag is the verb spelled as a flag, so the
/// envelope settles as `version` there too.
fn verb(command: &Option<Command>, version: bool) -> &'static str {
    match command {
        None if version => "version",
        None | Some(Command::Board) => "board",
        Some(Command::Next { .. }) => "next",
        Some(Command::Brief { .. }) => "brief",
        Some(Command::File { .. }) => "file",
        Some(Command::Comment { .. }) => "comment",
        Some(Command::Edit { .. }) => "edit",
        Some(Command::Link { .. }) => "link",
        Some(Command::Decompose { .. }) => "decompose",
        Some(Command::Procedures { .. }) => "procedures",
        Some(Command::Triage { .. }) => "triage",
        Some(Command::Claim { .. }) => "claim",
        Some(Command::Hold { .. }) => "hold",
        Some(Command::Answer { .. }) => "answer",
        Some(Command::Done { .. }) => "done",
        Some(Command::Explain { .. }) => "explain",
        Some(Command::Config { .. }) => "config",
        Some(Command::Bay { action }) => match action {
            None | Some(BayAction::List) => "bay list",
            Some(BayAction::Warm { .. }) => "bay warm",
            Some(BayAction::Release { .. }) => "bay release",
        },
        Some(Command::Version) => "version",
        Some(Command::Update { .. }) => "update",
        Some(Command::Doctor) => "doctor",
    }
}

/// Run the verb and pick the success exit code — 0 everywhere except
/// `hold`, whose 3 says the flight stopped with a question, `next`,
/// which picks its own — 1 is fufu's "no," a drained board, and 3 an
/// empty pick with work that needs you — and `doctor`, whose 1 says the
/// checks found something.
fn run(cli: &Cli) -> Result<i32, CliError> {
    match &cli.command {
        None if cli.version => cmd::version::run(cli.json)?,
        None | Some(Command::Board) => cmd::board::run(cli.json)?,
        Some(Command::Next { count, peek }) => {
            return cmd::next::run(cli.json, count.unwrap_or(1), *peek);
        }
        Some(Command::Brief { flight }) => cmd::brief::run(cli.json, flight)?,
        Some(Command::File {
            subject,
            message,
            procedure,
        }) => cmd::file::run(cli.json, subject, message.clone(), procedure.clone())?,
        Some(Command::Comment { flight, message }) => {
            cmd::comment::run(cli.json, flight, message.clone())?
        }
        Some(Command::Edit {
            target,
            subject,
            message,
        }) => cmd::edit::run(cli.json, target, subject.clone(), message.clone())?,
        Some(Command::Link { a, b }) => cmd::link::run(cli.json, a, b)?,
        Some(Command::Decompose { flight, parts }) => cmd::decompose::run(cli.json, flight, parts)?,
        Some(Command::Procedures { name }) => cmd::procedures::run(cli.json, name.as_deref())?,
        Some(Command::Triage {
            flight,
            procedure,
            message,
        }) => cmd::triage::run(
            cli.json,
            flight.as_deref(),
            procedure.clone(),
            message.clone(),
        )?,
        Some(Command::Claim { flight }) => cmd::claim::run(cli.json, flight)?,
        Some(Command::Hold { flight, message }) => {
            cmd::hold::run(cli.json, flight, message.clone())?;
            return Ok(3);
        }
        Some(Command::Answer { flight, message }) => {
            cmd::answer::run(cli.json, flight, message.clone())?
        }
        Some(Command::Done { flight }) => cmd::done::run(cli.json, flight.as_deref())?,
        Some(Command::Explain { id, list }) => cmd::explain::run(cli.json, id.as_deref(), *list)?,
        Some(Command::Config {
            key,
            value,
            unset,
            global,
        }) => cmd::config::run(cli.json, key.as_deref(), value.clone(), *unset, *global)?,
        Some(Command::Bay { action }) => cmd::bay::run(cli.json, action.as_ref())?,
        Some(Command::Version) => cmd::version::run(cli.json)?,
        Some(Command::Update { check }) => cmd::update::run(cli.json, *check)?,
        Some(Command::Doctor) => return cmd::doctor::run(cli.json),
    }
    Ok(0)
}

/// Render one failure and exit — the single failure path, fufu's
/// `report()`. `--json` gets the error envelope on stdout, so a machine
/// caller always has an envelope to parse; a human gets `ff-tower: {err}`
/// on stderr with the exits as a `try:` block. Both surfaces take their
/// exits from `explain::exits_for` — the raise site's when it spoke, the
/// registry's otherwise — and both exit by the id's namespace.
fn report(json: bool, cmd: &str, err: &CliError) -> ! {
    if json {
        println!("{}", machine::emit_error(cmd, err));
    } else {
        eprintln!("ff-tower: {err}");
        let exits = explain::exits_for(err);
        if !exits.is_empty() {
            eprintln!("  try:");
            for hint in exits {
                eprintln!("    {hint}");
            }
        }
    }
    std::process::exit(err.exit_code())
}

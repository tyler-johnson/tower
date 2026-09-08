//! The handshakes fufu asks before any verb runs — the declared-extension
//! contract's three pre-clap flags, answered here ahead of `Cli::parse()`.
//!
//! `ff extension add tower` spawns `ff-tower --ff-manifest` to learn what
//! tower is; `ff mcp` spawns `--ff-tools` to serve tower's verbs as typed
//! tools; `ff hook` spawns `--ff-skill <name>` for each skill the manifest
//! lists. Each is argv exactly `[flag]` or `[flag, name]`, env
//! `FF_NONINTERACTIVE=1` and nothing else — no `FF_REPO`, no
//! `FF_CONTRACT` — with stdin null and, for `--ff-tools`, a one-second
//! box. The reply is one line of envelope on stdout and exit 0; `error`
//! present is a refusal.
//!
//! The gate owns the line only when the flag is argv[1]. A flag anywhere
//! else falls through to clap, which refuses it as an unknown argument —
//! the handshakes are fufu's questions, not a person's, and they do not
//! ride a verb. Nothing here reads the environment, opens a store, or
//! spawns anything: the answers are compiled in, so they hold outside any
//! repository, which is where `ff extension add` asks.
//!
//! Every reply is the envelope whether or not `--json` was typed: the
//! callers are machines, and the contract reads stdout as JSON.

pub mod manifest;
pub mod skill;
pub mod tools;

use std::ffi::OsString;

use clap::CommandFactory;

use crate::cli::Cli;
use crate::error::CliError;
use crate::machine;

const MANIFEST: &str = "--ff-manifest";
const SKILL: &str = "--ff-skill";
const TOOLS: &str = "--ff-tools";

/// Answer a handshake if argv[1] is one, and hand back the exit code.
/// `None` is "not a handshake": the line is clap's.
pub fn answer(args: impl Iterator<Item = OsString>) -> Option<i32> {
    let mut args = args.skip(1);
    let flag = args.next()?;
    let flag = flag.to_str()?;
    if ![MANIFEST, SKILL, TOOLS].contains(&flag) {
        return None;
    }
    let rest: Vec<OsString> = args.collect();
    // A name that is not UTF-8 is not a skill name; clap says so.
    let rest: Vec<&str> = rest.iter().map(|arg| arg.to_str()).collect::<Option<_>>()?;

    let reply = match (flag, rest.as_slice()) {
        (MANIFEST, []) => Ok(machine::emit(flag, &manifest::manifest())),
        (SKILL, [name]) => skill::answer(name).map(|files| machine::emit(flag, &files)),
        (TOOLS, []) => Ok(machine::emit(flag, &tools::descriptors())),
        (SKILL, _) => Err(CliError::coded(
            "usage/bad-flags",
            format!("{flag} takes exactly one skill name; it is fufu's handshake, not a verb"),
            vec![],
        )),
        _ => Err(CliError::coded(
            "usage/bad-flags",
            format!("{flag} takes no arguments; it is fufu's handshake, not a verb"),
            vec![],
        )),
    };
    Some(match reply {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(err) => {
            println!("{}", machine::emit_error(flag, &err));
            err.exit_code()
        }
    })
}

/// The clap tree with its built-ins materialized — the `help` subcommand
/// and the auto flags exist to be skipped by name rather than by
/// accident of build order.
pub fn tree() -> clap::Command {
    let mut root = Cli::command();
    root.build();
    root
}

/// A command's verbs: its visible subcommands in clap order, `help`
/// excepted. The manifest's verb list and the tool list both walk this,
/// so the two cannot disagree about what a verb is.
pub fn verbs_of(cmd: &clap::Command) -> impl Iterator<Item = &clap::Command> {
    cmd.get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
}

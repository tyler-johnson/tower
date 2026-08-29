//! What the binary can fail with, and the id every failure answers to.
//!
//! fufu's convention, tower's registry: ids are `category/kebab-case`, and
//! the exit code derives from the namespace the way fufu's
//! `Error::exit_code()` derives it — `usage/*` is 2, `held/*` is 3,
//! anything else 1. The `held/*` namespace stays unused in practice:
//! hold's 3 is an outcome, and it rides the success path in `main.rs`.
//! The prose behind each id lives in `explain.rs`'s registry — `ff tower
//! explain <id>` — and the sync guards there hold the two surfaces
//! together; the wrapped errors name themselves through `id()` tables in
//! core, beside the variants, where the server reads the same answers.
//!
//! One forwarding rule: a refusal fufu shaped itself passes through
//! verbatim — its id, message, and exits are fufu's words, and wrapping
//! them in a tower id would hide the one part worth matching on.

use ff_tower_core::board::ResolveError;
use ff_tower_core::{config, ff, log, procedure, skill, verb};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Log(#[from] log::Error),
    /// A flight reference refused by core's resolver — its own id and
    /// exits, carried through.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    /// A config refusal — its own id, carried through.
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Ff(#[from] ff::Error),
    /// A definition the loader declined — its own id, carried through.
    #[error(transparent)]
    Procedure(#[from] procedure::Error),
    /// A skill layer that would not read — its own id, carried through.
    #[error(transparent)]
    Skill(#[from] skill::Error),
    /// A write verb refusing its input — core's table, so the server
    /// answers in the same words.
    #[error(transparent)]
    Verb(#[from] verb::Error),
    /// The server declining to start: a taken port, or a socket that
    /// would not open. The repository half of `serve`'s startup never
    /// lands here — see the `From` below.
    #[error(transparent)]
    Serve(ff_tower_serve::Error),
    /// A refusal tower shaped itself — a verb declining its input.
    #[error("{message}")]
    Coded {
        id: &'static str,
        message: String,
        exits: Vec<String>,
    },
}

impl CliError {
    pub fn coded(id: &'static str, message: impl Into<String>, exits: Vec<String>) -> CliError {
        CliError::Coded {
            id,
            message: message.into(),
            exits,
        }
    }

    /// The stable id — the machine-matchable half of every failure.
    /// Almost everything forwards: the wrapped errors name themselves in
    /// core, where the server reads the same tables.
    pub fn id(&self) -> &str {
        match self {
            CliError::Coded { id, .. } => id,
            CliError::Log(err) => err.id(),
            CliError::Resolve(err) => err.id(),
            CliError::Ff(err) => err.id(),
            // `Serve`'s repository half is flattened into `Log` by the
            // `From` below, so what survives to here always names
            // itself; the fallback is the id that half would have had.
            CliError::Serve(err) => err.id().unwrap_or("repo/error"),
            CliError::Procedure(err) => err.id(),
            CliError::Skill(err) => err.id(),
            CliError::Config(err) => err.id(),
            CliError::Verb(err) => err.id(),
        }
    }

    /// Commands that lead out of it. Empty where no command helps — the
    /// envelope carries `[]`, never null.
    pub fn exits(&self) -> Vec<String> {
        match self {
            CliError::Coded { exits, .. } => exits.clone(),
            CliError::Log(err) => err.exits(),
            CliError::Resolve(err) => err.exits(),
            CliError::Ff(err) => err.exits(),
            CliError::Procedure(err) => err.exits(),
            CliError::Verb(err) => err.exits(),
            CliError::Skill(_) => vec!["ff tower skills".to_string()],
            CliError::Config(config::Error::UnknownKey { .. }) => {
                vec!["ff tower config".to_string()]
            }
            CliError::Config(config::Error::BadValue { .. }) => {
                vec!["ff tower config <key>".to_string()]
            }
            _ => Vec::new(),
        }
    }

    /// The namespace is the exit code: `usage/*` 2, `held/*` 3, else 1.
    pub fn exit_code(&self) -> i32 {
        let id = self.id();
        if id.starts_with("usage/") {
            2
        } else if id.starts_with("held/") {
            3
        } else {
            1
        }
    }
}

/// Written by hand rather than derived, for the flattening: the server
/// validates the repository before it binds, and a store that refuses is
/// core's `log::Error` wearing a thin wrapper. It lands in `Log`, where
/// `identity/missing` and the rest already have their ids, instead of
/// growing a second table that could disagree with the first.
impl From<ff_tower_serve::Error> for CliError {
    fn from(err: ff_tower_serve::Error) -> CliError {
        match err {
            ff_tower_serve::Error::Repo(err) => CliError::Log(err),
            err => CliError::Serve(err),
        }
    }
}

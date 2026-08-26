//! What the binary can fail with, and the id every failure answers to.
//!
//! fufu's convention, tower's registry: ids are `category/kebab-case`, and
//! the exit code derives from the namespace the way fufu's
//! `Error::exit_code()` derives it — `usage/*` is 2, `held/*` is 3,
//! anything else 1. The `held/*` namespace stays unused in practice:
//! hold's 3 is an outcome, and it rides the success path in `main.rs`.
//! No `explain` verb and no registry file at this size; the mapping is
//! the match below.
//!
//! One forwarding rule: a refusal fufu shaped itself passes through
//! verbatim — its id, message, and exits are fufu's words, and wrapping
//! them in a tower id would hide the one part worth matching on.

use ff_tower_core::{ff, log, procedure};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Log(#[from] log::Error),
    #[error(transparent)]
    Ff(#[from] ff::Error),
    /// A definition the loader declined — its own id, carried through.
    #[error(transparent)]
    Procedure(#[from] procedure::Error),
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
    pub fn id(&self) -> &str {
        match self {
            CliError::Coded { id, .. } => id,
            CliError::Log(log::Error::Identity) => "identity/missing",
            CliError::Log(log::Error::Contended { .. }) => "log/contended",
            CliError::Log(log::Error::RefName { .. }) => "log/ref-name",
            CliError::Log(log::Error::NotTowerLog { .. }) => "log/not-tower",
            CliError::Log(log::Error::Version { .. }) => "log/version",
            CliError::Log(log::Error::Repo(_)) => "repo/error",
            CliError::Ff(ff::Error::Ff(refusal)) => &refusal.id,
            CliError::Ff(ff::Error::NotInstalled { .. }) => "ff/not-installed",
            CliError::Ff(ff::Error::Spawn { .. }) => "ff/spawn",
            CliError::Ff(ff::Error::Contract { .. }) => "ff/contract",
            CliError::Ff(ff::Error::Mismatched { .. }) => "ff/mismatched",
            CliError::Ff(ff::Error::Unparsable { .. }) => "ff/unparsable",
            CliError::Procedure(err) => err.id(),
        }
    }

    /// Commands that lead out of it. Empty where no command helps — the
    /// envelope carries `[]`, never null.
    pub fn exits(&self) -> Vec<String> {
        match self {
            CliError::Coded { exits, .. } => exits.clone(),
            CliError::Log(log::Error::Identity) => {
                vec!["git config user.email <email>".to_string()]
            }
            CliError::Ff(ff::Error::Ff(refusal)) => refusal.exits.clone(),
            CliError::Procedure(_) => vec!["ff tower procedures".to_string()],
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

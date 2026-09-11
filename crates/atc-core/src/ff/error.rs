//! What can go wrong on the way to a fufu answer.
//!
//! Five shapes, and the split that matters is the last one against the rest.
//! `Ff` is fufu saying no in its own words — a real, structured refusal that
//! came back through the contract, and the one kind a caller can reasonably
//! branch on. The other four are the seam failing to get an answer at all:
//! no `ff`, no launch, a contract tower does not read, or output that is not
//! an envelope. None of those are things fufu said; they are things that
//! happened instead of fufu saying anything.

use serde::Deserialize;

pub type Result<T> = std::result::Result<T, Error>;

/// A refusal fufu shaped itself, lifted out of the error envelope.
///
/// `id` is the stable half and the only part worth matching on —
/// `repo/not-found`, and its kin. The message is for a human and the wording
/// is fufu's to change. `exits` is the same block a terminal would have
/// printed: what to do about it, in fufu's vocabulary rather than tower's,
/// which is why tower passes it through rather than writing its own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Refusal {
    pub id: String,
    pub message: String,
    /// Commands that lead out of it. Absent in an older envelope, so it
    /// defaults rather than failing the parse.
    #[serde(default)]
    pub exits: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `ff` to spawn. fufu is tower's one runtime dependency and this is
    /// the single most likely way a fresh install fails, so it gets its own
    /// variant and says what to do instead of leaking an `io::Error`.
    #[error(
        "`{program}` is not on PATH — tower runs on fufu: https://github.com/tyler-johnson/fufu"
    )]
    NotInstalled { program: String },

    /// It exists and would not start.
    #[error("could not run `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    /// The envelope named a contract version tower does not read.
    ///
    /// Checked before the payload is touched, which is the whole reason the
    /// number is on every envelope: a shape tower cannot parse should say so
    /// in one line, not surface as a missing field three levels down.
    #[error(
        "`ff {verb}` speaks contract {found}; tower reads {expected}. Upgrade whichever is behind."
    )]
    Contract {
        verb: String,
        expected: u32,
        found: u32,
    },

    /// The envelope answered for a verb tower did not ask about. Not a
    /// failure fufu can produce on its own — it means something on PATH
    /// answering to `ff` is not fufu.
    #[error("asked `ff {asked}`, and the envelope answered for `{answered}`")]
    Mismatched { asked: String, answered: String },

    /// Nothing parseable came back. Carries what the process actually said,
    /// because this is the variant a person has to debug from: a usage error
    /// on stderr, a shim printing its own banner, a `--json` that was never
    /// passed.
    #[error("`ff {verb}` did not answer with a JSON envelope ({detail}){}{}",
        if stdout.is_empty() { String::new() } else { format!("\n  stdout: {stdout}") },
        if stderr.is_empty() { String::new() } else { format!("\n  stderr: {stderr}") })]
    Unparsable {
        verb: String,
        detail: String,
        stdout: String,
        stderr: String,
    },

    /// fufu refused, in its own words.
    #[error("{}", .0.message)]
    Ff(Refusal),
}

impl Error {
    /// The stable id, tower's `category/kebab-case`. One forwarding rule:
    /// a refusal fufu shaped itself passes through verbatim — its id is
    /// fufu's words, and wrapping it in a tower id would hide the one
    /// part worth matching on.
    pub fn id(&self) -> &str {
        match self {
            Error::Ff(refusal) => &refusal.id,
            Error::NotInstalled { .. } => "ff/not-installed",
            Error::Spawn { .. } => "ff/spawn",
            Error::Contract { .. } => "ff/contract",
            Error::Mismatched { .. } => "ff/mismatched",
            Error::Unparsable { .. } => "ff/unparsable",
        }
    }

    /// Commands that lead out of it: fufu's own exits when fufu said no,
    /// carried verbatim for the reason the id is; nothing otherwise.
    pub fn exits(&self) -> Vec<String> {
        match self {
            Error::Ff(refusal) => refusal.exits.clone(),
            _ => Vec::new(),
        }
    }

    /// The fufu error id when fufu is the one who said no, so a caller can
    /// match on `repo/not-found` without unwrapping the variant by hand.
    pub fn ff_id(&self) -> Option<&str> {
        match self {
            Error::Ff(refusal) => Some(refusal.id.as_str()),
            _ => None,
        }
    }
}

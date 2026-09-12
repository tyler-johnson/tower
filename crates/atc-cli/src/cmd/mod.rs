//! The verbs, in fufu's shape: one file per verb, the shared plumbing
//! here.
//!
//! A write verb is a read plus a local write. The repository is the
//! current directory — [`repo`] reads it and nothing spawns — and the
//! store opens on it. The lifecycle verbs live in core's `verb` module, where
//! the server mounts them too; their files here are argument handling
//! and the human render around one core call. What stays in this module
//! is the CLI's own half: the repository handles and the echo tail.

pub mod answer;
pub mod assign;
pub mod board;
pub mod brief;
pub mod callsign;
pub mod cancel;
pub mod comment;
pub mod config;
pub mod decompose;
pub mod doctor;
pub mod done;
pub mod edit;
pub mod explain;
pub mod file;
pub mod hold;
pub mod link;
pub mod next;
pub mod procedures;
pub mod serve;
pub mod session;
pub mod skills;
pub mod status;
pub mod trigger;
pub mod unlink;
pub mod update;
pub mod version;
pub mod whoami;

use std::path::PathBuf;

use crate::error::CliError;
use crate::render;
// The reference grammar, its resolution, and the display form live in
// core now, every surface's front door to one flight; so do the write
// verbs' shared guards and read-backs, in `verb`. The files here keep
// calling them as `super::…` through these re-exports.
pub use atc_core::board::{count, display, flight, parse_ref, resolve};
use atc_core::ff::Ff;
use atc_core::log::Store;

/// The repository tower was invoked in: the current directory. The
/// store discovers the worktree from it, so a subdirectory works.
pub fn repo() -> Result<PathBuf, CliError> {
    std::env::current_dir().map_err(|err| {
        CliError::coded(
            "repo/error",
            format!("cannot read the current directory: {err}"),
            vec![],
        )
    })
}

/// The repository handle for the verbs that spawn fufu, with core's
/// `ATC_FF` test seam applied.
pub fn ff() -> Result<Ff, CliError> {
    Ok(Ff::at(repo()?).env_program())
}

/// The store, opened on the current directory. The one place the
/// identity's notice prints: a session whose word was taken while it
/// was idle learns so here, one line on stderr, and the verb proceeds.
/// Serve opens its own stores per request and never prints.
pub fn store() -> Result<Store, CliError> {
    let store = Store::open(&repo()?)?;
    if let Some(notice) = &store.identity().notice {
        eprintln!("atc: {notice}");
    }
    Ok(store)
}

/// The standard dim tail — one string, every write verb.
pub fn tail(colored: bool) -> String {
    render::paint_dim("board: atc", colored)
}

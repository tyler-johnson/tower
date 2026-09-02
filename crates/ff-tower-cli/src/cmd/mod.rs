//! The verbs, in fufu's shape: one file per verb, the shared plumbing
//! here.
//!
//! A write verb is a read plus a local write. `Ff::here()` resolves the
//! repository — constructing the handle spawns nothing — and the store
//! opens on it. The lifecycle verbs live in core's `verb` module, where
//! the server mounts them too; their files here are argument handling
//! and the human render around one core call. What stays in this module
//! is the CLI's own half: the repository handles and the echo tail.

pub mod answer;
pub mod assign;
pub mod bay;
pub mod board;
pub mod brief;
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
pub mod skills;
pub mod status;
pub mod unlink;
pub mod update;
pub mod version;

use crate::error::CliError;
use crate::render;
// The reference grammar, its resolution, and the display form live in
// core now, every surface's front door to one flight; so do the write
// verbs' shared guards and read-backs, in `verb`. The files here keep
// calling them as `super::…` through these re-exports.
pub use ff_tower_core::board::{count, display, flight, parse_ref, resolve};
use ff_tower_core::ff::Ff;
use ff_tower_core::log::Store;

/// The repository handle for the verbs that spawn fufu, with core's
/// `TOWER_FF` test seam applied.
pub fn ff() -> Result<Ff, CliError> {
    Ok(Ff::here()?.env_program())
}

/// The store, opened on the repository fufu's dispatch handed us.
pub fn store() -> Result<Store, CliError> {
    let ff = Ff::here()?;
    Ok(Store::open(ff.repo())?)
}

/// The standard dim tail — one string, every write verb.
pub fn tail(colored: bool) -> String {
    render::paint_dim("board: ff tower", colored)
}

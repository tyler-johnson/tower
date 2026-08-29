//! The verbs, in fufu's shape: one file per verb, the shared plumbing
//! here.
//!
//! A write verb is a read plus a local write. `Ff::here()` resolves the
//! repository — constructing the handle spawns nothing — and the store
//! opens on it. The eight lifecycle verbs live in core's `verb` module
//! now, where the server mounts them too; their files here are argument
//! handling and the human render around one core call. What stays in
//! this module is the CLI's own half: the repository handles, `edit`'s
//! target resolution, and the echo tail.

pub mod answer;
pub mod bay;
pub mod board;
pub mod brief;
pub mod claim;
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
pub mod requeue;
pub mod serve;
pub mod skills;
pub mod take;
pub mod triage;
pub mod update;
pub mod version;

use crate::error::CliError;
use crate::render;
use ff_tower_core::board::Fold;
// The reference grammar, its resolution, and the display form live in
// core now, every surface's front door to one flight; so do the write
// verbs' shared guards and read-backs, in `verb`. The files here keep
// calling them as `super::…` through these re-exports.
pub use ff_tower_core::board::{FlightRef, count, display, flight, parse_ref, resolve};
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{EventId, Store};
pub use ff_tower_core::verb::{appended, appended_all, ensure_active};

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

/// Where an edit lands: a flight's record, or one comment on it.
pub enum EditTarget {
    Flight(EventId),
    Comment { flight: EventId, comment: EventId },
}

/// Resolve `edit`'s target: flights by any reference form, comments by
/// their full event id alone — the wire id is a comment's only name. A
/// sibling of `resolve` rather than a change to it, because every other
/// verb must keep refusing comment ids.
pub fn resolve_edit_target(fold: &Fold, text: &str) -> Result<EditTarget, CliError> {
    match parse_ref(text)? {
        FlightRef::Full(id) => {
            if fold.flights.iter().any(|flight| flight.id == id) {
                return Ok(EditTarget::Flight(id));
            }
            for flight in &fold.flights {
                if flight.comments.iter().any(|comment| comment.id == id) {
                    return Ok(EditTarget::Comment {
                        flight: flight.id.clone(),
                        comment: id,
                    });
                }
            }
            Err(CliError::coded(
                "flight/not-found",
                format!("`{text}` names neither a flight nor a comment"),
                vec!["ff tower".to_string()],
            ))
        }
        _ => Ok(EditTarget::Flight(resolve(fold, text)?)),
    }
}

/// The standard dim tail — one string, every write verb.
pub fn tail(colored: bool) -> String {
    render::paint_dim("board: ff tower", colored)
}

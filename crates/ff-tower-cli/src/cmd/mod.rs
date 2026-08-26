//! The verbs, in fufu's shape: one file per verb, the shared plumbing
//! here.
//!
//! A write verb is a read plus a local write. `Ff::here()` resolves the
//! repository — constructing the handle spawns nothing — the store opens
//! on it, and validation folds `read_all()`, the union of every writer,
//! so a verb can name a flight filed from another machine. Validation
//! lives at write time because a verb is the moment a typo is cheap to
//! catch; the fold stays tolerant of what got into the log anyway.

pub mod board;
pub mod comment;
pub mod file;
pub mod link;

use crate::error::CliError;
use crate::render;
use ff_tower_core::board::Fold;
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Event, EventId, Store};

/// The store, opened on the repository fufu's dispatch handed us.
pub fn store() -> Result<Store, CliError> {
    let ff = Ff::here()?;
    Ok(Store::open(ff.repo())?)
}

/// The event just appended, read back from this writer's chain so the
/// JSON payload is what the log holds — store-assigned time included —
/// not a reconstruction.
pub fn appended(store: &Store, id: &EventId) -> Result<Event, CliError> {
    Ok(store
        .read()?
        .into_iter()
        .find(|event| &event.id == id)
        .expect("the appended event is on the chain"))
}

/// Parse a positional as a flight id, refusing in the id's own words.
pub fn parse_flight(text: &str) -> Result<EventId, CliError> {
    text.parse()
        .map_err(|message: String| CliError::coded("usage/bad-flight", message, Vec::new()))
}

/// Refuse a target that is not a filed flight.
pub fn ensure_filed(fold: &Fold, id: &EventId) -> Result<(), CliError> {
    if fold.flights.iter().any(|flight| &flight.id == id) {
        return Ok(());
    }
    Err(CliError::coded(
        "flight/not-found",
        format!("no flight `{id}` on the board"),
        vec!["ff tower".to_string()],
    ))
}

/// The standard dim tail — one string, every write verb.
pub fn tail(colored: bool) -> String {
    render::paint_dim("board: ff tower", colored)
}

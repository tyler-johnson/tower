//! The board: a pure function of (repository, log).
//!
//! DESIGN.md's inbox, as a fold. The pipeline is three steps with the I/O
//! quarantined in the middle one: [`fold`] partitions the log's events
//! into flights and touches nothing else, [`gather`] makes the module's
//! only fufu spawns — exactly three, constant in flight count — and
//! [`enrich`] classifies each flight into a section over what `gather`
//! already fetched. [`assemble`] is the wiring, and the one call a render
//! needs.

mod flight;
mod model;
mod reads;

pub use flight::{Comment, Flight, Fold, Mark, Question, fold};
pub use model::{Board, FlightView, enrich};
pub use reads::{Reads, gather};

use crate::ff::{self, Ff};
use crate::log::Event;

/// One board: fold the log, gather the reads, enrich.
pub fn assemble(ff: &Ff, events: &[Event]) -> ff::Result<Board> {
    let reads = gather(ff)?;
    Ok(enrich(fold(events), &reads))
}

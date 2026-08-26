//! The board: a pure function of (repository, log).
//!
//! DESIGN.md's inbox, as a fold. The pipeline is four steps with the I/O
//! quarantined in the middle two: [`fold`] partitions the log's events
//! into flights and touches nothing else, [`gather`] makes three fufu
//! spawns constant in flight count, [`probe`] asks `collide` once per
//! distinct pair of in-flight branches — zero spawns in the solo norm —
//! and [`enrich`] classifies each flight into a section over what the
//! middle already fetched. [`assemble`] is the wiring, and the one call a
//! render needs. [`pick`] is `next`'s fold, riding the same probe output
//! as the board.

mod flight;
mod model;
mod pick;
mod reads;

pub use flight::{Comment, Flight, Fold, Mark, Question, fold};
pub use model::{Board, CollideView, FlightView, enrich};
pub use pick::{Passed, Pick, Picks, Skip, pick};
pub use reads::{BranchPairing, Reads, Verdicts, gather, probe};

use crate::ff::{self, Ff};
use crate::log::Event;

/// One board: fold the log, gather the reads, probe the pairs, enrich.
pub fn assemble(ff: &Ff, events: &[Event]) -> ff::Result<Board> {
    let fold = fold(events);
    let reads = gather(ff)?;
    let verdicts = probe(ff, &fold, &reads)?;
    Ok(enrich(fold, &reads, &verdicts))
}

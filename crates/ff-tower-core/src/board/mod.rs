//! The board: a pure function of (repository, log).
//!
//! DESIGN.md's inbox, as a fold. The pipeline is four steps with the I/O
//! quarantined in the middle two: [`fold`] partitions the log's events
//! into flights and touches nothing else, [`gather`] makes four fufu
//! spawns constant in flight count plus one `op log` per bay, [`probe`]
//! asks `collide` once per distinct pair of in-flight branches — zero
//! spawns in the solo norm — and [`enrich`] classifies each flight into a
//! section over what the middle already fetched. [`assemble`] is the
//! wiring, and the one call a render needs. [`pick`] is `next`'s fold,
//! riding the same probe output as the board. [`brief`] is one flight's
//! full record over the same reads, plus its standing over the walk's
//! own output — why it is where it is, and what it beat;
//! [`wants_verdicts`] tells the caller when the probes behind that walk
//! can change the answer. [`bays`] is the pool over
//! the same reads too: occupancy joined from the survey and the fold,
//! never registered, and [`assign`] hands each pick a bay out of that
//! same fold — the join `next` binds a flight in. [`doctor`] is the
//! health fold — stale bays and
//! drift as rows over the same reads plus the seam's own answer, and it
//! observes and complains, never enforces.

mod bay;
mod brief;
mod doctor;
mod flight;
mod model;
mod pick;
mod reads;

pub use bay::{BayView, Berth, assign, bays};
pub use brief::{Brief, CommentView, LinkView, Standing, brief, wants_verdicts};
pub use doctor::{Doctor, DoctorRow, Level, SeamHealth, doctor};
pub use flight::{Comment, Flight, Fold, Mark, Question, Route, fold};
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

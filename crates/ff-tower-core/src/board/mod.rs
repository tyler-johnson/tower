//! The board: a pure function of (repository, log).
//!
//! DESIGN.md's inbox, as a fold. The pipeline is four steps with the I/O
//! quarantined in the middle two: [`fold`] partitions the log's events
//! into flights and touches nothing else, [`gather`] makes four fufu
//! spawns constant in flight count plus one `op log` per bay, [`probe`]
//! asks `collide` once per distinct pair of in-flight branches — zero
//! spawns in the solo norm — and [`enrich`] groups each flight by the
//! status a person set, over what the middle already fetched, and audits
//! that status against the repository. [`assemble`] is the wiring, and
//! the one call a render needs; the clock and the stale threshold arrive
//! as arguments, because the fold reads neither for itself. [`pick`] is `next`'s fold,
//! riding the same probe output as the board. [`brief`] is one flight's
//! full record over the same reads, plus its standing over the walk's
//! own output — why it is where it is, and what it beat;
//! [`wants_verdicts`] tells the caller when the probes behind that walk
//! can change the answer, and [`history`] is the one thing the brief
//! cannot read off the fold — the moments themselves, filtered out of
//! the log the fold was built from, because last-wins marks keep no
//! record of how often a flight changed hands. [`bays`] is the pool over
//! the same reads too: occupancy joined from the survey and the fold,
//! never registered, and [`assign`] hands each pick a bay out of that
//! same fold — the join `next` binds a flight in. [`doctor`] is the
//! health fold — stale bays and
//! drift as rows over the same reads plus the seam's own answer, and it
//! observes and complains, never enforces. [`resolve`] is how a typed
//! reference becomes a flight id against the fold — every surface's
//! front door to one flight. [`Query`] is the second fold over the same
//! rows — filters, grouping, ordering and the display window as one
//! type, parsed once from a param string and shared by every surface,
//! where [`enrich`] stays the board's own fixed sectioning. [`views`] is
//! the saved-view set the fold minted from the log's `view_saved`
//! events, filtered to what one viewer sees.

mod bay;
mod brief;
mod doctor;
mod flight;
mod history;
mod model;
mod pick;
mod query;
mod reads;
mod resolve;
mod view;

pub use bay::{BayView, Berth, Pool, assign, bays};
pub use brief::{Brief, CommentView, LinkView, Standing, brief, wants_verdicts};
pub use doctor::{Doctor, DoctorRow, Level, SeamHealth, doctor};
pub use flight::{Comment, Flight, Fold, Mark, Question, fold};
pub use history::{Moment, history};
pub use model::{
    Board, ClosedWindow, CollideView, DEFAULT_CLOSED, FlightView, Rows, WaitingOnYou, enrich,
    parse_closed, rows,
};
pub use pick::{Passed, Pick, Picks, Skip, pick};
pub use query::{
    DEFAULT_SHOW, FIELDS, Field, Filter, Folded, Group, Mode, Op, Order, Query, QueryError, Value,
    When,
};
pub use reads::{BranchPairing, Reads, Verdicts, gather, probe};
pub use resolve::{FlightRef, ResolveError, count, display, flight, parse_ref, resolve};
pub use view::{View, views};

use crate::ff::{self, Ff};
use crate::log::Event;

/// One board: fold the log, gather the reads, probe the pairs, enrich.
///
/// `now`, `stale_after`, and `closed` ride in from the caller — the board
/// module reads no clock, no config, and no command line, so a board
/// stays a pure function of what it was handed. [`now`] takes the first,
/// `config::stale_flight_threshold` the second, and the third is
/// [`DEFAULT_CLOSED`] wherever nobody asked for another window.
pub fn assemble(
    ff: &Ff,
    events: &[Event],
    now: i64,
    stale_after: i64,
    closed: ClosedWindow,
) -> ff::Result<Board> {
    let fold = fold(events);
    let reads = gather(ff)?;
    let verdicts = probe(ff, &fold, &reads)?;
    Ok(enrich(fold, &reads, &verdicts, now, stale_after, closed))
}

/// Wall-clock seconds, taken once per invocation — the `now` every
/// surface hands the fold and its render, so one board and its rows
/// cannot disagree about what time it is. A clock before the epoch
/// answers `0`.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

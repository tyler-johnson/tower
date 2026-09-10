//! The board: a pure function of (repository, log).
//!
//! DESIGN.md's inbox, as a fold. The pipeline is three steps with the I/O
//! quarantined in the middle one: [`fold`] partitions the log's events
//! into flights and touches nothing else, [`gather`] makes three fufu
//! spawns constant in flight count, and [`enrich`] groups each flight by
//! the status a person set, over what the middle already fetched, and
//! audits that status against the repository. [`assemble`] is the wiring,
//! and the one call a render needs; the clock and the stale threshold
//! arrive as arguments, because the fold reads neither for itself.
//! [`pick`] is `next`'s fold, riding the same reads as the board.
//! [`brief`] is one flight's full record over the same reads, plus its
//! standing — why it is where it is — and [`history`] is the one thing
//! the brief cannot read off the fold — the moments themselves, filtered
//! out of the log the fold was built from, because last-wins marks keep
//! no record of how often a flight changed hands. [`doctor`] is the
//! health fold — the seam's own answer and the events off the board as
//! rows over the fold, and it observes and complains, never enforces.
//! [`resolve`] is how a typed reference becomes a flight id against the
//! fold — every surface's front door to one flight. [`Query`] is the
//! second fold over the same rows — filters, grouping, ordering and the
//! display window as one type, parsed once from a param string and
//! shared by every surface, where [`enrich`] stays the board's own fixed
//! sectioning. [`answer`] is the query's wiring the way [`assemble`] is
//! the board's: the same fold and gather, with [`rows`] and
//! [`Query::fold`] in place of [`enrich`]. [`views`] is the saved-view
//! set the fold minted from the log's `view_saved` events, filtered to
//! what one viewer sees.

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

pub use brief::{Brief, CommentView, LinkView, Standing, brief};
pub use doctor::{Doctor, DoctorRow, Level, SeamHealth, doctor};
pub use flight::{Comment, Flight, Fold, Mark, Question, fold};
pub use history::{Detail, Moment, history};
pub use model::{
    Board, ClosedWindow, DEFAULT_CLOSED, FlightView, Rows, WaitingOnYou, enrich, parse_closed, rows,
};
pub use pick::{Outcome, Pick, Picks, pick};
pub use query::{
    DEFAULT_SHOW, FIELDS, Field, Filter, Folded, Group, Mode, Op, Order, Query, QueryError, Value,
    When,
};
pub use reads::{Reads, gather};
pub use resolve::{FlightRef, ResolveError, count, display, flight, parse_ref, resolve};
pub use view::{View, views};

use crate::ff::{self, Ff};
use crate::log::Event;

/// One board: fold the log, gather the reads, enrich.
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
    Ok(enrich(fold, &reads, now, stale_after, closed))
}

/// One query's answer: fold the log, gather the reads, enrich every
/// flight into a row, and fold the rows through the query.
///
/// The same arguments as [`assemble`] with the query in place of the
/// closed window, which the query carries itself. `now` reaches both
/// the rows and the query's fold, so a relative filter and a row's age
/// read one clock.
pub fn answer(
    ff: &Ff,
    events: &[Event],
    now: i64,
    stale_after: i64,
    query: &Query,
) -> ff::Result<Folded> {
    let fold = fold(events);
    let reads = gather(ff)?;
    let rows = rows(fold, &reads, now, stale_after);
    Ok(query.fold(rows.flights, now))
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

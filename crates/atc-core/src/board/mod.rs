//! The board: a pure function of the log.
//!
//! DESIGN.md's inbox, as a fold. The pipeline is two steps and no I/O:
//! [`fold`] partitions the log's events into flights and touches nothing
//! else, and [`enrich`] groups each flight by the status its record
//! derives. [`assemble`] is the wiring, and the one call a render needs;
//! the clock arrives as an argument, because the fold reads none for
//! itself. [`pick`] is `next`'s lane walk over the same flights.
//! [`brief`] is one flight's full record over the same fold, plus its
//! standing — why it is where it is — and [`history`] is the one thing
//! the brief cannot read off the fold — the moments themselves, filtered
//! out of the log the fold was built from, because last-wins marks keep
//! no record of how often a flight changed hands. [`doctor`] is the
//! health fold — the seam's own answer and the events off the board as
//! rows over the fold, and it observes and complains, never enforces.
//! [`resolve`] is how a typed reference becomes a flight id against the
//! fold — every surface's front door to one flight. [`rewrite`] is the
//! same resolution over prose, at write time — a flight named in a body
//! or a note is stored as its wire id — and [`project`] puts the display
//! form back at render. [`Query`] is the
//! second fold over the same rows — filters, grouping, ordering and the
//! display window as one type, parsed once from a param string and
//! shared by every surface, where [`enrich`] stays the board's own fixed
//! sectioning. [`answer`] is the query's wiring the way [`assemble`] is
//! the board's: the same fold, with [`rows`] and [`Query::fold`] in
//! place of [`enrich`]. [`views`] is the saved-view
//! set the fold minted from the log's `view_saved` events, filtered to
//! what one viewer sees.

mod brief;
mod doctor;
mod flight;
mod history;
mod model;
mod pick;
mod query;
mod refs;
mod resolve;
mod view;

pub use brief::{Brief, CommentView, LinkView, Standing, brief};
pub use doctor::{Doctor, DoctorRow, Level, SeamHealth, doctor};
pub use flight::{Comment, Flight, Fold, Mark, Question, fold};
pub use history::{Detail, Moment, history};
pub use model::{
    Board, ClosedWindow, DEFAULT_CLOSED, FlightView, Rows, WaitingOnYou, enrich, parse_closed, rows,
};
pub use pick::{Lane, Outcome, Pick, Picks, pick, walk};
pub use query::{
    DEFAULT_SHOW, FIELDS, Field, Filter, Folded, Group, Mode, Op, Order, Query, QueryError, Value,
    When,
};
pub use refs::{named, project, rewrite};
pub use resolve::{FlightRef, ResolveError, count, display, flight, parse_ref, resolve};
pub use view::{View, views};

use crate::log::Event;

/// One board: fold the log, enrich.
///
/// `now`, `closed`, and `viewer` ride in from the caller — the board
/// module reads no clock, no command line, and no environment, so a
/// board stays a pure function of what it was handed. [`now`] takes the
/// first, the second is [`DEFAULT_CLOSED`] wherever nobody asked for
/// another window, and the third is the reader's callsign, which only
/// the `mine` flag reads.
pub fn assemble(events: &[Event], now: i64, closed: ClosedWindow, viewer: Option<&str>) -> Board {
    enrich(fold(events), now, closed, viewer)
}

/// One query's answer: fold the log, enrich every flight into a row, and
/// fold the rows through the query.
///
/// The same arguments as [`assemble`] with the query in place of the
/// closed window, which the query carries itself. `now` reaches the
/// query's fold, so a relative filter and a row's age read one clock.
pub fn answer(events: &[Event], now: i64, query: &Query, viewer: Option<&str>) -> Folded {
    let rows = rows(fold(events), viewer);
    query.fold(rows.flights, now)
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

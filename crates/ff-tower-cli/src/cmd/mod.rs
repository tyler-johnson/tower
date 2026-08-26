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

/// A flight reference as typed: a bare seq, or the full wire form.
pub enum FlightRef {
    Seq(u64),
    Full(EventId),
}

/// The syntactic half of naming a flight, before any store is opened. One
/// leading `#` is stripped for paste tolerance — output prints ids
/// `#`-prefixed, and what tower prints, tower accepts.
pub fn parse_ref(text: &str) -> Result<FlightRef, CliError> {
    let bare = text.strip_prefix('#').unwrap_or(text);
    if let Ok(seq) = bare.parse::<u64>() {
        return Ok(FlightRef::Seq(seq));
    }
    if let Ok(id) = bare.parse::<EventId>() {
        return Ok(FlightRef::Full(id));
    }
    Err(CliError::coded(
        "usage/bad-flight",
        format!("`{text}` is not a flight — `<seq>` or `<writer>.<seq>`"),
        Vec::new(),
    ))
}

/// Resolve a reference against the fold's filed flights. A bare seq must
/// match exactly one flight; the refusals quote the reference as the user
/// typed it, and an ambiguity names every candidate in full form.
pub fn resolve(fold: &Fold, text: &str) -> Result<EventId, CliError> {
    let not_found = || {
        CliError::coded(
            "flight/not-found",
            format!("no flight `{text}` on the board"),
            vec!["ff tower".to_string()],
        )
    };
    match parse_ref(text)? {
        FlightRef::Full(id) => {
            if fold.flights.iter().any(|flight| flight.id == id) {
                Ok(id)
            } else {
                Err(not_found())
            }
        }
        FlightRef::Seq(seq) => {
            let candidates: Vec<&EventId> = fold
                .flights
                .iter()
                .filter(|flight| flight.id.seq == seq)
                .map(|flight| &flight.id)
                .collect();
            match candidates.as_slice() {
                [] => Err(not_found()),
                [id] => Ok((*id).clone()),
                many => Err(CliError::coded(
                    "flight/ambiguous",
                    format!(
                        "`{text}` names {} flights: {}",
                        count(many.len()),
                        many.iter()
                            .map(|id| format!("`{id}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    vec!["ff tower".to_string()],
                )),
            }
        }
    }
}

/// Small counts in words, matching the refusal grammar's register.
fn count(n: usize) -> String {
    match n {
        2 => "two".to_string(),
        3 => "three".to_string(),
        4 => "four".to_string(),
        _ => n.to_string(),
    }
}

/// A resolved id in the board's display form, for verb echo lines. The
/// id itself joins the fold's flights in the writer count — `file` echoes
/// a flight its pre-append fold does not hold.
pub fn display(fold: &Fold, id: &EventId) -> String {
    let short = fold
        .flights
        .iter()
        .all(|flight| flight.id.writer == id.writer);
    render::flight_ref(&id.to_string(), short)
}

/// The standard dim tail — one string, every write verb.
pub fn tail(colored: bool) -> String {
    render::paint_dim("board: ff tower", colored)
}

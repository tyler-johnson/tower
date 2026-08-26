//! The verbs, in fufu's shape: one file per verb, the shared plumbing
//! here.
//!
//! A write verb is a read plus a local write. `Ff::here()` resolves the
//! repository — constructing the handle spawns nothing — the store opens
//! on it, and validation folds `read_all()`, the union of every writer,
//! so a verb can name a flight filed from another machine. Validation
//! lives at write time because a verb is the moment a typo is cheap to
//! catch; the fold stays tolerant of what got into the log anyway.

pub mod answer;
pub mod bay;
pub mod board;
pub mod brief;
pub mod claim;
pub mod comment;
pub mod decompose;
pub mod done;
pub mod file;
pub mod hold;
pub mod link;
pub mod next;

use crate::error::CliError;
use crate::render;
use ff_tower_core::board::{Flight, Fold};
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Event, EventId, Store};

/// The repository handle for the verbs that spawn fufu. The test seam:
/// environment carries addressing, argv carries verbs — the seam's own
/// discipline — and an env var cannot leak into an interactive shell the
/// way a hidden flag one autocomplete away could.
pub fn ff() -> Result<Ff, CliError> {
    let mut ff = Ff::here()?;
    if let Some(program) = std::env::var_os("TOWER_FF")
        && !program.is_empty()
    {
        ff = ff.program(program);
    }
    Ok(ff)
}

/// The store, opened on the repository fufu's dispatch handed us.
pub fn store() -> Result<Store, CliError> {
    let ff = Ff::here()?;
    Ok(Store::open(ff.repo())?)
}

/// The event just appended, read back from this writer's chain so the
/// JSON payload is what the log holds — store-assigned time included —
/// not a reconstruction.
pub fn appended(store: &Store, id: &EventId) -> Result<Event, CliError> {
    Ok(appended_all(store, std::slice::from_ref(id))?
        .into_iter()
        .next()
        .expect("the appended event is on the chain"))
}

/// The same, for a batch: one read of the chain, the events in the order
/// asked for rather than the chain's.
pub fn appended_all(store: &Store, ids: &[EventId]) -> Result<Vec<Event>, CliError> {
    let chain = store.read()?;
    Ok(ids
        .iter()
        .map(|id| {
            chain
                .iter()
                .find(|event| &event.id == id)
                .cloned()
                .expect("the appended event is on the chain")
        })
        .collect())
}

/// A flight reference as typed: a bare number, a `writer#n` pair, or the
/// full wire form `<writer>.<seq>`.
pub enum FlightRef {
    Number(u64),
    WriterNumber(String, u64),
    Full(EventId),
}

/// The syntactic half of naming a flight, before any store is opened. One
/// leading `#` is stripped for paste tolerance — output prints numbers
/// `#`-prefixed, and what tower prints, tower accepts (`#pi#3` pasted
/// still parses: the split is at the last `#`).
pub fn parse_ref(text: &str) -> Result<FlightRef, CliError> {
    let bare = text.strip_prefix('#').unwrap_or(text);
    if let Ok(number) = bare.parse::<u64>() {
        return Ok(FlightRef::Number(number));
    }
    if let Some((writer, digits)) = bare.rsplit_once('#')
        && let Ok(number) = digits.parse::<u64>()
    {
        return Ok(FlightRef::WriterNumber(writer.to_string(), number));
    }
    if let Ok(id) = bare.parse::<EventId>() {
        return Ok(FlightRef::Full(id));
    }
    Err(CliError::coded(
        "usage/bad-flight",
        format!("`{text}` is not a flight — `<n>`, `<writer>#<n>`, or `<writer>.<seq>`"),
        Vec::new(),
    ))
}

/// Resolve a reference against the fold's filed flights. A bare number
/// must match exactly one flight across writers; the refusals quote the
/// reference as the user typed it, and an ambiguity names every candidate
/// in `writer#n` form.
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
        FlightRef::WriterNumber(writer, number) => fold
            .flights
            .iter()
            .find(|flight| flight.id.writer == writer && flight.number == number)
            .map(|flight| flight.id.clone())
            .ok_or_else(not_found),
        FlightRef::Number(number) => {
            let candidates: Vec<&Flight> = fold
                .flights
                .iter()
                .filter(|flight| flight.number == number)
                .collect();
            match candidates.as_slice() {
                [] => Err(not_found()),
                [flight] => Ok(flight.id.clone()),
                many => Err(CliError::coded(
                    "flight/ambiguous",
                    format!(
                        "`{text}` names {} flights: {}",
                        count(many.len()),
                        many.iter()
                            .map(|flight| format!("`{}#{}`", flight.id.writer, flight.number))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    vec!["ff tower".to_string()],
                )),
            }
        }
    }
}

/// The fold's flight for a resolved id. Infallible after `resolve` — the
/// id came out of this fold's filed flights.
pub fn flight<'a>(fold: &'a Fold, id: &EventId) -> &'a Flight {
    fold.flights
        .iter()
        .find(|flight| &flight.id == id)
        .expect("resolved to a filed flight")
}

/// The flight, refused when it is already done. The lifecycle verbs stop
/// here; `comment` and `link` stay permissive on purpose — a note on the
/// record is fine.
pub fn ensure_active<'a>(fold: &'a Fold, id: &EventId) -> Result<&'a Flight, CliError> {
    let flight = flight(fold, id);
    if flight.done.is_some() {
        return Err(CliError::coded(
            "flight/done",
            format!("`{}` is done — the log keeps its record", display(fold, id)),
            vec!["ff tower".to_string()],
        ));
    }
    Ok(flight)
}

/// Small counts in words, matching the refusal grammar's register.
fn count(n: usize) -> String {
    match n {
        1 => "one".to_string(),
        2 => "two".to_string(),
        3 => "three".to_string(),
        4 => "four".to_string(),
        _ => n.to_string(),
    }
}

/// A resolved id in the board's display form — `#n`, or `writer#n` when
/// the fold's filed flights span more than one writer. Infallible after
/// `resolve`: the number lives on the fold's flight, so the flight must be
/// present — `file` re-folds after its append for exactly this.
pub fn display(fold: &Fold, id: &EventId) -> String {
    let short = fold
        .flights
        .iter()
        .all(|flight| flight.id.writer == id.writer);
    render::flight_ref(&id.writer, flight(fold, id).number, short)
}

/// The standard dim tail — one string, every write verb.
pub fn tail(colored: bool) -> String {
    render::paint_dim("board: ff tower", colored)
}

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
pub mod config;
pub mod decompose;
pub mod doctor;
pub mod done;
pub mod file;
pub mod hold;
pub mod link;
pub mod next;
pub mod procedures;
pub mod triage;
pub mod update;
pub mod version;

use crate::error::CliError;
use crate::render;
use ff_tower_core::board::{Flight, Fold};
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Event, EventId, Kind, PartStamp, Store};
use ff_tower_core::procedure::{Definition, Part};

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

/// Whose edge the parts hang from: `file`'s parent is the batch's own
/// first mint, `triage`'s already exists on the board.
pub enum Parent {
    Mint,
    Existing(EventId),
}

/// The classification batch, shared by `file` and `triage`: the head
/// event, then — when the definition has two or more parts — one filing
/// per part, the parent's edge to each part, and the `after` DAG between
/// parts, in the order the ids are read back out of. The head is the
/// caller's to build (`file` makes `Filed`, `triage` makes `Routed`); it
/// occupies `mint(0)` and the parts occupy `mint(1..)`.
///
/// A single-part definition collapses: the head carries the stamp and the
/// batch is one event — the reason this returns a plan rather than a
/// fixed shape.
pub fn classify(
    definition: &Definition,
    subject: &str,
    parent: Parent,
    head: impl FnOnce(Option<PartStamp>) -> Kind,
    mint: &dyn Fn(usize) -> EventId,
) -> Vec<Kind> {
    let parts = &definition.parts;
    if let [only] = parts.as_slice() {
        return vec![head(Some(stamp(only)))];
    }

    let mut kinds = vec![head(None)];
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached, so the part's subject carries the parent's.
    kinds.extend(parts.iter().map(|part| Kind::Filed {
        procedure: definition.name.clone(),
        subject: format!("{subject} · {}", part.id),
        body: String::new(),
        part: Some(stamp(part)),
    }));
    let from = match &parent {
        Parent::Mint => mint(0),
        Parent::Existing(id) => id.clone(),
    };
    kinds.extend((0..parts.len()).map(|offset| Kind::Linked {
        from: from.clone(),
        to: mint(offset + 1),
    }));
    for (offset, part) in parts.iter().enumerate() {
        for after in &part.after {
            // Infallible: the loader refused an `after` naming nothing.
            let at = parts
                .iter()
                .position(|other| &other.id == after)
                .expect("`after` names a part the loader validated");
            kinds.push(Kind::Linked {
                from: mint(offset + 1),
                to: mint(at + 1),
            });
        }
    }
    kinds
}

/// A definition's part, as the log carries it. The closed enums become
/// their names here; the log stays tolerant where the config stays closed.
pub fn stamp(part: &Part) -> PartStamp {
    PartStamp {
        id: part.id.clone(),
        crew: part.crew.name().to_string(),
        skill: part.skill.clone(),
        done: part.done.name().to_string(),
        bay: part.bay.map(|bay| bay.name().to_string()),
    }
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

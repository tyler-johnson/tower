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
pub mod edit;
pub mod explain;
pub mod file;
pub mod hold;
pub mod link;
pub mod next;
pub mod procedures;
pub mod requeue;
pub mod serve;
pub mod skills;
pub mod take;
pub mod triage;
pub mod update;
pub mod version;

use crate::error::CliError;
use crate::render;
use ff_tower_core::board::{Flight, Fold};
// The reference grammar and its resolution live in core now, every
// surface's front door to one flight; the verbs keep calling them as
// `super::parse_ref` and `super::resolve` through this re-export.
pub use ff_tower_core::board::{FlightRef, count, parse_ref, resolve};
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Event, EventId, Kind, PartStamp, Store};
use ff_tower_core::procedure::{Definition, Part};

/// The repository handle for the verbs that spawn fufu, with core's
/// `TOWER_FF` test seam applied.
pub fn ff() -> Result<Ff, CliError> {
    Ok(Ff::here()?.env_program())
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
        return vec![head(Some(stamp(definition, subject, only)))];
    }

    let mut kinds = vec![head(None)];
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached, so the part's subject carries the parent's.
    kinds.extend(parts.iter().map(|part| Kind::Filed {
        procedure: definition.name.clone(),
        subject: format!("{subject} · {}", part.id),
        body: String::new(),
        part: Some(stamp(definition, subject, part)),
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
///
/// `branch` is the definition's `subject` rule resolved once, at file
/// time: a procedure whose subject *is* a branch stamps the branch each
/// part flies on, so `next` binds from the stamp rather than reading the
/// definition again. The subject handed in is the flight's own, never the
/// `"{subject} · {part.id}"` a part is filed under — for `review`, the
/// branch is the parent's subject.
pub fn stamp(definition: &Definition, subject: &str, part: &Part) -> PartStamp {
    PartStamp {
        id: part.id.clone(),
        crew: part.crew.name().to_string(),
        skill: part.skill.clone(),
        done: part.done.name().to_string(),
        bay: part.bay.map(|bay| bay.name().to_string()),
        branch: (definition.subject.as_deref() == Some("branch")).then(|| subject.to_string()),
    }
}

/// Where an edit lands: a flight's record, or one comment on it.
pub enum EditTarget {
    Flight(EventId),
    Comment { flight: EventId, comment: EventId },
}

/// Resolve `edit`'s target: flights by any reference form, comments by
/// their full event id alone — the wire id is a comment's only name. A
/// sibling of `resolve` rather than a change to it, because every other
/// verb must keep refusing comment ids.
pub fn resolve_edit_target(fold: &Fold, text: &str) -> Result<EditTarget, CliError> {
    match parse_ref(text)? {
        FlightRef::Full(id) => {
            if fold.flights.iter().any(|flight| flight.id == id) {
                return Ok(EditTarget::Flight(id));
            }
            for flight in &fold.flights {
                if flight.comments.iter().any(|comment| comment.id == id) {
                    return Ok(EditTarget::Comment {
                        flight: flight.id.clone(),
                        comment: id,
                    });
                }
            }
            Err(CliError::coded(
                "flight/not-found",
                format!("`{text}` names neither a flight nor a comment"),
                vec!["ff tower".to_string()],
            ))
        }
        _ => Ok(EditTarget::Flight(resolve(fold, text)?)),
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
/// here; `comment`, `link`, and `edit` stay permissive on purpose — a
/// note on the record is fine, and a wrong word in a closed record is
/// exactly what `edit` is for.
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

//! `triage <flight> -p <procedure> [-m <why>]` — route one flight to a
//! procedure.
//!
//! One `routed` event re-stamps the flight, with the explanation stored
//! beside it — deterministic and stored, never recomputed, principle 11
//! applied to routing. The collapse rule is `file`'s, and the same
//! `classify` batch builds both: a single-part procedure stamps the
//! flight itself; a multi-part one makes the flight a parent and the
//! same atomic batch files its parts on `decompose`'s edges, so no part
//! is ever live and unlinked.
//!
//! Routing a claimed flight is allowed and the claim stands — a claim is
//! not a classification. Routing back to `open` is the undo, no special
//! case. Re-routing after a multi-part route leaves the old part flights
//! live, closed by hand.

use serde::Serialize;

use crate::board;
use crate::log::{Event, EventId, Kind, Store};
use crate::procedure;

use super::{Error, Parent, appended, appended_all, classify, ensure_active};

/// The envelope's `data`. Struct fields serialize in declaration order,
/// and this order — `routed, parts, linked` — is the one the CLI's
/// `json!` emitted before the payload moved here, so the bytes on the
/// wire never changed.
#[derive(Serialize)]
pub struct Routed {
    pub routed: Event,
    pub parts: Vec<Event>,
    pub linked: Vec<Event>,
}

/// The outcome: the payload, plus the ids a human render echoes — it
/// re-folds for the display numbers, so the machine path never has to.
pub struct Route {
    pub payload: Routed,
    pub flight: EventId,
    pub part_ids: Vec<EventId>,
}

pub fn route(
    store: &Store,
    flight: &str,
    procedure: &str,
    message: Option<String>,
) -> Result<Route, Error> {
    board::parse_ref(flight)?;
    let name = procedure.trim();
    if name.is_empty() {
        return Err(Error::EmptyProcedure);
    }

    let installed = procedure::registry(store.main_worktree().as_deref())?;
    let fold = board::fold(&store.read_all()?);
    let id = board::resolve(&fold, flight)?;
    let subject = ensure_active(&fold, &id)?.subject.clone();
    let definition = installed.require(name)?;

    let because = message.unwrap_or_default();
    let ids = store.append_with(|mint| {
        classify(
            definition,
            &subject,
            Parent::Existing(id.clone()),
            |part| Kind::Routed {
                flight: id.clone(),
                procedure: definition.name.clone(),
                part,
                because: because.clone(),
            },
            mint,
        )
    })?;
    let (routed, rest) = ids.split_first().expect("the routed event is the first");
    let parts = if definition.parts.len() == 1 {
        0
    } else {
        definition.parts.len()
    };
    let (minted, linked) = rest.split_at(parts);

    Ok(Route {
        payload: Routed {
            routed: appended(store, routed)?,
            parts: appended_all(store, minted)?,
            linked: appended_all(store, linked)?,
        },
        flight: id,
        part_ids: minted.to_vec(),
    })
}

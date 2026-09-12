//! `assign <flight> <lane>` — whose queue this is in.
//!
//! Four shapes: `me`, `agent`, `none` to clear the lane, or a callsign.
//! The stored field is the whole gate — the lane is what `next <lane>`
//! walks, and the unassigned lane is everyone's overflow, so assigning
//! is what moves a flight from one walk to another. `me` stores the
//! caller's callsign when there is one. A
//! malformed word refuses at this boundary; the wire itself stays a
//! free string, and a callsign needs no registration.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended, ensure_active, lane_word, stored_lane};

/// The envelope's `data`: the assignment, as the log holds it.
#[derive(Serialize)]
pub struct Assigned {
    pub assigned: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Assign {
    pub payload: Assigned,
    pub display: String,
    pub subject: String,
    /// The lane as stored — `me`, `agent`, a callsign, or `none` for the
    /// cleared lane.
    pub lane: String,
}

pub fn assign(store: &Store, flight: &str, lane: &str) -> Result<Assign, Error> {
    let assignee = stored_lane(lane_word(lane)?, store.callsign());
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Assigned {
        flight: flight.clone(),
        assignee: assignee.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one assigned event");

    Ok(Assign {
        payload: Assigned {
            assigned: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        subject,
        lane: assignee.unwrap_or_else(|| "none".to_string()),
    })
}

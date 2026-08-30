//! `assign <flight> <lane>` — whose queue this is in.
//!
//! Three words: `me`, `agent`, or `none` to clear the lane. The stored
//! field is the whole gate — `next` draws only from the agent lane, so
//! assigning is what opens or closes it. Anything else refuses at this
//! boundary; the wire itself stays a free string.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};
use crate::model::Assignee;

use super::{Error, appended, ensure_active};

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
    /// The lane as stored — `me`, `agent`, or `none` for the cleared
    /// lane.
    pub lane: String,
}

pub fn assign(store: &Store, flight: &str, lane: &str) -> Result<Assign, Error> {
    let assignee = match lane {
        "none" => None,
        word => match Assignee::parse(word) {
            Some(lane) => Some(lane.name().to_string()),
            None => {
                return Err(Error::BadAssignee {
                    word: word.to_string(),
                });
            }
        },
    };
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

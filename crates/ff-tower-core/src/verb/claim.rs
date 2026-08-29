//! `claim <flight>` — claim one specific flight, out of order.
//!
//! The claim is the motion: the flight moves into the air at assignment,
//! before any capture exists in a bay. Re-claiming is refused even for the
//! same author — a silent success that appended nothing would lie.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended, ensure_active};

/// The envelope's `data`: the claim, as the log holds it.
#[derive(Serialize)]
pub struct Claimed {
    pub claimed: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Claim {
    pub payload: Claimed,
    pub display: String,
    pub subject: String,
}

pub fn claim(store: &Store, flight: &str) -> Result<Claim, Error> {
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    if let Some(claim) = &filed.claim {
        return Err(Error::AlreadyClaimed {
            display: display(&fold, &flight),
            by: claim.by.clone(),
        });
    }
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Claimed {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one claimed event");

    Ok(Claim {
        payload: Claimed {
            claimed: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        subject,
    })
}

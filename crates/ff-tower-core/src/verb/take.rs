//! `take <flight>` — take the controls: crew this to you, agent off.
//!
//! The human override for `claim`'s refusal. `claim` will not reassign a
//! standing claim, because an agent silently stealing another's flight
//! would be a handoff nobody agreed to; a person authoring this event is
//! the consent that refusal declines to assume, so a take over someone
//! else's claim is allowed and names where the flight came from.
//!
//! The filed part stamp is never mutated — the overlay lives in the
//! flight's `taken` mark, so `requeue` can hand the flight back exactly
//! as it was filed and the payload keeps showing the stamp the log holds.
//! Taking twice is refused: a silent success that appended nothing would
//! lie.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended, ensure_active};

/// The envelope's `data`: the take, as the log holds it.
#[derive(Serialize)]
pub struct Taken {
    pub taken: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs —
/// `from` names whose claim the take displaced, when there was one.
pub struct Take {
    pub payload: Taken,
    pub display: String,
    pub subject: String,
    pub from: Option<String>,
}

pub fn take(store: &Store, flight: &str) -> Result<Take, Error> {
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    if filed.taken.is_some() {
        return Err(Error::AlreadyTaken {
            display: display(&fold, &flight),
        });
    }
    let subject = filed.subject.clone();
    let from = filed.claim.as_ref().map(|claim| claim.by.clone());

    let ids = store.append(vec![Kind::Taken {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one taken event");

    Ok(Take {
        payload: Taken {
            taken: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        subject,
        from,
    })
}

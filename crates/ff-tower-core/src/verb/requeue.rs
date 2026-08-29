//! `requeue <flight>` — hand the flight back to the pool.
//!
//! `take`'s reverse, and the recovery path an agent loop needs: a claim
//! nothing ever takes back keeps its flight out of the pool permanently,
//! so an agent that dies mid-flight would strand it. The requeue clears
//! the claim and the take together, which is what makes the pair exact
//! inverses — a flight you took goes back to the agent pool, and one an
//! agent merely claimed goes back untouched.
//!
//! A flight with an open question requeues fine: `answer` does not clear
//! a claim, so forcing an answer-then-requeue ordering would buy nothing.
//! The question stands and keeps the flight out of the pool until
//! answered, which is correct. A flight fufu holds requeues too — that is
//! a branch verdict, not tower's, and the pool reads it separately.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended, ensure_active};

/// The envelope's `data`: the requeue, as the log holds it.
#[derive(Serialize)]
pub struct Requeued {
    pub requeued: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Requeue {
    pub payload: Requeued,
    pub display: String,
    pub subject: String,
}

pub fn requeue(store: &Store, flight: &str) -> Result<Requeue, Error> {
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    if filed.claim.is_none() && filed.taken.is_none() {
        return Err(Error::Unclaimed {
            display: display(&fold, &flight),
        });
    }
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Requeued {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one requeued event");

    Ok(Requeue {
        payload: Requeued {
            requeued: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        subject,
    })
}

//! `done <flight>` — finish a flight: off the board, on the record.
//!
//! Always the asserted done this slice; DESIGN's done enum arrives with
//! procedures. The flight is required here: the CLI's bare form derives
//! it from the invoking worktree's chain and passes the derived id down,
//! and the server has no invoking worktree to derive from. Finishing a
//! waiting flight is allowed: abandoning the question is deliberate when
//! the flight itself is over.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended};

/// The envelope's `data`: the finish, as the log holds it.
#[derive(Serialize)]
pub struct Finished {
    pub done: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Done {
    pub payload: Finished,
    pub display: String,
    pub subject: String,
}

pub fn done(store: &Store, flight: &str) -> Result<Done, Error> {
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    // Not `ensure_active` — the duplicate case earns "already" wording,
    // and a derived flight already done lands here too.
    let filed = board::flight(&fold, &flight);
    if filed.done.is_some() {
        return Err(Error::AlreadyDone {
            display: display(&fold, &flight),
        });
    }
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Done {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one done event");

    Ok(Done {
        payload: Finished {
            done: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        subject,
    })
}

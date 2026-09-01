//! `hold <flight> -m <question>` — stop with a question attached.
//!
//! The one verb whose CLI success is exit 3: the envelope is a full data
//! envelope with the held event in it, and only the code says the flight
//! stopped with a question — fufu's held-is-an-outcome precedent. The 3
//! itself lives in the CLI's `main.rs`; here a hold succeeds like any
//! verb, and the server answers 200 — HTTP has no exit-code channel.
//!
//! The question is checked before the reference: a hold with nothing to
//! ask is refused whatever it names.
//!
//! Holding is stopping: the flight is no longer started, so the answer
//! releases it to Ready or Waiting by the graph, never back In
//! Progress — whoever pulls it next resumes in the warm bay.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended, ensure_active};

/// The envelope's `data`: the hold, as the log holds it.
#[derive(Serialize)]
pub struct Held {
    pub held: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Hold {
    pub payload: Held,
    pub display: String,
    pub question: String,
}

pub fn hold(store: &Store, flight: &str, message: Option<String>) -> Result<Hold, Error> {
    let Some(question) = message else {
        return Err(Error::NeedsQuestion);
    };
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    if let Some(open) = &filed.question {
        return Err(Error::AlreadyHeld {
            display: display(&fold, &flight),
            question: open.text.clone(),
        });
    }

    let ids = store.append(vec![Kind::Held {
        flight: flight.clone(),
        question: question.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one held event");

    Ok(Hold {
        payload: Held {
            held: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        question,
    })
}

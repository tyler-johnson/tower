//! `answer <flight> -m <answer>` — answer the question, release the
//! hold.
//!
//! The answer goes on the log's record and counts as the flight's motion;
//! it does not become a comment. A flight with no open question refuses —
//! an answer to nothing would append a gesture the board cannot show.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended, ensure_active};

/// The envelope's `data`: the answer, as the log holds it.
#[derive(Serialize)]
pub struct Answered {
    pub answered: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Answer {
    pub payload: Answered,
    pub display: String,
    pub answer: String,
}

pub fn answer(store: &Store, flight: &str, message: Option<String>) -> Result<Answer, Error> {
    let Some(answer) = message else {
        return Err(Error::NeedsAnswer);
    };
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    if filed.question.is_none() {
        return Err(Error::NotHeld {
            display: display(&fold, &flight),
        });
    }

    let ids = store.append(vec![Kind::Answered {
        flight: flight.clone(),
        answer: answer.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one answered event");

    Ok(Answer {
        payload: Answered {
            answered: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        answer,
    })
}

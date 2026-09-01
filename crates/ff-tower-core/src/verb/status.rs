//! `status <flight> <status>` — move a flight; `done` and `cancel` are
//! this verb carrying a payload.
//!
//! One `status` event, the byline the mover. The word assigns the facts
//! the fold stores — in triage, started, closed — and the fold derives
//! the status the board shows, so `ready` on a flight with a live
//! dependency lands it Waiting, and the echo says so. The word is
//! checked against the closed vocabulary here — the wire stays a free
//! string — and two words in it are refused by hand: `waiting` comes
//! from links and `held` from a question, and neither is a fact a word
//! can assign. A closed flight refuses every move, and an open question
//! refuses any move except `done` and `canceled`: abandoning the
//! question is deliberate when the flight itself is over, and everything
//! short of that goes through `answer`.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};
use crate::model::Status;

use super::{Error, appended, ensure_active};

/// The envelope's `data`: the move, as the log holds it. `done` and
/// `cancel` share it — the cmd name on the envelope is what says which
/// verb spoke.
#[derive(Serialize)]
pub struct Moved {
    pub status: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Move {
    pub payload: Moved,
    pub display: String,
    pub subject: String,
    /// The word the move wrote — the wire word.
    pub status: &'static str,
    /// The status the fold derives after the append — where the flight
    /// landed, which is the word unless the edges say Waiting.
    pub landed: String,
    /// The live dependencies holding it there, when `landed` is
    /// `waiting`; zero otherwise.
    pub waiting_on: usize,
}

pub fn status(
    store: &Store,
    flight: &str,
    word: &str,
    reason: Option<String>,
) -> Result<Move, Error> {
    let Some(target) = Status::parse(word) else {
        return Err(Error::BadStatus {
            word: word.to_string(),
        });
    };
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    match target {
        Status::Waiting => {
            return Err(Error::StatusWaiting {
                display: display(&fold, &flight),
            });
        }
        Status::Held => {
            return Err(Error::StatusHold {
                display: display(&fold, &flight),
            });
        }
        _ => {}
    }
    if let Some(question) = &filed.question
        && !matches!(target, Status::Done | Status::Canceled)
    {
        return Err(Error::StatusHeld {
            display: display(&fold, &flight),
            question: question.text.clone(),
        });
    }
    let subject = filed.subject.clone();
    append(store, &fold, flight, subject, target, reason)
}

/// `done <flight>` — finish a flight: off the board, on the record.
/// Not `ensure_active` — the duplicate case earns "already" wording, and
/// a derived flight already done lands here too. An open question is no
/// bar: abandoning it is deliberate when the flight itself is over.
pub fn done(store: &Store, flight: &str) -> Result<Move, Error> {
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = board::flight(&fold, &flight);
    if filed.closed() {
        return Err(Error::AlreadyDone {
            display: display(&fold, &flight),
        });
    }
    let subject = filed.subject.clone();
    append(store, &fold, flight, subject, Status::Done, None)
}

/// `cancel <flight> [-m <why>]` — off the board without the finish; the
/// reason rides the move.
pub fn cancel(store: &Store, flight: &str, message: Option<String>) -> Result<Move, Error> {
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;
    let filed = ensure_active(&fold, &flight)?;
    let subject = filed.subject.clone();
    append(store, &fold, flight, subject, Status::Canceled, message)
}

fn append(
    store: &Store,
    fold: &board::Fold,
    flight: crate::log::EventId,
    subject: String,
    target: Status,
    reason: Option<String>,
) -> Result<Move, Error> {
    let ids = store.append(vec![Kind::Status {
        flight: flight.clone(),
        status: target.name().to_string(),
        reason,
    }])?;
    let id = ids.into_iter().next().expect("one status event");

    // Re-fold to read where the word landed: the fold derives the
    // status, and the echo must say what the board will.
    let after = board::fold(&store.read_all()?);
    let landed = board::flight(&after, &flight);
    let waiting_on = if landed.status == "waiting" {
        landed
            .depends_on
            .iter()
            .filter(|dep| !board::flight(&after, dep).closed())
            .count()
    } else {
        0
    };

    Ok(Move {
        payload: Moved {
            status: appended(store, &id)?,
        },
        display: display(fold, &flight),
        subject,
        status: target.name(),
        landed: landed.status.clone(),
        waiting_on,
    })
}

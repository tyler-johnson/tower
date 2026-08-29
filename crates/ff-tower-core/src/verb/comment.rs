//! `comment <flight> -m <note>` — a note on a flight's record.
//!
//! fufu's `describe` gate minus the editor: tower opens no editor this
//! slice, so a missing message refuses unconditionally — a coded refusal,
//! never a clap `required = true`, so a machine caller gets an envelope.
//! No `ensure_active`: a note on a closed record is fine.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended};

/// The envelope's `data`: the comment, as the log holds it.
#[derive(Serialize)]
pub struct Commented {
    pub commented: Event,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Comment {
    pub payload: Commented,
    pub display: String,
}

pub fn comment(store: &Store, flight: &str, message: Option<String>) -> Result<Comment, Error> {
    let Some(text) = message else {
        return Err(Error::NeedsNote);
    };
    board::parse_ref(flight)?;
    let fold = board::fold(&store.read_all()?);
    let flight = board::resolve(&fold, flight)?;

    let ids = store.append(vec![Kind::Commented {
        flight: flight.clone(),
        text,
    }])?;
    let id = ids.into_iter().next().expect("one commented event");

    Ok(Comment {
        payload: Commented {
            commented: appended(store, &id)?,
        },
        display: display(&fold, &flight),
    })
}

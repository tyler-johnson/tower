//! `comment <flight> -m <note>` — a note on a flight's record.
//!
//! fufu's `describe` gate minus the editor: tower opens no editor this
//! slice, so a missing message refuses unconditionally — a coded refusal,
//! never a clap `required = true`, so a machine caller gets an envelope.
//! No `ensure_active`: a note on a closed record is fine.
//!
//! `--handoff` flags the note as the state of play: the brief pins the
//! newest flagged comment above the stream, and every prior one stays in
//! it. A flag, never a hold — the flight's status does not move, and the
//! question stays its own kind.
//!
//! A flight named in the note — `#3`, `writer#3` — is stored as its
//! wire id, by the resolution the flight argument gets: a bare number
//! two writers hold refuses the same way, and a match on nothing stays
//! as typed.

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
    pub handoff: bool,
}

pub fn comment(
    store: &Store,
    flight: &str,
    message: Option<String>,
    handoff: bool,
) -> Result<Comment, Error> {
    let Some(text) = message else {
        return Err(Error::NeedsNote);
    };
    board::parse_ref(flight)?;
    let fold = store.snapshot()?;
    let flight = board::resolve(&fold, flight)?;
    let text = board::rewrite(&fold, &text)?;

    let ids = store.append_synced(vec![Kind::Commented {
        flight: flight.clone(),
        text,
        handoff,
    }])?;
    let id = ids.into_iter().next().expect("one commented event");
    let fold = store.current()?;

    Ok(Comment {
        payload: Commented {
            commented: appended(store, &id)?,
        },
        display: display(&fold, &flight),
        handoff,
    })
}

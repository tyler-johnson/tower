//! `callsign <name>` — a session names its pilot, and holds the word on
//! its lease.
//!
//! The lease is the session's, keyed by the first session variable set,
//! and the word is a field in it: this verb fills the field and reports
//! it. No two live sessions on a machine hold one word — every other
//! lease holding it is read, a fresh lease or a live pid refuses, a
//! stale one with a dead or absent pid is removed and the word taken —
//! a lease past `leaseExpiry` is swept first, whatever its pid — and
//! `--force` takes it from a stale holder inside the window too,
//! never from a live pid. When the session's word changes, the open
//! flights it laned under the old word follow it: one `assigned` per
//! flight in a single append, byline the new word. Scoped to what this
//! session laned, because that is the authority it already had, and so
//! the move needs no flag; a flight a person laned into the old word by
//! hand stays, and the report counts it.
//!
//! Every refusal is a coded error in the house style, and the verb is a
//! write verb like any other: it appends to the log and reads the
//! window from git config, so it needs a repository; the lease itself
//! is per machine.

use serde::Serialize;

use crate::board::{Flight, display};
use crate::lease::{self, State};
use crate::log::{CLIENT_MARKERS, Kind, Store, usable_callsign};

use super::Error;

/// The envelope's `data`.
#[derive(Serialize)]
pub struct CallsignData {
    pub callsign: String,
    /// The word this session flew as before, and where it came from —
    /// `env` never, since the verb refuses under it.
    pub previous: Option<String>,
    pub source: &'static str,
    pub session: String,
    pub session_source: &'static str,
    pub pid: Option<u32>,
    pub lease: State,
    /// The same word as already held: a renewal that says so.
    pub renewed: bool,
    /// The stale holder the word was taken from, when there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took: Option<Took>,
    /// The flights re-laned from the previous word to this one.
    pub moved: Vec<Moved>,
    /// The flights left in the previous word's lane because another
    /// session, or a person with no session, laned them there.
    pub left: Vec<Left>,
}

#[derive(Serialize)]
pub struct Took {
    pub session: String,
    /// `stale 40s` — the holder's condition when the word was taken.
    pub detail: String,
}

#[derive(Serialize)]
pub struct Moved {
    pub flight: String,
    pub number: u64,
    pub subject: String,
}

#[derive(Serialize)]
pub struct Left {
    pub flight: String,
    pub number: u64,
    pub subject: String,
    /// The byline that laned it: the callsign, else the session, else
    /// the author.
    pub by: String,
}

/// The outcome: the payload, plus the echo facts a human render needs.
pub struct Callsign {
    pub payload: CallsignData,
    /// The previous word's source, for the render: `session`, `client`,
    /// or `login`.
    pub previous_source: Option<&'static str>,
    /// Each moved flight's display form, in `moved` order.
    pub moved_display: Vec<String>,
    /// Each left flight's display form, in `left` order.
    pub left_display: Vec<String>,
}

pub fn callsign(store: &Store, name: &str, force: bool) -> Result<Callsign, Error> {
    let identity = store.identity();
    let Some(session) = identity.leased_session() else {
        return Err(Error::CallsignNoSession);
    };
    if identity.callsign_source == Some("env") {
        return Err(Error::CallsignEnvSet {
            word: identity.callsign.clone().unwrap_or_default(),
        });
    }
    let word = usable_callsign(name).ok_or_else(|| Error::BadCallsign {
        word: name.to_string(),
    })?;
    if CLIENT_MARKERS.iter().any(|(_, client)| *client == word) {
        return Err(Error::CallsignClientWord { word });
    }

    let config = store.config();
    let window = crate::config::lease_window(&config);
    let expiry = crate::config::lease_expiry(&config);
    let took = hold(session, &word, force, window, expiry)?;

    let previous = identity.callsign.clone();
    let renewed = identity.callsign_source == Some("session") && previous.as_deref() == Some(&word);
    let pid = identity.pid;
    lease::update(session, |own| {
        own.client = identity.client.map(str::to_string);
        own.pid = pid.map(|pid| pid.pid);
        own.pid_start = pid.map(|pid| pid.start);
        own.callsign = Some(word.clone());
        true
    })
    .map_err(|err| Error::CallsignLease {
        detail: err.to_string(),
    })?;
    store.adopt_callsign(word.clone());

    let (moved, left, moved_display, left_display) =
        if identity.callsign_source == Some("session") && !renewed {
            let old = previous.as_deref().expect("a session word");
            relane(store, session, old, &word)?
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

    Ok(Callsign {
        payload: CallsignData {
            callsign: word,
            previous,
            source: "session",
            session: session.to_string(),
            session_source: identity.session_source.unwrap_or("launcher"),
            pid: pid.map(|pid| pid.pid),
            lease: State {
                fresh: true,
                age: std::time::Duration::ZERO,
                pid_alive: pid.map(|_| true),
            },
            renewed,
            took,
            moved,
            left,
        },
        previous_source: identity.callsign_source,
        moved_display,
        left_display,
    })
}

/// The hold: sweep every lease past `expiry` whatever its pid, then
/// every lease whose pid is dead, then weigh every other lease holding
/// the word. This session's own id is this session again — a resumed
/// session under a new process — and is skipped.
fn hold(
    own: &str,
    word: &str,
    force: bool,
    window: std::time::Duration,
    expiry: std::time::Duration,
) -> Result<Option<Took>, Error> {
    let mut took = None;
    lease::sweep(expiry);
    for (session, held, mtime) in lease::all() {
        if session == own {
            continue;
        }
        let pid = held.pid();
        if pid.is_some_and(|pid| !pid.alive()) {
            let _ = lease::release(&session);
            continue;
        }
        if held.callsign.as_deref() != Some(word) {
            continue;
        }
        let state = lease::state(mtime, pid, window);
        if let Some(pid) = pid.filter(|_| state.pid_alive == Some(true)) {
            return Err(if force {
                Error::CallsignLive {
                    word: word.to_string(),
                    session,
                    pid: pid.pid,
                }
            } else {
                Error::CallsignHeld {
                    word: word.to_string(),
                    session,
                    detail: state.detail(Some(pid)),
                }
            });
        }
        if state.fresh && !force {
            return Err(Error::CallsignHeld {
                word: word.to_string(),
                session,
                detail: state.detail(None),
            });
        }
        let _ = lease::release(&session);
        took = Some(Took {
            detail: state.detail(None),
            session,
        });
    }
    Ok(took)
}

/// The move: every open flight in `old`'s lane that this session laned
/// there follows to `new`, in one append; the rest are reported as
/// left, each with the byline that laned it.
#[allow(clippy::type_complexity)]
fn relane(
    store: &Store,
    session: &str,
    old: &str,
    new: &str,
) -> Result<(Vec<Moved>, Vec<Left>, Vec<String>, Vec<String>), Error> {
    let fold = store.snapshot()?;
    let mut moved = Vec::new();
    let mut left = Vec::new();
    let mut moved_display = Vec::new();
    let mut left_display = Vec::new();
    let mut batch = Vec::new();
    for flight in fold
        .flights
        .iter()
        .filter(|flight| !flight.closed() && flight.assignee.as_deref() == Some(old))
    {
        if laned_by(flight) == Some(session) {
            batch.push(Kind::Assigned {
                flight: flight.id.clone(),
                assignee: Some(new.to_string()),
            });
            moved.push(Moved {
                flight: flight.id.to_string(),
                number: flight.number,
                subject: flight.subject.clone(),
            });
            moved_display.push(display(&fold, &flight.id));
        } else {
            left.push(Left {
                flight: flight.id.to_string(),
                number: flight.number,
                subject: flight.subject.clone(),
                by: laned_byline(flight),
            });
            left_display.push(display(&fold, &flight.id));
        }
    }
    store.append_synced(batch)?;
    Ok((moved, left, moved_display, left_display))
}

/// The session on the event that set the lane: the last `assigned`, or
/// the filing when nothing re-laned it since.
fn laned_by(flight: &Flight) -> Option<&str> {
    match &flight.assigned {
        Some(mark) => mark.session.as_deref(),
        None => flight.filed_session.as_deref(),
    }
}

/// The byline on the event that set the lane, the way a render prints
/// one: the callsign, else the session, else the author.
fn laned_byline(flight: &Flight) -> String {
    let (callsign, session, by) = match &flight.assigned {
        Some(mark) => (
            mark.callsign.as_deref(),
            mark.session.as_deref(),
            mark.by.as_str(),
        ),
        None => (
            flight.filed_callsign.as_deref(),
            flight.filed_session.as_deref(),
            flight.filed_by.as_str(),
        ),
    };
    callsign.or(session).unwrap_or(by).to_string()
}

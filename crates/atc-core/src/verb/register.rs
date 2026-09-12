//! `register [<callsign>] [--kind <kind>] [-m <msg>] [--retire]` — the
//! roster: who flies here.
//!
//! A callsign is a chosen readable name for a pilot, person or agent,
//! shared across machines on purpose: the same role on two machines is
//! one pilot. Registration gives the roster a kind, a description, and
//! a last-seen line; it is not what makes a callsign usable. Assigning
//! to one needs no registration, and an unregistered callsign on an
//! event still folds — values are open everywhere in tower. Last wins
//! in log order: a second registration replaces the kind and the
//! description, a retirement drops the pilot, and a later registration
//! brings it back.

use serde::Serialize;

use crate::board;
use crate::log::{Event, Kind, Store, usable_callsign};
use crate::model::PilotKind;

use super::{Error, appended};

/// The envelope's `data`: what was registered, and the event as the log
/// holds it.
#[derive(Serialize)]
pub struct Registered {
    pub callsign: String,
    pub kind: String,
    pub description: String,
    pub event: Event,
}

/// The outcome: the payload is the whole echo — the human render reads
/// the three words back off it.
pub struct Register {
    pub payload: Registered,
}

/// The envelope's `data` for a retirement.
#[derive(Serialize)]
pub struct Retired {
    pub callsign: String,
    pub event: Event,
}

pub struct Retire {
    pub payload: Retired,
}

/// Put a callsign on the roster, or rewrite its entry. `kind` is held
/// to [`PilotKind`] here and stored as its name; the description is
/// stored as given, empty included.
pub fn register(
    store: &Store,
    callsign: &str,
    kind: &str,
    description: String,
) -> Result<Register, Error> {
    let callsign = usable_callsign(callsign).ok_or_else(|| Error::BadCallsign {
        word: callsign.to_string(),
    })?;
    let kind = PilotKind::parse(kind)
        .ok_or_else(|| Error::BadKind {
            word: kind.to_string(),
        })?
        .name()
        .to_string();
    let ids = store.append(vec![Kind::Registered {
        callsign: callsign.clone(),
        kind: kind.clone(),
        description: description.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one registered event");
    Ok(Register {
        payload: Registered {
            callsign,
            kind,
            description,
            event: appended(store, &id)?,
        },
    })
}

/// Take a callsign off the roster. Refused when it is not there: a
/// retirement of nobody would append a gesture the roster cannot show.
pub fn retire(store: &Store, callsign: &str) -> Result<Retire, Error> {
    let fold = board::fold(&store.read_all()?);
    if !fold.roster.iter().any(|pilot| pilot.callsign == callsign) {
        return Err(Error::CallsignNotFound {
            callsign: callsign.to_string(),
        });
    }
    let ids = store.append(vec![Kind::Unregistered {
        callsign: callsign.to_string(),
    }])?;
    let id = ids.into_iter().next().expect("one unregistered event");
    Ok(Retire {
        payload: Retired {
            callsign: callsign.to_string(),
            event: appended(store, &id)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use atc_testsupport::Repo;

    fn store() -> (Repo, Store) {
        let repo = Repo::new();
        repo.pin_writer("pi");
        let store = Store::open(repo.path()).expect("open");
        (repo, store)
    }

    fn roster(store: &Store) -> Vec<(String, String, String)> {
        board::fold(&store.read_all().expect("read"))
            .roster
            .into_iter()
            .map(|pilot| (pilot.callsign, pilot.kind, pilot.description))
            .collect()
    }

    #[test]
    fn a_registration_lands_and_a_second_one_rewrites_in_place() {
        let (_repo, store) = store();
        let first = register(&store, "claude", "agent", "Claude Code".to_string()).expect("lands");
        assert_eq!(first.payload.callsign, "claude");
        assert_eq!(first.payload.kind, "agent");
        assert_eq!(first.payload.event.kind.name(), "registered");
        register(&store, "tyler", "person", String::new()).expect("lands");
        register(
            &store,
            "claude",
            "agent",
            "every Claude Code session".to_string(),
        )
        .expect("rewrites");
        assert_eq!(
            roster(&store),
            [
                (
                    "claude".to_string(),
                    "agent".to_string(),
                    "every Claude Code session".to_string()
                ),
                ("tyler".to_string(), "person".to_string(), String::new()),
            ],
            "registration order, the rewrite in place"
        );
    }

    #[test]
    fn a_retirement_drops_the_pilot_and_a_later_registration_returns_it() {
        let (_repo, store) = store();
        register(&store, "claude", "agent", String::new()).expect("lands");
        register(&store, "tyler", "person", String::new()).expect("lands");
        let retired = retire(&store, "claude").expect("retires");
        assert_eq!(retired.payload.callsign, "claude");
        assert_eq!(retired.payload.event.kind.name(), "unregistered");
        assert_eq!(
            roster(&store)
                .iter()
                .map(|(callsign, ..)| callsign.as_str())
                .collect::<Vec<_>>(),
            ["tyler"]
        );
        register(&store, "claude", "agent", String::new()).expect("returns");
        assert_eq!(
            roster(&store)
                .iter()
                .map(|(callsign, ..)| callsign.as_str())
                .collect::<Vec<_>>(),
            ["tyler", "claude"],
            "back at the end"
        );
    }

    #[test]
    fn the_refusals_carry_their_ids() {
        let (_repo, store) = store();
        let bad = register(&store, "two words", "agent", String::new())
            .err()
            .expect("not a callsign");
        assert_eq!(bad.id(), "usage/bad-callsign");
        assert_eq!(
            bad.to_string(),
            "`two words` is not a callsign — one word, no spaces, at most 64 bytes, and not me, agent, or none"
        );
        for lane in ["me", "agent", "none"] {
            assert_eq!(
                register(&store, lane, "agent", String::new())
                    .err()
                    .expect("a lane word is not a callsign")
                    .id(),
                "usage/bad-callsign"
            );
        }
        let bad = register(&store, "claude", "bot", String::new())
            .err()
            .expect("not a kind");
        assert_eq!(bad.id(), "usage/bad-kind");
        assert_eq!(
            bad.to_string(),
            "`bot` is not a pilot kind — person or agent"
        );
        let bad = retire(&store, "nobody").err().expect("not on the roster");
        assert_eq!(bad.id(), "callsign/not-found");
        assert_eq!(bad.to_string(), "`nobody` is not on the roster");
        assert_eq!(bad.exits(), ["atc register"]);
        assert!(roster(&store).is_empty(), "nothing landed");
    }
}

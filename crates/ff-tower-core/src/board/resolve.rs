//! Naming a flight: the reference grammar, and its resolution against
//! the fold.
//!
//! Pure over `Fold` and `&str`, like the rest of the board — no store,
//! no spawn. Every surface that takes a flight reference (the CLI's
//! verbs, the server's routes) parses and resolves through here, so the
//! grammar and its refusals cannot fork per surface.

use crate::board::{Flight, Fold};
use crate::log::EventId;

/// A flight reference as typed: a bare number, a `writer#n` pair, or the
/// full wire form `<writer>.<seq>`.
#[derive(Debug)]
pub enum FlightRef {
    Number(u64),
    WriterNumber(String, u64),
    Full(EventId),
}

/// What naming a flight can be refused with. Each variant carries a
/// stable id and its own exits, so a caller with no registry of its own
/// still answers in the same words the CLI does.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// Text that is no reference at all — refused before any store is
    /// opened.
    #[error("`{text}` is not a flight — `<n>`, `<writer>#<n>`, or `<writer>.<seq>`")]
    BadRef { text: String },

    /// The reference parsed, and nothing filed matches it.
    #[error("no flight `{text}` on the board")]
    NotFound { text: String },

    /// A bare number that names more than one flight across writers; the
    /// candidates arrive in `writer#n` form, already backticked.
    #[error("`{text}` names {} flights: {}", count(.candidates.len()), .candidates.join(", "))]
    Ambiguous {
        text: String,
        candidates: Vec<String>,
    },
}

impl ResolveError {
    /// The stable id, tower's `category/kebab-case`.
    pub fn id(&self) -> &'static str {
        match self {
            ResolveError::BadRef { .. } => "usage/bad-flight",
            ResolveError::NotFound { .. } => "flight/not-found",
            ResolveError::Ambiguous { .. } => "flight/ambiguous",
        }
    }

    /// Commands that lead out of it. One answer for all three: the board
    /// shows what is actually filed, in the same display form the
    /// grammar accepts.
    pub fn exits(&self) -> Vec<String> {
        vec!["ff tower".to_string()]
    }
}

/// The syntactic half of naming a flight, before any store is opened. One
/// leading `#` is stripped for paste tolerance — output prints numbers
/// `#`-prefixed, and what tower prints, tower accepts (`#pi#3` pasted
/// still parses: the split is at the last `#`).
pub fn parse_ref(text: &str) -> Result<FlightRef, ResolveError> {
    let bare = text.strip_prefix('#').unwrap_or(text);
    if let Ok(number) = bare.parse::<u64>() {
        return Ok(FlightRef::Number(number));
    }
    if let Some((writer, digits)) = bare.rsplit_once('#')
        && let Ok(number) = digits.parse::<u64>()
    {
        return Ok(FlightRef::WriterNumber(writer.to_string(), number));
    }
    if let Ok(id) = bare.parse::<EventId>() {
        return Ok(FlightRef::Full(id));
    }
    Err(ResolveError::BadRef {
        text: text.to_string(),
    })
}

/// Resolve a reference against the fold's filed flights. A bare number
/// must match exactly one flight across writers; the refusals quote the
/// reference as the user typed it, and an ambiguity names every candidate
/// in `writer#n` form.
pub fn resolve(fold: &Fold, text: &str) -> Result<EventId, ResolveError> {
    let not_found = || ResolveError::NotFound {
        text: text.to_string(),
    };
    match parse_ref(text)? {
        FlightRef::Full(id) => {
            if fold.flights.iter().any(|flight| flight.id == id) {
                Ok(id)
            } else {
                Err(not_found())
            }
        }
        FlightRef::WriterNumber(writer, number) => fold
            .flights
            .iter()
            .find(|flight| flight.id.writer == writer && flight.number == number)
            .map(|flight| flight.id.clone())
            .ok_or_else(not_found),
        FlightRef::Number(number) => {
            let candidates: Vec<&Flight> = fold
                .flights
                .iter()
                .filter(|flight| flight.number == number)
                .collect();
            match candidates.as_slice() {
                [] => Err(not_found()),
                [flight] => Ok(flight.id.clone()),
                many => Err(ResolveError::Ambiguous {
                    text: text.to_string(),
                    candidates: many
                        .iter()
                        .map(|flight| format!("`{}#{}`", flight.id.writer, flight.number))
                        .collect(),
                }),
            }
        }
    }
}

/// Small counts in words, matching the refusal grammar's register.
pub fn count(n: usize) -> String {
    match n {
        1 => "one".to_string(),
        2 => "two".to_string(),
        3 => "three".to_string(),
        4 => "four".to_string(),
        _ => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::fold;
    use crate::log::{Event, Kind};

    fn filed(id: &str, time: i64, subject: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: "open".to_string(),
                subject: subject.to_string(),
                body: String::new(),
                part: None,
            },
        }
    }

    #[test]
    fn every_reference_form_parses_and_a_leading_hash_strips() {
        assert!(matches!(parse_ref("3"), Ok(FlightRef::Number(3))));
        assert!(matches!(parse_ref("#3"), Ok(FlightRef::Number(3))));
        match parse_ref("pi#3") {
            Ok(FlightRef::WriterNumber(writer, 3)) => assert_eq!(writer, "pi"),
            _ => panic!("`pi#3` is the writer-number form"),
        }
        // The pasted form: the split is at the last `#`.
        match parse_ref("#pi#3") {
            Ok(FlightRef::WriterNumber(writer, 3)) => assert_eq!(writer, "pi"),
            _ => panic!("`#pi#3` is the writer-number form"),
        }
        match parse_ref("pi.3") {
            Ok(FlightRef::Full(id)) => {
                assert_eq!(id.writer, "pi");
                assert_eq!(id.seq, 3);
            }
            _ => panic!("`pi.3` is the wire form"),
        }
    }

    #[test]
    fn text_that_is_no_reference_is_the_usage_refusal() {
        let err = parse_ref("banana").expect_err("not a flight");
        assert_eq!(err.id(), "usage/bad-flight");
        assert_eq!(
            err.to_string(),
            "`banana` is not a flight — `<n>`, `<writer>#<n>`, or `<writer>.<seq>`"
        );
        assert_eq!(err.exits(), ["ff tower"]);
    }

    #[test]
    fn a_reference_naming_nothing_filed_is_not_found() {
        let fold = fold(&[filed("pi.1", 10, "the one flight")]);
        for text in ["2", "qi#1", "pi.9"] {
            let err = resolve(&fold, text).expect_err("nothing filed matches");
            assert_eq!(err.id(), "flight/not-found");
            assert_eq!(err.to_string(), format!("no flight `{text}` on the board"));
        }
        assert_eq!(
            resolve(&fold, "1").expect("the bare number binds"),
            "pi.1".parse().expect("id")
        );
    }

    #[test]
    fn a_bare_number_two_writers_hold_is_ambiguous() {
        let fold = fold(&[filed("pi.1", 10, "from pi"), filed("qi.1", 20, "from qi")]);
        let err = resolve(&fold, "1").expect_err("two writers hold #1");
        assert_eq!(err.id(), "flight/ambiguous");
        assert_eq!(err.to_string(), "`1` names two flights: `pi#1`, `qi#1`");
        // The writer-number form still binds exactly.
        assert_eq!(
            resolve(&fold, "qi#1").expect("exact"),
            "qi.1".parse().expect("id")
        );
    }
}

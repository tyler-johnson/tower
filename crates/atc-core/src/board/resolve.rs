//! Naming a flight: the reference grammar, and its resolution against
//! the fold.
//!
//! Pure over `Fold` and `&str`, like the rest of the board — no store,
//! no spawn. Every surface that takes a flight reference (the CLI's
//! verbs, the server's routes) parses and resolves through here, so the
//! grammar and its refusals cannot fork per surface.

use crate::board::{Flight, Fold};
use crate::log::EventId;

/// A global number, writer-local ordinal, provisional guess, or permanent wire ID.
#[derive(Debug)]
pub enum FlightRef {
    Number(u64),
    WriterNumber(String, u64),
    Provisional(Option<String>, u64),
    Full(EventId),
}

/// What naming a flight can be refused with. Each variant carries a
/// stable id and its own exits, so a caller with no registry of its own
/// still answers in the same words the CLI does.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// Text that is no reference at all — refused before any store is
    /// opened.
    #[error(
        "`{text}` is not a flight — `<n>`, `<writer>#<n>`, `~<n>`, `<writer>~<n>`, or `<writer>.<seq>`"
    )]
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
        vec!["atc".to_string()]
    }
}

/// The syntactic half of naming a flight, before any store is opened. One
/// leading `#` is stripped for paste tolerance — output prints numbers
/// `#`-prefixed, and what tower prints, tower accepts (`#pi#3` pasted
/// still parses: the split is at the last `#`).
pub fn parse_ref(text: &str) -> Result<FlightRef, ResolveError> {
    let bare = text.strip_prefix('#').unwrap_or(text);
    if let Some((writer, digits)) = bare.rsplit_once('~')
        && let Ok(number) = digits.parse::<u64>()
        && !writer.contains(['~', '#'])
    {
        return Ok(FlightRef::Provisional(
            (!writer.is_empty()).then(|| writer.to_string()),
            number,
        ));
    }
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

/// Resolve against this snapshot. Numbered folds use bare numbers for global claims and tilde forms for currently provisional flights. Legacy folds retain writer-local bare numbers until migration supplies counter context. Refusals quote the input and identify each ambiguous candidate.
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
                .filter(|flight| {
                    flight.global_number == Some(number)
                        || (fold.counter.is_none()
                            && flight.global_number.is_none()
                            && flight.number == number)
                })
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
        FlightRef::Provisional(writer, number) => {
            let candidates: Vec<_> = fold
                .flights
                .iter()
                .filter(|flight| {
                    writer
                        .as_ref()
                        .is_none_or(|writer| &flight.id.writer == writer)
                        && flight.provisional_number == Some(number)
                })
                .collect();
            match candidates.as_slice() {
                [] => Err(not_found()),
                [flight] => Ok(flight.id.clone()),
                many => Err(ResolveError::Ambiguous {
                    text: text.to_string(),
                    candidates: many
                        .iter()
                        .map(|flight| format!("`{}~{number}`", flight.id.writer))
                        .collect(),
                }),
            }
        }
    }
}

/// The fold's flight for a resolved id. Infallible after `resolve` — the
/// id came out of this fold's filed flights.
pub fn flight<'a>(fold: &'a Fold, id: &EventId) -> &'a Flight {
    fold.flights
        .iter()
        .find(|flight| &flight.id == id)
        .expect("resolved to a filed flight")
}

/// The sole display rule: a confirmed global claim prints `#n`; a provisional guess prints `~n`, writer-qualified when ambiguous. Folds without counter context retain the legacy ordinal display until migration. The ID must name a flight in this fold.
pub fn display(fold: &Fold, id: &EventId) -> String {
    let selected = flight(fold, id);
    if let Some(number) = selected.global_number {
        return format!("#{number}");
    }
    if let Some(number) = selected.provisional_number {
        let ambiguous = fold
            .flights
            .iter()
            .any(|other| other.id != *id && other.provisional_number == Some(number));
        return if ambiguous {
            format!("{}~{number}", id.writer)
        } else {
            format!("~{number}")
        };
    }
    if fold.counter.is_some() {
        // Counter exhaustion has no representable guess; the permanent wire name remains usable.
        return id.to_string();
    }
    let short = fold
        .flights
        .iter()
        .all(|flight| flight.id.writer == id.writer);
    let number = flight(fold, id).number;
    if short {
        format!("#{number}")
    } else {
        format!("{}#{number}", id.writer)
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
            session: None,
            callsign: None,
            id,
            kind: Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: String::new(),
                status: "backlog".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
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
            "`banana` is not a flight — `<n>`, `<writer>#<n>`, `~<n>`, `<writer>~<n>`, or `<writer>.<seq>`"
        );
        assert_eq!(err.exits(), ["atc"]);
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

    #[test]
    fn global_claims_and_writer_aliases_are_independent() {
        let mut events = vec![filed("pi.1", 10, "pi"), filed("qi.1", 20, "qi")];
        let mut claim = filed("pi.2", 30, "unused");
        claim.kind = Kind::Numbered {
            flight: "pi.1".parse().expect("id"),
            number: 7,
            reservation: None,
        };
        events.push(claim.clone());
        claim.id = "pi.3".parse().expect("id");
        claim.kind = Kind::Numbered {
            flight: "qi.1".parse().expect("id"),
            number: 8,
            reservation: None,
        };
        events.push(claim);
        let fold = crate::board::fold_numbered(&events, 8);
        for (reference, id) in [
            ("7", "pi.1"),
            ("#8", "qi.1"),
            ("pi#1", "pi.1"),
            ("qi#1", "qi.1"),
            ("pi.1", "pi.1"),
        ] {
            assert_eq!(resolve(&fold, reference).expect("resolve").to_string(), id);
        }
        assert_eq!(display(&fold, &"pi.1".parse().expect("id")), "#7");
        assert_eq!(display(&fold, &"qi.1".parse().expect("id")), "#8");
        assert!(resolve(&fold, "1").is_err());
        assert!(resolve(&fold, "~7").is_err());
    }

    #[test]
    fn provisional_guesses_use_counter_and_per_writer_filed_order() {
        // Union order can differ from a writer's filing order.
        let events = vec![
            filed("pi.9", 10, "later"),
            filed("qi.1", 20, "other"),
            filed("pi.1", 30, "first"),
        ];
        let fold = crate::board::fold_numbered(&events, 4);
        let id = |text: &str| text.parse::<EventId>().expect("id");
        assert_eq!(display(&fold, &id("pi.1")), "pi~5");
        assert_eq!(display(&fold, &id("qi.1")), "qi~5");
        assert_eq!(display(&fold, &id("pi.9")), "~6");
        assert!(matches!(
            resolve(&fold, "~5"),
            Err(ResolveError::Ambiguous { .. })
        ));
        assert_eq!(resolve(&fold, "pi~5").expect("qualified"), id("pi.1"));
        assert_eq!(resolve(&fold, "~6").expect("unique"), id("pi.9"));
        assert!(resolve(&fold, "5").is_err());
        assert_eq!(
            crate::board::rewrite(&fold, "after pi~5 and ~6.").expect("rewrite"),
            "after #pi.1 and #pi.9."
        );
        let advanced = crate::board::fold_numbered(&events, 10);
        assert!(resolve(&advanced, "pi~5").is_err());
        assert_eq!(resolve(&advanced, "pi~11").expect("new guess"), id("pi.1"));
    }

    #[test]
    fn latest_claim_wins_without_changing_the_writer_alias() {
        let mut events = vec![filed("pi.1", 10, "one")];
        for (seq, number) in [(2, 1), (3, 42)] {
            let mut claim = filed(&format!("pi.{seq}"), 20 + seq, "unused");
            claim.kind = Kind::Numbered {
                flight: "pi.1".parse().expect("id"),
                number,
                reservation: None,
            };
            events.push(claim);
        }
        let fold = crate::board::fold_numbered(&events, 42);
        assert_eq!(
            resolve(&fold, "42").expect("latest"),
            resolve(&fold, "pi#1").expect("ordinal")
        );
        assert!(resolve(&fold, "1").is_err());
        assert!(resolve(&fold, "~1").is_err());
        assert_eq!(fold.flights[0].global_number, Some(42));
        assert!(fold.unrouted.is_empty());
        let history = crate::board::history(&events, &fold.flights[0].id);
        assert!(matches!(
            history.last().expect("history").detail,
            Some(crate::board::Detail::Numbered { number: 42 })
        ));
    }

    #[test]
    fn an_exhausted_counter_keeps_wire_names_usable() {
        let fold = crate::board::fold_numbered(&[filed("pi.1", 10, "one")], u64::MAX);
        assert_eq!(display(&fold, &"pi.1".parse().expect("id")), "pi.1");
        assert!(resolve(&fold, "pi.1").is_ok());
    }
}

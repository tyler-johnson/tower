//! The event: what a person (or an agent) said, as data.
//!
//! Everything in the store is one of these. `id`, `author`, `writer`, and
//! `time` are assigned by the store at append; `kind` is what the caller
//! authored. The wire shape is one JSON object per event, batched into an
//! array in `events.json`:
//!
//! ```json
//! {"id":"pi.17","author":"tyler@example.com","writer":"pi","time":1787638881,
//!  "kind":"filed","body":{"procedure":"open","subject":"…","body":"…"}}
//! ```
//!
//! `body` survives one pass unparsed (`Box<RawValue>`) and is matched into
//! [`Kind`] second — the same two-pass discipline the seam uses on fufu's
//! envelope. A kind this binary does not know becomes [`Kind::Unknown`]
//! with its payload byte-for-byte intact. That is not defensive habit: a
//! union merge can put a newer tower's events in front of an older tower's
//! fold, and dropping them would silently lose authored intent.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

/// `<writer>.<seq>` — monotonic per writer, unique across writers because
/// the writer id is, and readable: `pi.17` says which machine wrote it.
///
/// A flight is identified by the id of the `filed` event that created it,
/// and that id is what goes on every fufu call as `--session pi.17`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventId {
    pub writer: String,
    pub seq: u64,
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.writer, self.seq)
    }
}

impl std::str::FromStr for EventId {
    type Err = String;

    /// Split at the last dot. A sanitized writer contains no dots, but
    /// taking the last keeps a foreign id that does addressable rather
    /// than unparsable.
    fn from_str(text: &str) -> std::result::Result<EventId, String> {
        let malformed = || format!("`{text}` is not `<writer>.<seq>`");
        let (writer, seq) = text.rsplit_once('.').ok_or_else(malformed)?;
        let seq: u64 = seq.parse().map_err(|_| malformed())?;
        if writer.is_empty() {
            return Err(malformed());
        }
        Ok(EventId {
            writer: writer.to_string(),
            seq,
        })
    }
}

impl Serialize for EventId {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for EventId {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<EventId, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// One authored gesture. Minting, referencing, pairing, and the four
/// lifecycle marks — enough vocabulary for a flight to be filed, claimed,
/// held on a question, answered, and finished. `promote` lands with its
/// verb.
#[derive(Debug, Clone)]
pub enum Kind {
    /// Mints a flight; the flight's id is this event's id.
    Filed {
        procedure: String,
        subject: String,
        body: String,
    },
    /// A note on the record, local.
    Commented { flight: EventId, text: String },
    /// `from` depends on `to` — a declared dependency, stored intent.
    Linked { from: EventId, to: EventId },
    /// Puts the flight in the air at assignment; the claimant is the
    /// event's author.
    Claimed { flight: EventId },
    /// Stops the flight with a question — waiting on you until answered.
    Held { flight: EventId, question: String },
    /// Answers the open question and releases the hold.
    Answered { flight: EventId, answer: String },
    /// Takes the flight off the board; the log keeps its record.
    Done { flight: EventId },
    /// A kind from a newer tower, preserved verbatim so the fold can carry
    /// it even though this binary cannot read it.
    Unknown { kind: String, body: Box<RawValue> },
}

impl Kind {
    /// The wire name.
    pub fn name(&self) -> &str {
        match self {
            Kind::Filed { .. } => "filed",
            Kind::Commented { .. } => "commented",
            Kind::Linked { .. } => "linked",
            Kind::Claimed { .. } => "claimed",
            Kind::Held { .. } => "held",
            Kind::Answered { .. } => "answered",
            Kind::Done { .. } => "done",
            Kind::Unknown { kind, .. } => kind,
        }
    }
}

/// One event, as appended and as read back.
#[derive(Debug, Clone)]
pub struct Event {
    pub id: EventId,
    pub author: String,
    pub writer: String,
    pub time: i64,
    pub kind: Kind,
}

/// The wire shape: `kind` a bare string, `body` unparsed. The typed enum
/// is assembled second, so an unknown kind is a value instead of a parse
/// failure.
#[derive(Serialize, Deserialize)]
struct Wire {
    id: EventId,
    author: String,
    writer: String,
    time: i64,
    kind: String,
    body: Box<RawValue>,
}

#[derive(Serialize, Deserialize)]
struct FiledBody {
    procedure: String,
    subject: String,
    body: String,
}

#[derive(Serialize, Deserialize)]
struct CommentedBody {
    flight: EventId,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct LinkedBody {
    from: EventId,
    to: EventId,
}

#[derive(Serialize, Deserialize)]
struct ClaimedBody {
    flight: EventId,
}

#[derive(Serialize, Deserialize)]
struct HeldBody {
    flight: EventId,
    question: String,
}

#[derive(Serialize, Deserialize)]
struct AnsweredBody {
    flight: EventId,
    answer: String,
}

#[derive(Serialize, Deserialize)]
struct DoneBody {
    flight: EventId,
}

impl Serialize for Event {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let body = match &self.kind {
            Kind::Filed {
                procedure,
                subject,
                body,
            } => serde_json::value::to_raw_value(&FiledBody {
                procedure: procedure.clone(),
                subject: subject.clone(),
                body: body.clone(),
            }),
            Kind::Commented { flight, text } => serde_json::value::to_raw_value(&CommentedBody {
                flight: flight.clone(),
                text: text.clone(),
            }),
            Kind::Linked { from, to } => serde_json::value::to_raw_value(&LinkedBody {
                from: from.clone(),
                to: to.clone(),
            }),
            Kind::Claimed { flight } => serde_json::value::to_raw_value(&ClaimedBody {
                flight: flight.clone(),
            }),
            Kind::Held { flight, question } => serde_json::value::to_raw_value(&HeldBody {
                flight: flight.clone(),
                question: question.clone(),
            }),
            Kind::Answered { flight, answer } => serde_json::value::to_raw_value(&AnsweredBody {
                flight: flight.clone(),
                answer: answer.clone(),
            }),
            Kind::Done { flight } => serde_json::value::to_raw_value(&DoneBody {
                flight: flight.clone(),
            }),
            Kind::Unknown { body, .. } => Ok(body.clone()),
        }
        .map_err(serde::ser::Error::custom)?;
        Wire {
            id: self.id.clone(),
            author: self.author.clone(),
            writer: self.writer.clone(),
            time: self.time,
            kind: self.kind.name().to_string(),
            body,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Event, D::Error> {
        let Wire {
            id,
            author,
            writer,
            time,
            kind,
            body,
        } = Wire::deserialize(deserializer)?;
        // A known kind with a body that does not parse is an error — it is
        // this binary's own vocabulary, malformed. Only a kind this binary
        // has never heard of is preserved rather than refused.
        let kind = if kind == "filed" {
            let FiledBody {
                procedure,
                subject,
                body,
            } = serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Filed {
                procedure,
                subject,
                body,
            }
        } else if kind == "commented" {
            let CommentedBody { flight, text } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Commented { flight, text }
        } else if kind == "linked" {
            let LinkedBody { from, to } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Linked { from, to }
        } else if kind == "claimed" {
            let ClaimedBody { flight } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Claimed { flight }
        } else if kind == "held" {
            let HeldBody { flight, question } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Held { flight, question }
        } else if kind == "answered" {
            let AnsweredBody { flight, answer } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Answered { flight, answer }
        } else if kind == "done" {
            let DoneBody { flight } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Done { flight }
        } else {
            Kind::Unknown { kind, body }
        };
        Ok(Event {
            id,
            author,
            writer,
            time,
            kind,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_round_trips_through_text() {
        let id: EventId = "pi.17".parse().expect("parse");
        assert_eq!(id.writer, "pi");
        assert_eq!(id.seq, 17);
        assert_eq!(id.to_string(), "pi.17");
        assert!("pi".parse::<EventId>().is_err());
        assert!(".17".parse::<EventId>().is_err());
        assert!("pi.x".parse::<EventId>().is_err());
    }

    #[test]
    fn a_known_kind_parses_and_an_unknown_kind_is_kept() {
        let filed = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"procedure":"open","subject":"s","body":"b"}}"#;
        let event: Event = serde_json::from_str(filed).expect("parse");
        assert!(matches!(event.kind, Kind::Filed { .. }));

        let future = r#"{"id":"pi.2","author":"a@b.c","writer":"pi","time":8,"kind":"promoted","body":{"flight":"pi.1"}}"#;
        let event: Event = serde_json::from_str(future).expect("parse");
        let Kind::Unknown { kind, body } = &event.kind else {
            panic!("a future kind must be preserved, not dropped");
        };
        assert_eq!(kind, "promoted");
        assert_eq!(body.get(), r#"{"flight":"pi.1"}"#);
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(r#""kind":"promoted""#));
        assert!(json.contains(r#""flight":"pi.1""#));
    }

    #[test]
    fn a_known_kind_with_a_broken_body_is_an_error() {
        let broken = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"nope":true}}"#;
        assert!(serde_json::from_str::<Event>(broken).is_err());
        // The lifecycle vocabulary is this binary's own: a `held` with no
        // question is malformed, not a kind from the future.
        let broken = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"held","body":{"flight":"pi.1"}}"#;
        assert!(serde_json::from_str::<Event>(broken).is_err());
    }

    #[test]
    fn the_lifecycle_kinds_round_trip() {
        let flight: EventId = "pi.1".parse().expect("id");
        let kinds = [
            Kind::Claimed {
                flight: flight.clone(),
            },
            Kind::Held {
                flight: flight.clone(),
                question: "which retry path?".to_string(),
            },
            Kind::Answered {
                flight: flight.clone(),
                answer: "the outer one".to_string(),
            },
            Kind::Done { flight },
        ];
        for (seq, kind) in kinds.into_iter().enumerate() {
            let event = Event {
                id: EventId {
                    writer: "pi".to_string(),
                    seq: seq as u64 + 2,
                },
                author: "a@b.c".to_string(),
                writer: "pi".to_string(),
                time: 7,
                kind,
            };
            let json = serde_json::to_string(&event).expect("serialize");
            assert!(json.contains(&format!(r#""kind":"{}""#, event.kind.name())));
            let back: Event = serde_json::from_str(&json).expect("parse");
            assert_eq!(back.kind.name(), event.kind.name());
            assert!(
                !matches!(back.kind, Kind::Unknown { .. }),
                "a lifecycle kind must parse as itself"
            );
        }
    }
}

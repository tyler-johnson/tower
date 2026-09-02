//! The event: what a person (or an agent) said, as data.
//!
//! Everything in the store is one of these. `id`, `author`, `writer`, and
//! `time` are assigned by the store at append; `kind` is what the caller
//! authored. The wire shape is one JSON object per event, batched into an
//! array in `events.json`:
//!
//! ```json
//! {"id":"pi.17","author":"tyler@example.com","writer":"pi","time":1787638881,
//!  "kind":"filed","body":{"subject":"…","body":"…","status":"triage",…}}
//! ```
//!
//! `body` survives one pass unparsed (`Box<RawValue>`) and is matched into
//! [`Kind`] second — the same two-pass discipline the seam uses on fufu's
//! envelope. A kind this binary does not know becomes [`Kind::Unknown`]
//! with its payload byte-for-byte intact. That is not defensive habit: a
//! union merge can put a newer tower's events in front of an older tower's
//! fold, and dropping them would silently lose authored intent.
//!
//! Statuses, assignees, and done conditions are free strings on this wire
//! while the verbs and the procedure loader hold closed enums — the crew
//! precedent, kept. A known kind with a body that does not parse is an
//! error by design, so a closed enum here would mean one future value
//! taking the whole board down rather than one flight. The refusal
//! belongs at the boundary where a person is typing.

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

/// One authored gesture. Minting, referencing, pairing, the stored
/// fields' moves, and the question pair — enough vocabulary for a flight
/// to be filed, assigned, moved, held on a question, answered, and
/// closed.
#[derive(Debug, Clone)]
pub enum Kind {
    /// Mints a flight; the flight's id is this event's id. Every stored
    /// field is seeded here — the status word included, so a filing
    /// says outright which facts the flight was born with.
    Filed {
        /// Provenance only: the procedure this filing was minted under,
        /// when there was one. Nothing derives from it after the mint.
        procedure: Option<String>,
        subject: String,
        body: String,
        /// A status word: `triage`, `ready` (cleared), `in_progress`,
        /// `done`, `canceled`. The word assigns the fold's facts and the
        /// fold derives the status; `waiting` and `held` in an old log
        /// read as cleared.
        status: String,
        assignee: Option<String>,
        priority: String,
        labels: Vec<String>,
        skill: Option<String>,
        bay: Option<String>,
        done: String,
        /// The branch this flight flies on, resolved at file time from a
        /// definition's `subject = "branch"`. The definition is read
        /// once, so a later `next` reads this rather than the registry.
        branch: Option<String>,
    },
    /// Moves a flight: the word assigns the fold's facts last-wins —
    /// in triage, started, closed — and the fold derives the status, the
    /// byline the mover. Done and Canceled ride here like every other
    /// move, and `next`'s pull appends one per pick in a single batch —
    /// the append is the exclusivity, the byline the pilot.
    Status {
        flight: EventId,
        status: String,
        /// Why it moved — absent when unsaid.
        reason: Option<String>,
    },
    /// Re-lanes a flight: the assignee overwritten last-wins; absent
    /// clears it.
    Assigned {
        flight: EventId,
        assignee: Option<String>,
    },
    /// A note on the record, local.
    Commented { flight: EventId, text: String },
    /// Rewords a flight's fields, or a comment's text — an overlay the
    /// fold applies last-wins per field; the log keeps every prior word.
    Edited {
        /// A flight's id, or a comment's event id.
        target: EventId,
        /// The new subject; flights only, `None` when unchanged.
        subject: Option<String>,
        /// The new body — or, on a comment target, the new text. `None`
        /// when unchanged.
        body: Option<String>,
        priority: Option<String>,
        /// Wholesale replacement — the label set is one value.
        labels: Option<Vec<String>>,
        skill: Option<String>,
        bay: Option<String>,
    },
    /// `from` depends on `to` — a declared dependency, stored intent.
    Linked { from: EventId, to: EventId },
    /// Takes back a declared dependency: the edge `from` → `to` leaves
    /// the record.
    Unlinked { from: EventId, to: EventId },
    /// Saves a view. `view` absent mints one — its id is this event's id,
    /// its owner this event's author; `view` present replaces that view's
    /// three fields wholesale, last-wins in log order. `query` is
    /// `Query::render`'s text: a string on the wire, a struct only in
    /// memory.
    ViewSaved {
        view: Option<EventId>,
        name: String,
        query: String,
        shared: bool,
    },
    /// Removes a view from the set. Final: a later save naming it is a
    /// no-op.
    ViewDeleted { view: EventId },
    /// Stops the flight with a question — the fold derives Held while
    /// it stands, waiting on you until answered — and clears started:
    /// holding is stopping.
    Held { flight: EventId, question: String },
    /// Answers the open question. No status rides it: the fold derives
    /// Ready or Waiting from the facts and the edges beneath.
    Answered { flight: EventId, answer: String },
    /// The lazy pass routing a Triage flight under a procedure — the one
    /// automated stamp. Self-contained on `Filed`'s discipline: the
    /// definition is read at pass time and the resolved overlay copied
    /// in, so the fold never reads config. `status` is a word assigning
    /// the facts like a `Status` event's — `ready`, cleared, whether the
    /// routing collapsed onto the flight or made it a parent whose edges
    /// fold it Waiting; the field options overlay where `Some` and leave
    /// the standing value where `None`.
    Routed {
        flight: EventId,
        procedure: String,
        /// Which rule fired; `""` on events written before rules had
        /// names.
        rule: String,
        /// Render-ready — "matched label chore"; `""` historic.
        because: String,
        status: Option<String>,
        assignee: Option<String>,
        priority: Option<String>,
        labels: Option<Vec<String>>,
        skill: Option<String>,
        bay: Option<String>,
        done: Option<String>,
        /// A definition's `subject = "branch"`, resolved at pass time.
        branch: Option<String>,
    },
    /// A kind from a newer tower, preserved verbatim so the fold can carry
    /// it even though this binary cannot read it. Old logs land here too:
    /// the retired `claimed`/`taken`/`requeued`/`done` kinds deserialize
    /// as unknown rather than as anything at all.
    Unknown { kind: String, body: Box<RawValue> },
}

/// The kinds tower once wrote and no longer reads. An old log's events
/// land on [`Kind::Unknown`] exactly as a newer tower's do, and only this
/// list tells the two apart — what is behind this binary can never route,
/// what is ahead routes on the next upgrade — so doctor reads it to say
/// which of the two a person is looking at.
pub const RETIRED_KINDS: &[&str] = &["claimed", "taken", "requeued", "done"];

impl Kind {
    /// The wire name.
    pub fn name(&self) -> &str {
        match self {
            Kind::Filed { .. } => "filed",
            Kind::Status { .. } => "status",
            Kind::Assigned { .. } => "assigned",
            Kind::Commented { .. } => "commented",
            Kind::Edited { .. } => "edited",
            Kind::Linked { .. } => "linked",
            Kind::Unlinked { .. } => "unlinked",
            Kind::ViewSaved { .. } => "view_saved",
            Kind::ViewDeleted { .. } => "view_deleted",
            Kind::Held { .. } => "held",
            Kind::Answered { .. } => "answered",
            Kind::Routed { .. } => "routed",
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

fn default_status() -> String {
    "triage".to_string()
}

fn default_priority() -> String {
    "none".to_string()
}

fn default_done() -> String {
    "asserted".to_string()
}

/// The stored fields ride the filing with serde defaults, so a body
/// written before the stored model — `procedure` and `part`, no status —
/// still parses: it folds as a Triage flight with default fields, and
/// its `part` key is simply not read. Tolerant on purpose: no
/// `deny_unknown_fields` on any event body.
#[derive(Serialize, Deserialize)]
struct FiledBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    procedure: Option<String>,
    subject: String,
    body: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assignee: Option<String>,
    #[serde(default = "default_priority")]
    priority: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bay: Option<String>,
    #[serde(default = "default_done")]
    done: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct StatusBody {
    flight: EventId,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// `assignee` skipped when absent — absent on the wire is the cleared
/// lane, not a lane called null.
#[derive(Serialize, Deserialize)]
struct AssignedBody {
    flight: EventId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assignee: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CommentedBody {
    flight: EventId,
    text: String,
}

/// Every option follows one discipline — `default`, skipped when absent
/// — so an unchanged field leaves no key on the wire.
#[derive(Serialize, Deserialize)]
struct EditedBody {
    target: EventId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bay: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct LinkedBody {
    from: EventId,
    to: EventId,
}

/// `view` absent is the mint; `shared` is always written, so a saved
/// view says outright whether it is personal.
#[derive(Serialize, Deserialize)]
struct ViewSavedBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    view: Option<EventId>,
    name: String,
    query: String,
    #[serde(default)]
    shared: bool,
}

#[derive(Serialize, Deserialize)]
struct ViewDeletedBody {
    view: EventId,
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

/// Only `flight` and `procedure` are required, and everything else
/// defaults: the live dogfood chain carries pre-stored-model `routed`
/// events shaped `{flight, procedure, because, part}`, and a known kind
/// with a broken body is an error by design — so the old shape must
/// parse, its `part` stamp simply not read. An all-default routed folds
/// as a procedure stamp and nothing else.
#[derive(Serialize, Deserialize)]
struct RoutedBody {
    flight: EventId,
    procedure: String,
    #[serde(default)]
    rule: String,
    #[serde(default)]
    because: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bay: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    done: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
}

impl Serialize for Event {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let body = match &self.kind {
            Kind::Filed {
                procedure,
                subject,
                body,
                status,
                assignee,
                priority,
                labels,
                skill,
                bay,
                done,
                branch,
            } => serde_json::value::to_raw_value(&FiledBody {
                procedure: procedure.clone(),
                subject: subject.clone(),
                body: body.clone(),
                status: status.clone(),
                assignee: assignee.clone(),
                priority: priority.clone(),
                labels: labels.clone(),
                skill: skill.clone(),
                bay: bay.clone(),
                done: done.clone(),
                branch: branch.clone(),
            }),
            Kind::Status {
                flight,
                status,
                reason,
            } => serde_json::value::to_raw_value(&StatusBody {
                flight: flight.clone(),
                status: status.clone(),
                reason: reason.clone(),
            }),
            Kind::Assigned { flight, assignee } => serde_json::value::to_raw_value(&AssignedBody {
                flight: flight.clone(),
                assignee: assignee.clone(),
            }),
            Kind::Commented { flight, text } => serde_json::value::to_raw_value(&CommentedBody {
                flight: flight.clone(),
                text: text.clone(),
            }),
            Kind::Edited {
                target,
                subject,
                body,
                priority,
                labels,
                skill,
                bay,
            } => serde_json::value::to_raw_value(&EditedBody {
                target: target.clone(),
                subject: subject.clone(),
                body: body.clone(),
                priority: priority.clone(),
                labels: labels.clone(),
                skill: skill.clone(),
                bay: bay.clone(),
            }),
            // One body for both edge kinds: the wire shape is identical,
            // and history's loose parse reads `from`/`to` either way.
            Kind::Linked { from, to } | Kind::Unlinked { from, to } => {
                serde_json::value::to_raw_value(&LinkedBody {
                    from: from.clone(),
                    to: to.clone(),
                })
            }
            Kind::ViewSaved {
                view,
                name,
                query,
                shared,
            } => serde_json::value::to_raw_value(&ViewSavedBody {
                view: view.clone(),
                name: name.clone(),
                query: query.clone(),
                shared: *shared,
            }),
            Kind::ViewDeleted { view } => {
                serde_json::value::to_raw_value(&ViewDeletedBody { view: view.clone() })
            }
            Kind::Held { flight, question } => serde_json::value::to_raw_value(&HeldBody {
                flight: flight.clone(),
                question: question.clone(),
            }),
            Kind::Answered { flight, answer } => serde_json::value::to_raw_value(&AnsweredBody {
                flight: flight.clone(),
                answer: answer.clone(),
            }),
            Kind::Routed {
                flight,
                procedure,
                rule,
                because,
                status,
                assignee,
                priority,
                labels,
                skill,
                bay,
                done,
                branch,
            } => serde_json::value::to_raw_value(&RoutedBody {
                flight: flight.clone(),
                procedure: procedure.clone(),
                rule: rule.clone(),
                because: because.clone(),
                status: status.clone(),
                assignee: assignee.clone(),
                priority: priority.clone(),
                labels: labels.clone(),
                skill: skill.clone(),
                bay: bay.clone(),
                done: done.clone(),
                branch: branch.clone(),
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
                status,
                assignee,
                priority,
                labels,
                skill,
                bay,
                done,
                branch,
            } = serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Filed {
                procedure,
                subject,
                body,
                status,
                assignee,
                priority,
                labels,
                skill,
                bay,
                done,
                branch,
            }
        } else if kind == "status" {
            let StatusBody {
                flight,
                status,
                reason,
            } = serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Status {
                flight,
                status,
                reason,
            }
        } else if kind == "assigned" {
            let AssignedBody { flight, assignee } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Assigned { flight, assignee }
        } else if kind == "commented" {
            let CommentedBody { flight, text } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Commented { flight, text }
        } else if kind == "edited" {
            let EditedBody {
                target,
                subject,
                body,
                priority,
                labels,
                skill,
                bay,
            } = serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Edited {
                target,
                subject,
                body,
                priority,
                labels,
                skill,
                bay,
            }
        } else if kind == "linked" {
            let LinkedBody { from, to } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Linked { from, to }
        } else if kind == "unlinked" {
            let LinkedBody { from, to } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Unlinked { from, to }
        } else if kind == "view_saved" {
            let ViewSavedBody {
                view,
                name,
                query,
                shared,
            } = serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::ViewSaved {
                view,
                name,
                query,
                shared,
            }
        } else if kind == "view_deleted" {
            let ViewDeletedBody { view } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::ViewDeleted { view }
        } else if kind == "held" {
            let HeldBody { flight, question } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Held { flight, question }
        } else if kind == "answered" {
            let AnsweredBody { flight, answer } =
                serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Answered { flight, answer }
        } else if kind == "routed" {
            let RoutedBody {
                flight,
                procedure,
                rule,
                because,
                status,
                assignee,
                priority,
                labels,
                skill,
                bay,
                done,
                branch,
            } = serde_json::from_str(body.get()).map_err(serde::de::Error::custom)?;
            Kind::Routed {
                flight,
                procedure,
                rule,
                because,
                status,
                assignee,
                priority,
                labels,
                skill,
                bay,
                done,
                branch,
            }
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

    /// A bare filing's kind — the shape `ff tower file "x"` writes.
    fn plain_filed(subject: &str, body: &str) -> Kind {
        Kind::Filed {
            procedure: None,
            subject: subject.to_string(),
            body: body.to_string(),
            status: "triage".to_string(),
            assignee: None,
            priority: "none".to_string(),
            labels: Vec::new(),
            skill: None,
            bay: None,
            done: "asserted".to_string(),
            branch: None,
        }
    }

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
        let filed = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"subject":"s","body":"b","status":"ready"}}"#;
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
    fn a_plain_filing_serializes_exactly_as_it_did_before_parts() {
        // The pinned byte shape: absent options leave no key, empty
        // labels leave no key, and the three defaulted strings are
        // written out — a filing says outright where the flight was born.
        let event = Event {
            id: "pi.1".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: plain_filed("s", "b"),
        };
        assert_eq!(
            serde_json::to_string(&event).expect("serialize"),
            r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"subject":"s","body":"b","status":"triage","priority":"none","done":"asserted"}}"#
        );
    }

    #[test]
    fn an_old_shape_filing_still_parses_with_defaulted_fields() {
        // A body written before the stored model: `procedure` a bare
        // string, a `part` stamp, no status. It folds as a Triage flight
        // with default fields, and the stamp is simply not read.
        let filed = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"procedure":"review","subject":"the retry test · pass","body":"","part":{"id":"pass","crew":"agent","skill":"review","done":"asserted"}}}"#;
        let event: Event = serde_json::from_str(filed).expect("parse");
        let Kind::Filed {
            procedure,
            subject,
            status,
            assignee,
            priority,
            labels,
            skill,
            done,
            ..
        } = event.kind
        else {
            panic!("a filing parses as a filing");
        };
        assert_eq!(procedure.as_deref(), Some("review"));
        assert_eq!(subject, "the retry test · pass");
        assert_eq!(status, "triage");
        assert!(assignee.is_none(), "the old stamp's crew is not read");
        assert_eq!(priority, "none");
        assert!(labels.is_empty());
        assert!(skill.is_none());
        assert_eq!(done, "asserted");
    }

    #[test]
    fn a_retired_lifecycle_kind_folds_as_unknown() {
        // `claimed` and its siblings left the vocabulary at the stored
        // model; an old log's events survive as unknown, byte-intact.
        for kind in ["claimed", "taken", "requeued", "done"] {
            let old = format!(
                r#"{{"id":"pi.2","author":"a@b.c","writer":"pi","time":8,"kind":"{kind}","body":{{"flight":"pi.1"}}}}"#
            );
            let event: Event = serde_json::from_str(&old).expect("parse");
            let Kind::Unknown { kind: name, body } = &event.kind else {
                panic!("a retired kind is unknown, not refused: {kind}");
            };
            assert_eq!(name, kind);
            assert_eq!(body.get(), r#"{"flight":"pi.1"}"#);
        }
    }

    #[test]
    fn a_routing_round_trips_and_omits_the_fields_it_does_not_overlay() {
        let event = Event {
            id: "pi.9".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::Routed {
                flight: "pi.1".parse().expect("id"),
                procedure: "review".to_string(),
                rule: "github-reviews".to_string(),
                because: "matched label chore".to_string(),
                status: Some("ready".to_string()),
                assignee: Some("agent".to_string()),
                priority: None,
                labels: None,
                skill: Some("review".to_string()),
                bay: None,
                done: None,
                branch: Some("feather".to_string()),
            },
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(r#""kind":"routed""#), "got {json}");
        assert!(json.contains(r#""rule":"github-reviews""#), "got {json}");
        assert!(json.contains(r#""status":"ready""#), "got {json}");
        assert!(
            !json.contains(r#""priority""#),
            "an unchanged field leaves no key: {json}"
        );
        let back: Event = serde_json::from_str(&json).expect("parse");
        let Kind::Routed {
            procedure,
            because,
            assignee,
            skill,
            branch,
            labels,
            ..
        } = back.kind
        else {
            panic!("a routing parses as a routing");
        };
        assert_eq!(procedure, "review");
        assert_eq!(because, "matched label chore");
        assert_eq!(assignee.as_deref(), Some("agent"));
        assert_eq!(skill.as_deref(), Some("review"));
        assert_eq!(branch.as_deref(), Some("feather"));
        assert!(labels.is_none());
    }

    #[test]
    fn a_pre_stored_model_routing_still_parses_with_defaults() {
        // The live dogfood chain's shape: `{flight, procedure, because,
        // part}`. It parses as a routing that stamps the procedure and
        // moves nothing, and the `part` stamp is simply not read.
        let old = r#"{"id":"pi.9","author":"a@b.c","writer":"pi","time":7,"kind":"routed","body":{"flight":"pi.1","procedure":"review","because":"matched","part":{"id":"pass","crew":"agent"}}}"#;
        let event: Event = serde_json::from_str(old).expect("parse");
        let Kind::Routed {
            procedure,
            rule,
            because,
            status,
            assignee,
            priority,
            labels,
            skill,
            bay,
            done,
            branch,
            ..
        } = event.kind
        else {
            panic!("an old routing parses as a routing");
        };
        assert_eq!(procedure, "review");
        assert_eq!(rule, "", "no rule name before rules had names");
        assert_eq!(because, "matched");
        assert!(status.is_none(), "an all-default routing moves nothing");
        assert!(
            assignee.is_none()
                && priority.is_none()
                && labels.is_none()
                && skill.is_none()
                && bay.is_none()
                && done.is_none()
                && branch.is_none()
        );
    }

    #[test]
    fn a_stored_field_filing_round_trips_and_omits_what_it_does_not_carry() {
        let event = Event {
            id: "pi.2".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::Filed {
                procedure: Some("review".to_string()),
                subject: "the retry test · pass".to_string(),
                body: String::new(),
                status: "ready".to_string(),
                assignee: Some("agent".to_string()),
                priority: "high".to_string(),
                labels: vec!["chore".to_string()],
                skill: Some("review".to_string()),
                bay: None,
                done: "asserted".to_string(),
                branch: Some("feather".to_string()),
            },
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(r#""status":"ready""#), "got {json}");
        assert!(json.contains(r#""assignee":"agent""#), "got {json}");
        assert!(json.contains(r#""labels":["chore"]"#), "got {json}");
        assert!(json.contains(r#""branch":"feather""#), "got {json}");
        assert!(
            !json.contains(r#""bay""#),
            "an absent bay leaves no key: {json}"
        );

        let back: Event = serde_json::from_str(&json).expect("parse");
        let Kind::Filed {
            status,
            assignee,
            priority,
            labels,
            skill,
            bay,
            branch,
            ..
        } = back.kind
        else {
            panic!("a filing parses as a filing");
        };
        assert_eq!(status, "ready");
        assert_eq!(assignee.as_deref(), Some("agent"));
        assert_eq!(priority, "high");
        assert_eq!(labels, ["chore"]);
        assert_eq!(skill.as_deref(), Some("review"));
        assert!(bay.is_none());
        assert_eq!(branch.as_deref(), Some("feather"));
    }

    #[test]
    fn a_status_this_binary_has_never_heard_of_still_parses() {
        // The tolerance rule: the enum is closed at the verbs and open
        // here, so a newer tower's value costs one flight's fidelity
        // rather than the whole board.
        let filed = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"subject":"s","body":"","status":"parked","assignee":"pair"}}"#;
        let event: Event = serde_json::from_str(filed).expect("parse");
        let Kind::Filed {
            status, assignee, ..
        } = event.kind
        else {
            panic!("a filing parses as a filing");
        };
        assert_eq!(status, "parked");
        assert_eq!(assignee.as_deref(), Some("pair"));
    }

    #[test]
    fn a_known_kind_with_a_broken_body_is_an_error() {
        let broken = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"filed","body":{"nope":true}}"#;
        assert!(serde_json::from_str::<Event>(broken).is_err());
        // The lifecycle vocabulary is this binary's own: a `held` with no
        // question is malformed, not a kind from the future.
        let broken = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"held","body":{"flight":"pi.1"}}"#;
        assert!(serde_json::from_str::<Event>(broken).is_err());
        // So is a `status` that names no status.
        let broken = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"status","body":{"flight":"pi.1"}}"#;
        assert!(serde_json::from_str::<Event>(broken).is_err());
    }

    #[test]
    fn a_status_move_round_trips_and_omits_an_unsaid_reason() {
        let unsaid = Event {
            id: "pi.5".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::Status {
                flight: "pi.1".parse().expect("id"),
                status: "in_progress".to_string(),
                reason: None,
            },
        };
        let json = serde_json::to_string(&unsaid).expect("serialize");
        assert!(json.contains(r#""kind":"status""#), "got {json}");
        assert!(
            json.contains(r#""body":{"flight":"pi.1","status":"in_progress"}"#),
            "an unsaid reason leaves no key: {json}"
        );

        let said = Event {
            id: "pi.6".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 8,
            kind: Kind::Status {
                flight: "pi.1".parse().expect("id"),
                status: "canceled".to_string(),
                reason: Some("superseded by #9".to_string()),
            },
        };
        let json = serde_json::to_string(&said).expect("serialize");
        let back: Event = serde_json::from_str(&json).expect("parse");
        let Kind::Status { status, reason, .. } = back.kind else {
            panic!("a status parses as a status");
        };
        assert_eq!(status, "canceled");
        assert_eq!(reason.as_deref(), Some("superseded by #9"));
    }

    #[test]
    fn an_assignment_round_trips_and_absent_is_the_cleared_lane() {
        let cleared = Event {
            id: "pi.5".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::Assigned {
                flight: "pi.1".parse().expect("id"),
                assignee: None,
            },
        };
        let json = serde_json::to_string(&cleared).expect("serialize");
        assert!(
            json.contains(r#""body":{"flight":"pi.1"}"#),
            "the cleared lane leaves no key: {json}"
        );
        let back: Event = serde_json::from_str(&json).expect("parse");
        let Kind::Assigned { assignee, .. } = back.kind else {
            panic!("an assignment parses as one");
        };
        assert!(assignee.is_none());

        let laned = Event {
            id: "pi.6".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 8,
            kind: Kind::Assigned {
                flight: "pi.1".parse().expect("id"),
                assignee: Some("agent".to_string()),
            },
        };
        let json = serde_json::to_string(&laned).expect("serialize");
        assert!(json.contains(r#""assignee":"agent""#), "got {json}");
    }

    #[test]
    fn an_edit_round_trips_and_omits_the_fields_it_does_not_carry() {
        let edit = |subject: Option<&str>, body: Option<&str>| Event {
            id: "pi.9".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::Edited {
                target: "pi.1".parse().expect("id"),
                subject: subject.map(str::to_string),
                body: body.map(str::to_string),
                priority: None,
                labels: None,
                skill: None,
                bay: None,
            },
        };

        let subject_only = serde_json::to_string(&edit(Some("new subject"), None)).expect("json");
        assert!(
            subject_only.contains(r#""kind":"edited""#),
            "got {subject_only}"
        );
        assert!(
            subject_only.contains(r#""body":{"target":"pi.1","subject":"new subject"}"#),
            "an unchanged field leaves no key: {subject_only}"
        );

        let body_only = serde_json::to_string(&edit(None, Some("new body"))).expect("json");
        assert!(
            body_only.contains(r#""body":{"target":"pi.1","body":"new body"}"#),
            "got {body_only}"
        );

        let both: Event = serde_json::from_str(
            &serde_json::to_string(&edit(Some("s"), Some("b"))).expect("json"),
        )
        .expect("parse");
        let Kind::Edited {
            target,
            subject,
            body,
            ..
        } = both.kind
        else {
            panic!("an edit parses as an edit");
        };
        assert_eq!(target.to_string(), "pi.1");
        assert_eq!(subject.as_deref(), Some("s"));
        assert_eq!(body.as_deref(), Some("b"));
    }

    #[test]
    fn a_field_edit_round_trips_with_labels_wholesale() {
        let event = Event {
            id: "pi.9".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::Edited {
                target: "pi.1".parse().expect("id"),
                subject: None,
                body: None,
                priority: Some("high".to_string()),
                labels: Some(vec!["chore".to_string(), "web".to_string()]),
                skill: Some("review".to_string()),
                bay: Some("warm".to_string()),
            },
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(r#""priority":"high""#), "got {json}");
        assert!(json.contains(r#""labels":["chore","web"]"#), "got {json}");
        let back: Event = serde_json::from_str(&json).expect("parse");
        let Kind::Edited {
            priority,
            labels,
            skill,
            bay,
            ..
        } = back.kind
        else {
            panic!("an edit parses as an edit");
        };
        assert_eq!(priority.as_deref(), Some("high"));
        assert_eq!(
            labels.as_deref(),
            Some(&["chore".to_string(), "web".to_string()][..])
        );
        assert_eq!(skill.as_deref(), Some("review"));
        assert_eq!(bay.as_deref(), Some("warm"));
    }

    #[test]
    fn an_edit_with_no_target_is_an_error() {
        // A known kind with a broken body is malformed, not a kind from
        // the future.
        let broken = r#"{"id":"pi.1","author":"a@b.c","writer":"pi","time":7,"kind":"edited","body":{"subject":"s"}}"#;
        assert!(serde_json::from_str::<Event>(broken).is_err());
    }

    #[test]
    fn the_lifecycle_kinds_round_trip() {
        let flight: EventId = "pi.1".parse().expect("id");
        let kinds = [
            Kind::Status {
                flight: flight.clone(),
                status: "ready".to_string(),
                reason: None,
            },
            Kind::Assigned {
                flight: flight.clone(),
                assignee: Some("agent".to_string()),
            },
            Kind::Held {
                flight: flight.clone(),
                question: "which retry path?".to_string(),
            },
            Kind::Answered {
                flight: flight.clone(),
                answer: "the outer one".to_string(),
            },
            Kind::Linked {
                from: flight.clone(),
                to: "pi.9".parse().expect("id"),
            },
            Kind::Unlinked {
                from: flight.clone(),
                to: "pi.9".parse().expect("id"),
            },
            Kind::ViewSaved {
                view: Some(flight.clone()),
                name: "mine".to_string(),
                query: "assignee=me".to_string(),
                shared: false,
            },
            Kind::ViewDeleted { view: flight },
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

    #[test]
    fn a_view_save_with_no_view_key_is_the_mint() {
        let minted = r#"{"id":"pi.3","author":"a@b.c","writer":"pi","time":7,"kind":"view_saved","body":{"name":"mine","query":"assignee=me","shared":true}}"#;
        let event: Event = serde_json::from_str(minted).expect("parse");
        let Kind::ViewSaved {
            view,
            name,
            query,
            shared,
        } = event.kind
        else {
            panic!("a view save parses as one");
        };
        assert!(view.is_none(), "no `view` key is the mint");
        assert_eq!(name, "mine");
        assert_eq!(query, "assignee=me");
        assert!(shared);
        // `shared` defaults false when an older writer left it out.
        let bare = r#"{"id":"pi.3","author":"a@b.c","writer":"pi","time":7,"kind":"view_saved","body":{"name":"mine","query":""}}"#;
        let event: Event = serde_json::from_str(bare).expect("parse");
        let Kind::ViewSaved { shared, .. } = event.kind else {
            panic!("a view save parses as one");
        };
        assert!(!shared);
    }

    #[test]
    fn a_minting_view_save_serializes_without_a_view_key() {
        // The pinned byte shape: the mint leaves no `view` key, and
        // `shared` is always written.
        let event = Event {
            id: "pi.3".parse().expect("id"),
            author: "a@b.c".to_string(),
            writer: "pi".to_string(),
            time: 7,
            kind: Kind::ViewSaved {
                view: None,
                name: "mine".to_string(),
                query: "assignee=me&group=status".to_string(),
                shared: false,
            },
        };
        assert_eq!(
            serde_json::to_string(&event).expect("serialize"),
            r#"{"id":"pi.3","author":"a@b.c","writer":"pi","time":7,"kind":"view_saved","body":{"name":"mine","query":"assignee=me&group=status","shared":false}}"#
        );
    }
}

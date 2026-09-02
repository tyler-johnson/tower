//! The history: what happened to one flight, read off the log.
//!
//! The fold keeps last-wins marks, not events — a flight claimed and
//! requeued three times carries one `claim` and one `requeued_at` — so
//! the moments cannot come from [`Flight`](super::Flight). They come from
//! the events the fold was built out of, filtered to the ones that name
//! this flight, in log order.
//!
//! Deliberately thin. A moment says who did what, when, and the words the
//! verb took: the status word, the lane, the fields an edit touched, the
//! other end of an edge. It does not repeat the subject, the body, the
//! open question, or a comment's text, which sit flat on the
//! [`Brief`](super::Brief) and would go stale here.

use serde::{Deserialize, Serialize};

use crate::log::{Event, EventId, Kind};

/// One gesture on a flight's record.
#[derive(Debug, Serialize)]
pub struct Moment {
    /// The event's wire id.
    pub id: String,
    pub at: i64,
    /// The event's author, verbatim — never assumed to be the reader.
    pub by: String,
    /// The kind's own name; an unknown kind carries its own string
    /// through.
    pub what: String,
    /// What the gesture carried, per kind; absent when the kind carries
    /// nothing the brief does not already say.
    #[serde(flatten)]
    pub detail: Option<Detail>,
}

/// The detail a kind carries. Untagged and flattened, so on the wire the
/// keys sit flat beside `what` and `what` stays a string.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Detail {
    /// The word the person used, verbatim: `ready`, `in_progress`,
    /// `done`, `canceled`; the fold decides where it lands. `reason` is a
    /// cancel's `-m`, which no other surface carries.
    Status {
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// The lane; `None` (JSON `null`) is the clearing.
    Assigned { assignee: Option<String> },
    /// The wire names of the fields the edit touched, in the enum's
    /// order: subject, body, priority, labels, skill, bay. `comment` is
    /// the comment's event id when the target was a comment rather than
    /// the flight.
    Edited {
        fields: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comment: Option<String>,
    },
    /// Both ends, wire ids, `from` depends on `to`; the same shape under
    /// `linked` and `unlinked`.
    Edge { from: String, to: String },
    /// Which procedure and rule fired, and the render-ready `because`,
    /// which the brief does not carry. `rule` and `because` may be `""`
    /// on historic events; skipped when empty.
    Routed {
        procedure: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        rule: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        because: String,
    },
}

/// The loose read of an unknown kind's body: the four field names every
/// known kind uses to name a flight. Unknown fields are ignored, so a
/// newer tower's gesture is judged on what this one can recognize and
/// degrades to a labeled row rather than vanishing — which is what
/// [`Kind::Unknown`]'s own doc promises the fold. A body whose `flight`
/// is not a wire id at all fails this parse and is skipped: best-effort
/// is the whole contract here, and guessing would be worse.
#[derive(Deserialize)]
struct Names {
    #[serde(default)]
    flight: Option<EventId>,
    #[serde(default)]
    target: Option<EventId>,
    #[serde(default)]
    from: Option<EventId>,
    #[serde(default)]
    to: Option<EventId>,
}

/// Every event naming this flight, oldest first — the reading order
/// [`Brief::comments`](super::Brief::comments) already uses.
///
/// `Edited` lands on a comment's event id as well as the flight's own: a
/// reword is a gesture on the flight, and the log's order means a
/// comment's id is always known before an edit can target it.
pub fn history(events: &[Event], flight: &EventId) -> Vec<Moment> {
    let mut comments: Vec<&EventId> = Vec::new();
    let mut moments = Vec::new();
    for event in events {
        let names = match &event.kind {
            // A filing mints the flight: the event's own id is the name.
            Kind::Filed { .. } => &event.id == flight,
            Kind::Status { flight: on, .. }
            | Kind::Assigned { flight: on, .. }
            | Kind::Commented { flight: on, .. }
            | Kind::Held { flight: on, .. }
            | Kind::Answered { flight: on, .. }
            | Kind::Routed { flight: on, .. } => on == flight,
            Kind::Edited { target, .. } => target == flight || comments.contains(&target),
            Kind::Linked { from, to } | Kind::Unlinked { from, to } => {
                from == flight || to == flight
            }
            // A view names no flight.
            Kind::ViewSaved { .. } | Kind::ViewDeleted { .. } => false,
            Kind::Unknown { body, .. } => {
                serde_json::from_str::<Names>(body.get()).is_ok_and(|names| {
                    [names.flight, names.target, names.from, names.to]
                        .iter()
                        .flatten()
                        .any(|id| id == flight)
                })
            }
        };
        if let Kind::Commented { flight: on, .. } = &event.kind
            && on == flight
        {
            comments.push(&event.id);
        }
        if names {
            moments.push(Moment {
                id: event.id.to_string(),
                at: event.time,
                by: event.author.clone(),
                what: event.kind.name().to_string(),
                detail: detail(&event.kind, &comments),
            });
        }
    }
    moments
}

/// The words a kind carries beyond its name. `filed`, `commented`,
/// `held`, and `answered` carry `None`: their words sit flat on the brief.
/// An unknown kind's words are unknowable.
fn detail(kind: &Kind, comments: &[&EventId]) -> Option<Detail> {
    match kind {
        Kind::Status { status, reason, .. } => Some(Detail::Status {
            status: status.clone(),
            reason: reason.clone(),
        }),
        Kind::Assigned { assignee, .. } => Some(Detail::Assigned {
            assignee: assignee.clone(),
        }),
        Kind::Edited {
            target,
            subject,
            body,
            priority,
            labels,
            skill,
            bay,
        } => {
            let fields = [
                ("subject", subject.is_some()),
                ("body", body.is_some()),
                ("priority", priority.is_some()),
                ("labels", labels.is_some()),
                ("skill", skill.is_some()),
                ("bay", bay.is_some()),
            ]
            .into_iter()
            .filter(|(_, set)| *set)
            .map(|(name, _)| name.to_string())
            .collect();
            Some(Detail::Edited {
                fields,
                comment: comments.contains(&target).then(|| target.to_string()),
            })
        }
        Kind::Linked { from, to } | Kind::Unlinked { from, to } => Some(Detail::Edge {
            from: from.to_string(),
            to: to.to_string(),
        }),
        Kind::Routed {
            procedure,
            rule,
            because,
            ..
        } => Some(Detail::Routed {
            procedure: procedure.clone(),
            rule: rule.clone(),
            because: because.clone(),
        }),
        Kind::Filed { .. }
        | Kind::Commented { .. }
        | Kind::Held { .. }
        | Kind::Answered { .. }
        | Kind::ViewSaved { .. }
        | Kind::ViewDeleted { .. }
        | Kind::Unknown { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind,
        }
    }

    fn filed(id: &str, time: i64) -> Event {
        event(
            id,
            time,
            Kind::Filed {
                procedure: None,
                subject: "the work".to_string(),
                body: String::new(),
                status: "triage".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: String::new(),
                branch: None,
            },
        )
    }

    fn json(moments: &[Moment]) -> Vec<serde_json::Value> {
        moments
            .iter()
            .map(|moment| serde_json::to_value(moment).expect("serializes"))
            .collect()
    }

    #[test]
    fn an_edit_names_its_fields_in_enum_order_and_its_comment_target() {
        let flight: EventId = "pi.1".parse().expect("id");
        let events = vec![
            filed("pi.1", 1),
            event(
                "pi.2",
                2,
                Kind::Commented {
                    flight: flight.clone(),
                    text: "a note".to_string(),
                },
            ),
            event(
                "pi.3",
                3,
                Kind::Edited {
                    target: flight.clone(),
                    bay: Some("warm".to_string()),
                    skill: Some("review".to_string()),
                    labels: Some(vec!["chore".to_string()]),
                    priority: Some("high".to_string()),
                    body: Some("the body".to_string()),
                    subject: Some("the subject".to_string()),
                },
            ),
            event(
                "pi.4",
                4,
                Kind::Edited {
                    target: "pi.2".parse().expect("id"),
                    subject: None,
                    body: Some("a fuller note".to_string()),
                    priority: None,
                    labels: None,
                    skill: None,
                    bay: None,
                },
            ),
        ];
        let rows = json(&history(&events, &flight));
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[2]["fields"],
            serde_json::json!(["subject", "body", "priority", "labels", "skill", "bay"])
        );
        assert!(rows[2].get("comment").is_none(), "{}", rows[2]);
        assert_eq!(rows[3]["fields"], serde_json::json!(["body"]));
        assert_eq!(rows[3]["comment"], serde_json::json!("pi.2"));
    }

    #[test]
    fn a_clearing_is_a_null_lane_and_a_filing_carries_the_four_keys_alone() {
        let flight: EventId = "pi.1".parse().expect("id");
        let events = vec![
            filed("pi.1", 1),
            event(
                "pi.2",
                2,
                Kind::Assigned {
                    flight: flight.clone(),
                    assignee: None,
                },
            ),
        ];
        let rows = json(&history(&events, &flight));
        assert_eq!(
            rows[0],
            serde_json::json!({"id": "pi.1", "at": 1, "by": "a@b.c", "what": "filed"})
        );
        let lane = rows[1].as_object().expect("an object");
        assert!(lane.contains_key("assignee"), "{}", rows[1]);
        assert!(lane["assignee"].is_null(), "{}", rows[1]);
    }
}

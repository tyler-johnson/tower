//! The mint batch: a definition's flights as `filed` events, shared by
//! `file` and `decompose`'s procedure form.

use crate::log::{EventId, Kind};
use crate::procedure::{Definition, FlightDef};

/// Whose edge the flights hang from: `file`'s parent is the batch's own
/// first mint, `decompose`'s already exists on the board.
pub enum Parent {
    Mint,
    Existing(EventId),
}

/// The caller's field flags, overlaid where they apply: on the one
/// flight of a collapsed mint, and on the parent of a multi-flight one.
/// `assignee` arrives already validated — a lane name, or `None`.
#[derive(Default)]
pub struct Fields {
    pub message: Option<String>,
    pub priority: Option<String>,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    pub assignee: Option<String>,
    pub bay: Option<String>,
}

/// The mint batch for one definition. Under `Parent::Mint`, the head
/// occupies `mint(0)`: a single-flight definition collapses onto it —
/// one `filed`, born Ready, the definition's fields under the caller's
/// flags — and a multi-flight one makes it a parent, born Waiting, that
/// carries the caller's flags while the children carry the definition's.
/// Under `Parent::Existing`, every minted event is a child of the id
/// given. Children keep `"{subject} · {id}"` subjects, are born Ready
/// with no `after` and Waiting with any, and ride `linked` edges — the
/// parent's to each, then the `after` DAG — in the order the ids are
/// read back out of.
pub fn classify(
    definition: &Definition,
    subject: &str,
    fields: &Fields,
    parent: Parent,
    mint: &dyn Fn(usize) -> EventId,
) -> Vec<Kind> {
    let flights = &definition.flights;
    if let (Parent::Mint, [only]) = (&parent, flights.as_slice()) {
        return vec![filed(
            definition,
            only,
            subject.to_string(),
            fields.message.clone().unwrap_or_default(),
            "ready",
            subject,
            Some(fields),
        )];
    }

    let mut kinds = Vec::new();
    if matches!(parent, Parent::Mint) {
        // The parent row: the caller's flags land here, and it waits on
        // every child by edge, so it is born Waiting outright.
        kinds.push(Kind::Filed {
            procedure: Some(definition.name.clone()),
            subject: subject.to_string(),
            body: fields.message.clone().unwrap_or_default(),
            status: "waiting".to_string(),
            assignee: fields.assignee.clone(),
            priority: fields
                .priority
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            labels: fields.labels.clone(),
            skill: fields.skill.clone(),
            bay: fields.bay.clone(),
            done: "asserted".to_string(),
            branch: None,
        });
    }
    let head = kinds.len();
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached, so the child's subject carries the parent's.
    kinds.extend(flights.iter().map(|flight| {
        let born = if flight.after.is_empty() {
            "ready"
        } else {
            "waiting"
        };
        filed(
            definition,
            flight,
            format!("{subject} · {}", flight.id),
            String::new(),
            born,
            subject,
            None,
        )
    }));
    let from = match &parent {
        Parent::Mint => mint(0),
        Parent::Existing(id) => id.clone(),
    };
    kinds.extend((0..flights.len()).map(|offset| Kind::Linked {
        from: from.clone(),
        to: mint(head + offset),
    }));
    for (offset, flight) in flights.iter().enumerate() {
        for after in &flight.after {
            // Infallible: the loader refused an `after` naming nothing.
            let at = flights
                .iter()
                .position(|other| &other.id == after)
                .expect("`after` names a flight the loader validated");
            kinds.push(Kind::Linked {
                from: mint(head + offset),
                to: mint(head + at),
            });
        }
    }
    kinds
}

/// One definition flight as the log carries it: the closed enums become
/// their names, the caller's flags win where an overlay rides, and the
/// definition's `subject = "branch"` rule resolves once — the subject
/// handed in is the flight's own, never the `"{subject} · {id}"` a child
/// is filed under, so for `review` the branch is the parent's subject.
fn filed(
    definition: &Definition,
    flight: &FlightDef,
    subject: String,
    body: String,
    born: &str,
    branch_subject: &str,
    overlay: Option<&Fields>,
) -> Kind {
    let own_assignee = Some(flight.assignee.name().to_string());
    let own_bay = flight.bay.map(|bay| bay.name().to_string());
    let (assignee, priority, labels, skill, bay) = match overlay {
        Some(fields) => (
            fields.assignee.clone().or(own_assignee),
            fields
                .priority
                .clone()
                .or_else(|| flight.priority.clone())
                .unwrap_or_else(|| "none".to_string()),
            if fields.labels.is_empty() {
                flight.labels.clone()
            } else {
                fields.labels.clone()
            },
            fields.skill.clone().or_else(|| flight.skill.clone()),
            fields.bay.clone().or(own_bay),
        ),
        None => (
            own_assignee,
            flight
                .priority
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            flight.labels.clone(),
            flight.skill.clone(),
            own_bay,
        ),
    };
    Kind::Filed {
        procedure: Some(definition.name.clone()),
        subject,
        body,
        status: born.to_string(),
        assignee,
        priority,
        labels,
        skill,
        bay,
        done: flight.done.name().to_string(),
        branch: (definition.subject.as_deref() == Some("branch"))
            .then(|| branch_subject.to_string()),
    }
}

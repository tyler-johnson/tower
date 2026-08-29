//! `file <subject> [-m <body>] [-p <procedure>]` — mint a flight under a
//! procedure.
//!
//! `-p` is a classification, not a stamp: the named definition is looked
//! up in the registry, refused when it is not installed, and its parts are
//! filed with it. The definition is read here and never again — each
//! part's crew, skill, done and bay are copied into the log, so editing a
//! definition afterwards cannot disturb a flight already in the air.
//!
//! Two shapes fall out of the part count. **One part** collapses onto the
//! flight: the flight *is* the part and carries its stamp, because
//! `ff tower file "fix the typo"` must not cost two flights to say one
//! thing. **Two or more** file a parent plus one flight per part, on the
//! same `linked` edges `decompose` writes — as that verb's own docs
//! predicted, a definition's parts replace the arguments and the edges
//! stay the same. All of it in one `append_with`: two appends would leave
//! a window where the parent is live, unlinked, and claimable.
//!
//! Still no fufu spawn. The registry's repository layer resolves through
//! `Store::main_worktree`, which reads the common dir and runs nothing.

use serde::Serialize;

use crate::log::{Event, EventId, Kind, Store};
use crate::procedure;

use super::{Error, Parent, appended, appended_all, classify};

/// The envelope's `data`. Struct fields serialize in declaration order,
/// and this order — `filed, linked, parts` — is the alphabetical one the
/// CLI's `json!` emitted before the payload moved here, so the bytes on
/// the wire never changed.
#[derive(Serialize)]
pub struct Filed {
    pub filed: Event,
    pub linked: Vec<Event>,
    pub parts: Vec<Event>,
}

/// The outcome: the payload, plus the ids a human render echoes — it
/// re-folds for the display numbers, so the machine path never has to.
pub struct File {
    pub payload: Filed,
    pub parent: EventId,
    pub part_ids: Vec<EventId>,
}

pub fn file(
    store: &Store,
    subject: &str,
    message: Option<String>,
    procedure: Option<String>,
) -> Result<File, Error> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(Error::EmptySubject);
    }
    let name = match procedure {
        Some(name) => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(Error::EmptyProcedure);
            }
            name
        }
        None => "open".to_string(),
    };

    let installed = procedure::registry(store.main_worktree().as_deref())?;
    let definition = installed.require(&name)?;

    let body = message.unwrap_or_default();
    let ids = store.append_with(|mint| {
        classify(
            definition,
            subject,
            Parent::Mint,
            |part| Kind::Filed {
                procedure: definition.name.clone(),
                subject: subject.to_string(),
                body: body.clone(),
                part,
            },
            mint,
        )
    })?;
    let (parent, rest) = ids.split_first().expect("the parent is the first event");
    let parts = if definition.parts.len() == 1 {
        0
    } else {
        definition.parts.len()
    };
    let (filed, linked) = rest.split_at(parts);

    Ok(File {
        payload: Filed {
            filed: appended(store, parent)?,
            linked: appended_all(store, linked)?,
            parts: appended_all(store, filed)?,
        },
        parent: parent.clone(),
        part_ids: filed.to_vec(),
    })
}

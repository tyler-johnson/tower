//! The classification batch, shared by `file` and `triage`.

use crate::log::{EventId, Kind, PartStamp};
use crate::procedure::{Definition, Part};

/// Whose edge the parts hang from: `file`'s parent is the batch's own
/// first mint, `triage`'s already exists on the board.
pub enum Parent {
    Mint,
    Existing(EventId),
}

/// The classification batch, shared by `file` and `triage`: the head
/// event, then — when the definition has two or more parts — one filing
/// per part, the parent's edge to each part, and the `after` DAG between
/// parts, in the order the ids are read back out of. The head is the
/// caller's to build (`file` makes `Filed`, `triage` makes `Routed`); it
/// occupies `mint(0)` and the parts occupy `mint(1..)`.
///
/// A single-part definition collapses: the head carries the stamp and the
/// batch is one event — the reason this returns a plan rather than a
/// fixed shape.
pub fn classify(
    definition: &Definition,
    subject: &str,
    parent: Parent,
    head: impl FnOnce(Option<PartStamp>) -> Kind,
    mint: &dyn Fn(usize) -> EventId,
) -> Vec<Kind> {
    let parts = &definition.parts;
    if let [only] = parts.as_slice() {
        return vec![head(Some(stamp(definition, subject, only)))];
    }

    let mut kinds = vec![head(None)];
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached, so the part's subject carries the parent's.
    kinds.extend(parts.iter().map(|part| Kind::Filed {
        procedure: definition.name.clone(),
        subject: format!("{subject} · {}", part.id),
        body: String::new(),
        part: Some(stamp(definition, subject, part)),
    }));
    let from = match &parent {
        Parent::Mint => mint(0),
        Parent::Existing(id) => id.clone(),
    };
    kinds.extend((0..parts.len()).map(|offset| Kind::Linked {
        from: from.clone(),
        to: mint(offset + 1),
    }));
    for (offset, part) in parts.iter().enumerate() {
        for after in &part.after {
            // Infallible: the loader refused an `after` naming nothing.
            let at = parts
                .iter()
                .position(|other| &other.id == after)
                .expect("`after` names a part the loader validated");
            kinds.push(Kind::Linked {
                from: mint(offset + 1),
                to: mint(at + 1),
            });
        }
    }
    kinds
}

/// A definition's part, as the log carries it. The closed enums become
/// their names here; the log stays tolerant where the config stays closed.
///
/// `branch` is the definition's `subject` rule resolved once, at file
/// time: a procedure whose subject *is* a branch stamps the branch each
/// part flies on, so `next` binds from the stamp rather than reading the
/// definition again. The subject handed in is the flight's own, never the
/// `"{subject} · {part.id}"` a part is filed under — for `review`, the
/// branch is the parent's subject.
pub fn stamp(definition: &Definition, subject: &str, part: &Part) -> PartStamp {
    PartStamp {
        id: part.id.clone(),
        crew: part.crew.name().to_string(),
        skill: part.skill.clone(),
        done: part.done.name().to_string(),
        bay: part.bay.map(|bay| bay.name().to_string()),
        branch: (definition.subject.as_deref() == Some("branch")).then(|| subject.to_string()),
    }
}

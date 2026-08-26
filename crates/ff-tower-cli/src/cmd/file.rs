//! `ff tower file <subject> [-m <body>] [-p <procedure>]` — mint a flight
//! under a procedure.
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

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::{Kind, PartStamp};
use ff_tower_core::procedure::{self, Definition, Part};

pub fn run(
    json: bool,
    subject: &str,
    message: Option<String>,
    procedure: Option<String>,
) -> Result<(), CliError> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(CliError::coded(
            "usage/empty-subject",
            "the subject is empty",
            Vec::new(),
        ));
    }
    let name = match procedure {
        Some(name) => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(CliError::coded(
                    "usage/empty-procedure",
                    "`-p` names an empty procedure",
                    Vec::new(),
                ));
            }
            name
        }
        None => "open".to_string(),
    };

    let store = super::store()?;
    let installed = procedure::registry(store.main_worktree().as_deref())?;
    let definition = installed.get(&name).ok_or_else(|| {
        CliError::coded(
            "procedure/not-found",
            format!(
                "no procedure `{name}` — installed: {}",
                installed.names().join(", ")
            ),
            vec!["ff tower procedures".to_string()],
        )
    })?;

    let body = message.unwrap_or_default();
    let ids = store.append_with(|mint| plan(definition, subject, &body, mint))?;
    let (parent, rest) = ids.split_first().expect("the parent is the first event");
    let parts = if definition.parts.len() == 1 {
        0
    } else {
        definition.parts.len()
    };
    let (filed, linked) = rest.split_at(parts);

    if json {
        println!(
            "{}",
            machine::emit(
                "file",
                &serde_json::json!({
                    "filed": super::appended(&store, parent)?,
                    "parts": super::appended_all(&store, filed)?,
                    "linked": super::appended_all(&store, linked)?,
                })
            )
        );
    } else {
        // Re-fold after the append: the echo's numbers live on the fold's
        // flights, so the flights must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        println!(
            "filed {} under {name}: {subject}",
            render::paint_id(&super::display(&fold, parent), colored)
        );
        let refs: Vec<String> = filed.iter().map(|id| super::display(&fold, id)).collect();
        let subjects: Vec<String> = definition
            .parts
            .iter()
            .map(|part| format!("{subject} · {}", part.id))
            .collect();
        let id_width = width(&refs);
        let subject_width = width(&subjects);
        for ((reference, text), part) in refs.iter().zip(&subjects).zip(&definition.parts) {
            println!(
                "· {}  {text:<subject_width$}  {}",
                render::paint_id(&format!("{reference:<id_width$}"), colored),
                render::paint_dim(part.crew.name(), colored),
            );
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

/// The batch, in the order the ids are read back out of: the parent, then
/// one filing per part, then the parent's edge to each part, then the
/// `after` DAG between parts.
///
/// A single-part procedure returns one event and nothing else — the
/// collapse rule, and the reason this returns a plan rather than a fixed
/// shape.
fn plan(
    definition: &Definition,
    subject: &str,
    body: &str,
    mint: &dyn Fn(usize) -> ff_tower_core::log::EventId,
) -> Vec<Kind> {
    let parts = &definition.parts;
    if let [only] = parts.as_slice() {
        return vec![Kind::Filed {
            procedure: definition.name.clone(),
            subject: subject.to_string(),
            body: body.to_string(),
            part: Some(stamp(only)),
        }];
    }

    let mut kinds = vec![Kind::Filed {
        procedure: definition.name.clone(),
        subject: subject.to_string(),
        body: body.to_string(),
        part: None,
    }];
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached, so the part's subject carries the parent's.
    kinds.extend(parts.iter().map(|part| Kind::Filed {
        procedure: definition.name.clone(),
        subject: format!("{subject} · {}", part.id),
        body: String::new(),
        part: Some(stamp(part)),
    }));
    kinds.extend((0..parts.len()).map(|offset| Kind::Linked {
        from: mint(0),
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
fn stamp(part: &Part) -> PartStamp {
    PartStamp {
        id: part.id.clone(),
        crew: part.crew.name().to_string(),
        skill: part.skill.clone(),
        done: part.done.name().to_string(),
        bay: part.bay.map(|bay| bay.name().to_string()),
    }
}

fn width(items: &[String]) -> usize {
    items
        .iter()
        .map(|text| text.chars().count())
        .max()
        .unwrap_or(0)
}

//! `ff tower decompose <flight> [<procedure> | <part>…]` — make a flight
//! a parent.
//!
//! Two forms, told apart by the arguments: exactly one argument that
//! names an installed procedure mints the definition's flights beneath
//! the parent — statuses falling out of the edges, Ready with no `after`
//! and Waiting with any; anything else is the by-hand form, one subject
//! per argument, each filed bare — Triage, like any bare filing, cleared
//! by a person's own gesture. Either way the children ride ordinary
//! `linked` edges, so a sub-flight is indistinguishable from a
//! hand-declared dependency, and the filings and the edges land in one
//! `append_with` — two appends would leave a window where the parent is
//! live, unlinked, and pullable.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;
use ff_tower_core::procedure;
use ff_tower_core::verb::{Fields, Parent, classify};

pub fn run(json: bool, flight: &str, parts: &[String]) -> Result<(), CliError> {
    super::parse_ref(flight)?;
    if parts.is_empty() {
        return Err(CliError::coded(
            "usage/no-parts",
            "there is nothing to split into",
            Vec::new(),
        ));
    }
    let subjects: Vec<String> = parts.iter().map(|part| part.trim().to_string()).collect();
    if subjects.iter().any(String::is_empty) {
        return Err(CliError::coded(
            "usage/empty-subject",
            "a sub-flight's subject is empty",
            Vec::new(),
        ));
    }

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let parent = super::resolve(&fold, flight)?;
    let parent_row = super::ensure_active(&fold, &parent)?;
    let procedure = parent_row.procedure.clone();
    let subject = parent_row.subject.clone();

    // The procedure form: one argument, and it names an installed
    // definition. A subject that happens to collide with a procedure
    // name is spelled around by decomposing with two arguments or a
    // different name — documented in the help.
    let installed = procedure::registry(store.main_worktree().as_deref())?;
    if let [name] = subjects.as_slice()
        && let Some(definition) = installed.get(name)
    {
        let ids = store.append_with(|mint| {
            classify(
                definition,
                &subject,
                &Fields::default(),
                Parent::Existing(parent.clone()),
                mint,
            )
        })?;
        let (filed, linked) = ids.split_at(definition.flights.len());
        return echo(json, &store, &fold, &parent, filed, linked);
    }

    // The by-hand form: the filings first, then one edge per sub-flight
    // naming the id its filing is about to take — which is why this is
    // `append_with` and not a batch built ahead of it.
    let ids = store.append_with(|mint| {
        let mut kinds: Vec<Kind> = subjects
            .iter()
            .map(|subject| Kind::Filed {
                // Provenance follows the parent; the fields are a bare
                // filing's — Triage, no lane, defaults.
                procedure: procedure.clone(),
                subject: subject.clone(),
                body: String::new(),
                status: "triage".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            })
            .collect();
        kinds.extend((0..subjects.len()).map(|offset| Kind::Linked {
            from: parent.clone(),
            to: mint(offset),
        }));
        kinds
    })?;
    let (filed, linked) = ids.split_at(subjects.len());
    echo(json, &store, &fold, &parent, filed, linked)
}

/// The shared tail: the machine envelope, or the re-folded human echo.
fn echo(
    json: bool,
    store: &ff_tower_core::log::Store,
    fold: &board::Fold,
    parent: &ff_tower_core::log::EventId,
    filed: &[ff_tower_core::log::EventId],
    linked: &[ff_tower_core::log::EventId],
) -> Result<(), CliError> {
    if json {
        println!(
            "{}",
            machine::emit(
                "decompose",
                &serde_json::json!({
                    "parent": parent.to_string(),
                    "filed": super::appended_all(store, filed)?,
                    "linked": super::appended_all(store, linked)?,
                })
            )
        );
        return Ok(());
    }
    // Re-fold after the append: the echo's numbers live on the fold's
    // flights, so the flights must be in it.
    let after = board::fold(&store.read_all()?);
    let colored = render::colored();
    let refs: Vec<String> = filed.iter().map(|id| super::display(&after, id)).collect();
    let width = refs
        .iter()
        .map(|reference| reference.chars().count())
        .max()
        .unwrap_or(0);
    let rows: Vec<(&str, &str)> = filed
        .iter()
        .map(|id| {
            let flight = super::flight(&after, id);
            (flight.subject.as_str(), flight.status.as_str())
        })
        .collect();
    let noun = if filed.len() == 1 {
        "sub-flight"
    } else {
        "sub-flights"
    };
    println!(
        "decomposed {} into {} {noun}",
        render::paint_id(&super::display(fold, parent), colored),
        super::count(filed.len())
    );
    for (reference, (subject, status)) in refs.iter().zip(&rows) {
        println!(
            "· {}  {subject}  {}",
            render::paint_id(&format!("{reference:<width$}"), colored),
            render::paint_dim(&status.replace('_', " "), colored),
        );
    }
    println!("{}", super::tail(colored));
    Ok(())
}

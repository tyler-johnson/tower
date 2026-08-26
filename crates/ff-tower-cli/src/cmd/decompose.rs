//! `ff tower decompose <flight> <part>…` — file the parts, and declare the
//! parent's dependency on each of them.
//!
//! Parts ride the existing `linked` edge, so a part is indistinguishable
//! from a hand-declared dependency: `pick` skips the parent until every
//! part is done, and nothing derives the parent's own done. The filings
//! and the edges go down in one `append_with` — two appends would leave a
//! window where the parent is live, unlinked, and claimable.
//!
//! Parts arrive as arguments, one subject each, inheriting the parent's
//! procedure stamp. That is the by-hand half of the verb; when procedures
//! land, a definition's parts replace the arguments, not the edges.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: &str, parts: &[String]) -> Result<(), CliError> {
    super::parse_ref(flight)?;
    if parts.is_empty() {
        return Err(CliError::coded(
            "usage/no-parts",
            "there are no parts to split into",
            Vec::new(),
        ));
    }
    let subjects: Vec<String> = parts.iter().map(|part| part.trim().to_string()).collect();
    if subjects.iter().any(String::is_empty) {
        return Err(CliError::coded(
            "usage/empty-subject",
            "a part's subject is empty",
            Vec::new(),
        ));
    }

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let parent = super::resolve(&fold, flight)?;
    let procedure = super::ensure_active(&fold, &parent)?.procedure.clone();

    // The filings first, then one edge per part naming the id its filing
    // is about to take — which is why this is `append_with` and not a
    // batch built ahead of it.
    let ids = store.append_with(|mint| {
        let mut kinds: Vec<Kind> = subjects
            .iter()
            .map(|subject| Kind::Filed {
                procedure: procedure.clone(),
                subject: subject.clone(),
                body: String::new(),
            })
            .collect();
        kinds.extend((0..subjects.len()).map(|offset| Kind::Linked {
            from: parent.clone(),
            to: mint(offset),
        }));
        kinds
    })?;
    let (filed, linked) = ids.split_at(subjects.len());

    if json {
        println!(
            "{}",
            machine::emit(
                "decompose",
                &serde_json::json!({
                    "parent": parent.to_string(),
                    "filed": super::appended_all(&store, filed)?,
                    "linked": super::appended_all(&store, linked)?,
                })
            )
        );
    } else {
        // Re-fold after the append: the echo's numbers live on the fold's
        // flights, so the flights must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        let refs: Vec<String> = filed.iter().map(|id| super::display(&fold, id)).collect();
        let width = refs
            .iter()
            .map(|reference| reference.chars().count())
            .max()
            .unwrap_or(0);
        let noun = if subjects.len() == 1 { "part" } else { "parts" };
        println!(
            "decomposed {} into {} {noun}",
            render::paint_id(&super::display(&fold, &parent), colored),
            super::count(subjects.len())
        );
        for (reference, subject) in refs.iter().zip(&subjects) {
            println!(
                "· {}  {subject}",
                render::paint_id(&format!("{reference:<width$}"), colored)
            );
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

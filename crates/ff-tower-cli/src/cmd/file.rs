//! `ff tower file [<procedure>] <subject> [flags]` — put work on the
//! board.
//!
//! Two positionals, resolved here: both given name a procedure and a
//! subject; one alone is a bare filing — never guessed as a procedure
//! name — and none is a coded refusal. The verb's body lives in core's
//! `verb::file`, where the server mounts it too; this file is the human
//! echo — the filing line, and one row per minted flight when the
//! definition had two or more. The rows read off the payload's own
//! events, so the echo shows what the log holds, and the re-fold here is
//! for their display numbers alone — the machine path never folds.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;
use ff_tower_core::verb::{self, Fields};

pub fn run(
    json: bool,
    first: Option<&str>,
    second: Option<&str>,
    fields: Fields,
) -> Result<(), CliError> {
    let (procedure, subject) = match (first, second) {
        (Some(procedure), Some(subject)) => (Some(procedure), subject),
        (Some(subject), None) => (None, subject),
        (None, _) => {
            return Err(CliError::coded(
                "usage/empty-subject",
                "the subject is empty",
                Vec::new(),
            ));
        }
    };

    let store = super::store()?;
    let outcome = verb::file(&store, subject, fields, procedure)?;

    if json {
        println!("{}", machine::emit("file", &outcome.payload));
    } else {
        // Re-fold after the append: the echo's numbers live on the fold's
        // flights, so the flights must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        let Kind::Filed {
            procedure,
            subject,
            status,
            ..
        } = &outcome.payload.filed.kind
        else {
            unreachable!("`file` files");
        };
        let landed = match procedure.as_deref() {
            Some(name) => format!("under {name}"),
            None => format!("in {}", status.replace('_', " ")),
        };
        println!(
            "filed {} {landed}: {subject}",
            render::paint_id(&super::display(&fold, &outcome.parent), colored)
        );
        let refs: Vec<String> = outcome
            .part_ids
            .iter()
            .map(|id| super::display(&fold, id))
            .collect();
        // The status is the fold's, not the filing's word: every part
        // is filed cleared, and the edges are what make one Waiting.
        let rows: Vec<(&str, String)> = outcome
            .payload
            .parts
            .iter()
            .zip(&outcome.part_ids)
            .map(|(event, id)| {
                let Kind::Filed {
                    subject, assignee, ..
                } = &event.kind
                else {
                    unreachable!("a minted row is a filing")
                };
                let mut note = board::flight(&fold, id).status.replace('_', " ");
                if let Some(lane) = assignee.as_deref() {
                    note.push_str(&format!(" · {lane}"));
                }
                (subject.as_str(), note)
            })
            .collect();
        let id_width = width(refs.iter().map(String::as_str));
        let subject_width = width(rows.iter().map(|(subject, _)| *subject));
        for (reference, (text, note)) in refs.iter().zip(&rows) {
            println!(
                "· {}  {text:<subject_width$}  {}",
                render::paint_id(&format!("{reference:<id_width$}"), colored),
                render::paint_dim(note, colored),
            );
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

fn width<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    items.map(|text| text.chars().count()).max().unwrap_or(0)
}

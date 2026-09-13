//! `atc file [<procedure>] <subject> [flags]` — put work on the
//! board.
//!
//! Two positionals, resolved here: both given name a procedure and a
//! subject; one alone is a bare filing — never guessed as a procedure
//! name — and none is a coded refusal. The verb's body lives in core's
//! `verb::file`, where the server mounts it too; this file is the human
//! echo — the filing line, and one row per minted flight when the
//! definition had two or more. Both render paths use the payload's
//! post-append board rows.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::log::Kind;
use atc_core::verb::{self, Fields};

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
        let colored = render::colored();
        let (parent, parts) = outcome
            .payload
            .flights
            .split_first()
            .expect("file returns its filing");
        let procedure = &parent.procedure;
        let subject = &parent.subject;
        let status = &parent.status;
        // A routed filing says which rule chose the procedure, so the
        // line reads `filed #3 under chores · matched label chore: …`.
        let landed = match procedure.as_deref() {
            Some(name) => match outcome.payload.routed.as_ref().map(|event| &event.kind) {
                Some(Kind::Routed { because, .. }) => format!("under {name} · {because}"),
                _ => format!("under {name}"),
            },
            None => format!("in {}", status.replace('_', " ")),
        };
        println!(
            "filed {} {landed}: {subject}",
            render::paint_id(&parent.display, colored)
        );
        let refs: Vec<String> = parts.iter().map(|row| row.display.clone()).collect();
        // The status is the fold's, not the filing's word: every part
        // is filed cleared, and the edges are what make one Waiting.
        let rows: Vec<(&str, String)> = parts
            .iter()
            .map(|row| {
                let mut note = row.status.replace('_', " ");
                if let Some(lane) = row.assignee.as_deref() {
                    note.push_str(&format!(" · {lane}"));
                }
                (row.subject.as_str(), note)
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

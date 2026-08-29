//! `ff tower file <subject> [-m <body>] [-p <procedure>]` — mint a flight
//! under a procedure.
//!
//! The verb's body lives in core's `verb::file`, where the server mounts
//! it too; this file is the human echo — the filing line, and one row
//! per part when the definition had two or more. The part rows read off
//! the payload's own events, so the echo shows what the log holds, and
//! the re-fold here is for their display numbers alone — the machine
//! path never folds.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;
use ff_tower_core::verb;

pub fn run(
    json: bool,
    subject: &str,
    message: Option<String>,
    procedure: Option<String>,
) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::file(&store, subject, message, procedure)?;

    if json {
        println!("{}", machine::emit("file", &outcome.payload));
    } else {
        // Re-fold after the append: the echo's numbers live on the fold's
        // flights, so the flights must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        let Kind::Filed {
            procedure: name,
            subject,
            ..
        } = &outcome.payload.filed.kind
        else {
            unreachable!("`file` files");
        };
        println!(
            "filed {} under {name}: {subject}",
            render::paint_id(&super::display(&fold, &outcome.parent), colored)
        );
        let refs: Vec<String> = outcome
            .part_ids
            .iter()
            .map(|id| super::display(&fold, id))
            .collect();
        let rows: Vec<(&str, &str)> = outcome
            .payload
            .parts
            .iter()
            .map(|event| {
                let Kind::Filed {
                    subject,
                    part: Some(stamp),
                    ..
                } = &event.kind
                else {
                    unreachable!("a part row is a stamped filing")
                };
                (subject.as_str(), stamp.crew.as_str())
            })
            .collect();
        let id_width = width(refs.iter().map(String::as_str));
        let subject_width = width(rows.iter().map(|(subject, _)| *subject));
        for (reference, (text, crew)) in refs.iter().zip(&rows) {
            println!(
                "· {}  {text:<subject_width$}  {}",
                render::paint_id(&format!("{reference:<id_width$}"), colored),
                render::paint_dim(crew, colored),
            );
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

fn width<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    items.map(|text| text.chars().count()).max().unwrap_or(0)
}

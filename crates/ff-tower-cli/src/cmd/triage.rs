//! `ff tower triage [<flight> -p <procedure> [-m <why>]]` — the walk
//! through the unclassified pile.
//!
//! Bare is the pile: every live flight still filed under `open`,
//! regardless of part — plain filings, pre-procedure filings, and
//! hand-decomposed parts and their parents alike. Claimed and held
//! flights stay listed, because a claim does not classify. The pile is a
//! pure log read — no gather, no probe — and rendering it always exits 0.
//!
//! Named is the route, whose body lives in core's `verb::route` where
//! the server mounts it too; what stays here is the human echo — the
//! routed line, and one row per part when the definition had two or
//! more. The part rows read off the payload's own events, so the echo
//! shows what the log holds, and the re-fold here is for their display
//! numbers alone.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Flight};
use ff_tower_core::log::Kind;
use ff_tower_core::verb;

pub fn run(
    json: bool,
    flight: Option<&str>,
    procedure: Option<String>,
    message: Option<String>,
) -> Result<(), CliError> {
    match (flight, procedure) {
        (None, None) => pile(json),
        (Some(flight), Some(name)) => route(json, flight, &name, message),
        (Some(_), None) => Err(CliError::coded(
            "usage/no-procedure",
            "a flight but no procedure — `-p <name>` says where it routes",
            vec!["ff tower procedures".to_string()],
        )),
        (None, Some(_)) => Err(CliError::coded(
            "usage/no-flight",
            "a procedure but no flight to route to it",
            vec!["ff tower triage".to_string()],
        )),
    }
}

/// The pile: live flights whose stored stamp still says `open`.
fn pile(json: bool) -> Result<(), CliError> {
    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let pile: Vec<&Flight> = fold
        .flights
        .iter()
        .filter(|flight| flight.done.is_none() && flight.procedure == "open")
        .collect();

    if json {
        let rows: Vec<serde_json::Value> = pile
            .iter()
            .map(|flight| {
                serde_json::json!({
                    "flight": flight.id.to_string(),
                    "number": flight.number,
                    "subject": flight.subject,
                    "filed_by": flight.filed_by,
                    "filed_at": flight.filed_at,
                    "claimed_by": flight.claim.as_ref().map(|claim| claim.by.clone()),
                    "question": flight.question.as_ref().map(|q| q.text.clone()),
                })
            })
            .collect();
        println!(
            "{}",
            machine::emit("triage", &serde_json::json!({ "pile": rows }))
        );
        return Ok(());
    }

    let colored = render::colored();
    if pile.is_empty() {
        println!("nothing unclassified");
        return Ok(());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let refs: Vec<String> = pile
        .iter()
        .map(|flight| super::display(&fold, &flight.id))
        .collect();
    let id_width = width(refs.iter().map(String::as_str));
    for (reference, flight) in refs.iter().zip(&pile) {
        println!(
            "· {}  {}",
            render::paint_id(&format!("{reference:<id_width$}"), colored),
            flight.subject
        );
        let mut phrases = Vec::new();
        if let Some(question) = &flight.question {
            phrases.push(render::paint_warn(&question.text, colored));
        }
        if let Some(claim) = &flight.claim {
            phrases.push(render::paint_dim(
                &format!("claimed by {}", claim.by),
                colored,
            ));
        }
        phrases.push(render::paint_dim(
            &format!(
                "filed by {} {}",
                flight.filed_by,
                render::age(now, flight.filed_at)
            ),
            colored,
        ));
        println!("    {}", phrases.join(&render::paint_dim(" · ", colored)));
    }
    let noun = if pile.len() == 1 { "flight" } else { "flights" };
    println!(
        "{}",
        render::paint_dim(
            &format!(
                "{} {noun} unclassified · ff tower triage <flight> -p <procedure> to route one",
                pile.len()
            ),
            colored
        )
    );
    Ok(())
}

/// The route: one `routed` event, and — for a multi-part procedure — the
/// part filings and edges in the same atomic batch.
fn route(json: bool, flight: &str, name: &str, message: Option<String>) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::route(&store, flight, name, message)?;

    if json {
        println!("{}", machine::emit("triage", &outcome.payload));
    } else {
        // Re-fold after the append: the echo's numbers live on the fold's
        // flights, so the minted parts must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        let Kind::Routed {
            procedure: name,
            because,
            ..
        } = &outcome.payload.routed.kind
        else {
            unreachable!("`triage` routes");
        };
        println!(
            "routed {} to {name}: {}",
            render::paint_id(&super::display(&fold, &outcome.flight), colored),
            super::flight(&fold, &outcome.flight).subject
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
        if !because.is_empty() {
            println!("{}", render::paint_dim(because, colored));
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

fn width<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    items.map(|text| text.chars().count()).max().unwrap_or(0)
}

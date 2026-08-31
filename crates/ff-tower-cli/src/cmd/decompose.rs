//! `ff tower decompose <flight> [<procedure> | <part>…]` — make a flight
//! a parent.
//!
//! Two forms, told apart by the arguments: exactly one argument that
//! names an installed procedure mints the definition's flights beneath
//! the parent; anything else is the by-hand form, one subject per
//! argument. The verb's body lives in core's `verb::decompose`, where
//! the server mounts it too; this file is the human echo — the parent's
//! line, then one row per minted sub-flight. The re-fold here is for
//! their display numbers and their folded statuses alone; the machine
//! path never folds.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str, parts: &[String]) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::decompose(&store, flight, parts)?;

    if json {
        println!("{}", machine::emit("decompose", &outcome.payload));
        return Ok(());
    }
    // Re-fold after the append: the echo's numbers live on the fold's
    // flights, so the flights must be in it.
    let after = board::fold(&store.read_all()?);
    let colored = render::colored();
    let filed = &outcome.filed_ids;
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
        render::paint_id(&super::display(&after, &outcome.parent), colored),
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

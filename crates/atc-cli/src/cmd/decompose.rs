//! `atc decompose <flight> [<procedure> | <part>…]` — make a flight
//! a parent.
//!
//! Two forms, told apart by the arguments: exactly one argument that
//! names an installed procedure mints the definition's flights beneath
//! the parent; anything else is the by-hand form, one subject per
//! argument. The verb's body lives in core's `verb::decompose`, where
//! the server mounts it too; this file is the human echo — the parent's
//! line, then one row per minted sub-flight, using the same post-append
//! board rows as the machine response.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::verb;

pub fn run(json: bool, flight: &str, parts: &[String]) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::decompose(&store, flight, parts)?;

    if json {
        println!("{}", machine::emit("decompose", &outcome.payload));
        return Ok(());
    }
    let colored = render::colored();
    let (parent, filed) = outcome
        .payload
        .flights
        .split_first()
        .expect("decompose returns its parent");
    let refs: Vec<String> = filed.iter().map(|row| row.display.clone()).collect();
    let width = refs
        .iter()
        .map(|reference| reference.chars().count())
        .max()
        .unwrap_or(0);
    let rows: Vec<(&str, &str)> = filed
        .iter()
        .map(|flight| (flight.subject.as_str(), flight.status.as_str()))
        .collect();
    let noun = if filed.len() == 1 {
        "sub-flight"
    } else {
        "sub-flights"
    };
    println!(
        "decomposed {} into {} {noun}",
        render::paint_id(&parent.display, colored),
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

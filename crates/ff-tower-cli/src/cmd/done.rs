//! `ff tower done [<flight>]` — finish a flight: off the board, on the
//! record.
//!
//! The verb's body lives in core's `verb::done`, flight required, where
//! the server mounts it too. What stays here is the bare form: `done`
//! with no argument derives the current flight from the invoking
//! worktree's chain — the newest session-tagged operation names it — so
//! the bare path is the one place this verb spawns fufu, once, and it
//! passes the derived id down like a typed one.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Fold};
use ff_tower_core::log::EventId;
use ff_tower_core::verb;

pub fn run(json: bool, flight: Option<&str>) -> Result<(), CliError> {
    if let Some(text) = flight {
        super::parse_ref(text)?;
    }

    let store = super::store()?;
    let flight = match flight {
        Some(text) => text.to_string(),
        None => {
            let fold = board::fold(&store.read_all()?);
            derived(&fold)?.to_string()
        }
    };
    let outcome = verb::done(&store, &flight)?;

    if json {
        println!("{}", machine::emit("done", &outcome.payload));
    } else {
        let colored = render::colored();
        println!(
            "done {}: {}",
            render::paint_id(&outcome.display, colored),
            outcome.subject
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

/// The invoking worktree's flight: the newest session-tagged operation
/// on its own chain — one `op log` spawn, and the bare path's only one.
/// The refusals share `usage/needs-flight` with the old bare refusal:
/// same gap, same remedy, and exit 2 stays stable for machine callers.
fn derived(fold: &Fold) -> Result<EventId, CliError> {
    let ops = super::ff()?.op_log("session(glob:*)")?;
    let mut newest = None;
    for op in &ops {
        let Some(tag) = op.session.as_deref() else {
            continue;
        };
        match newest {
            Some((_, time)) if time >= op.time => {}
            _ => newest = Some((tag, op.time)),
        }
    }
    let Some((tag, _)) = newest else {
        return Err(CliError::coded(
            "usage/needs-flight",
            "no session-tagged work on this worktree — name the flight",
            vec!["ff tower done <flight>".to_string()],
        ));
    };
    let unfiled = || {
        CliError::coded(
            "usage/needs-flight",
            format!(
                "`{tag}` tags this worktree's newest work, and no such flight is filed — name the flight"
            ),
            vec!["ff tower done <flight>".to_string()],
        )
    };
    let id: EventId = tag.parse().map_err(|_| unfiled())?;
    if !fold.flights.iter().any(|flight| flight.id == id) {
        return Err(unfiled());
    }
    Ok(id)
}

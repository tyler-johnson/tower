//! `ff tower done [<flight>]` — finish a flight: off the board, on the
//! record.
//!
//! Always the asserted done this slice; DESIGN's done enum arrives with
//! procedures. Bare `done` derives the current flight from the invoking
//! worktree's chain — the newest session-tagged operation names it — so
//! the bare path is the one place this verb spawns fufu, once. Finishing
//! a waiting flight is allowed: abandoning the question is deliberate
//! when the flight itself is over.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Fold};
use ff_tower_core::log::{EventId, Kind};

pub fn run(json: bool, flight: Option<&str>) -> Result<(), CliError> {
    if let Some(text) = flight {
        super::parse_ref(text)?;
    }

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = match flight {
        Some(text) => super::resolve(&fold, text)?,
        None => derived(&fold)?,
    };
    // Not `ensure_active` — the duplicate case earns "already" wording,
    // and a derived flight already done lands here too.
    let filed = super::flight(&fold, &flight);
    if filed.done.is_some() {
        return Err(CliError::coded(
            "flight/done",
            format!("`{}` is already done", super::display(&fold, &flight)),
            vec!["ff tower".to_string()],
        ));
    }
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Done {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one done event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("done", &serde_json::json!({ "done": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "done {}: {subject}",
            render::paint_id(&super::display(&fold, &flight), colored)
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

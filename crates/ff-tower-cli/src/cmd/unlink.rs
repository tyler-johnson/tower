//! `ff tower unlink <a> <b>` — take back a declared dependency. With
//! Waiting derived from the edges, this is the only way to disagree
//! with one.
//!
//! One `unlinked` event naming the edge; the fold drops it, and the
//! dependent derives Ready at the next render with no further event.
//! An edge that is not on the record is refused: there is nothing to
//! take back.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, a: &str, b: &str) -> Result<(), CliError> {
    super::parse_ref(a)?;
    super::parse_ref(b)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let from = super::resolve(&fold, a)?;
    let to = super::resolve(&fold, b)?;
    // No self-link check: a self-edge can never be on the record, so
    // it falls to the one refusal.
    let declared = super::flight(&fold, &from).depends_on.contains(&to);
    if !declared {
        return Err(CliError::coded(
            "link/missing",
            format!(
                "`{}` does not depend on `{}`",
                super::display(&fold, &from),
                super::display(&fold, &to)
            ),
            Vec::new(),
        ));
    }

    let ids = store.append(vec![Kind::Unlinked {
        from: from.clone(),
        to: to.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one unlinked event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("unlink", &serde_json::json!({ "unlinked": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "unlinked {}: no longer depends on {}",
            render::paint_id(&super::display(&fold, &from), colored),
            render::paint_id(&super::display(&fold, &to), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

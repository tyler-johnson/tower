//! `ff tower link <a> <b>` — declare that `a` depends on `b`.
//!
//! Stored intent, one edge per event. The identical edge declared twice
//! is refused; the fold would render it twice, and nothing in the log
//! means it twice.

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
    // Self-link fires on the resolved ids — `link 3 pi.3` naming one
    // flight twice is a self-link only resolution can see.
    if from == to {
        return Err(CliError::coded(
            "usage/self-link",
            format!("`{}` cannot depend on itself", super::display(&fold, &from)),
            Vec::new(),
        ));
    }
    let declared = fold
        .flights
        .iter()
        .find(|flight| flight.id == from)
        .expect("resolved to a filed flight")
        .depends_on
        .contains(&to);
    if declared {
        return Err(CliError::coded(
            "link/exists",
            format!(
                "`{}` already depends on `{}`",
                super::display(&fold, &from),
                super::display(&fold, &to)
            ),
            Vec::new(),
        ));
    }

    let ids = store.append(vec![Kind::Linked {
        from: from.clone(),
        to: to.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one linked event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("link", &serde_json::json!({ "linked": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "linked {}: depends on {}",
            render::paint_id(&super::display(&fold, &from), colored),
            render::paint_id(&super::display(&fold, &to), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

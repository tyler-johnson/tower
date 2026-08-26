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
    let from = super::parse_flight(a)?;
    let to = super::parse_flight(b)?;
    if from == to {
        return Err(CliError::coded(
            "usage/self-link",
            format!("`{from}` cannot depend on itself"),
            Vec::new(),
        ));
    }

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    super::ensure_filed(&fold, &from)?;
    super::ensure_filed(&fold, &to)?;
    let declared = fold
        .flights
        .iter()
        .find(|flight| flight.id == from)
        .expect("ensured filed")
        .depends_on
        .contains(&to);
    if declared {
        return Err(CliError::coded(
            "link/exists",
            format!("`{from}` already depends on `{to}`"),
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
            render::paint_id(&from.to_string(), colored),
            render::paint_id(&to.to_string(), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

//! `ff tower claim <flight>` — claim one specific flight, out of order.
//!
//! The claim is the motion: the flight moves into the air at assignment,
//! before any capture exists in a bay. Re-claiming is refused even for the
//! same author — a silent success that appended nothing would lie.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = super::resolve(&fold, flight)?;
    let filed = super::ensure_active(&fold, &flight)?;
    if let Some(claim) = &filed.claim {
        return Err(CliError::coded(
            "claim/taken",
            format!(
                "`{}` is already claimed by {}",
                super::display(&fold, &flight),
                claim.by
            ),
            Vec::new(),
        ));
    }
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Claimed {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one claimed event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("claim", &serde_json::json!({ "claimed": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "claimed {}: {subject}",
            render::paint_id(&super::display(&fold, &flight), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

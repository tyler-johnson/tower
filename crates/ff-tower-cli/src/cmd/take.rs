//! `ff tower take <flight>` — take the controls: crew this to you, agent
//! off.
//!
//! The human override for `claim`'s refusal. `claim` will not reassign a
//! standing claim, because an agent silently stealing another's flight
//! would be a handoff nobody agreed to; a person authoring this event is
//! the consent that refusal declines to assume, so a take over someone
//! else's claim is allowed and names where the flight came from.
//!
//! The filed part stamp is never mutated — the overlay lives in the
//! flight's `taken` mark, so `requeue` can hand the flight back exactly
//! as it was filed and `--json` keeps showing the stamp the log holds.
//! Taking twice is refused: a silent success that appended nothing would
//! lie.

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
    if filed.taken.is_some() {
        return Err(CliError::coded(
            "take/taken",
            format!("`{}` is already yours", super::display(&fold, &flight)),
            Vec::new(),
        ));
    }
    let subject = filed.subject.clone();
    let from = filed.claim.as_ref().map(|claim| claim.by.clone());

    let ids = store.append(vec![Kind::Taken {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one taken event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("take", &serde_json::json!({ "taken": event }))
        );
    } else {
        let colored = render::colored();
        let name = render::paint_id(&super::display(&fold, &flight), colored);
        match from {
            Some(who) => println!("took {name} from {who}: {subject}"),
            None => println!("took {name}: {subject}"),
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

//! `ff tower take <flight>` — take the controls: crew this to you, agent
//! off.
//!
//! The verb's body lives in core's `verb::take`, where the server mounts
//! it too; this file is the argument order and the echo, which names
//! whose claim the take displaced.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let outcome = verb::take(&store, flight)?;

    if json {
        println!("{}", machine::emit("take", &outcome.payload));
    } else {
        let colored = render::colored();
        let name = render::paint_id(&outcome.display, colored);
        match &outcome.from {
            Some(who) => println!("took {name} from {who}: {}", outcome.subject),
            None => println!("took {name}: {}", outcome.subject),
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

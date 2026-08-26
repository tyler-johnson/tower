//! `ff tower comment <flight> -m <note>` — a note on a flight's record.
//!
//! fufu's `describe` gate minus the editor: tower opens no editor this
//! slice, so a missing `-m` refuses unconditionally — a coded refusal,
//! never a clap `required = true`, so a `--json` caller gets an envelope.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let Some(text) = message else {
        return Err(CliError::coded(
            "usage/needs-message",
            "no note given",
            vec!["ff tower comment <flight> -m <note>".to_string()],
        ));
    };
    super::parse_ref(flight)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = super::resolve(&fold, flight)?;

    let ids = store.append(vec![Kind::Commented {
        flight: flight.clone(),
        text,
    }])?;
    let id = ids.into_iter().next().expect("one commented event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("comment", &serde_json::json!({ "commented": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "commented on {}",
            render::paint_id(&super::display(&fold, &flight), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

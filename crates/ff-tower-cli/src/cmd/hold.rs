//! `ff tower hold <flight> -m <question>` — stop with a question attached.
//!
//! The one verb whose success is exit 3: the envelope is a full data
//! envelope with the held event in it, and only the code says the flight
//! stopped with a question — fufu's held-is-an-outcome precedent. The 3
//! itself lives in `main.rs`; this file returns `Ok(())` like any verb.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let Some(question) = message else {
        return Err(CliError::coded(
            "usage/needs-message",
            "no question given",
            vec!["ff tower hold <flight> -m <question>".to_string()],
        ));
    };
    super::parse_ref(flight)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = super::resolve(&fold, flight)?;
    let filed = super::ensure_active(&fold, &flight)?;
    if let Some(open) = &filed.question {
        return Err(CliError::coded(
            "hold/exists",
            format!(
                "`{}` is already held: {}",
                super::display(&fold, &flight),
                open.text
            ),
            vec!["ff tower answer <flight> -m <answer>".to_string()],
        ));
    }

    let ids = store.append(vec![Kind::Held {
        flight: flight.clone(),
        question: question.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one held event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("hold", &serde_json::json!({ "held": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "held {}: {question}",
            render::paint_id(&super::display(&fold, &flight), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

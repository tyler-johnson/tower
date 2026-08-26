//! `ff tower answer <flight> -m <answer>` — answer the question, release
//! the hold.
//!
//! The answer goes on the log's record and counts as the flight's motion;
//! it does not become a comment. A flight with no open question refuses —
//! an answer to nothing would append a gesture the board cannot show.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: &str, message: Option<String>) -> Result<(), CliError> {
    let Some(answer) = message else {
        return Err(CliError::coded(
            "usage/needs-message",
            "no answer given",
            vec!["ff tower answer <flight> -m <answer>".to_string()],
        ));
    };
    super::parse_ref(flight)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = super::resolve(&fold, flight)?;
    let filed = super::ensure_active(&fold, &flight)?;
    if filed.question.is_none() {
        return Err(CliError::coded(
            "answer/not-held",
            format!("`{}` has no open question", super::display(&fold, &flight)),
            vec!["ff tower".to_string()],
        ));
    }

    let ids = store.append(vec![Kind::Answered {
        flight: flight.clone(),
        answer: answer.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one answered event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("answer", &serde_json::json!({ "answered": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "answered {}: {answer}",
            render::paint_id(&super::display(&fold, &flight), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

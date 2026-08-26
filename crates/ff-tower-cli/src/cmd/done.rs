//! `ff tower done <flight>` — finish a flight: off the board, on the
//! record.
//!
//! Always the asserted done this slice; DESIGN's done enum arrives with
//! procedures. Bare `done` refuses — deriving "the current flight" needs
//! the bay, which arrives with bays. Finishing a waiting flight is allowed:
//! abandoning the question is deliberate when the flight itself is over.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: Option<&str>) -> Result<(), CliError> {
    let Some(text) = flight else {
        return Err(CliError::coded(
            "usage/needs-flight",
            "no flight given — naming the current one arrives with bays",
            vec!["ff tower done <flight>".to_string()],
        ));
    };
    super::parse_ref(text)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = super::resolve(&fold, text)?;
    // Not `ensure_active` — the duplicate case earns "already" wording.
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

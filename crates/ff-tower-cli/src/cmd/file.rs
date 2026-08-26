//! `ff tower file <subject> [-m <body>] [-p <procedure>]` — mint a flight.
//!
//! The procedure is a free string this slice; validation against
//! definitions arrives when procedures do. `-m` is the body and genuinely
//! optional, like `commit -m`.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(
    json: bool,
    subject: &str,
    message: Option<String>,
    procedure: Option<String>,
) -> Result<(), CliError> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(CliError::coded(
            "usage/empty-subject",
            "the subject is empty",
            Vec::new(),
        ));
    }
    let procedure = match procedure {
        Some(name) => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(CliError::coded(
                    "usage/empty-procedure",
                    "`-p` names an empty procedure",
                    Vec::new(),
                ));
            }
            name
        }
        None => "open".to_string(),
    };

    let store = super::store()?;
    let ids = store.append(vec![Kind::Filed {
        procedure: procedure.clone(),
        subject: subject.to_string(),
        body: message.unwrap_or_default(),
    }])?;
    let id = ids.into_iter().next().expect("one filed event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("file", &serde_json::json!({ "filed": event }))
        );
    } else {
        // Re-fold after the append: the echo's number lives on the fold's
        // flight, so the flight must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        println!(
            "filed {} under {procedure}: {subject}",
            render::paint_id(&super::display(&fold, &id), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

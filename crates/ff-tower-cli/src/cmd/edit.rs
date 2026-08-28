//! `ff tower edit <target> [-s <subject>] [-m <msg>]` — reword a flight's
//! subject/body, or a comment's text by its event id.
//!
//! An overlay event, not a rewrite: the fold applies it last-wins per
//! field and the log keeps every prior word. No `ensure_active` —
//! permissive like `comment` and `link`, because a wrong word in a closed
//! record is the motivating case. `-m ""` is accepted for the same reason
//! `comment` checks no emptiness: clearing a body or a comment's text is
//! a legitimate edit.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

use super::EditTarget;

pub fn run(
    json: bool,
    target: &str,
    subject: Option<String>,
    message: Option<String>,
) -> Result<(), CliError> {
    if subject.is_none() && message.is_none() {
        return Err(CliError::coded(
            "usage/needs-edit",
            "nothing to change — `-s` rewords the subject, `-m` the body or comment text",
            vec![
                "ff tower edit <target> -s <subject>".to_string(),
                "ff tower edit <target> -m <msg>".to_string(),
            ],
        ));
    }
    // `file`'s discipline: the subject is trimmed, and trimmed to nothing
    // it would put a blank head on the board.
    let subject = subject.map(|subject| subject.trim().to_string());
    if subject.as_deref() == Some("") {
        return Err(CliError::coded(
            "usage/empty-subject",
            "the new subject is empty",
            Vec::new(),
        ));
    }
    super::parse_ref(target)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let target = super::resolve_edit_target(&fold, target)?;
    if subject.is_some()
        && let EditTarget::Comment { .. } = &target
    {
        return Err(CliError::coded(
            "usage/subject-on-comment",
            "a comment has no subject — `-m` rewords its text",
            vec!["ff tower edit <target> -m <msg>".to_string()],
        ));
    }

    let edited = match &target {
        EditTarget::Flight(flight) => flight.clone(),
        EditTarget::Comment { comment, .. } => comment.clone(),
    };
    let ids = store.append(vec![Kind::Edited {
        target: edited,
        subject,
        body: message,
    }])?;
    let id = ids.into_iter().next().expect("one edited event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("edit", &serde_json::json!({ "edited": event }))
        );
    } else {
        let colored = render::colored();
        match &target {
            EditTarget::Flight(flight) => println!(
                "edited {}",
                render::paint_id(&super::display(&fold, flight), colored)
            ),
            EditTarget::Comment { flight, comment } => println!(
                "edited comment {} on {}",
                render::paint_id(&comment.to_string(), colored),
                render::paint_id(&super::display(&fold, flight), colored)
            ),
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

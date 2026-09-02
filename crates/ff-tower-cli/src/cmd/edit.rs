//! `ff tower edit <target> [-s <subject>] [-m <msg>] [field flags]` —
//! reword a flight's record, or a comment's text by its event id.
//!
//! The verb's body lives in core's `verb::edit`, where the server
//! mounts it too; this file is the argument order and the echo.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::verb::{self, EditTarget};

pub fn run(json: bool, target: &str, overlay: verb::Overlay) -> Result<(), CliError> {
    let store = super::store()?;
    let outcome = verb::edit(&store, target, overlay)?;

    if json {
        println!("{}", machine::emit("edit", &outcome.payload));
    } else {
        let colored = render::colored();
        match &outcome.target {
            EditTarget::Flight(_) => {
                println!("edited {}", render::paint_id(&outcome.display, colored))
            }
            EditTarget::Comment { comment, .. } => println!(
                "edited comment {} on {}",
                render::paint_id(&comment.to_string(), colored),
                render::paint_id(&outcome.display, colored)
            ),
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

//! `ff tower explain [<id>] [--list]` — look up an error id and see what
//! it means. Runs outside any repository: a pure registry lookup, no
//! store, no fufu spawn — fufu's `ff explain`, tower's ids.

use serde::Serialize;

use crate::error::CliError;
use crate::{explain, machine};

/// The list envelope: `{entries: […]}`, fufu's shape.
#[derive(Serialize)]
struct Listing {
    entries: &'static [explain::Entry],
}

pub fn run(json: bool, id: Option<&str>, list: bool) -> Result<(), CliError> {
    if list {
        if json {
            println!(
                "{}",
                machine::emit(
                    "explain",
                    &Listing {
                        entries: explain::ENTRIES,
                    }
                )
            );
        } else {
            print!("{}", explain::render_list());
        }
        return Ok(());
    }

    let Some(id) = id else {
        return Err(CliError::coded(
            "usage/bad-flags",
            "explain requires an id, or --list",
            vec!["ff tower explain --list".into()],
        ));
    };

    let entry = explain::find(id).ok_or_else(|| explain::unknown_id(id))?;
    if json {
        println!("{}", machine::emit("explain", entry));
    } else {
        print!("{}", explain::render(entry));
    }
    Ok(())
}

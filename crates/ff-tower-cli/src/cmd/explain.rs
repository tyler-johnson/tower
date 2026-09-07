//! `ff tower explain [<id>] [--list]` — look up an error id and see what
//! it means. Runs outside any repository: a pure registry lookup, no
//! store, no fufu spawn — fufu's `ff explain`, tower's ids.

use serde::Serialize;

use crate::error::CliError;
use crate::{explain, machine};

/// One entry as the wire spells it: the id namespaced, everything else
/// the entry's own. `Entry` itself stays bare — the registry keys on the
/// bare id — and this is the shape that goes out, so `data.id` here and
/// `error.id` on a refusal are the same string and an agent can join
/// them.
#[derive(Serialize)]
struct Wire {
    id: String,
    summary: &'static str,
    detail: &'static str,
    exits: &'static [&'static str],
}

impl From<&'static explain::Entry> for Wire {
    fn from(entry: &'static explain::Entry) -> Wire {
        Wire {
            id: machine::namespaced(entry.id),
            summary: entry.summary,
            detail: entry.detail,
            exits: entry.exits,
        }
    }
}

/// The list envelope: `{entries: […]}`, fufu's shape.
#[derive(Serialize)]
struct Listing {
    entries: Vec<Wire>,
}

pub fn run(json: bool, id: Option<&str>, list: bool) -> Result<(), CliError> {
    if list {
        if json {
            println!(
                "{}",
                machine::emit(
                    "explain",
                    &Listing {
                        entries: explain::ENTRIES.iter().map(Wire::from).collect(),
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

    // Both spellings land on the same entry: the namespaced one a person
    // pasted out of an envelope, and the bare one fufu hands over after
    // splitting `tower/` off itself.
    let id = explain::strip(id);
    let entry = explain::find(id).ok_or_else(|| explain::unknown_id(id))?;
    if json {
        println!("{}", machine::emit("explain", &Wire::from(entry)));
    } else {
        print!("{}", explain::render(entry));
    }
    Ok(())
}

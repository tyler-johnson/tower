//! `ff tower procedures [<name>]` — what is installed, and where to fork
//! it.
//!
//! Read-only, and beside `file` and the pass one of the few surfaces
//! that reads a definition. Bare is the list: every name, the layer it
//! came from, and the flights it stamps out with their lanes. Named is
//! the detail page: the match rules by name with their predicates —
//! adapter-keyed ones marked inert, because no adapter exists to fire
//! them, while field-keyed ones route Triage on the lazy pass — every
//! flight with assignee, skill, `after`, and `done`, and the path a
//! fork of it belongs at.
//!
//! No fufu spawn, like `file`: the repository layer resolves through
//! `Store::main_worktree`.

use std::path::PathBuf;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::procedure::{self, Definition, Registry};

pub fn run(json: bool, name: Option<&str>) -> Result<(), CliError> {
    let store = super::store()?;
    let root = store.main_worktree();
    let installed = procedure::registry(root.as_deref())?;

    match name {
        None => {
            if json {
                println!(
                    "{}",
                    machine::emit(
                        "procedures",
                        &procedure::Listing {
                            procedures: installed.definitions().collect()
                        }
                    )
                );
            } else {
                print!("{}", list(&installed, render::colored()));
            }
        }
        Some(name) => {
            let definition = installed.require(name)?;
            if json {
                println!(
                    "{}",
                    machine::emit(
                        "procedures",
                        &procedure::One {
                            procedure: definition
                        }
                    )
                );
            } else {
                print!("{}", detail(definition, root.as_deref(), render::colored()));
            }
        }
    }
    Ok(())
}

/// The list: a head line per procedure, its flights under it, and the
/// footer's count in the board's grammar.
fn list(installed: &Registry, colored: bool) -> String {
    let names: Vec<String> = installed
        .definitions()
        .map(|definition| definition.name.clone())
        .collect();
    let name_width = width(&names);

    let mut out = String::new();
    for definition in installed.definitions() {
        out.push_str(&format!(
            "{}  {}\n",
            render::paint_id(&format!("{:<name_width$}", definition.name), colored),
            render::paint_dim(definition.source.layer(), colored),
        ));
        let ids: Vec<String> = definition
            .flights
            .iter()
            .map(|flight| flight.id.clone())
            .collect();
        let id_width = width(&ids);
        for (id, flight) in ids.iter().zip(&definition.flights) {
            out.push_str(&format!(
                "· {id:<id_width$}  {}\n",
                render::paint_dim(flight.assignee.name(), colored)
            ));
        }
        out.push('\n');
    }

    let noun = if installed.len() == 1 {
        "procedure"
    } else {
        "procedures"
    };
    out.push_str(&render::paint_dim(
        &format!(
            "{} {noun} · ff tower procedures <name> for one",
            installed.len()
        ),
        colored,
    ));
    out.push('\n');
    out
}

/// One procedure in full. An adapter-keyed rule says outright that it
/// cannot fire — a rule that looks live and never runs is the kind of
/// thing you debug for an hour — while a field-keyed one is live on the
/// pass and prints its predicates plain.
fn detail(definition: &Definition, repo_root: Option<&std::path::Path>, colored: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}  {}\n",
        render::paint_id(&definition.name, colored),
        render::paint_dim(definition.source.layer(), colored),
    ));
    if let Some(subject) = definition.subject.as_deref() {
        out.push_str(&format!(
            "    {}\n",
            render::paint_dim(&format!("subject {subject}"), colored)
        ));
    }

    if !definition.matches.is_empty() {
        out.push('\n');
        out.push_str("match\n");
        let names: Vec<String> = definition
            .matches
            .iter()
            .map(|rule| rule.name.clone())
            .collect();
        let rule_width = width(&names);
        for (name, rule) in names.iter().zip(&definition.matches) {
            let mut phrases = Vec::new();
            if let Some(source) = rule.source.as_deref() {
                phrases.push(format!("source {source}"));
            }
            if let Some(event) = rule.event.as_deref() {
                phrases.push(format!("event {event}"));
            }
            if let Some(label) = rule.label.as_deref() {
                phrases.push(format!("label {label}"));
            }
            if let Some(priority) = rule.priority.as_deref() {
                phrases.push(format!("priority {priority}"));
            }
            if let Some(skill) = rule.skill.as_deref() {
                phrases.push(format!("skill {skill}"));
            }
            if let Some(assignee) = rule.assignee.as_deref() {
                phrases.push(format!("assignee {assignee}"));
            }
            if rule.source.is_some() || rule.event.is_some() {
                phrases.push("inert until an adapter can fire it".to_string());
            }
            out.push_str(&format!(
                "· {name:<rule_width$}  {}\n",
                render::paint_dim(&phrases.join(" · "), colored)
            ));
        }
    }

    out.push('\n');
    out.push_str("flights\n");
    let ids: Vec<String> = definition
        .flights
        .iter()
        .map(|flight| flight.id.clone())
        .collect();
    let id_width = width(&ids);
    for (id, flight) in ids.iter().zip(&definition.flights) {
        let mut phrases = vec![flight.assignee.name().to_string()];
        if let Some(skill) = flight.skill.as_deref() {
            phrases.push(format!("skill {skill}"));
        }
        if let Some(priority) = flight.priority.as_deref() {
            phrases.push(format!("priority {priority}"));
        }
        if !flight.labels.is_empty() {
            phrases.push(flight.labels.join(", "));
        }
        if let Some(bay) = flight.bay {
            phrases.push(format!("bay {}", bay.name()));
        }
        if !flight.after.is_empty() {
            phrases.push(format!("after {}", flight.after.join(", ")));
        }
        phrases.push(format!("done {}", flight.done.name()));
        out.push_str(&format!(
            "· {id:<id_width$}  {}\n",
            render::paint_dim(&phrases.join(" · "), colored)
        ));
    }

    out.push('\n');
    out.push_str(&render::paint_dim(
        &provenance(definition, repo_root),
        colored,
    ));
    out.push('\n');
    out
}

/// Where it came from, or — for a built-in — where a fork of it belongs.
/// The repository layer is offered first: a forked procedure is normally
/// the team's, and the user layer is the one that roams with a person.
fn provenance(definition: &Definition, repo_root: Option<&std::path::Path>) -> String {
    match definition.source.path() {
        Some(path) => format!("file: {}", path.display()),
        None => match fork_target(&definition.name, repo_root) {
            Some(path) => format!("fork: {}", path.display()),
            None => "built-in · shipped in the binary".to_string(),
        },
    }
}

fn fork_target(name: &str, repo_root: Option<&std::path::Path>) -> Option<PathBuf> {
    let dir = match repo_root {
        Some(root) => procedure::repo_dir(root),
        None => procedure::user_dir()?,
    };
    Some(dir.join(format!("{name}.toml")))
}

fn width(items: &[String]) -> usize {
    items
        .iter()
        .map(|text| text.chars().count())
        .max()
        .unwrap_or(0)
}

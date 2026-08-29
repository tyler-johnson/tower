//! `ff tower procedures [<name>]` — what is installed, and where to fork
//! it.
//!
//! Read-only, and the only surface that reads a definition outside of
//! `file`. Bare is the list: every name, the layer it came from, and its
//! parts with their crews. Named is the detail page: the match rules —
//! marked inert, because they only ever fire on adapter signals and there
//! are no adapters — every part with crew, skill, `after`, and `done`, and
//! the path a fork of it belongs at.
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

/// The list: a head line per procedure, its parts under it, and the
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
            .parts
            .iter()
            .map(|part| part.id.clone())
            .collect();
        let id_width = width(&ids);
        for (id, part) in ids.iter().zip(&definition.parts) {
            out.push_str(&format!(
                "· {id:<id_width$}  {}\n",
                render::paint_dim(part.crew.name(), colored)
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

/// One procedure in full. The match block says outright that it cannot
/// fire — a rule that looks live and never runs is the kind of thing you
/// debug for an hour.
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
        out.push_str("match");
        out.push_str(&format!(
            "  {}\n",
            render::paint_dim("inert until an adapter can fire it", colored)
        ));
        for rule in &definition.matches {
            out.push_str(&format!("· {} · {}\n", rule.source, rule.event));
        }
    }

    out.push('\n');
    out.push_str("parts\n");
    let ids: Vec<String> = definition
        .parts
        .iter()
        .map(|part| part.id.clone())
        .collect();
    let id_width = width(&ids);
    for (id, part) in ids.iter().zip(&definition.parts) {
        let mut phrases = vec![part.crew.name().to_string()];
        if let Some(skill) = part.skill.as_deref() {
            phrases.push(format!("skill {skill}"));
        }
        if let Some(bay) = part.bay {
            phrases.push(format!("bay {}", bay.name()));
        }
        if !part.after.is_empty() {
            phrases.push(format!("after {}", part.after.join(", ")));
        }
        phrases.push(format!("done {}", part.done.name()));
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

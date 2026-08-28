//! `ff tower skills [<name>]` — what is installed, raw enough to fork.
//!
//! Read-only, and it spawns no fufu: the repository layer resolves
//! through `Store::main_worktree`, `procedures`' precedent. Bare is the
//! list — name, layer, and the front matter's one-line description —
//! with the fork paths in the footer. Named is the file itself, byte for
//! byte: human and piped output are the same bytes, so a redirect into a
//! harness's skill path or a fork's starting point never needs a flag.

use std::path::Path;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::skill::{self, Registry};

pub fn run(json: bool, name: Option<&str>) -> Result<(), CliError> {
    let store = super::store()?;
    let root = store.main_worktree();
    let installed = skill::registry(root.as_deref())?;

    match name {
        None => {
            if json {
                let all: Vec<serde_json::Value> = installed
                    .skills()
                    .map(|skill| {
                        serde_json::json!({
                            "name": skill.name,
                            "source": skill.source,
                            "summary": skill.summary(),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    machine::emit("skills", &serde_json::json!({ "skills": all }))
                );
            } else {
                print!("{}", list(&installed, root.as_deref(), render::colored()));
            }
        }
        Some(name) => {
            let skill = installed.get(name).ok_or_else(|| {
                CliError::coded(
                    "skill/not-found",
                    format!(
                        "no skill `{name}` — installed: {}",
                        installed.names().join(", ")
                    ),
                    vec!["ff tower skills".to_string()],
                )
            })?;
            if json {
                println!(
                    "{}",
                    machine::emit(
                        "skills",
                        &serde_json::json!({ "skill": {
                            "name": skill.name,
                            "source": skill.source,
                            "text": skill.text,
                        }})
                    )
                );
            } else {
                // The file, exactly as stored — nothing prepended,
                // nothing appended, so the bytes pipe as they list.
                print!("{}", skill.text);
            }
        }
    }
    Ok(())
}

/// The list: one aligned row per skill, the count line in the board's
/// grammar, and where a fork of one belongs. The repository layer is
/// offered first, `procedures`' reason: a forked skill is normally the
/// team's, and the user layer is the one that roams with a person.
fn list(installed: &Registry, repo_root: Option<&Path>, colored: bool) -> String {
    let names: Vec<String> = installed.skills().map(|skill| skill.name.clone()).collect();
    let name_width = width(&names);
    let layers: Vec<String> = installed
        .skills()
        .map(|skill| skill.source.layer().to_string())
        .collect();
    let layer_width = width(&layers);

    let mut out = String::new();
    for skill in installed.skills() {
        out.push_str(&format!(
            "{}  {}  {}\n",
            render::paint_id(&format!("{:<name_width$}", skill.name), colored),
            render::paint_dim(&format!("{:<layer_width$}", skill.source.layer()), colored),
            skill.summary(),
        ));
    }
    out.push('\n');

    let noun = if installed.len() == 1 {
        "skill"
    } else {
        "skills"
    };
    out.push_str(&render::paint_dim(
        &format!(
            "{} {noun} · ff tower skills <name> for one, raw",
            installed.len()
        ),
        colored,
    ));
    out.push('\n');

    let mut targets = Vec::new();
    if let Some(root) = repo_root {
        targets.push(skill::repo_dir(root).join("<name>.md"));
    }
    if let Some(user) = skill::user_dir() {
        targets.push(user.join("<name>.md"));
    }
    if !targets.is_empty() {
        out.push_str(&render::paint_dim(
            &format!(
                "fork: {}",
                targets
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" · ")
            ),
            colored,
        ));
        out.push('\n');
    }
    out
}

fn width(items: &[String]) -> usize {
    items
        .iter()
        .map(|text| text.chars().count())
        .max()
        .unwrap_or(0)
}

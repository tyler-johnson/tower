//! Produced skills share the shipped skill's install roots. A receipt records the directories tower owns in shared roots.

use crate::{error::CliError, manifest, registry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

const RECEIPT: &str = ".atc-adapter-skills.json";
#[derive(Clone, Serialize, Deserialize)]
struct Installed {
    owner: String,
    version: String,
    files: Vec<manifest::SkillFile>,
}
type Skills = BTreeMap<String, Installed>;
static PREPARED: OnceLock<Skills> = OnceLock::new();

/// Ask once per hook invocation, never from passive status inspection. Failed replies leave earlier installed skills intact.
pub fn prepare() {
    PREPARED.get_or_init(|| {
        let mut skills = Skills::new();
        for entry in registry::read().declared() {
            for name in &entry.manifest.skills {
                let files = entry
                    .resolve()
                    .ok_or_else(|| {
                        CliError::coded(
                            "adapter/not-found",
                            format!("atc-{} is not on PATH", entry.name()),
                            vec!["atc doctor".into()],
                        )
                    })
                    .and_then(|path| manifest::ask_skill(&path, entry.name(), name));
                match files {
                    Ok(files)
                        if !super::skill::SKILLS.iter().any(|s| s.name == name)
                            && !skills.contains_key(name) =>
                    {
                        skills.insert(
                            name.clone(),
                            Installed {
                                owner: entry.name().into(),
                                version: entry.manifest.version.clone(),
                                files,
                            },
                        );
                    }
                    Ok(_) => eprintln!(
                        "atc: skill {name} collides with another installed skill; omitted"
                    ),
                    Err(err) => eprintln!("atc: skill {name}: {err}"),
                }
            }
        }
        skills
    });
}

fn receipt(root: &Path) -> Result<Skills, CliError> {
    let path = root.join(RECEIPT);
    let body = match std::fs::read(&path) {
        Ok(body) => body,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Skills::new()),
        Err(err) => return Err(super::failed(&path, err)),
    };
    let skills: Skills =
        serde_json::from_slice(&body).map_err(|err| super::malformed(&path, err))?;
    for (name, skill) in &skills {
        if !crate::adapter::valid_name(&skill.owner)
            || !manifest::skill_name(&skill.owner, name)
            || super::skill::SKILLS.iter().any(|s| s.name == name)
        {
            return Err(super::malformed(&path, "invalid skill owner or name"));
        }
        manifest::parse_skill(serde_json::json!({"files": skill.files}))?;
    }
    Ok(skills)
}

fn wanted(name: &str, skill: &Installed) -> bool {
    registry::read()
        .get(&skill.owner)
        .is_some_and(|entry| entry.manifest.skills.iter().any(|s| s == name))
}

fn matches(root: &Path, name: &str, skill: &Installed) -> bool {
    skill.files.iter().all(|file| {
        std::fs::read_to_string(root.join(name).join(&file.path))
            .is_ok_and(|text| text == file.content)
    })
}

/// No child processes: compare declared names and versions with the installed receipt and file bytes.
pub fn current(root: &Path) -> bool {
    let Ok(installed) = receipt(root) else {
        return false;
    };
    if installed
        .iter()
        .any(|(name, skill)| !wanted(name, skill) || !matches(root, name, skill))
    {
        return false;
    }
    for entry in registry::read().declared() {
        for name in &entry.manifest.skills {
            let Some(skill) = installed.get(name) else {
                return false;
            };
            if skill.owner != entry.name() || skill.version != entry.manifest.version {
                return false;
            }
            if let Some(fresh) = PREPARED.get().and_then(|skills| skills.get(name))
                && (skill.files != fresh.files || !matches(root, name, fresh))
            {
                return false;
            }
        }
    }
    true
}

pub fn write(root: &Path) -> Result<(), CliError> {
    let mut installed = receipt(root)?;
    let retired: Vec<_> = installed
        .iter()
        .filter(|(name, skill)| !wanted(name, skill))
        .map(|(name, _)| name.clone())
        .collect();
    for name in retired {
        remove_dir(&root.join(&name))?;
        installed.remove(&name);
    }
    if let Some(prepared) = PREPARED.get() {
        for (name, skill) in prepared {
            let dir = root.join(name);
            if dir.exists() && !installed.contains_key(name) {
                eprintln!(
                    "atc: {} already exists and was not installed by tower; skill omitted",
                    dir.display()
                );
                continue;
            }
            remove_dir(&dir)?;
            for file in &skill.files {
                let path = dir.join(&file.path);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| super::failed(parent, err))?;
                }
                std::fs::write(&path, &file.content).map_err(|err| super::failed(&path, err))?;
            }
            installed.insert(name.clone(), skill.clone());
        }
    }
    let path = root.join(RECEIPT);
    if installed.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|err| super::failed(&path, err))?;
        }
    } else {
        let body = serde_json::to_string_pretty(&installed).expect("skills serialize");
        super::plugin::write_if_changed(&path, &body)?;
    }
    Ok(())
}

pub fn remove(root: &Path) -> Result<bool, CliError> {
    let installed = receipt(root)?;
    for name in installed.keys() {
        remove_dir(&root.join(name))?;
    }
    let path = root.join(RECEIPT);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|err| super::failed(&path, err))?;
        return Ok(true);
    }
    Ok(!installed.is_empty())
}

fn remove_dir(path: &Path) -> Result<(), CliError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            let result = if meta.file_type().is_symlink() {
                std::fs::remove_file(path)
            } else {
                std::fs::remove_dir_all(path)
            };
            result.map_err(|err| super::failed(path, err))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(super::failed(path, err)),
    }
}

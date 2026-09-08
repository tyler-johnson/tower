//! `--ff-skill <name>`: the files behind one skill the manifest lists,
//! for `ff hook` to install.
//!
//! The table holds `tower`, the manual from `integ::SKILL`. #107 adds
//! `tower-plan` and `tower-loop` from `integ::PLAN` and `integ::LOOP`.
//! fufu only asks for names the manifest lists, and the manifest lists
//! this table, so a name that reaches `answer` and misses is an
//! `ff hook` from a manifest this binary did not write.

use serde::Serialize;

use crate::error::CliError;

/// One file of a skill, at a path relative to the skill's directory.
pub struct SkillFile {
    pub path: &'static str,
    pub content: &'static str,
}

/// One skill: its name as the manifest lists it, and its files. Exactly
/// one of them is `SKILL.md` at the root.
pub struct Skill {
    pub name: &'static str,
    pub files: &'static [SkillFile],
}

/// Every skill this binary ships.
pub const SKILLS: &[Skill] = &[Skill {
    name: "tower",
    files: &[SkillFile {
        path: "SKILL.md",
        content: crate::integ::SKILL,
    }],
}];

/// The reply's `data`: `files`, each `{path, content}`.
#[derive(Debug, Serialize)]
pub struct Files {
    pub files: Vec<File>,
}

#[derive(Debug, Serialize)]
pub struct File {
    pub path: &'static str,
    pub content: &'static str,
}

pub fn answer(name: &str) -> Result<Files, CliError> {
    let skill = SKILLS
        .iter()
        .find(|skill| skill.name == name)
        .ok_or_else(|| {
            CliError::coded(
                "skill/unknown",
                format!("ff-tower ships no skill named `{name}`"),
                vec![],
            )
        })?;
    Ok(Files {
        files: skill
            .files
            .iter()
            .map(|file| File {
                path: file.path,
                content: file.content,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Component, Path};

    use super::*;

    /// fufu's rules for a skill reply, applied to every table entry.
    #[test]
    fn every_shipped_skill_is_whole() {
        const CAP: usize = 8 * 1024 * 1024;
        for skill in SKILLS {
            let name = skill.name;
            assert!(
                name == "tower" || name.starts_with("tower-"),
                "`{name}`: a skill is `tower` or `tower-<x>`"
            );
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "`{name}`: [A-Za-z0-9_-] only"
            );
            assert!(!skill.files.is_empty(), "`{name}` ships no files");
            let mut seen = std::collections::HashSet::new();
            let mut total = 0usize;
            let mut roots = 0usize;
            for file in skill.files {
                assert!(
                    Path::new(file.path)
                        .components()
                        .all(|part| matches!(part, Component::Normal(_))),
                    "`{name}`: {} is not a plain relative path",
                    file.path
                );
                assert!(seen.insert(file.path), "`{name}`: {} twice", file.path);
                if file.path == "SKILL.md" {
                    roots += 1;
                }
                total += file.content.len();
            }
            assert_eq!(roots, 1, "`{name}`: exactly one SKILL.md at the root");
            assert!(total <= CAP, "`{name}`: {total} bytes is over 8 MiB");
        }
    }

    #[test]
    fn a_name_the_table_lacks_is_refused() {
        let err = answer("tower-nothing").expect_err("no such skill");
        assert_eq!(err.id(), "skill/unknown");
        assert_eq!(err.exit_code(), 1);
    }
}

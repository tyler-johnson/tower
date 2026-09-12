//! The skills the binary ships, for the clients that read them.
//!
//! The notice is budgeted, because it is context every session pays for
//! whether or not it is needed. These are the other half of that bargain
//! — the manual and the three worked examples, on a shelf, costing
//! nothing until a client decides the situation calls for one. One text
//! each, verbatim, for every client that reads skills: they agree on the
//! file's name and on its front matter, so there is nothing per-vendor
//! to adapt.
//!
//! Delivery is the same story `claude.rs` tells: a directory per skill
//! that tower owns outright, written whole and removed whole, with no
//! foreign content to preserve. Claude takes them inside the plugin it
//! already owns; Codex takes directories beside the settings file it
//! does not.
//!
//! They live as markdown next to the Rust, embedded whole. Each embedded
//! constant is the staleness fingerprint — byte drift on disk reads as
//! "an older tower wrote it" — so the files carry no version or hash of
//! their own.

use std::path::{Path, PathBuf};

use super::{Mechanism, Wiring};
use crate::error::CliError;

/// One shipped skill: the directory name a client reads it by, and the
/// text, front matter first.
pub struct Skill {
    pub name: &'static str,
    pub text: &'static str,
}

/// The `tower` skill: the advanced manual, on fufu's model.
pub const SKILL: &str = include_str!("skill.md");

/// Every skill an install writes, in the order the reports name them.
/// The manual first, then the three worked examples the loop is flown
/// with — `/tower:plan`, `/tower:work`, `/tower:review` in Claude Code.
pub const SKILLS: [Skill; 4] = [
    Skill {
        name: "tower",
        text: SKILL,
    },
    Skill {
        name: "plan",
        text: include_str!("plan.md"),
    },
    Skill {
        name: "work",
        text: include_str!("work.md"),
    },
    Skill {
        name: "review",
        text: include_str!("review.md"),
    },
];

/// The one file in a skill's directory. The clients' convention, not
/// tower's.
const FILE: &str = "SKILL.md";

fn path(root: &Path, skill: &Skill) -> PathBuf {
    root.join(skill.name).join(FILE)
}

/// Every skill's name, for a report line.
pub fn names() -> String {
    SKILLS
        .iter()
        .map(|skill| skill.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `root/<name>/SKILL.md` for each of the four.
pub fn write_all(root: &Path) -> Result<(), CliError> {
    for skill in &SKILLS {
        let dir = root.join(skill.name);
        std::fs::create_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
        let file = path(root, skill);
        std::fs::write(&file, skill.text).map_err(|err| super::failed(&file, err))?;
    }
    Ok(())
}

/// Every skill's directory, removed. Answers whether there was anything
/// to remove, so a caller can report honestly instead of claiming a
/// change it did not make.
pub fn remove_all(root: &Path) -> Result<bool, CliError> {
    let mut removed = false;
    for skill in &SKILLS {
        let dir = root.join(skill.name);
        if !dir.exists() {
            continue;
        }
        std::fs::remove_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
        removed = true;
    }
    Ok(removed)
}

/// What is on disk under `root`, held against what this binary ships.
///
/// All four byte-equal is `Wired`. Any of them missing or drifted while
/// at least one is there is `Partial` rather than missing: what is on
/// disk works, it simply describes a tower that has moved, and that is
/// a repair `atc hook -u` makes and never an outage. None at all is
/// `NotWired`.
pub fn wiring(root: &Path) -> Wiring {
    let mut present = 0usize;
    let mut current = 0usize;
    for skill in &SKILLS {
        match std::fs::read_to_string(path(root, skill)) {
            Ok(text) if text == skill.text => {
                present += 1;
                current += 1;
            }
            Ok(_) => present += 1,
            Err(_) => {}
        }
    }
    if present == 0 {
        Wiring::NotWired
    } else if current == SKILLS.len() {
        Wiring::Wired {
            mechanism: Mechanism::Plugin,
            at: root.to_path_buf(),
        }
    } else {
        Wiring::Partial {
            missing: "an older tower wrote them".into(),
            at: root.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn they_round_trip_through_a_directory() {
        let home = tempfile::TempDir::new().unwrap();
        let root = home.path().join("skills");
        assert_eq!(wiring(&root), Wiring::NotWired);

        write_all(&root).unwrap();
        assert!(matches!(wiring(&root), Wiring::Wired { .. }));
        for skill in &SKILLS {
            assert_eq!(
                std::fs::read_to_string(path(&root, skill)).unwrap(),
                skill.text,
                "{} lands byte for byte",
                skill.name
            );
        }

        std::fs::write(path(&root, &SKILLS[1]), "an older tower wrote this").unwrap();
        assert!(
            matches!(wiring(&root), Wiring::Partial { .. }),
            "text that is not what this binary ships is a repair, not a hole"
        );
        std::fs::remove_dir_all(root.join(SKILLS[2].name)).unwrap();
        assert!(
            matches!(wiring(&root), Wiring::Partial { .. }),
            "one of four missing is a repair too"
        );

        assert!(remove_all(&root).unwrap());
        assert!(!remove_all(&root).unwrap(), "nothing left to take");
        assert_eq!(wiring(&root), Wiring::NotWired);
    }
}

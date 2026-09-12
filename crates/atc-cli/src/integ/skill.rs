//! The skills the binary ships, for the clients that read them.
//!
//! The notice is budgeted, because it is context every session pays for
//! whether or not it is needed. This is the other half of that bargain
//! — the manual, on a shelf, costing nothing until a client decides the
//! situation calls for it. One text, verbatim, for every client that
//! reads skills: they agree on the file's name and on its front matter,
//! so there is nothing per-vendor to adapt.
//!
//! Delivery is the same story `claude.rs` tells: a directory per skill
//! that tower owns outright, written whole and removed whole, with no
//! foreign content to preserve. Claude takes them inside the plugin it
//! already owns; Codex inside the plugin tower writes for it, under
//! `skills/` the same way.
//!
//! It lives as markdown next to the Rust, embedded whole. The embedded
//! constant is the staleness fingerprint — byte drift on disk reads as
//! "an older tower wrote it" — so the file carries no version or hash of
//! its own.
//!
//! A skill an older tower shipped and this one does not is recognized by
//! the front matter it shipped with and removed on the next write, so
//! `hook -u` is the repair for a retired skill the way it is for a
//! drifted one.

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

/// The manual — `/tower:tower` in Claude Code, `$tower` in Codex.
pub const SKILLS: [Skill; 1] = [Skill {
    name: "tower",
    text: SKILL,
}];

/// Skills an older tower shipped and this one does not: the three
/// worked examples, written by the hook from ba1203e until they went
/// back to docs/skills/. Each is recognized by the front matter it
/// shipped with, name and description byte for byte, so a skill of
/// the user's own under the same name — possible under `~/.codex/skills`,
/// where an older tower wrote — is never touched.
const RETIRED: [(&str, &str); 3] = [
    (
        "plan",
        "decompose a goal into linked flights — solo mode's entry point",
    ),
    (
        "review",
        "first-pass a branch — fix the mechanical half, hold the rest",
    ),
    (
        "work",
        "claim, do, hold or commit, repeat — the loop that pairs with `atc next`",
    ),
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

/// The retired directories under `root` an older tower wrote.
fn stale(root: &Path) -> Vec<PathBuf> {
    RETIRED
        .iter()
        .map(|(name, description)| {
            let dir = root.join(name);
            let head = format!("---\nname: {name}\ndescription: {description}\n");
            (dir, head)
        })
        .filter(|(dir, head)| {
            std::fs::read_to_string(dir.join(FILE)).is_ok_and(|text| text.starts_with(head))
        })
        .map(|(dir, _)| dir)
        .collect()
}

/// `root/<name>/SKILL.md` for each shipped skill, after any retired one
/// goes.
pub fn write_all(root: &Path) -> Result<(), CliError> {
    for dir in stale(root) {
        std::fs::remove_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
    }
    for skill in &SKILLS {
        let dir = root.join(skill.name);
        std::fs::create_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
        let file = path(root, skill);
        std::fs::write(&file, skill.text).map_err(|err| super::failed(&file, err))?;
    }
    Ok(())
}

/// Every skill's directory, removed, retired ones included. Answers
/// whether there was anything to remove, so a caller can report honestly
/// instead of claiming a change it did not make.
pub fn remove_all(root: &Path) -> Result<bool, CliError> {
    let mut removed = false;
    let shipped = SKILLS.iter().map(|skill| root.join(skill.name));
    for dir in shipped.chain(stale(root)) {
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
/// Byte-equal is `Wired`. Drifted is `Partial` rather than missing: what
/// is on disk works, it simply describes a tower that has moved, and
/// that is a repair `atc hook -u` makes and never an outage. A retired
/// skill on disk is `Partial` too. Nothing at all is `NotWired`.
pub fn wiring(root: &Path) -> Wiring {
    let stale = stale(root);
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
    if present == 0 && stale.is_empty() {
        Wiring::NotWired
    } else if current == SKILLS.len() && stale.is_empty() {
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
        assert_eq!(
            std::fs::read_to_string(path(&root, &SKILLS[0])).unwrap(),
            SKILLS[0].text,
            "the manual lands byte for byte"
        );

        std::fs::write(path(&root, &SKILLS[0]), "an older tower wrote this").unwrap();
        assert!(
            matches!(wiring(&root), Wiring::Partial { .. }),
            "text that is not what this binary ships is a repair, not a hole"
        );

        assert!(remove_all(&root).unwrap());
        assert!(!remove_all(&root).unwrap(), "nothing left to take");
        assert_eq!(wiring(&root), Wiring::NotWired);
    }

    /// The front matter is the fingerprint: a retired skill an older
    /// tower wrote is taken on the next write, and a file of the user's
    /// own under the same name is neither counted nor touched.
    #[test]
    fn a_retired_skill_is_a_repair_and_a_users_own_is_not() {
        let home = tempfile::TempDir::new().unwrap();
        let root = home.path().join("skills");
        write_all(&root).unwrap();

        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(
            work.join(FILE),
            "---\nname: work\ndescription: claim, do, hold or commit, repeat — the loop that pairs with `atc next`\n---\n# work\n",
        )
        .unwrap();
        assert!(
            matches!(wiring(&root), Wiring::Partial { .. }),
            "a retired skill on disk is a repair"
        );

        let plan = root.join("plan");
        std::fs::create_dir_all(&plan).unwrap();
        let mine = "---\nname: plan\ndescription: mine\n---\n";
        std::fs::write(plan.join(FILE), mine).unwrap();
        assert!(
            matches!(wiring(&root), Wiring::Partial { .. }),
            "still the retired work"
        );

        write_all(&root).unwrap();
        assert!(!work.exists(), "the retired skill goes");
        assert_eq!(
            std::fs::read_to_string(plan.join(FILE)).unwrap(),
            mine,
            "the user's own stands"
        );
        assert!(
            matches!(wiring(&root), Wiring::Wired { .. }),
            "the user's own is invisible to the count"
        );

        assert!(remove_all(&root).unwrap());
        assert_eq!(
            std::fs::read_to_string(plan.join(FILE)).unwrap(),
            mine,
            "uninstall leaves the user's own too"
        );
    }
}

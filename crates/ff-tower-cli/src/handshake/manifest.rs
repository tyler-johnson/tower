//! `--ff-manifest`: what tower is, in fufu's declared-extension shape.
//!
//! The verbs are walked out of the clap tree rather than listed, so a
//! verb added to `cli.rs` joins the manifest by existing; what a verb
//! cannot carry in clap is its read-only bit, which is the one table
//! here. `bay` is one verb — its sub-verbs are its actions, not verbs of
//! their own.

use serde::Serialize;

use ff_tower_core::machine::{CONTRACT, NAME};

use super::skill;

/// The manifest, field order fixed by the struct: fufu reads the keys it
/// knows and keeps the rest.
#[derive(Serialize)]
pub struct Manifest {
    pub name: &'static str,
    pub version: &'static str,
    pub contract: u32,
    pub verbs: Vec<Verb>,
    pub undoable: bool,
    pub briefing: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<&'static str>,
    pub tools: bool,
}

#[derive(Serialize)]
pub struct Verb {
    pub name: String,
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

pub fn manifest() -> Manifest {
    Manifest {
        name: NAME,
        version: env!("CARGO_PKG_VERSION"),
        // tower's own number, never `FF_CONTRACT` from the environment:
        // the manifest says what this binary speaks, and fufu compares.
        contract: CONTRACT,
        verbs: verbs(),
        // Every write is a gix append to the log refs
        // (`core/src/log/mod.rs`), outside fufu's operation log, so `ff
        // undo` cannot take one back.
        undoable: false,
        briefing: true,
        // The same table `--ff-skill` answers from, so fufu is never
        // promised a skill the binary would then refuse.
        skills: skill::SKILLS.iter().map(|skill| skill.name).collect(),
        tools: true,
    }
}

/// Every top-level verb in clap order, with clap's one-line `about` as
/// its summary.
pub fn verbs() -> Vec<Verb> {
    let tree = super::tree();
    super::verbs_of(&tree)
        .map(|sub| Verb {
            name: sub.get_name().to_string(),
            read_only: read_only(sub.get_name()).unwrap_or(false),
            summary: sub.get_about().map(ToString::to_string),
        })
        .collect()
}

/// Whether a verb only reads. An explicit match over every verb name,
/// `lanes()`'s exhaustive-table discipline: a verb this table has not
/// heard of is `None`, which the manifest reads as "writes" and a unit
/// test reads as a verb nobody sorted.
pub fn read_only(verb: &str) -> Option<bool> {
    match verb {
        "board" | "brief" | "procedures" | "skills" | "explain" | "version" | "doctor"
        | "briefing" => Some(true),
        "next" | "file" | "comment" | "edit" | "link" | "unlink" | "decompose" | "assign"
        | "status" | "cancel" | "hold" | "answer" | "done" | "config" | "bay" | "update"
        | "serve" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verb_is_sorted_by_the_read_only_table() {
        let tree = super::super::tree();
        let names: Vec<&str> = super::super::verbs_of(&tree)
            .map(clap::Command::get_name)
            .collect();
        assert!(names.len() >= 24, "the walk is broken: {names:?}");
        for name in &names {
            assert!(
                read_only(name).is_some(),
                "`{name}` is a verb the read-only table has not sorted"
            );
        }
        // And the table names no verb clap does not have.
        for name in [
            "board",
            "brief",
            "procedures",
            "skills",
            "explain",
            "version",
            "doctor",
            "briefing",
            "next",
            "file",
            "comment",
            "edit",
            "link",
            "unlink",
            "decompose",
            "assign",
            "status",
            "cancel",
            "hold",
            "answer",
            "done",
            "config",
            "bay",
            "update",
            "serve",
        ] {
            assert!(
                names.contains(&name),
                "`{name}` is in the table, not the tree"
            );
        }
    }

    #[test]
    fn the_manifest_serializes_in_fufus_shape() {
        let value = serde_json::to_value(manifest()).expect("serializes");
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        // serde_json's map sorts, so compare as a set.
        let mut want = vec![
            "name", "version", "contract", "verbs", "undoable", "briefing", "tools",
        ];
        want.sort_unstable();
        assert_eq!(keys, want, "no `skills` while the table is empty: {value}");
        assert_eq!(value["name"], "tower");
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["contract"], 1);
        assert_eq!(value["undoable"], false);
        assert_eq!(value["briefing"], true);
        assert_eq!(value["tools"], true);
        let verbs = value["verbs"].as_array().expect("verbs");
        assert!(verbs.iter().any(|verb| verb["name"] == "bay"));
        assert!(
            !verbs.iter().any(|verb| verb["name"] == "list"),
            "bay's actions are not verbs: {verbs:?}"
        );
        for verb in verbs {
            let name = verb["name"].as_str().expect("name");
            assert!(!name.contains(' '), "one word: {name}");
            assert!(verb["read_only"].is_boolean(), "{name}");
            assert!(
                !verb["summary"].as_str().unwrap_or_default().is_empty(),
                "{name} has no summary"
            );
        }
    }
}

//! `ff tower skills [<name>]` — the read-only shelf of prose.
//!
//! The verb spawns no fufu, so no `TOWER_FF` appears here. It does read
//! the registry, so every spawn points `XDG_CONFIG_HOME` at the fixture's
//! own tempdir: a suite that read the developer's real
//! `~/.config/tower/skills` would pass or fail by whose machine it is.
//!
//! The engine ships empty, so every listing here is over files the test
//! wrote itself — `docs/skills/`'s shape, not anything in the binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

/// A skill file with the front matter a harness redirect depends on,
/// `docs/skills/work.md`'s shape. Written by the fixture, so the
/// byte-for-byte comparison has a source to hold the output against.
const WORK_MD: &str = "\
---
name: tower-work
description: claim, do, hold or commit, repeat — the loop that pairs with `ff tower next`
---

# work

You are the crew of a loop over `ff tower next`.
";

const PLAN_MD: &str = "\
---
name: tower-plan
description: decompose a goal into linked flights — solo mode's entry point
---

# plan

Turn a goal into flights tower stores.
";

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .output()
        .expect("spawn ff-tower")
}

/// The fixture's own config root, beside the repository inside the
/// tempdir and never created — an empty user layer.
fn xdg(repo: &Path) -> PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .join("xdg")
}

/// A skill in the fixture's user layer, under the config root every
/// spawn is pointed at.
fn install_user(repo: &Path, name: &str, text: &str) {
    let dir = xdg(repo).join("tower").join("skills");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join(format!("{name}.md")), text).expect("write");
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("an envelope")
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

/// A repository with the two example skills installed in its own layer.
fn stocked() -> Repo {
    let repo = repo();
    repo.write(".tower/skills/plan.md", PLAN_MD);
    repo.write(".tower/skills/work.md", WORK_MD);
    repo
}

#[test]
fn an_empty_shelf_says_so_and_where_a_skill_goes() {
    // The engine ships empty, so this is the fresh box's answer — not a
    // bare zero, and not a fault.
    let repo = repo();
    let out = stdout(&ff_tower(repo.path(), &["skills"]));
    assert!(out.starts_with("no skills installed\n"), "{out}");
    assert!(
        out.contains(&format!(
            "author: {} · {}\n",
            repo.path()
                .join(".tower")
                .join("skills")
                .join("<name>.md")
                .display(),
            xdg(repo.path())
                .join("tower")
                .join("skills")
                .join("<name>.md")
                .display()
        )),
        "both homes, repository first: {out}"
    );
    assert!(
        out.contains("examples: docs/skills/ in the tower repository\n"),
        "{out}"
    );

    // And the JSON form is an empty set rather than an absence.
    let all = envelope(&ff_tower(repo.path(), &["skills", "--json"]));
    assert_eq!(all["data"]["skills"], serde_json::json!([]));
}

#[test]
fn the_listing_is_what_is_installed_with_layers_and_descriptions() {
    let repo = stocked();
    let out = stdout(&ff_tower(repo.path(), &["skills"]));
    assert!(
        out.contains("plan  repo  decompose a goal into linked flights"),
        "{out}"
    );
    assert!(
        out.contains("work  repo  claim, do, hold or commit, repeat"),
        "{out}"
    );
    assert!(
        out.contains("2 skills · ff tower skills <name> for one, raw"),
        "{out}"
    );
    // The footer offers both fork homes, repository first.
    assert!(
        out.contains(&format!(
            "fork: {}",
            repo.path()
                .join(".tower")
                .join("skills")
                .join("<name>.md")
                .display()
        )),
        "{out}"
    );
    // Joined rather than spelled with slashes: the footer prints a real
    // path, and Windows separates it with backslashes.
    assert!(
        out.contains(
            &xdg(repo.path())
                .join("tower")
                .join("skills")
                .join("<name>.md")
                .display()
                .to_string()
        ),
        "{out}"
    );
}

#[test]
fn a_named_skill_is_the_raw_file_byte_for_byte() {
    let repo = stocked();
    let out = ff_tower(repo.path(), &["skills", "work"]);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        WORK_MD,
        "the named output is the source file, nothing else"
    );
}

#[test]
fn the_json_forms_carry_summary_and_text() {
    let repo = stocked();
    let all = envelope(&ff_tower(repo.path(), &["skills", "--json"]));
    assert_eq!(all["cmd"], serde_json::json!("skills"));
    let skills = all["data"]["skills"].as_array().expect("skills");
    assert_eq!(skills.len(), 2);
    assert_eq!(skills[0]["name"], serde_json::json!("plan"));
    assert_eq!(
        skills[0]["source"],
        serde_json::json!({
            "layer": "repo",
            "path": repo.path().join(".tower").join("skills").join("plan.md").display().to_string(),
        })
    );
    assert!(
        skills[0]["summary"]
            .as_str()
            .is_some_and(|summary| !summary.is_empty())
    );

    let one = envelope(&ff_tower(repo.path(), &["skills", "work", "--json"]));
    let work = &one["data"]["skill"];
    assert_eq!(work["name"], serde_json::json!("work"));
    assert_eq!(work["source"]["layer"], serde_json::json!("repo"));
    assert_eq!(work["text"], serde_json::json!(WORK_MD));
}

#[test]
fn a_name_that_is_not_installed_is_refused_naming_the_set() {
    let repo = stocked();
    let out = ff_tower(repo.path(), &["skills", "ghost", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("skill/not-found")
    );
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("no skill `ghost` — installed: plan, work")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower skills"])
    );

    // On an empty shelf the same refusal says as much rather than
    // trailing an empty list.
    let bare = self::repo();
    let out = ff_tower(bare.path(), &["skills", "ghost", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        self::envelope(&out)["error"]["message"],
        serde_json::json!("no skill `ghost` — nothing installed")
    );
}

#[test]
fn a_repo_fork_shadows_the_user_layer_wholesale_and_says_so() {
    let repo = repo();
    install_user(repo.path(), "work", WORK_MD);
    install_user(repo.path(), "plan", PLAN_MD);
    repo.write(
        ".tower/skills/work.md",
        "# work, ours\n\nPush when green.\n",
    );

    let out = stdout(&ff_tower(repo.path(), &["skills"]));
    assert!(out.contains("work  repo  work, ours"), "{out}");
    assert!(!out.contains("user  claim, do"), "{out}");
    // A name the repository layer does not carry is the user's still.
    assert!(
        out.contains("plan  user  decompose a goal into linked flights"),
        "{out}"
    );

    let raw = stdout(&ff_tower(repo.path(), &["skills", "work"]));
    assert_eq!(raw, "# work, ours\n\nPush when green.\n");

    let one = envelope(&ff_tower(repo.path(), &["skills", "work", "--json"]));
    assert_eq!(
        one["data"]["skill"]["source"]["layer"],
        serde_json::json!("repo")
    );
    assert_eq!(
        one["data"]["skill"]["source"]["path"],
        serde_json::json!(
            repo.path()
                .join(".tower")
                .join("skills")
                .join("work.md")
                .display()
                .to_string()
        )
    );
}

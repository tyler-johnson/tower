//! `ff tower skills [<name>]` — the read-only shelf of prose.
//!
//! The verb spawns no fufu, so no `TOWER_FF` appears here. It does read
//! the registry, so every spawn points `XDG_CONFIG_HOME` at the fixture's
//! own tempdir: a suite that read the developer's real
//! `~/.config/tower/skills` would pass or fail by whose machine it is.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

/// The shipped file, for the byte-for-byte comparison: named output is
/// the raw markdown, so a redirect must reproduce the source exactly.
const WORK_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ff-tower-core/src/skill/builtin/work.md"
));

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
fn xdg(repo: &Path) -> std::path::PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .join("xdg")
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

#[test]
fn the_listing_is_the_shipped_set_with_layers_and_descriptions() {
    let repo = repo();
    let out = stdout(&ff_tower(repo.path(), &["skills"]));
    assert!(
        out.contains("plan    built-in  decompose a goal into linked flights"),
        "{out}"
    );
    assert!(
        out.contains("review  built-in  first-pass a branch"),
        "{out}"
    );
    assert!(
        out.contains("work    built-in  claim, do, hold or commit, repeat"),
        "{out}"
    );
    assert!(
        out.contains("3 skills · ff tower skills <name> for one, raw"),
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
    assert!(out.contains("tower/skills/<name>.md"), "{out}");
}

#[test]
fn a_named_skill_is_the_raw_file_byte_for_byte() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["skills", "work"]);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        WORK_MD,
        "the named output is the source file, nothing else"
    );
    assert!(WORK_MD.starts_with("---\n"), "the front matter ships");
}

#[test]
fn the_json_forms_carry_summary_and_text() {
    let repo = repo();
    let all = envelope(&ff_tower(repo.path(), &["skills", "--json"]));
    assert_eq!(all["cmd"], serde_json::json!("skills"));
    let skills = all["data"]["skills"].as_array().expect("skills");
    assert_eq!(skills.len(), 3);
    assert_eq!(skills[0]["name"], serde_json::json!("plan"));
    assert_eq!(
        skills[0]["source"],
        serde_json::json!({"layer": "built-in", "path": null})
    );
    assert!(
        skills[0]["summary"]
            .as_str()
            .is_some_and(|summary| !summary.is_empty())
    );

    let one = envelope(&ff_tower(repo.path(), &["skills", "work", "--json"]));
    let work = &one["data"]["skill"];
    assert_eq!(work["name"], serde_json::json!("work"));
    assert_eq!(
        work["source"],
        serde_json::json!({"layer": "built-in", "path": null})
    );
    assert_eq!(work["text"], serde_json::json!(WORK_MD));
}

#[test]
fn a_name_that_is_not_installed_is_refused_naming_the_set() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["skills", "ghost", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("skill/not-found")
    );
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("no skill `ghost` — installed: plan, review, work")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower skills"])
    );
}

#[test]
fn a_repo_fork_shadows_the_built_in_wholesale_and_says_so() {
    let repo = repo();
    repo.write(
        ".tower/skills/work.md",
        "# work, ours\n\nPush when green.\n",
    );

    let out = stdout(&ff_tower(repo.path(), &["skills"]));
    assert!(out.contains("work    repo      work, ours"), "{out}");
    assert!(!out.contains("built-in  claim, do"), "{out}");

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

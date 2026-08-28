//! `ff tower explain` against the real binary: the error-id lookup,
//! fufu's treatment. Most runs sit in a plain tempdir outside any
//! repository with `TOWER_FF=/nonexistent` — the verb is a pure registry
//! lookup, so a reach for fufu or a store would be the failure these
//! tests notice. The one repo-backed test pins the `exits_for` seam: a
//! raise site with no exits of its own gains the registry lookup on both
//! failure surfaces.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

fn ff_tower(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .current_dir(dir)
        .env("TOWER_FF", "/nonexistent")
        .env("XDG_CACHE_HOME", dir.join("cache"))
        // The update cache root forks to `LOCALAPPDATA` on Windows.
        .env("LOCALAPPDATA", dir.join("cache"))
        .env("XDG_CONFIG_HOME", dir.join("xdg"))
        .env("HOME", dir)
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", dir)
        .env_remove("FF_REPO")
        .output()
        .expect("spawn ff-tower")
}

fn stdout(out: &Output) -> String {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn envelope(out: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("an envelope")
}

#[test]
fn a_known_id_renders_id_summary_detail_and_the_try_block() {
    let dir = tempfile::TempDir::new().unwrap();
    let text = stdout(&ff_tower(dir.path(), &["explain", "flight/not-found"]));

    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("flight/not-found"), "{text}");
    assert_eq!(lines.next(), Some("no such flight on the board"), "{text}");
    assert_eq!(lines.next(), Some(""), "a blank before the detail: {text}");
    assert!(text.contains("The reference parsed"), "{text}");
    assert!(text.contains("  try:\n    ff tower\n"), "{text}");
    // The detail wraps at 80 columns.
    assert!(
        text.lines().all(|line| line.chars().count() <= 80),
        "{text}"
    );
}

#[test]
fn an_entry_with_no_exits_prints_no_try_block() {
    let dir = tempfile::TempDir::new().unwrap();
    let text = stdout(&ff_tower(dir.path(), &["explain", "usage/self-link"]));
    assert!(text.starts_with("usage/self-link\n"), "{text}");
    assert!(!text.contains("try:"), "{text}");
}

#[test]
fn the_list_aligns_ids_beside_summaries() {
    let dir = tempfile::TempDir::new().unwrap();
    let text = stdout(&ff_tower(dir.path(), &["explain", "--list"]));

    let not_found = text
        .lines()
        .find(|line| line.starts_with("flight/not-found"))
        .expect("the list carries flight/not-found");
    assert!(not_found.contains("no such flight on the board"), "{text}");
    // Every row is one aligned `id  summary` pair: the summaries start
    // in one column, two spaces past the widest id.
    let width = text
        .lines()
        .filter_map(|line| line.split("  ").next())
        .map(|id| id.trim_end().len())
        .max()
        .expect("rows");
    for line in text.lines() {
        let id = line.split_whitespace().next().expect("an id");
        assert!(id.contains('/'), "every row leads with an id: {line}");
        assert!(
            line.chars().nth(width + 1).is_some(),
            "summaries align past the widest id: {line}"
        );
    }
}

#[test]
fn json_carries_the_entry_and_the_list() {
    let dir = tempfile::TempDir::new().unwrap();

    let out = ff_tower(dir.path(), &["explain", "flight/not-found", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let single = envelope(&out);
    assert_eq!(single["tower"], serde_json::json!(1));
    assert_eq!(single["cmd"], serde_json::json!("explain"));
    let data = &single["data"];
    assert_eq!(data["id"], serde_json::json!("flight/not-found"));
    assert_eq!(
        data["summary"],
        serde_json::json!("no such flight on the board")
    );
    assert!(data["detail"].as_str().is_some_and(|d| !d.is_empty()));
    assert_eq!(data["exits"], serde_json::json!(["ff tower"]));

    let out = ff_tower(dir.path(), &["explain", "--list", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let listing = envelope(&out);
    assert_eq!(listing["cmd"], serde_json::json!("explain"));
    let entries = listing["data"]["entries"].as_array().expect("entries");
    assert!(entries.len() > 25, "the whole catalog: {}", entries.len());
    for entry in entries {
        for key in ["id", "summary", "detail", "exits"] {
            assert!(entry.get(key).is_some(), "an entry is missing `{key}`");
        }
    }
    assert!(
        entries
            .iter()
            .any(|entry| entry["id"] == serde_json::json!("usage/unknown-error-id")),
        "the verb's own refusal is in its own catalog"
    );
}

#[test]
fn an_unknown_id_refuses_with_exit_2_and_names_the_list() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["explain", "nonsense", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(envelope["cmd"], serde_json::json!("explain"));
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("usage/unknown-error-id")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower explain --list"])
    );
    assert!(envelope.get("data").is_none(), "data and error, never both");
}

#[test]
fn a_slash_shaped_unknown_id_points_at_fufus_registry() {
    // Forwarded fufu refusals live in fufu's registry, so a slash-shaped
    // miss earns the `ff explain` hint beside the list.
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["explain", "repo/bare", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower explain --list", "ff explain repo/bare"])
    );
}

#[test]
fn a_flight_shaped_argument_points_at_brief() {
    // Someone reaching for the verb's old meaning: `explain 15` was a
    // flight explanation once, and the brief is where that lives now.
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["explain", "15", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower explain --list", "ff tower brief 15"])
    );
}

#[test]
fn bare_explain_refuses_with_exit_2() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["explain"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("an id, or --list"), "{stderr}");
    assert!(stderr.contains("ff tower explain --list"), "{stderr}");

    let out = ff_tower(dir.path(), &["explain", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("usage/bad-flags")
    );
}

/// The `exits_for` seam, end to end: a raise site that passes no exits
/// gains `ff tower explain <id>` on both failure surfaces — the human
/// `try:` block and the JSON envelope — so a coded failure never dead-ends.
#[test]
fn a_raise_with_no_exits_gains_the_registry_lookup() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let spawn = |args: &[&str]| -> Output {
        Command::new(env!("CARGO_BIN_EXE_ff-tower"))
            .args(args)
            .env("FF_REPO", repo.path())
            .env(
                "XDG_CONFIG_HOME",
                repo.path().parent().expect("nested").join("xdg"),
            )
            .output()
            .expect("spawn ff-tower")
    };
    stdout(&spawn(&["file", "the flight"]));

    let out = spawn(&["link", "1", "1"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("  try:\n    ff tower explain usage/self-link\n"),
        "{stderr}"
    );

    let out = spawn(&["link", "1", "1", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("usage/self-link")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower explain usage/self-link"])
    );
}

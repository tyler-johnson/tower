//! `ff tower brief` against real repositories: the full record in one
//! read, the JSON round-trip, and the refusals.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::ff::Ff;
use ff_tower_testsupport::Repo;

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .output()
        .expect("spawn ff-tower")
}

/// The fixture's own config root, beside the repository inside the
/// tempdir and never created — an empty user layer. A suite that read the
/// developer's real `~/.config/tower/procedures` would pass or fail by
/// whose machine it is running on.
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

/// Assert a refusal: the exit code, and the envelope's error id.
fn refusal(output: &Output, code: i32, id: &str) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let envelope = envelope(output);
    assert_eq!(envelope["error"]["id"], serde_json::json!(id));
    envelope
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

/// Two linked flights, a body on the first, a comment on the first.
fn repo_with_a_record() -> Repo {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the dependent", "-m", "the body of the work"],
    ));
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["link", "1", "2"]));
    stdout(&ff_tower(
        repo.path(),
        &["comment", "1", "-m", "a note on the record"],
    ));
    repo
}

#[test]
fn brief_renders_the_full_record_both_link_directions() {
    let repo = repo_with_a_record();

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("#1  the dependent"), "{text}");
    assert!(text.contains("the body of the work"), "{text}");
    assert!(text.contains("depends on\n· #2  the dependency"), "{text}");
    assert!(text.contains("comments\n"), "{text}");
    assert!(text.contains("tests@tower.invalid"), "{text}");
    assert!(text.contains("a note on the record"), "{text}");
    assert!(text.contains("filed "), "{text}");
    assert!(text.contains("board: ff tower"), "{text}");
    assert!(
        !text.contains('\x1b'),
        "piped output has escape bytes: {text:?}"
    );

    let text = stdout(&ff_tower(repo.path(), &["brief", "2"]));
    assert!(text.contains("#2  the dependency"), "{text}");
    assert!(text.contains("blocks\n· #1  the dependent"), "{text}");
    assert!(!text.contains("depends on"), "{text}");
}

#[test]
fn a_part_stamp_gets_its_own_line_under_the_head() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the retry test", "-p", "review"],
    ));

    let text = stdout(&ff_tower(repo.path(), &["brief", "2"]));
    assert!(text.contains("#2  the retry test · pass\n"), "{text}");
    assert!(
        text.contains("    part pass · agent · skill review · done asserted\n"),
        "{text}"
    );

    let text = stdout(&ff_tower(repo.path(), &["brief", "3"]));
    assert!(
        text.contains("    part smoke · you · bay warm · done asserted\n"),
        "{text}"
    );

    // The parent is no part, so it gets no line.
    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(!text.contains("part "), "{text}");

    let envelope = envelope(&ff_tower(repo.path(), &["brief", "2", "--json"]));
    assert_eq!(envelope["data"]["part"]["crew"], serde_json::json!("agent"));
    assert!(
        envelope["data"]["part"]["bay"].is_null(),
        "absent facts are null: {envelope}"
    );
}

#[test]
fn json_round_trips_the_brief() {
    let repo = repo_with_a_record();

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("brief"));
    let data = &envelope["data"];
    assert_eq!(data["id"], serde_json::json!("pi.1"));
    assert_eq!(data["subject"], serde_json::json!("the dependent"));
    assert_eq!(data["body"], serde_json::json!("the body of the work"));
    assert_eq!(
        data["comments"][0]["text"],
        serde_json::json!("a note on the record")
    );
    assert_eq!(data["depends_on"][0]["flight"], serde_json::json!("pi.2"));
    assert_eq!(
        data["depends_on"][0]["subject"],
        serde_json::json!("the dependency")
    );
    assert_eq!(data["depends_on"][0]["done"], serde_json::json!(false));
    // Absent facts are null, never missing keys.
    assert!(data["branch"].is_null(), "{data}");
    assert!(data["done_by"].is_null(), "{data}");
    assert!(data["question"].is_null(), "{data}");
}

#[test]
fn a_session_tagged_flight_carries_its_branch_and_tip() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "branch work"]));

    repo.ff(&["start", "-b", "work"]);
    repo.write("work.txt", "work\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "work: a commit"]);

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["branch"], serde_json::json!("work"));
    let tip = envelope["data"]["tip"].as_str().expect("a tip").to_string();

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains(&format!("on work {}", &tip[..8])), "{text}");
}

#[test]
fn a_held_flight_renders_its_question() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck work"]));
    let out = ff_tower(repo.path(), &["hold", "1", "-m", "which color?"]);
    assert_eq!(out.status.code(), Some(3));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("which color?"), "{text}");
    assert!(text.contains("asked "), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["question"], serde_json::json!("which color?"));
    assert_eq!(data["asked_by"], serde_json::json!("tests@tower.invalid"));
}

#[test]
fn a_done_flight_still_briefs_with_its_mark() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "finished work", "-m", "what it was about"],
    ));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("done by tests@tower.invalid"), "{text}");
    assert!(text.contains("what it was about"), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["done_by"], serde_json::json!("tests@tower.invalid"));
    assert!(data["done_at"].is_i64(), "{data}");
}

#[test]
fn a_done_dependency_marks_its_link_row() {
    let repo = repo_with_a_record();
    stdout(&ff_tower(repo.path(), &["done", "2"]));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("· #2  the dependency  done"), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["depends_on"][0]["done"], serde_json::json!(true));
}

#[test]
fn bare_seq_and_hash_prefixed_and_full_references_all_resolve() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));
    for reference in ["1", "#1", "pi.1"] {
        let text = stdout(&ff_tower(repo.path(), &["brief", reference]));
        assert!(text.contains("#1  the flight"), "`{reference}`: {text}");
    }
}

#[test]
fn an_unknown_flight_refuses_not_found() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the only flight"]));
    let out = ff_tower(repo.path(), &["brief", "9", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn a_malformed_reference_refuses_bad_flight() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["brief", "not-an-id", "--json"]);
    refusal(&out, 2, "usage/bad-flight");
}

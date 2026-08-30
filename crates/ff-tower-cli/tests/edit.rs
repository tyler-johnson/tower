//! `ff tower edit` against real repositories: the overlay on a flight's
//! subject and body, a comment reworded by its printed id, the permissive
//! stance on done flights, the envelope, and the refusals.

use std::path::Path;
use std::process::{Command, Output};

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

/// One flight with a body and a comment; the comment's wire id comes back
/// out of the brief, the way a user would find it.
fn repo_with_a_comment() -> (Repo, String) {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the flight", "-m", "the body"],
    ));
    stdout(&ff_tower(
        repo.path(),
        &["comment", "1", "-m", "a note with a tpyo"],
    ));
    let brief = envelope(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    let id = brief["data"]["comments"][0]["id"]
        .as_str()
        .expect("a comment id")
        .to_string();
    (repo, id)
}

#[test]
fn a_subject_edit_rewords_the_flight_and_echoes() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the wrogn subject"]));

    let out = stdout(&ff_tower(
        repo.path(),
        &["edit", "1", "-s", "the right subject"],
    ));
    assert_eq!(out, "edited #1\nboard: ff tower\n");

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("#1  the right subject"), "{text}");
    assert!(text.contains("edited · by tests@tower.invalid"), "{text}");
    let board = stdout(&ff_tower(repo.path(), &["board"]));
    assert!(board.contains("the right subject"), "{board}");
}

#[test]
fn a_body_edit_rewords_the_body_alone() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the subject", "-m", "the old body"],
    ));
    stdout(&ff_tower(repo.path(), &["edit", "1", "-m", "the new body"]));

    let brief = envelope(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    let data = &brief["data"];
    assert_eq!(data["subject"], serde_json::json!("the subject"));
    assert_eq!(data["body"], serde_json::json!("the new body"));
    assert_eq!(data["edited_by"], serde_json::json!("tests@tower.invalid"));
    assert!(data["edited_at"].is_i64(), "{data}");
}

#[test]
fn a_comment_edits_by_its_printed_id() {
    let (repo, comment) = repo_with_a_comment();

    let out = stdout(&ff_tower(
        repo.path(),
        &["edit", &comment, "-m", "a note without a typo"],
    ));
    assert_eq!(
        out,
        format!("edited comment {comment} on #1\nboard: ff tower\n")
    );

    let brief = envelope(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    let row = &brief["data"]["comments"][0];
    assert_eq!(row["text"], serde_json::json!("a note without a typo"));
    assert_eq!(row["id"], serde_json::json!(comment));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains(&comment), "the header carries the id: {text}");
    assert!(text.contains("a note without a typo"), "{text}");
}

#[test]
fn a_done_flight_still_edits() {
    // The permissive stance: a wrong word in a closed record is the
    // motivating case.
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished wrogn"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    stdout(&ff_tower(
        repo.path(),
        &["edit", "1", "-s", "finished right"],
    ));
    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("#1  finished right"), "{text}");
    assert!(text.contains("done — tests@tower.invalid"), "{text}");
}

#[test]
fn the_envelope_carries_the_overlay_as_appended() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));

    let out = ff_tower(repo.path(), &["edit", "1", "-m", "the new body", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("edit"));
    let edited = &envelope["data"]["edited"];
    assert_eq!(edited["kind"], serde_json::json!("edited"));
    assert_eq!(edited["body"]["target"], serde_json::json!("pi.1"));
    assert_eq!(edited["body"]["body"], serde_json::json!("the new body"));
    let body = edited["body"].as_object().expect("a body object");
    assert!(
        !body.contains_key("subject"),
        "an unchanged field leaves no key: {body:?}"
    );
}

#[test]
fn no_flags_refuses_needs_edit() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));
    let out = ff_tower(repo.path(), &["edit", "1", "--json"]);
    refusal(&out, 2, "usage/needs-edit");
}

#[test]
fn a_subject_on_a_comment_refuses() {
    let (repo, comment) = repo_with_a_comment();
    let out = ff_tower(
        repo.path(),
        &["edit", &comment, "-s", "not a subject", "--json"],
    );
    refusal(&out, 2, "usage/subject-on-comment");
}

#[test]
fn an_empty_subject_refuses() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));
    let out = ff_tower(repo.path(), &["edit", "1", "-s", "   ", "--json"]);
    refusal(&out, 2, "usage/empty-subject");
}

#[test]
fn a_malformed_reference_refuses_bad_flight() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["edit", "not-an-id", "-s", "s", "--json"]);
    refusal(&out, 2, "usage/bad-flight");
}

#[test]
fn an_id_naming_nothing_refuses_not_found() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));
    let out = ff_tower(repo.path(), &["edit", "pi.99", "-s", "s", "--json"]);
    let envelope = refusal(&out, 1, "flight/not-found");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .expect("a message")
            .contains("neither a flight nor a comment"),
        "{envelope}"
    );
}

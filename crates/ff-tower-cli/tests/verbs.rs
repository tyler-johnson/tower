//! The write verbs, spawned the way fufu's dispatch spawns them: `FF_REPO`
//! in the environment, argv carrying the verb. file/comment/link never
//! spawn fufu, so no `TOWER_FF` appears here — a wrong `ff` on PATH could
//! not break these even if it tried.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .output()
        .expect("spawn ff-tower")
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

#[test]
fn file_echoes_the_filing_and_the_tail() {
    let repo = repo();
    let out = stdout(&ff_tower(
        repo.path(),
        &["file", "fix the flaky retry test"],
    ));
    assert_eq!(
        out,
        "filed pi.1 under open: fix the flaky retry test\nboard: ff tower\n"
    );
}

#[test]
fn file_json_round_trips_body_and_procedure() {
    let repo = repo();
    let out = ff_tower(
        repo.path(),
        &[
            "file",
            "try the verbs",
            "-m",
            "body text",
            "-p",
            "review",
            "--json",
        ],
    );
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("file"));
    let filed = &envelope["data"]["filed"];
    assert_eq!(filed["id"], serde_json::json!("pi.1"));
    assert_eq!(filed["kind"], serde_json::json!("filed"));
    assert_eq!(filed["body"]["subject"], serde_json::json!("try the verbs"));
    assert_eq!(filed["body"]["body"], serde_json::json!("body text"));
    assert_eq!(filed["body"]["procedure"], serde_json::json!("review"));
}

#[test]
fn a_filed_flight_lands_in_the_open_section() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "land on the board"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        board["data"]["open"][0]["subject"],
        serde_json::json!("land on the board")
    );
}

#[test]
fn an_empty_subject_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "   ", "--json"]);
    refusal(&out, 2, "usage/empty-subject");
}

#[test]
fn an_empty_procedure_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "a subject", "-p", "", "--json"]);
    refusal(&out, 2, "usage/empty-procedure");
}

#[test]
fn a_comment_counts_on_the_board() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    let out = stdout(&ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note"]));
    assert_eq!(out, "commented on pi.1\nboard: ff tower\n");
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"][0]["comments"], serde_json::json!(1));
}

#[test]
fn comment_json_carries_the_appended_event() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    let out = ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("comment"));
    let commented = &envelope["data"]["commented"];
    assert_eq!(commented["id"], serde_json::json!("pi.2"));
    assert_eq!(commented["body"]["flight"], serde_json::json!("pi.1"));
    assert_eq!(commented["body"]["text"], serde_json::json!("a note"));
}

#[test]
fn a_comment_on_a_missing_flight_is_refused() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["comment", "pi.99", "-m", "x", "--json"]);
    let envelope = refusal(&out, 1, "flight/not-found");
    assert_eq!(envelope["error"]["exits"], serde_json::json!(["ff tower"]));

    // The human rendering of the same refusal: stderr, with the try block.
    let out = ff_tower(repo.path(), &["comment", "pi.99", "-m", "x"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("ff-tower: "), "got {stderr:?}");
    assert!(stderr.contains("  try:\n    ff tower\n"), "got {stderr:?}");
}

#[test]
fn a_comment_without_a_note_is_refused() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["comment", "pi.1", "--json"]);
    let envelope = refusal(&out, 2, "usage/needs-message");
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower comment <flight> -m <note>"])
    );
}

#[test]
fn a_malformed_flight_id_is_refused_in_the_ids_words() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["comment", "not-an-id", "-m", "x", "--json"]);
    let envelope = refusal(&out, 2, "usage/bad-flight");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`not-an-id` is not `<writer>.<seq>`")
    );
}

#[test]
fn link_declares_the_edge_both_ways() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["file", "the dependent"]));
    let out = stdout(&ff_tower(repo.path(), &["link", "pi.2", "pi.1"]));
    assert_eq!(out, "linked pi.2: depends on pi.1\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let by_id = |id: &str| {
        open.iter()
            .find(|view| view["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no {id} in open"))
    };
    assert_eq!(by_id("pi.2")["depends_on"], serde_json::json!(["pi.1"]));
    assert_eq!(by_id("pi.1")["blocks"], serde_json::json!(["pi.2"]));
}

#[test]
fn a_self_link_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["link", "pi.1", "pi.1", "--json"]);
    refusal(&out, 2, "usage/self-link");
}

#[test]
fn a_link_to_a_missing_flight_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the only flight"]));
    let out = ff_tower(repo.path(), &["link", "pi.1", "pi.9", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn the_same_edge_twice_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["file", "the dependent"]));
    stdout(&ff_tower(repo.path(), &["link", "pi.2", "pi.1"]));
    let out = ff_tower(repo.path(), &["link", "pi.2", "pi.1", "--json"]);
    refusal(&out, 1, "link/exists");
}

#[test]
fn a_comment_shows_in_the_rendered_note_line() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    stdout(&ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note"]));
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("1 comment"), "the note line counts: {out}");
}

#[test]
fn a_missing_identity_is_a_coded_envelope() {
    let repo = repo();
    repo.git(&["config", "--unset", "user.email"]);
    let output = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(["file", "a subject", "--json"])
        .env("FF_REPO", repo.path())
        // The machine's own git config must not answer for the fixture.
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("EMAIL")
        .output()
        .expect("spawn ff-tower");
    let envelope = refusal(&output, 1, "identity/missing");
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["git config user.email <email>"])
    );
}

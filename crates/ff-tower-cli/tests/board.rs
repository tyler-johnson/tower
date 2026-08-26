//! The binary, spawned the way fufu's dispatch spawns it: `FF_REPO` in the
//! environment — the production handshake, not a test-only door.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::log::{Kind, Store};
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

/// A repository with one filed flight on its log.
fn repo_with_a_filing() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    Store::open(repo.path())
        .expect("open")
        .append(vec![Kind::Filed {
            procedure: "open".to_string(),
            subject: "write the doctor verb".to_string(),
            body: String::new(),
        }])
        .expect("append");
    repo
}

#[test]
fn json_emits_towers_envelope_and_round_trips() {
    let repo = repo_with_a_filing();
    let out = stdout(&ff_tower(repo.path(), &["--json"]));

    let envelope: serde_json::Value = serde_json::from_str(&out).expect("an envelope");
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("board"));
    let data = envelope["data"].as_object().expect("data is an object");
    for key in [
        "waiting_on_you",
        "in_the_air",
        "holding",
        "open",
        "unrouted",
    ] {
        assert!(data.contains_key(key), "data is missing `{key}`");
    }
    assert_eq!(data["open"][0]["id"], serde_json::json!("pi.1"));
}

#[test]
fn bare_and_the_board_alias_agree_byte_for_byte() {
    let repo = repo_with_a_filing();
    let bare = stdout(&ff_tower(repo.path(), &["--json"]));
    let alias = stdout(&ff_tower(repo.path(), &["board", "--json"]));
    assert_eq!(bare, alias);
}

#[test]
fn a_piped_render_is_plain_text_and_names_its_sections() {
    let repo = repo_with_a_filing();
    let out = stdout(&ff_tower(repo.path(), &[]));

    assert!(
        !out.contains('\x1b'),
        "piped output has escape bytes: {out:?}"
    );
    assert!(out.contains("open\n"), "the section header is named: {out}");
    assert!(out.contains("pi.1"));
    assert!(out.contains("write the doctor verb"));
    assert!(out.contains("1 flight · ff tower file to add one"));
}

#[test]
fn an_empty_repository_renders_the_empty_board_hint() {
    let repo = Repo::new();
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert_eq!(out, "nothing on the board · ff tower file to add one\n");
}

#[test]
fn a_missing_ff_exits_one_and_names_fufu() {
    let repo = Repo::new();
    let output = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .env("FF_REPO", repo.path())
        .env("TOWER_FF", "/nonexistent/ff")
        .output()
        .expect("spawn ff-tower");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("ff-tower: "), "got {stderr:?}");
    assert!(stderr.contains("fufu"), "the dependency is named: {stderr}");
}

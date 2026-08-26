//! `ff tower next` against real repositories: the greedy admission, the
//! peek, and the exit-1 no.

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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

#[test]
fn next_claims_the_one_open_flight_and_the_second_ask_is_the_exit_1_no() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the one flight"]));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #1: the one flight"), "{text}");
    assert!(text.contains("board: ff tower"), "{text}");

    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(board.contains("in the air"), "{board}");
    assert!(board.contains("claimed"), "{board}");

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1), "an empty pick is fufu's no");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready"), "{text}");
}

#[test]
fn peek_reads_without_claiming() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the one flight"]));

    let out = ff_tower(repo.path(), &["next", "--peek"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("ready #1: the one flight"), "{text}");
    assert!(!text.contains("claimed"), "{text}");

    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(board.contains("open"), "{board}");
    assert!(
        !board.contains("in the air"),
        "the peek wrote nothing: {board}"
    );

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    assert_eq!(
        envelope["data"]["picked"][0]["flight"],
        serde_json::json!("pi.1")
    );
}

#[test]
fn an_undone_dependency_is_passed_as_waiting_and_the_dependency_is_claimed() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the dependent"]));
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["link", "1", "2"]));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #2: the dependency"), "{text}");
    assert!(text.contains("passed #1 · waiting on #2"), "{text}");

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    let envelope = envelope(&out);
    let passed = &envelope["data"]["passed"];
    assert_eq!(passed[0]["flight"], serde_json::json!("pi.1"));
    assert_eq!(passed[0]["reason"], serde_json::json!("waiting"));
    assert_eq!(passed[0]["on"], serde_json::json!(["pi.2"]));
}

#[test]
fn deconfliction_passes_the_collider_claims_the_clear_one_and_pins_the_json() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "left work"]));
    stdout(&ff_tower(repo.path(), &["file", "right work"]));
    stdout(&ff_tower(repo.path(), &["file", "third work"]));
    stdout(&ff_tower(repo.path(), &["claim", "1"]));

    repo.ff(&["start", "-b", "left"]);
    repo.write("shared.txt", "left side\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "left: touch shared"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("shared.txt", "right side\n");
    Ff::at(repo.path())
        .session("pi.2")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "right: touch shared"]);

    // The peek first, so the JSON pins the same pick the claiming run
    // takes a line below — after the claim the pool would be different.
    let out = ff_tower(repo.path(), &["next", "-n", "2", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    let picked = envelope["data"]["picked"].as_array().expect("picked");
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0]["flight"], serde_json::json!("pi.3"));
    let passed = &envelope["data"]["passed"];
    assert_eq!(passed[0]["flight"], serde_json::json!("pi.2"));
    assert_eq!(passed[0]["reason"], serde_json::json!("collides"));
    assert_eq!(passed[0]["with"], serde_json::json!("pi.1"));
    assert_eq!(passed[0]["paths"], serde_json::json!(["shared.txt"]));

    let out = ff_tower(repo.path(), &["next", "-n", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #3: third work"), "{text}");
    assert!(
        text.contains("passed #2 · collides with #1 on shared.txt"),
        "{text}"
    );

    let board = stdout(&ff_tower(repo.path(), &["--json"]));
    let board: serde_json::Value = serde_json::from_str(&board).expect("an envelope");
    let air = board["data"]["in_the_air"].as_array().expect("in_the_air");
    let three = air
        .iter()
        .find(|view| view["id"] == serde_json::json!("pi.3"))
        .expect("pi.3 is in the air");
    assert!(three["claimed_by"].is_string(), "{three}");
}

#[test]
fn a_zero_count_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["next", "-n", "0", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("usage/bad-count")
    );
}

#[test]
fn an_empty_pick_under_json_is_a_data_envelope_not_an_error() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("next"));
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    assert!(
        envelope.get("error").is_none(),
        "data and error, never both"
    );
}

#[test]
fn a_parent_waits_on_its_parts_and_stays_undone_when_they_land() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    stdout(&ff_tower(
        repo.path(),
        &["decompose", "1", "part one", "part two"],
    ));

    let text = stdout(&ff_tower(repo.path(), &["next", "--peek"]));
    assert!(text.contains("ready #2: part one"), "{text}");
    assert!(text.contains("passed #1 · waiting on #2, #3"), "{text}");

    stdout(&ff_tower(repo.path(), &["done", "2"]));
    stdout(&ff_tower(repo.path(), &["done", "3"]));

    // Every part done makes the parent claimable, not finished — nothing
    // derives a parent's done, only `ff tower done` ends it.
    let text = stdout(&ff_tower(repo.path(), &["next", "--peek"]));
    assert!(text.contains("ready #1: a broad task"), "{text}");
    assert!(!text.contains("waiting"), "{text}");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0]["id"], serde_json::json!("pi.1"));
}

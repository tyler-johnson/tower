//! `atc register` against real repositories: the roster listed,
//! registered, rewritten, retired, and returned; the last-seen line;
//! and the four refusals.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::Repo;

fn atc(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atc"))
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        // A developer's own session and callsign must not stamp the
        // fixture's events: last seen below asserts on the fixture's.
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("ATC_CALLSIGN")
        .output()
        .expect("spawn atc")
}

/// The spawn under a callsign, the way a harness exports one.
fn atc_as(repo: &Path, args: &[&str], callsign: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atc"))
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env("ATC_CALLSIGN", callsign)
        .output()
        .expect("spawn atc")
}

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

fn roster(repo: &Path) -> Vec<serde_json::Value> {
    envelope(&atc(repo, &["register", "--json"]))["data"]["roster"]
        .as_array()
        .expect("a roster")
        .clone()
}

#[test]
fn an_empty_roster_says_how_to_add_one() {
    let repo = repo();
    let out = stdout(&atc(repo.path(), &["register"]));
    assert_eq!(
        out,
        "no callsigns registered · atc register <callsign> --kind person|agent -m \"…\"\n"
    );
    assert!(roster(repo.path()).is_empty());
}

#[test]
fn register_echoes_and_lists_with_last_seen() {
    let repo = repo();
    let out = stdout(&atc(
        repo.path(),
        &[
            "register",
            "claude",
            "--kind",
            "agent",
            "-m",
            "Claude Code sessions",
        ],
    ));
    assert_eq!(
        out,
        "registered claude · agent: Claude Code sessions\nboard: atc\n"
    );
    let out = stdout(&atc(
        repo.path(),
        &["register", "tyler", "--kind", "person"],
    ));
    assert_eq!(out, "registered tyler · person\nboard: atc\n");

    // Never flown yet: `never`. One event under the callsign, and the
    // roster says when.
    let out = stdout(&atc(repo.path(), &["register"]));
    assert_eq!(
        out,
        "claude  agent   never  Claude Code sessions\ntyler   person  never\n"
    );
    stdout(&atc(repo.path(), &["file", "flown work"]));
    stdout(&atc_as(
        repo.path(),
        &["status", "1", "in_progress"],
        "claude",
    ));
    let out = stdout(&atc(repo.path(), &["register"]));
    assert!(
        out.starts_with("claude  agent   seen 0s ago  Claude Code sessions\n"),
        "{out}"
    );
    assert!(out.ends_with("tyler   person  never\n"), "{out}");

    let pilots = roster(repo.path());
    assert_eq!(pilots.len(), 2);
    assert_eq!(pilots[0]["callsign"], serde_json::json!("claude"));
    assert_eq!(pilots[0]["kind"], serde_json::json!("agent"));
    assert_eq!(
        pilots[0]["description"],
        serde_json::json!("Claude Code sessions")
    );
    assert_eq!(pilots[0]["by"], serde_json::json!("tests@tower.invalid"));
    assert!(pilots[0]["registered_at"].is_number());
    assert!(pilots[0]["last_seen"].is_number(), "{pilots:?}");
    assert!(pilots[1]["last_seen"].is_null(), "{pilots:?}");
    assert_eq!(pilots[1]["description"], serde_json::json!(""));
}

#[test]
fn register_json_carries_the_appended_event() {
    let repo = repo();
    let out = atc(
        repo.path(),
        &["register", "claude", "--kind", "agent", "--json"],
    );
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("register"));
    let data = &envelope["data"];
    assert_eq!(data["callsign"], serde_json::json!("claude"));
    assert_eq!(data["kind"], serde_json::json!("agent"));
    assert_eq!(data["description"], serde_json::json!(""));
    assert_eq!(data["event"]["kind"], serde_json::json!("registered"));
    assert_eq!(data["event"]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        data["event"]["body"],
        serde_json::json!({"callsign": "claude", "kind": "agent", "description": ""})
    );
}

#[test]
fn a_second_registration_rewrites_in_place_and_retire_takes_it_off() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["register", "claude", "--kind", "agent"],
    ));
    stdout(&atc(
        repo.path(),
        &["register", "qwen-review", "--kind", "agent"],
    ));
    stdout(&atc(
        repo.path(),
        &["register", "claude", "--kind", "person", "-m", "rewritten"],
    ));
    let pilots = roster(repo.path());
    assert_eq!(pilots[0]["callsign"], serde_json::json!("claude"));
    assert_eq!(pilots[0]["kind"], serde_json::json!("person"));
    assert_eq!(pilots[0]["description"], serde_json::json!("rewritten"));

    let out = stdout(&atc(repo.path(), &["register", "-d", "qwen-review"]));
    assert_eq!(out, "retired qwen-review\nboard: atc\n");
    let names: Vec<String> = roster(repo.path())
        .iter()
        .map(|pilot| pilot["callsign"].as_str().expect("callsign").to_string())
        .collect();
    assert_eq!(names, ["claude"]);

    let out = atc(repo.path(), &["register", "--retire", "claude", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["data"]["callsign"], serde_json::json!("claude"));
    assert_eq!(
        envelope["data"]["event"]["kind"],
        serde_json::json!("unregistered")
    );
    assert!(roster(repo.path()).is_empty());

    // A later registration brings it back.
    stdout(&atc(
        repo.path(),
        &["register", "claude", "--kind", "agent"],
    ));
    assert_eq!(roster(repo.path()).len(), 1);
}

#[test]
fn a_retired_callsign_still_lanes_and_still_stamps() {
    // The roster is a list, never a permission: assigning to a retired
    // callsign stores it, and its events still carry it.
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["register", "qwen-review", "--kind", "agent"],
    ));
    stdout(&atc(repo.path(), &["register", "-d", "qwen-review"]));
    stdout(&atc(repo.path(), &["file", "still routed"]));
    stdout(&atc(repo.path(), &["assign", "1", "qwen-review"]));
    stdout(&atc_as(repo.path(), &["next"], "qwen-review"));
    let board = envelope(&atc(repo.path(), &["--json"]));
    let flown = &board["data"]["in_progress"][0];
    assert_eq!(flown["assignee"], serde_json::json!("qwen-review"));
    assert_eq!(flown["status_callsign"], serde_json::json!("qwen-review"));
}

#[test]
fn the_four_refusals() {
    let repo = repo();
    let out = atc(
        repo.path(),
        &["register", "two words", "--kind", "agent", "--json"],
    );
    let envelope = refusal(&out, 2, "usage/bad-callsign");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!(
            "`two words` is not a callsign — one word, no spaces, at most 64 bytes, and not me, agent, or none"
        )
    );
    for lane in ["me", "agent", "none"] {
        let out = atc(
            repo.path(),
            &["register", lane, "--kind", "agent", "--json"],
        );
        refusal(&out, 2, "usage/bad-callsign");
    }

    let out = atc(
        repo.path(),
        &["register", "claude", "--kind", "bot", "--json"],
    );
    let envelope = refusal(&out, 2, "usage/bad-kind");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`bot` is not a pilot kind — person or agent")
    );

    let out = atc(repo.path(), &["register", "claude", "--json"]);
    let envelope = refusal(&out, 2, "usage/needs-kind");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("registering a callsign needs --kind person or agent")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["atc register <callsign> --kind <kind>"])
    );

    let out = atc(repo.path(), &["register", "-d", "nobody", "--json"]);
    let envelope = refusal(&out, 1, "callsign/not-found");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`nobody` is not on the roster")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["atc register"])
    );

    // The human form of one, for the try block.
    let out = atc(repo.path(), &["register", "claude"]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("atc: registering a callsign needs --kind person or agent"),
        "{err}"
    );
    assert!(roster(repo.path()).is_empty(), "nothing landed");
}

#[test]
fn an_unusable_variable_is_ignored_not_fatal() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "quiet"]));
    let out = atc_as(repo.path(), &["status", "1", "in_progress"], "two words");
    stdout(&out);
    let board = envelope(&atc(repo.path(), &["--json"]));
    assert!(
        board["data"]["in_progress"][0]["status_callsign"].is_null(),
        "{board}"
    );
}

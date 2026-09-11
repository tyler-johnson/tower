//! `atc briefing` against real repositories: the empty board, the
//! ready count, and the failure outside a repository.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::Repo;

fn atc(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atc"))
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .env_remove("CLAUDE_CODE_SESSION_ID")
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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

/// The human line, held to fufu's rules: one line, under the cap.
fn line(repo: &Repo) -> String {
    let text = stdout(&atc(repo.path(), &["briefing"]));
    assert_eq!(text.lines().count(), 1, "{text}");
    let line = text.trim().to_string();
    assert!(line.chars().count() <= 240, "{line}");
    line
}

#[test]
fn an_empty_board_has_nothing_ready() {
    let repo = repo();
    let line = line(&repo);
    assert!(line.contains("nothing ready"), "{line}");

    let out = atc(repo.path(), &["briefing", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("an envelope");
    assert_eq!(v["atc"], 1);
    assert_eq!(v["cmd"], "briefing");
    assert_eq!(v["data"]["line"], line);
}

#[test]
fn ready_flights_are_counted() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "one", "--status", "ready"]));
    assert!(line(&repo).contains("1 flight ready"), "{}", line(&repo));
    stdout(&atc(repo.path(), &["file", "two", "--status", "ready"]));
    assert!(line(&repo).contains("2 flights ready"), "{}", line(&repo));
}

#[test]
fn outside_a_repository_the_failure_is_the_ordinary_one() {
    let dir = tempfile::TempDir::new().unwrap();
    let spawn = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_atc"))
            .args(args)
            .current_dir(dir.path())
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env("ATC_FF", "/nonexistent")
            .env("HOME", dir.path())
            .output()
            .expect("spawn atc")
    };
    let out = spawn(&["briefing"]);
    assert!(!out.status.success());
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!out.stderr.is_empty());

    let out = spawn(&["briefing", "--json"]);
    assert!(!out.status.success());
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("an envelope");
    assert_eq!(v["cmd"], "briefing");
    assert!(
        v["error"]["id"].as_str().unwrap().contains('/'),
        "a coded id: {v}"
    );
}

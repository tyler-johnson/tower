//! `atc whoami` against real repositories: the three lines and the JSON
//! under a terminal's variables, under an agent's beside them, under a
//! launcher's; bare `atc callsign` printing the same text under its own
//! `cmd`; and the ordinary refusal outside a repository.
//!
//! Every spawn gets a scratch HOME, so leases land under the fixture's
//! `.local/state` and never under the developer's.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::{Repo, scrub};

fn root(repo: &Path) -> &Path {
    repo.parent().expect("the fixture nests the repository")
}

fn command(cwd: &Path, home: &Path, vars: &[(&str, &str)], args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    command
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env_remove("GIT_CONFIG_GLOBAL")
        .env("ATC_FF", "/nonexistent");
    scrub(&mut command);
    for (name, value) in vars {
        command.env(name, value);
    }
    command
}

fn atc(repo: &Path, vars: &[(&str, &str)], args: &[&str]) -> Output {
    command(repo, root(repo), vars, args)
        .output()
        .expect("spawn atc")
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

/// This test process's own pid: alive for as long as the test runs, on
/// the target that can read it.
fn own_pid() -> Option<u32> {
    cfg!(target_os = "linux").then(std::process::id)
}

/// Under the terminal's variables: source `shell`, the shell's pid, the
/// login name as the callsign or none, the lease this call created, and
/// the writer line last.
#[test]
fn a_terminal_is_a_session_with_source_shell() {
    let repo = repo();
    let path = repo.path();
    let pid = std::process::id().to_string();
    let terminal: [(&str, &str); 2] = [("ATC_SHELL_SESSION", "t1"), ("ATC_SHELL_PID", &pid)];

    let text = stdout(&atc(path, &terminal, &["whoami"]));
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert!(
        first.starts_with("callsign ") && first.ends_with("(ATC_SHELL_SESSION t1)"),
        "{first}"
    );
    let lease = lines.next().unwrap();
    assert!(lease.starts_with("lease fresh, renewed "), "{lease}");
    match own_pid() {
        Some(pid) => assert!(lease.ends_with(&format!(", pid {pid} alive")), "{lease}"),
        None => assert!(lease.ends_with(", no pid"), "{lease}"),
    }
    assert_eq!(lines.next(), Some("writer pi · author tests@tower.invalid"));
    assert_eq!(lines.next(), None);

    let out = atc(path, &terminal, &["whoami", "--json"]);
    let v = envelope(&out);
    assert_eq!(v["cmd"], "whoami");
    let data = &v["data"];
    assert_eq!(data["writer"], "pi");
    assert_eq!(data["author"], "tests@tower.invalid");
    assert_eq!(data["client"], serde_json::Value::Null);
    assert_eq!(data["session"], "t1");
    assert_eq!(data["session_source"], "shell");
    assert_eq!(data["lease"]["fresh"], true);
    assert_eq!(data["pid"], serde_json::json!(own_pid()));
    let mut keys: Vec<&str> = data
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "author",
            "callsign",
            "callsign_source",
            "client",
            "lease",
            "pid",
            "session",
            "session_source",
            "writer"
        ]
    );
    assert!(
        root(path).join(".local/state/atc/leases/t1").is_file(),
        "the read is a heartbeat"
    );
}

/// An agent under a wired terminal is its own session: Claude's row is
/// read before the terminal's, and the pid is `CLAUDE_PID`, never the
/// shell's. A launcher's `ATC_SESSION` beats both.
#[test]
fn an_agent_under_a_terminal_is_its_own_session() {
    let repo = repo();
    let path = repo.path();
    let pid = std::process::id().to_string();

    let out = atc(
        path,
        &[
            ("ATC_SHELL_SESSION", "t1"),
            ("ATC_SHELL_PID", "1"),
            ("CLAUDE_CODE_SESSION_ID", "c1"),
            ("CLAUDE_PID", &pid),
            ("CLAUDECODE", "1"),
        ],
        &["whoami", "--json"],
    );
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["session"], "c1", "{data}");
    assert_eq!(data["session_source"], "claude");
    assert_eq!(data["client"], "claude");
    assert_eq!(data["callsign"], "claude");
    assert_eq!(data["callsign_source"], "client");
    assert_eq!(
        data["pid"],
        serde_json::json!(own_pid()),
        "CLAUDE_PID: {data}"
    );
    assert!(root(path).join(".local/state/atc/leases/c1").is_file());
    assert!(
        !root(path).join(".local/state/atc/leases/t1").exists(),
        "the terminal's row was not read"
    );
    let text = stdout(&atc(
        path,
        &[
            ("ATC_SHELL_SESSION", "t1"),
            ("CLAUDE_CODE_SESSION_ID", "c1"),
            ("CLAUDECODE", "1"),
        ],
        &["whoami"],
    ));
    assert!(
        text.starts_with("callsign claude — client (CLAUDE_CODE_SESSION_ID c1)\n"),
        "{text}"
    );
    assert!(
        text.ends_with("writer pi · author tests@tower.invalid · client claude\n"),
        "{text}"
    );

    let out = atc(
        path,
        &[
            ("ATC_SESSION", "w1"),
            ("ATC_SHELL_SESSION", "t1"),
            ("CLAUDE_CODE_SESSION_ID", "c1"),
        ],
        &["whoami", "--json"],
    );
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["session"], "w1");
    assert_eq!(data["session_source"], "launcher");
}

/// Bare `atc callsign` prints what `atc whoami` prints, under its own
/// envelope `cmd`.
#[test]
fn bare_callsign_prints_the_same_text() {
    let repo = repo();
    let path = repo.path();
    let terminal = [("ATC_SHELL_SESSION", "t1")];
    stdout(&atc(path, &terminal, &["callsign", "alpha"]));

    let whoami = stdout(&atc(path, &terminal, &["whoami"]));
    let callsign = stdout(&atc(path, &terminal, &["callsign"]));
    assert_eq!(whoami, callsign);
    assert!(
        whoami.starts_with("callsign alpha — session (ATC_SHELL_SESSION t1)\n"),
        "{whoami}"
    );

    let whoami = envelope(&atc(path, &terminal, &["whoami", "--json"]));
    let callsign = envelope(&atc(path, &terminal, &["callsign", "--json"]));
    assert_eq!(whoami["cmd"], "whoami");
    assert_eq!(callsign["cmd"], "callsign");
    assert_eq!(whoami["data"], callsign["data"]);

    // No session at all: the callsign line says so, and the writer line
    // still prints.
    let text = stdout(&atc(path, &[], &["whoami"]));
    assert_eq!(
        text,
        "callsign none · no session\nwriter pi · author tests@tower.invalid\n"
    );
}

/// Outside a repository the ordinary refusal: the store will not open.
#[test]
fn outside_a_repository_is_the_ordinary_refusal() {
    let elsewhere = tempfile::TempDir::new().unwrap();
    let home = tempfile::TempDir::new().unwrap();
    let out = command(elsewhere.path(), home.path(), &[], &["whoami", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v = envelope(&out);
    assert_eq!(v["cmd"], "whoami");
    assert!(
        v["error"]["id"].as_str().unwrap().starts_with("repo/"),
        "{v}"
    );
}

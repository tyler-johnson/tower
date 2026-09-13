//! `atc whoami` against real repositories: the three lines and the JSON
//! under a terminal's variables, under an agent's beside them, under a
//! launcher's; bare `atc callsign` printing the same text under its own
//! `cmd`; and the ordinary refusal outside a repository.
//!
//! Every spawn gets a scratch HOME, so leases land under the fixture's
//! `.local/state` and never under the developer's.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::{Repo, as_reported, scrub};

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

/// Copilot's shell tool gets its session from the client, ahead of the inherited terminal's row, and no inherited pid.
#[test]
fn copilot_has_its_own_session_and_callsign_under_a_terminal() {
    let repo = repo();
    let out = atc(
        repo.path(),
        &[
            ("COPILOT_CLI", "1"),
            ("COPILOT_AGENT_SESSION_ID", "copilot-1"),
            ("ATC_SHELL_SESSION", "terminal-1"),
            ("ATC_SHELL_PID", "1"),
        ],
        &["whoami", "--json"],
    );
    stdout(&out);
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["session"], "copilot-1");
    assert_eq!(data["session_source"], "copilot");
    assert_eq!(data["client"], "copilot");
    assert_eq!(data["callsign"], "copilot");
    assert_eq!(data["pid"], serde_json::Value::Null);
    assert!(
        root(repo.path())
            .join(".local/state/atc/leases/copilot-1")
            .exists()
    );
    assert!(
        !root(repo.path())
            .join(".local/state/atc/leases/terminal-1")
            .exists()
    );
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
    let basename = path.file_name().unwrap().to_string_lossy();
    assert_eq!(
        lines.next(),
        Some(format!("repos {basename}").as_str()),
        "the repository this read ran in, by basename"
    );
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
    assert_eq!(
        data["repos"],
        serde_json::json!([as_reported(path).display().to_string()]),
        "the full root: {data}"
    );
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
            "repos",
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

/// Qwen Code's session variable is a row of its own, under its marker:
/// source `qwen`, no pid, and the client's word as the callsign.
#[test]
fn a_qwen_session_is_its_own_row() {
    let repo = repo();
    let path = repo.path();
    let out = atc(
        path,
        &[("QWEN_CODE_SESSION_ID", "q1"), ("QWEN_CODE", "1")],
        &["whoami", "--json"],
    );
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["session"], "q1", "{data}");
    assert_eq!(data["session_source"], "qwen");
    assert_eq!(data["client"], "qwen");
    assert_eq!(data["callsign"], "qwen");
    assert_eq!(data["callsign_source"], "client");
    assert_eq!(data["pid"], serde_json::Value::Null, "{data}");
    assert!(root(path).join(".local/state/atc/leases/q1").is_file());
}

/// OpenCode's session variable is a row of its own: tower's plugin sets
/// `OPENCODE_SESSION_ID` on every shell command, and that one variable
/// is the session row and a marker — source `opencode`, client
/// `opencode`, and the pid from `OPENCODE_PID`, the shell tool's own,
/// when it is there. The shell tool's own `OPENCODE=1` is the marker
/// without a session.
#[test]
fn an_opencode_session_is_its_own_row() {
    let repo = repo();
    let path = repo.path();
    let out = atc(path, &[("OPENCODE", "1")], &["whoami", "--json"]);
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["client"], "opencode", "{data}");
    assert_eq!(data["callsign"], "opencode", "{data}");
    assert_eq!(data["session"], serde_json::Value::Null, "{data}");

    let out = atc(
        path,
        &[("OPENCODE_SESSION_ID", "o1")],
        &["whoami", "--json"],
    );
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["session"], "o1", "{data}");
    assert_eq!(data["session_source"], "opencode");
    assert_eq!(data["client"], "opencode");
    assert_eq!(data["callsign"], "opencode");
    assert_eq!(data["callsign_source"], "client");
    assert_eq!(data["pid"], serde_json::Value::Null, "{data}");
    assert!(root(path).join(".local/state/atc/leases/o1").is_file());

    let pid = std::process::id().to_string();
    let out = atc(
        path,
        &[
            ("OPENCODE_SESSION_ID", "o2"),
            ("OPENCODE_PID", &pid),
            ("OPENCODE", "1"),
        ],
        &["whoami", "--json"],
    );
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["session"], "o2", "{data}");
    assert_eq!(
        data["pid"],
        serde_json::json!(own_pid()),
        "OPENCODE_PID: {data}"
    );
    assert_eq!(
        data["lease"]["pid_alive"],
        serde_json::json!(own_pid().map(|_| true)),
        "{data}"
    );
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

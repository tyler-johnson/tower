//! `atc callsign` against real repositories: the lease under
//! `ATC_SESSION`, the move, the bare read, the refusals, the hold
//! between two sessions, the drop line, the window setting, and the
//! heartbeat every verb is.
//!
//! Every spawn gets a scratch HOME, so leases land under the fixture's
//! `.local/state` and never under the developer's. The pid cases run
//! where `/proc` is: elsewhere no pid is stored and the window rules.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

use atc_testsupport::{Repo, scrub};

fn root(repo: &Path) -> &Path {
    repo.parent().expect("the fixture nests the repository")
}

/// The binary at the fixture, under the session and pid given.
fn command(repo: &Path, session: Option<&str>, pid: Option<u32>, args: &[&str]) -> Command {
    let home = root(repo);
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    command
        .args(args)
        .current_dir(repo)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env_remove("GIT_CONFIG_GLOBAL")
        .env("ATC_FF", "/nonexistent");
    scrub(&mut command);
    if let Some(session) = session {
        command.env("ATC_SESSION", session);
    }
    if let Some(pid) = pid {
        command.env("ATC_PID", pid.to_string());
    }
    command
}

fn atc(repo: &Path, session: Option<&str>, pid: Option<u32>, args: &[&str]) -> Output {
    command(repo, session, pid, args)
        .output()
        .expect("spawn atc")
}

/// The same spawn with one more variable set — a client's mark, the
/// launcher's callsign.
fn atc_with(repo: &Path, session: Option<&str>, var: (&str, &str), args: &[&str]) -> Output {
    command(repo, session, None, args)
        .env(var.0, var.1)
        .output()
        .expect("spawn atc")
}

/// This test process's own pid: alive for as long as the test runs.
fn own_pid() -> Option<u32> {
    cfg!(target_os = "linux").then(std::process::id)
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

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
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
    assert_eq!(envelope["error"]["id"], serde_json::json!(id), "{envelope}");
    envelope
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

fn lease(repo: &Path, session: &str) -> PathBuf {
    root(repo).join(".local/state/atc/leases").join(session)
}

fn lease_body(repo: &Path, session: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(lease(repo, session)).expect("a lease file");
    serde_json::from_str(&text).expect("a lease body")
}

/// Write a lease by hand: the word, an optional pid with a start time,
/// and an mtime `age` seconds into the past.
fn plant(repo: &Path, session: &str, word: &str, pid: Option<(u32, u64)>, age: u64) {
    let path = lease(repo, session);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let body = serde_json::json!({
        "session": session,
        "client": null,
        "pid": pid.map(|(pid, _)| pid),
        "pid_start": pid.map(|(_, start)| start),
        "callsign": word,
    });
    std::fs::write(&path, body.to_string()).unwrap();
    let then = SystemTime::now() - Duration::from_secs(age);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(then)
        .unwrap();
}

fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// The board's lanes by number, from the JSON envelope.
fn lanes(repo: &Path) -> Vec<(u64, Option<String>)> {
    let board = envelope(&atc(repo, None, None, &["--json"]));
    let mut rows = Vec::new();
    for key in ["ready", "in_progress"] {
        for row in board["data"][key].as_array().into_iter().flatten() {
            rows.push((
                row["number"].as_u64().unwrap(),
                row["assignee"].as_str().map(str::to_string),
            ));
        }
    }
    rows.sort();
    rows
}

// ---- the word and the move -------------------------------------------------

/// `callsign alpha` then `assign me` stores `alpha`; `callsign beta`
/// re-lanes that flight and reports it, and the three that stay — laned
/// by another session, by a shell with no session, and one closed —
/// stay, the first two counted as left.
#[test]
fn the_word_is_stored_by_me_and_follows_the_session_when_it_changes() {
    let repo = repo();
    let path = repo.path();
    for subject in ["one", "two", "three", "four"] {
        stdout(&atc(path, None, None, &["file", subject]));
    }
    let s1 = Some("s1");
    let pid = own_pid();

    let text = stdout(&atc(path, s1, pid, &["callsign", "alpha"]));
    assert_eq!(text, "callsign alpha\nboard: atc\n");
    stdout(&atc(path, s1, pid, &["assign", "1", "me"]));
    stdout(&atc(path, s1, pid, &["assign", "2", "alpha"]));
    stdout(&atc(path, Some("other"), None, &["assign", "3", "alpha"]));
    stdout(&atc(path, None, None, &["assign", "4", "alpha"]));
    stdout(&atc(path, s1, pid, &["done", "2"]));
    assert_eq!(
        lanes(path),
        [
            (1, Some("alpha".into())),
            (3, Some("alpha".into())),
            (4, Some("alpha".into())),
        ]
    );

    let out = atc(path, s1, pid, &["callsign", "beta", "--json"]);
    let data = envelope(&out)["data"].clone();
    assert!(out.status.success(), "{data}");
    assert_eq!(data["callsign"], "beta");
    assert_eq!(data["previous"], "alpha");
    assert_eq!(data["source"], "session");
    assert_eq!(data["session"], "s1");
    assert_eq!(data["session_source"], "launcher");
    assert_eq!(data["renewed"], false);
    assert!(data.get("took").is_none(), "{data}");
    assert_eq!(data["lease"]["fresh"], true);
    let moved = data["moved"].as_array().unwrap();
    assert_eq!(moved.len(), 1, "{data}");
    assert_eq!(moved[0]["number"], 1);
    assert_eq!(moved[0]["subject"], "one");
    let left = data["left"].as_array().unwrap();
    assert_eq!(left.len(), 2, "{data}");
    assert_eq!(left[0]["number"], 3);
    assert_eq!(left[0]["by"], "other");
    assert_eq!(left[1]["number"], 4);
    assert_eq!(left[1]["by"], "tests@tower.invalid");
    assert_eq!(
        lanes(path),
        [
            (1, Some("beta".into())),
            (3, Some("alpha".into())),
            (4, Some("alpha".into())),
        ]
    );
    // The closed flight kept its lane and was not counted.
    let brief = envelope(&atc(path, None, None, &["brief", "2", "--json"]));
    assert_eq!(brief["data"]["assignee"], "alpha");

    // The move is one append, byline the new word, session underneath.
    let history = brief_history(path, "1");
    let last = history.last().unwrap();
    assert_eq!(last["what"], "assigned", "{last}");
    assert_eq!(last["callsign"], "beta", "{last}");
    assert_eq!(last["session"], "s1", "{last}");

    // The human render, on a second change.
    stdout(&atc(path, s1, pid, &["assign", "3", "beta"]));
    let text = stdout(&atc(path, s1, pid, &["callsign", "gamma"]));
    assert_eq!(
        text,
        "callsign gamma (was beta, this session)\n\
         re-laned #1 to gamma: one\n\
         re-laned #3 to gamma: three\n\
         board: atc\n"
    );
    let text = stdout(&atc(path, s1, pid, &["callsign", "delta"]));
    assert!(
        text.starts_with("callsign delta (was gamma, this session)\n"),
        "{text}"
    );
    stdout(&atc(path, None, None, &["assign", "1", "delta"]));
    let text = stdout(&atc(path, s1, pid, &["callsign", "epsilon"]));
    assert_eq!(
        text,
        "callsign epsilon (was delta, this session)\n\
         re-laned #3 to epsilon: three\n\
         left in delta, assigned by tests@tower.invalid: #1 one\n\
         board: atc\n"
    );
}

fn brief_history(repo: &Path, flight: &str) -> Vec<serde_json::Value> {
    envelope(&atc(repo, None, None, &["brief", flight, "--json"]))["data"]["history"]
        .as_array()
        .cloned()
        .expect("a history")
}

/// A flight assigned `claude` by a session with no lease word does not
/// move on that session's first `atc callsign`: the client word was
/// never a lease, and the report says where the word came from.
#[test]
fn the_first_callsign_moves_nothing_from_a_client_word() {
    let repo = repo();
    let path = repo.path();
    stdout(&atc(path, None, None, &["file", "one"]));
    let claude = ("CLAUDECODE", "1");
    stdout(&atc_with(path, Some("c1"), claude, &["assign", "1", "me"]));
    assert_eq!(lanes(path), [(1, Some("claude".into()))]);

    let out = atc_with(path, Some("c1"), claude, &["callsign", "named", "--json"]);
    stdout(&out);
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["previous"], "claude");
    assert_eq!(data["moved"], serde_json::json!([]));
    assert_eq!(data["left"], serde_json::json!([]));
    assert_eq!(lanes(path), [(1, Some("claude".into()))]);

    assert_eq!(
        stdout(&atc_with(path, Some("c2"), claude, &["callsign", "other"])),
        "callsign other (was claude, the client)\nboard: atc\n"
    );
    // Once the word is the session's, the client word underneath stays
    // in the lease.
    assert_eq!(lease_body(path, "c2")["client"], "claude");
}

/// The same word again is a renewal that says so, and moves nothing.
#[test]
fn the_same_word_is_a_renewal() {
    let repo = repo();
    let path = repo.path();
    let s1 = Some("s1");
    stdout(&atc(path, s1, None, &["callsign", "alpha"]));
    let out = atc(path, s1, None, &["callsign", "alpha", "--json"]);
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["renewed"], true, "{data}");
    assert_eq!(data["previous"], "alpha");
    assert_eq!(
        stdout(&atc(path, s1, None, &["callsign", "alpha"])),
        "callsign alpha (renewed)\nboard: atc\n"
    );
}

// ---- the bare read ---------------------------------------------------------

/// Bare `atc callsign` is the identity read: the word with source
/// `session`, the session and its variable, the lease fresh, the pid.
#[test]
fn bare_callsign_reports_the_identity() {
    let repo = repo();
    let path = repo.path();
    let pid = own_pid();
    stdout(&atc(path, Some("s1"), pid, &["callsign", "beta"]));

    let text = stdout(&atc(path, Some("s1"), pid, &["callsign"]));
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("callsign beta — session (ATC_SESSION s1)")
    );
    let lease_line = lines.next().unwrap();
    assert!(
        lease_line.starts_with("lease fresh, renewed "),
        "{lease_line}"
    );
    match pid {
        Some(pid) => assert!(
            lease_line.ends_with(&format!(", pid {pid} alive")),
            "{lease_line}"
        ),
        None => assert!(lease_line.ends_with(", no pid"), "{lease_line}"),
    }
    assert_eq!(lines.next(), None);

    let out = atc(path, Some("s1"), pid, &["callsign", "--json"]);
    let v = envelope(&out);
    assert_eq!(v["cmd"], "callsign");
    let data = &v["data"];
    assert_eq!(data["writer"], "pi");
    assert_eq!(data["author"], "tests@tower.invalid");
    assert_eq!(data["client"], serde_json::Value::Null);
    assert_eq!(data["session"], "s1");
    assert_eq!(data["session_source"], "launcher");
    assert_eq!(data["callsign"], "beta");
    assert_eq!(data["callsign_source"], "session");
    assert_eq!(data["lease"]["fresh"], true);
    assert!(data["lease"]["age"].is_u64(), "{data}");
    assert_eq!(data["pid"], serde_json::json!(pid));

    // The other sources: the client, the launcher's variable, none.
    let text = stdout(&atc_with(
        path,
        Some("s2"),
        ("CLAUDECODE", "1"),
        &["callsign"],
    ));
    assert!(
        text.starts_with("callsign claude — client (ATC_SESSION s2)\n"),
        "{text}"
    );
    let out = atc_with(
        path,
        Some("s2"),
        ("ATC_CALLSIGN", "launched"),
        &["callsign", "--json"],
    );
    stdout(&out);
    let data = envelope(&out)["data"].clone();
    assert_eq!(data["callsign"], "launched");
    assert_eq!(data["callsign_source"], "env");
    assert_eq!(data["session_source"], "launcher");
    let text = stdout(&atc(path, None, None, &["callsign"]));
    assert_eq!(text, "callsign none · no session\n");
    let data = envelope(&atc(path, None, None, &["callsign", "--json"]))["data"].clone();
    assert_eq!(data["callsign"], serde_json::Value::Null);
    assert_eq!(data["session"], serde_json::Value::Null);
    assert_eq!(data["lease"], serde_json::Value::Null);
}

// ---- the refusals ----------------------------------------------------------

/// Each refusal by its id: no session, the launcher's variable set, a
/// lane word, a client's word.
#[test]
fn the_four_refusals() {
    let repo = repo();
    let path = repo.path();

    let out = atc(path, None, None, &["callsign", "y", "--json"]);
    let v = refusal(&out, 1, "callsign/no-session");
    assert_eq!(
        v["error"]["exits"],
        serde_json::json!(["ATC_CALLSIGN=<name> atc <verb>"])
    );
    assert!(!lease(path, "y").exists());

    let out = atc_with(
        path,
        Some("s1"),
        ("ATC_CALLSIGN", "x"),
        &["callsign", "y", "--json"],
    );
    let v = refusal(&out, 1, "callsign/env-set");
    assert_eq!(
        v["error"]["message"],
        "ATC_CALLSIGN is x — the launcher's word wins, and a lease under it would never be read"
    );
    // The lease exists — every session that touched tower has one —
    // and holds no word.
    assert_eq!(lease_body(path, "s1")["callsign"], serde_json::Value::Null);

    for word in ["me", "agent", "none", "two words", ""] {
        let out = atc(path, Some("s1"), None, &["callsign", word, "--json"]);
        refusal(&out, 2, "usage/bad-callsign");
    }
    let out = atc(path, Some("s1"), None, &["callsign", "me"]);
    assert_eq!(
        stderr(&out),
        "atc: `me` is not a callsign — one word, no spaces, at most 64 bytes, and not me, agent, or none\n  try:\n    atc callsign\n"
    );

    for word in ["claude", "codex", "cursor", "gemini"] {
        let out = atc(path, Some("s1"), None, &["callsign", word, "--json"]);
        let v = refusal(&out, 1, "callsign/client-word");
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains(&format!("`{word}` is a client's word")),
            "{v}"
        );
    }
    assert_eq!(lease_body(path, "s1")["callsign"], serde_json::Value::Null);
}

// ---- the hold --------------------------------------------------------------

/// A live pid holds the word: refused with and without `--force`,
/// naming the session and the pid. A dead pid is swept and the word
/// taken, the holder's lease gone.
#[cfg(target_os = "linux")]
#[test]
fn a_live_pid_holds_the_word_and_a_dead_one_does_not() {
    let repo = repo();
    let path = repo.path();
    let mut sleeper = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a process to be alive");
    let live = sleeper.id();
    stdout(&atc(path, Some("s1"), Some(live), &["callsign", "beta"]));
    assert_eq!(lease_body(path, "s1")["pid"], live);

    let out = atc(path, Some("s2"), None, &["callsign", "beta", "--json"]);
    let v = refusal(&out, 1, "callsign/held");
    assert_eq!(
        v["error"]["message"],
        format!("`beta` is flying: session s1, pid {live}")
    );
    assert_eq!(
        v["error"]["exits"],
        serde_json::json!(["atc callsign <name> --force", "atc callsign"])
    );
    let out = atc(
        path,
        Some("s2"),
        None,
        &["callsign", "beta", "--force", "--json"],
    );
    let v = refusal(&out, 1, "callsign/live");
    assert_eq!(
        v["error"]["message"],
        format!(
            "`beta` is flying: session s1, pid {live} — a running session keeps its word, --force or not"
        )
    );
    assert!(lease(path, "s1").is_file(), "a refusal removes nothing");
    assert_eq!(lease_body(path, "s2")["callsign"], serde_json::Value::Null);

    // The pid dies: swept on the next callsign, and the word is free.
    sleeper.kill().unwrap();
    sleeper.wait().unwrap();
    let out = atc(path, Some("s2"), None, &["callsign", "beta", "--json"]);
    let data = envelope(&out)["data"].clone();
    assert!(out.status.success(), "{data}");
    assert!(
        data.get("took").is_none(),
        "a swept lease is not a take: {data}"
    );
    assert!(
        !lease(path, "s1").exists(),
        "the dead holder's lease is gone"
    );
    assert_eq!(lease_body(path, "s2")["callsign"], "beta");

    // A reused pid — same number, another start time — is dead too.
    plant(path, "s3", "gamma", Some((std::process::id(), 1)), 0);
    let out = atc(path, Some("s2"), None, &["callsign", "gamma", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!lease(path, "s3").exists());
}

/// With no pid the window rules: a holder older than the window is
/// taken, a younger one refused with its staleness, and `--force` takes
/// the younger one naming the session.
#[test]
fn without_a_pid_the_window_rules_and_force_takes_a_fresh_holder() {
    let repo = repo();
    let path = repo.path();

    plant(path, "s1", "beta", None, 400);
    let out = atc(path, Some("s2"), None, &["callsign", "beta", "--json"]);
    let data = envelope(&out)["data"].clone();
    assert!(out.status.success(), "{data}");
    assert_eq!(data["took"]["session"], "s1");
    assert_eq!(data["took"]["detail"], "stale 6m");
    assert!(!lease(path, "s1").exists());

    plant(path, "s1", "gamma", None, 40);
    let out = atc(path, Some("s2"), None, &["callsign", "gamma", "--json"]);
    let v = refusal(&out, 1, "callsign/held");
    assert_eq!(
        v["error"]["message"],
        "`gamma` is flying: session s1, stale 40s"
    );
    assert!(lease(path, "s1").is_file());
    assert_eq!(
        lease_body(path, "s2")["callsign"],
        "beta",
        "s2 keeps its word"
    );

    let text = stdout(&atc(
        path,
        Some("s2"),
        None,
        &["callsign", "gamma", "--force"],
    ));
    assert_eq!(
        text,
        "callsign gamma (was beta, this session)\n\
         took gamma from session s1, stale 40s\n\
         board: atc\n"
    );
    assert!(!lease(path, "s1").exists());
    assert_eq!(lease_body(path, "s2")["callsign"], "gamma");
}

/// The same session under a new pid is the session again: the word is
/// kept and the lease shows the new pid.
#[cfg(target_os = "linux")]
#[test]
fn the_same_session_under_a_new_pid_keeps_its_word() {
    let repo = repo();
    let path = repo.path();
    let mut first = Command::new("sleep").arg("30").spawn().unwrap();
    stdout(&atc(
        path,
        Some("s1"),
        Some(first.id()),
        &["callsign", "beta"],
    ));
    assert_eq!(lease_body(path, "s1")["pid"], first.id());
    first.kill().unwrap();
    first.wait().unwrap();

    let own = std::process::id();
    let text = stdout(&atc(path, Some("s1"), Some(own), &["callsign"]));
    assert!(
        text.starts_with("callsign beta — session (ATC_SESSION s1)\n"),
        "{text}"
    );
    assert_eq!(lease_body(path, "s1")["pid"], own);
    assert_eq!(lease_body(path, "s1")["callsign"], "beta");
    let text = stdout(&atc(path, Some("s1"), Some(own), &["callsign", "beta"]));
    assert_eq!(text, "callsign beta (renewed)\nboard: atc\n");
}

// ---- the drop line ---------------------------------------------------------

/// A session whose stale lease's word was taken drops to its client
/// word on the next `atc` call, prints the one line on stderr, and the
/// verb proceeds; the call after is quiet.
#[test]
fn a_taken_word_is_dropped_with_one_line_on_stderr() {
    let repo = repo();
    let path = repo.path();
    stdout(&atc(path, None, None, &["file", "one"]));
    plant(path, "s1", "beta", None, 400);
    stdout(&atc(path, Some("s2"), None, &["callsign", "beta"]));
    // The take removed s1's lease; s1 still believes it holds the word.
    plant(path, "s1", "beta", None, 400);

    let claude = ("CLAUDECODE", "1");
    let out = atc_with(path, Some("s1"), claude, &["assign", "1", "me"]);
    assert_eq!(
        stderr(&out),
        "atc: callsign beta was taken by session s2 while idle; you are claude\n"
    );
    assert_eq!(
        stdout(&out),
        "assigned #1 to claude: one\nboard: atc\n",
        "the verb proceeds under the client word"
    );
    assert_eq!(lease_body(path, "s1")["callsign"], serde_json::Value::Null);
    assert_eq!(lease_body(path, "s1")["client"], "claude");

    let out = atc_with(path, Some("s1"), claude, &["callsign"]);
    assert_eq!(stderr(&out), "", "said once");
    assert!(
        stdout(&out).starts_with("callsign claude — client (ATC_SESSION s1)\n"),
        "{}",
        stdout(&out)
    );

    // A stale lease whose word nobody took is renewed and kept, and
    // `--json` never carries the line.
    plant(path, "s3", "gamma", None, 400);
    let out = atc(path, Some("s3"), None, &["callsign", "--json"]);
    assert_eq!(stderr(&out), "");
    assert_eq!(envelope(&out)["data"]["callsign"], "gamma");
    assert_eq!(lease_body(path, "s3")["callsign"], "gamma");
    assert!(
        mtime(&lease(path, "s3")) > SystemTime::now() - Duration::from_secs(60),
        "renewed"
    );

    // Under `ATC_CALLSIGN` the lease's word is never read, so nothing
    // is dropped and nothing said.
    plant(path, "s4", "beta", None, 400);
    let out = atc_with(
        path,
        Some("s4"),
        ("ATC_CALLSIGN", "launched"),
        &["callsign", "--json"],
    );
    assert_eq!(stderr(&out), "");
    assert_eq!(envelope(&out)["data"]["callsign"], "launched");
    assert_eq!(lease_body(path, "s4")["callsign"], "beta");
}

// ---- the window ------------------------------------------------------------

/// `leaseWindow 30s` is honored: a holder 45 seconds old is taken where
/// the default would refuse it, and `xyz` is refused at the setting.
#[test]
fn the_lease_window_is_honored_and_validated() {
    let repo = repo();
    let path = repo.path();
    plant(path, "s1", "beta", None, 45);
    let out = atc(path, Some("s2"), None, &["callsign", "beta", "--json"]);
    refusal(&out, 1, "callsign/held");

    stdout(&atc(path, None, None, &["config", "leaseWindow", "30s"]));
    let out = atc(path, Some("s2"), None, &["callsign", "beta", "--json"]);
    let data = envelope(&out)["data"].clone();
    assert!(out.status.success(), "{data}");
    assert_eq!(data["took"]["detail"], "stale 45s");

    let out = atc(
        path,
        None,
        None,
        &["config", "leaseWindow", "xyz", "--json"],
    );
    let v = refusal(&out, 2, "usage/bad-value");
    assert_eq!(
        v["error"]["message"],
        "invalid value for leaseWindow: want a duration like 30s, 2m, or 1h"
    );
    assert_eq!(
        stdout(&atc(path, None, None, &["config", "leaseWindow"])),
        "30s\n"
    );
}

// ---- the heartbeat ---------------------------------------------------------

/// The first `atc` call under a fresh session creates the lease with no
/// word and the client underneath; every later verb moves its mtime;
/// no session leases nothing.
#[test]
fn every_verb_under_a_session_is_a_heartbeat() {
    let repo = repo();
    let path = repo.path();
    let s9 = Some("s9");
    let pid = own_pid();
    assert!(!lease(path, "s9").exists());
    stdout(
        &command(path, s9, pid, &["--json"])
            .env("CLAUDECODE", "1")
            .output()
            .unwrap(),
    );
    let body = lease_body(path, "s9");
    assert_eq!(body["session"], "s9");
    assert_eq!(body["client"], "claude");
    assert_eq!(body["callsign"], serde_json::Value::Null);
    assert_eq!(body["pid"], serde_json::json!(pid));

    let path_s9 = lease(path, "s9");
    let age = || {
        let then = SystemTime::now() - Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&path_s9)
            .unwrap()
            .set_modified(then)
            .unwrap();
        mtime(&path_s9)
    };
    stdout(&atc(path, s9, pid, &["file", "one"]));
    for args in [
        vec!["brief", "1"],
        vec!["assign", "1", "agent"],
        vec!["next", "agent", "--peek"],
        vec!["explain", "--list"],
        vec!["config"],
    ] {
        let before = age();
        let out = atc(path, s9, pid, &args);
        if args[0] == "explain" || args[0] == "config" {
            // No store opened: no heartbeat, the `atc config` class.
            assert_eq!(mtime(&path_s9), before, "{args:?}");
        } else {
            assert!(out.status.success() || args[0] == "next", "{args:?}");
            assert!(mtime(&path_s9) > before, "{args:?} renews the lease");
        }
    }
    assert_eq!(lease_body(path, "s9")["callsign"], serde_json::Value::Null);

    stdout(&atc(path, None, None, &["brief", "1"]));
    let leases: Vec<_> = std::fs::read_dir(root(path).join(".local/state/atc/leases"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(leases.len(), 1, "no session, no lease: {leases:?}");
}

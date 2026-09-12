//! `atc trigger` against real repositories: the notice at a boundary in
//! each client's envelope, the lease renewed on a boundary and on
//! activity and released at the end, silence on everything else, and
//! the bare form a person runs.
//!
//! Every spawn gets a scratch HOME, so the lease lands under the
//! fixture's `.local/state` and never under the developer's.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, SystemTime};

use atc_testsupport::{Repo, scrub};

/// The fixture's own config root and HOME, beside the repository inside
/// the tempdir: nothing here reads the developer's real files.
fn root(repo: &Path) -> &Path {
    repo.parent().expect("the fixture nests the repository")
}

fn command(cwd: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    command
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env_remove("XDG_STATE_HOME")
        .env("ATC_FF", "/nonexistent");
    scrub(&mut command);
    command
}

/// The source form, fed a payload on stdin (or nothing at all), under
/// the session variable when one is given.
fn trigger(
    cwd: &Path,
    home: &Path,
    source: &str,
    session: Option<&str>,
    stdin: Option<&str>,
) -> Output {
    let mut command = command(cwd, home, &["trigger", source]);
    if let Some(session) = session {
        command.env("CLAUDE_CODE_SESSION_ID", session);
    }
    let mut child = command
        .stdin(match stdin {
            Some(_) => Stdio::piped(),
            None => Stdio::null(),
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn atc");
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(text.as_bytes())
            .expect("write stdin");
    }
    child.wait_with_output().expect("wait for atc")
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

/// Exit 0 with nothing on either stream: the machine contract.
fn silent(output: &Output, what: &str) {
    assert_eq!(output.status.code(), Some(0), "{what}");
    assert!(
        output.stdout.is_empty(),
        "{what}: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "{what}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

fn payload(repo: &Path, event: Option<&str>, session: Option<&str>) -> String {
    let mut fields = serde_json::Map::new();
    fields.insert("cwd".into(), repo.display().to_string().into());
    if let Some(event) = event {
        fields.insert("hook_event_name".into(), event.into());
    }
    if let Some(session) = session {
        fields.insert("session_id".into(), session.into());
    }
    serde_json::Value::Object(fields).to_string()
}

fn lease(home: &Path, session: &str) -> PathBuf {
    home.join(".local/state/atc/leases").join(session)
}

fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// Set a file's mtime a minute into the past, so a renewal is a move
/// the clock's resolution cannot hide.
fn age(path: &Path) -> SystemTime {
    let then = SystemTime::now() - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(then)
        .unwrap();
    mtime(path)
}

// ---- the dispatch ----------------------------------------------------------

/// A boundary prints the notice in the client's envelope and creates
/// the lease; activity renews it silently; the end removes it; a name
/// outside the table does nothing.
#[test]
fn the_three_classes_and_nothing_else() {
    let repo = repo();
    let home = root(repo.path());
    stdout(
        &command(repo.path(), home, &["file", "one", "--status", "ready"])
            .output()
            .unwrap(),
    );
    let elsewhere = tempfile::TempDir::new().unwrap();
    let lease = lease(home, "s1");

    let boundary = payload(repo.path(), Some("SessionStart"), None);
    let out = trigger(
        elsewhere.path(),
        home,
        "claude",
        Some("s1"),
        Some(&boundary),
    );
    let text = stdout(&out);
    assert!(text.starts_with("tower (`atc`) keeps"), "{text}");
    assert!(
        text.trim_end().ends_with("1 flight ready. Run `atc`."),
        "{text}"
    );
    assert!(lease.is_file(), "the boundary creates the lease");

    let before = age(&lease);
    let activity = payload(repo.path(), Some("PreToolUse"), None);
    silent(
        &trigger(
            elsewhere.path(),
            home,
            "claude",
            Some("s1"),
            Some(&activity),
        ),
        "activity",
    );
    assert!(mtime(&lease) > before, "activity renews the lease");
    for event in ["UserPromptSubmit", "Stop"] {
        let before = age(&lease);
        let activity = payload(repo.path(), Some(event), None);
        silent(
            &trigger(
                elsewhere.path(),
                home,
                "claude",
                Some("s1"),
                Some(&activity),
            ),
            event,
        );
        assert!(mtime(&lease) > before, "{event} renews the lease");
    }

    let before = age(&lease);
    let nonsense = payload(repo.path(), Some("Nonsense"), None);
    silent(
        &trigger(
            elsewhere.path(),
            home,
            "claude",
            Some("s1"),
            Some(&nonsense),
        ),
        "an unknown event",
    );
    assert_eq!(mtime(&lease), before, "an unknown event touches nothing");

    let end = payload(repo.path(), Some("SessionEnd"), None);
    silent(
        &trigger(elsewhere.path(), home, "claude", Some("s1"), Some(&end)),
        "the end",
    );
    assert!(!lease.exists(), "the end releases the lease");
    // Released twice is still nothing to say.
    silent(
        &trigger(elsewhere.path(), home, "claude", Some("s1"), Some(&end)),
        "the end again",
    );
}

/// A payload with no event name is a boundary — an older config's
/// `SessionStart` entry, or a person piping by hand — so it prints the
/// notice and takes the lease.
#[test]
fn no_event_name_is_a_boundary() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();
    let bare = payload(repo.path(), None, None);
    let out = trigger(elsewhere.path(), home, "claude", Some("s2"), Some(&bare));
    let text = stdout(&out);
    assert!(text.starts_with("tower (`atc`) keeps"), "{text}");
    assert!(lease(home, "s2").is_file());
}

/// Every client's boundary goes out in its own envelope, and the notice
/// is the same text inside each.
#[test]
fn the_boundary_is_wrapped_the_way_each_client_reads_it() {
    let repo = repo();
    let home = root(repo.path());
    stdout(
        &command(repo.path(), home, &["file", "one", "--status", "ready"])
            .output()
            .unwrap(),
    );
    let elsewhere = tempfile::TempDir::new().unwrap();

    let plain = |source: &str, event: &str| -> String {
        let text = payload(repo.path(), Some(event), None);
        let out = trigger(elsewhere.path(), home, source, Some("s3"), Some(&text));
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        stdout(&out)
    };
    for source in ["claude", "codex"] {
        let text = plain(source, "SessionStart");
        assert!(text.starts_with("tower (`atc`) keeps"), "{source}: {text}");
        assert!(
            text.trim_end().ends_with("1 flight ready. Run `atc`."),
            "{source}: {text}"
        );
    }
    let gemini: serde_json::Value = serde_json::from_str(&plain("gemini", "SessionStart")).unwrap();
    let carried = gemini["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(carried.starts_with("tower (`atc`) keeps"), "{carried}");
    let cursor: serde_json::Value = serde_json::from_str(&plain("cursor", "sessionStart")).unwrap();
    let carried = cursor["additional_context"].as_str().unwrap();
    assert!(carried.starts_with("tower (`atc`) keeps"), "{carried}");

    // Each client's table is its own: Claude's activity name means
    // nothing to the two that wire the boundary alone.
    for source in ["gemini", "cursor"] {
        let text = payload(repo.path(), Some("PreToolUse"), None);
        silent(
            &trigger(elsewhere.path(), home, source, Some("s3"), Some(&text)),
            source,
        );
    }
}

/// `--json` on the source form is the machine envelope, the way the
/// alias answered it.
#[test]
fn the_source_form_takes_json() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();
    let boundary = payload(repo.path(), Some("SessionStart"), None);
    let mut command = command(elsewhere.path(), home, &["--json", "trigger", "claude"]);
    command.stdin(Stdio::piped()).stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(boundary.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("an envelope");
    assert_eq!(v["cmd"], "trigger");
    assert!(
        v["data"]["text"]
            .as_str()
            .unwrap()
            .starts_with("tower (`atc`) keeps")
    );
}

// ---- the key ---------------------------------------------------------------

/// Without the variable, the payload's `session_id` keys the lease; with
/// neither, a boundary still prints and nothing is leased.
#[test]
fn the_payload_session_keys_the_lease_when_the_variable_is_absent() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();

    let boundary = payload(repo.path(), Some("SessionStart"), Some("p1"));
    let out = trigger(elsewhere.path(), home, "claude", None, Some(&boundary));
    stdout(&out);
    assert!(lease(home, "p1").is_file(), "keyed by the payload");

    // The variable wins over the payload when both are there.
    let out = trigger(
        elsewhere.path(),
        home,
        "claude",
        Some("v1"),
        Some(&boundary),
    );
    stdout(&out);
    assert!(lease(home, "v1").is_file(), "keyed by the variable");

    // A payload session that cannot be a file name is no session.
    let bad = payload(repo.path(), Some("SessionStart"), Some("../escape"));
    let out = trigger(elsewhere.path(), home, "claude", None, Some(&bad));
    stdout(&out);

    let none = payload(repo.path(), Some("SessionStart"), None);
    let out = trigger(elsewhere.path(), home, "claude", None, Some(&none));
    let text = stdout(&out);
    assert!(text.starts_with("tower (`atc`) keeps"), "{text}");
    let leases: Vec<_> = std::fs::read_dir(home.join(".local/state/atc/leases"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(leases.len(), 2, "no session, no lease: {leases:?}");
}

/// `XDG_STATE_HOME` is the state root when set, the way it is for every
/// XDG tool.
#[test]
fn xdg_state_home_is_honored() {
    let repo = repo();
    let home = root(repo.path());
    let state = tempfile::TempDir::new().unwrap();
    let elsewhere = tempfile::TempDir::new().unwrap();
    let activity = payload(repo.path(), Some("PreToolUse"), None);
    let mut command = command(elsewhere.path(), home, &["trigger", "claude"]);
    command
        .env("CLAUDE_CODE_SESSION_ID", "x1")
        .env("XDG_STATE_HOME", state.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(activity.as_bytes())
        .unwrap();
    silent(&child.wait_with_output().unwrap(), "activity under XDG");
    assert!(state.path().join("atc/leases/x1").is_file());
    assert!(!lease(home, "x1").exists());
}

// ---- silence ---------------------------------------------------------------

/// Machine surface: a source tower did not write, a repository that is
/// not there, garbage on stdin, no stdin — exit 0 and nothing said.
#[test]
fn a_source_it_does_not_know_and_a_place_with_no_board_are_silent() {
    let elsewhere = tempfile::TempDir::new().unwrap();
    let home = elsewhere.path();
    silent(
        &trigger(home, home, "anything", Some("s9"), None),
        "an unknown source",
    );
    silent(
        &trigger(
            home,
            home,
            "anything",
            Some("s9"),
            Some("{\"hook_event_name\":\"SessionStart\"}"),
        ),
        "an unknown source with a payload",
    );
    assert!(
        !home.join(".local/state/atc/leases/s9").exists(),
        "an unknown source leases nothing"
    );
    for stdin in [
        Some(format!(
            r#"{{"hook_event_name":"SessionStart","cwd":{}}}"#,
            serde_json::Value::String(home.display().to_string())
        )),
        Some("this is not json".to_string()),
        Some(String::new()),
        None,
    ] {
        for source in ["claude", "codex", "cursor", "gemini"] {
            silent(
                &trigger(home, home, source, Some("s9"), stdin.as_deref()),
                &format!("{source}: {stdin:?}"),
            );
        }
    }
    // Outside a repository the boundary still takes the lease: the
    // session is real even where there is no board to speak of.
    assert!(home.join(".local/state/atc/leases/s9").is_file());
}

// ---- the bare form ---------------------------------------------------------

/// Bare `atc trigger` is the notice for the repository you are in, and
/// `--json` carries the same fields the alias did under `cmd` trigger.
#[test]
fn bare_trigger_is_the_notice_for_here() {
    let repo = repo();
    let home = root(repo.path());
    let atc = |args: &[&str]| command(repo.path(), home, args).output().unwrap();
    let text = stdout(&atc(&["trigger"]));
    assert!(text.starts_with("tower (`atc`) keeps"), "{text}");
    assert!(
        text.trim_end()
            .ends_with("Nothing filed here yet. Run `atc`."),
        "{text}"
    );
    stdout(&atc(&["file", "one", "--status", "ready"]));
    let out = atc(&["trigger", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("an envelope");
    assert_eq!(v["atc"], 1);
    assert_eq!(v["cmd"], "trigger");
    assert!(
        v["data"]["text"]
            .as_str()
            .unwrap()
            .ends_with("1 flight ready. Run `atc`.")
    );
    assert_eq!(v["data"]["ready"], 1);
    assert_eq!(v["data"]["filed"], 1);
    assert_eq!(v["data"]["on"], serde_json::json!([]));
    assert!(v["data"]["callsign"].is_null());
    assert!(
        !home.join(".local/state").exists(),
        "the bare form leases nothing"
    );

    // Outside a repository, the ordinary refusal.
    let dir = tempfile::TempDir::new().unwrap();
    let out = command(dir.path(), dir.path(), &["trigger", "--json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("an envelope");
    assert_eq!(v["cmd"], "trigger");
    assert!(v["error"]["id"].as_str().unwrap().contains('/'), "{v}");
}

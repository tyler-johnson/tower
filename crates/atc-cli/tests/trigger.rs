//! `atc trigger` against real repositories: the notice at a boundary in
//! each client's envelope, the lease renewed on a boundary and on
//! activity and released at the end, silence on everything else, the
//! shell source that reads no payload, and the bare form a person runs.
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
    trigger_env(cwd, home, source, session, stdin, &[])
}

/// The same, with more variables set — a client's marker.
fn trigger_env(
    cwd: &Path,
    home: &Path,
    source: &str,
    session: Option<&str>,
    stdin: Option<&str>,
    env: &[(&str, &str)],
) -> Output {
    let mut command = command(cwd, home, &["trigger", source]);
    if let Some(session) = session {
        command.env("CLAUDE_CODE_SESSION_ID", session);
    }
    for (name, value) in env {
        command.env(name, value);
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

/// Every lease name in the state directory, the heartbeat's `sweep`
/// marker and the `.lock` sidecars left out: neither is a session.
fn lease_names(home: &Path) -> Vec<std::ffi::OsString> {
    let Ok(entries) = std::fs::read_dir(home.join(".local/state/atc/leases")) else {
        return Vec::new();
    };
    entries
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name != "sweep" && !name.to_string_lossy().ends_with(".lock"))
        .collect()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
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
/// is the same text inside each, and the retired sources answer as they
/// always did.
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

    let under = |source: &str, event: &str, env: &[(&str, &str)]| -> String {
        let text = payload(repo.path(), Some(event), None);
        let out = trigger_env(elsewhere.path(), home, source, Some("s3"), Some(&text), env);
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        stdout(&out)
    };
    let plain = |source: &str, event: &str| under(source, event, &[]);
    let is_plain = |text: &str, what: &str| {
        assert!(text.starts_with("tower (`atc`) keeps"), "{what}: {text}");
        assert!(
            text.trim_end().ends_with("1 flight ready. Run `atc`."),
            "{what}: {text}"
        );
    };
    let carried = |text: &str, path: &[&str]| -> String {
        let mut value: serde_json::Value = serde_json::from_str(text).unwrap();
        for key in path {
            value = value[*key].take();
        }
        value
            .as_str()
            .unwrap_or_else(|| panic!("{path:?} in {text}"))
            .to_string()
    };

    for source in ["claude", "codex"] {
        is_plain(&plain(source, "SessionStart"), source);
    }
    // A Codex hook process carries no marker; the shell tool's marker
    // changes nothing about the envelope either way.
    is_plain(
        &under("codex", "SessionStart", &[("CODEX_SANDBOX", "1")]),
        "codex under its marker",
    );

    // The retired sources, each in the envelope it was written with.
    let text = carried(
        &plain("gemini", "SessionStart"),
        &["hookSpecificOutput", "additionalContext"],
    );
    is_plain(&text, "gemini");
    let text = carried(&plain("cursor", "sessionStart"), &["additional_context"]);
    is_plain(&text, "cursor");
    // Qwen, in the field it inherited.
    let text = carried(
        &plain("qwen", "SessionStart"),
        &["hookSpecificOutput", "additionalContext"],
    );
    is_plain(&text, "qwen");

    // Each source's table is its own: Claude's activity name means
    // nothing to the two retired ones that wired the boundary alone,
    // and renews the lease silently on Codex.
    for source in ["gemini", "cursor"] {
        let text = payload(repo.path(), Some("PreToolUse"), None);
        silent(
            &trigger(elsewhere.path(), home, source, Some("s3"), Some(&text)),
            source,
        );
    }
    let lease = lease(home, "s3");
    let before = age(&lease);
    let text = payload(repo.path(), Some("PreToolUse"), None);
    silent(
        &trigger(elsewhere.path(), home, "codex", Some("s3"), Some(&text)),
        "codex activity",
    );
    assert!(mtime(&lease) > before, "activity renews the lease");
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
    let leases = lease_names(home);
    assert_eq!(leases.len(), 2, "no session, no lease: {leases:?}");
}

/// The heartbeat sweeps once per `leaseSweep`, through the marker: the
/// first activity under a session removes an expired lease and writes
/// the marker an hour out; the next leaves an expired lease alone while
/// the marker is ahead; a marker past due sweeps again, and the
/// repository's `leaseSweep` sets the next due time; outside a
/// repository the defaults rule.
#[test]
fn the_heartbeat_sweeps_once_per_interval() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();
    let activity = payload(repo.path(), Some("PreToolUse"), None);
    let marker = home.join(".local/state/atc/leases/sweep");
    let plant = |name: &str| {
        let path = lease(home, name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!(r#"{{"session":"{name}"}}"#)).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(90_000))
            .unwrap();
        path
    };
    let due = || -> u64 {
        std::fs::read_to_string(&marker)
            .expect("a marker")
            .trim()
            .parse()
            .expect("a unix second")
    };

    let x1 = plant("x1");
    let before = now_secs();
    silent(
        &trigger(
            elsewhere.path(),
            home,
            "claude",
            Some("c1"),
            Some(&activity),
        ),
        "the first activity",
    );
    assert!(!x1.exists(), "no marker: the expired lease is swept");
    assert!(lease(home, "c1").is_file(), "the heartbeat's own lease");
    let first = due();
    assert!(
        (before + 3_600..=before + 3_602).contains(&first),
        "an hour out: {first} from {before}"
    );

    let x2 = plant("x2");
    silent(
        &trigger(
            elsewhere.path(),
            home,
            "claude",
            Some("c1"),
            Some(&activity),
        ),
        "the second activity",
    );
    assert!(x2.exists(), "the marker is ahead: no sweep");
    assert_eq!(due(), first, "the marker is left as it was");

    std::fs::write(&marker, (now_secs() - 1).to_string()).unwrap();
    repo.git(&["config", "tower.leaseSweep", "5m"]);
    let before = now_secs();
    silent(
        &trigger(
            elsewhere.path(),
            home,
            "claude",
            Some("c1"),
            Some(&activity),
        ),
        "past due",
    );
    assert!(!x2.exists(), "past due: swept");
    let next = due();
    assert!(
        (before + 300..=before + 302).contains(&next),
        "the repository's leaseSweep: {next} from {before}"
    );

    // Outside a repository — the payload's cwd a bare tempdir — the
    // defaults rule.
    std::fs::remove_file(&marker).unwrap();
    let x3 = plant("x3");
    let bare = payload(elsewhere.path(), Some("PreToolUse"), None);
    let before = now_secs();
    silent(
        &trigger(elsewhere.path(), home, "claude", Some("c1"), Some(&bare)),
        "no repository",
    );
    assert!(
        !x3.exists(),
        "no repository: the default expiry still sweeps"
    );
    let next = due();
    assert!(
        (before + 3_600..=before + 3_602).contains(&next),
        "no repository: the default interval, {next} from {before}"
    );
}

/// The session table is the key's: `ATC_SESSION` beats
/// `CLAUDE_CODE_SESSION_ID` even under Claude's own hook, and the pid
/// stored is the row's own — `CLAUDE_PID` under Claude's row, and none
/// under the launcher's when it hands down no `ATC_PID`.
#[test]
fn the_launcher_row_beats_the_client_row_and_the_pid_is_the_rows_own() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();
    let activity = payload(repo.path(), Some("PreToolUse"), None);
    let run = |vars: &[(&str, String)]| {
        let mut command = command(elsewhere.path(), home, &["trigger", "claude"]);
        for (name, value) in vars {
            command.env(name, value);
        }
        command
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
        silent(&child.wait_with_output().unwrap(), "activity");
    };
    let body = |session: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(lease(home, session)).unwrap()).unwrap()
    };
    let own = std::process::id().to_string();

    run(&[
        ("CLAUDE_CODE_SESSION_ID", "c1".to_string()),
        ("CLAUDE_PID", own.clone()),
    ]);
    let stored = body("c1");
    if cfg!(target_os = "linux") {
        assert_eq!(stored["pid"], std::process::id(), "{stored}");
        assert!(stored["pid_start"].is_u64(), "{stored}");
    }
    assert_eq!(stored["callsign"], serde_json::Value::Null);
    assert_eq!(stored["session"], "c1");

    run(&[
        ("ATC_SESSION", "w1".to_string()),
        ("CLAUDE_CODE_SESSION_ID", "c2".to_string()),
        ("CLAUDE_PID", own.clone()),
    ]);
    assert!(
        lease(home, "w1").is_file(),
        "the launcher's row keys the lease"
    );
    assert!(
        !lease(home, "c2").exists(),
        "the client's row is not read past it"
    );
    let stored = body("w1");
    assert_eq!(
        stored["pid"],
        serde_json::Value::Null,
        "the pid is the launcher row's own, never Claude's: {stored}"
    );

    // The heartbeat never rewrites a body under one root: the pid
    // stored at creation stays through a renewal under another.
    let before = age(&lease(home, "c1"));
    run(&[
        ("CLAUDE_CODE_SESSION_ID", "c1".to_string()),
        ("CLAUDE_PID", "1".to_string()),
    ]);
    assert!(mtime(&lease(home, "c1")) > before);
    if cfg!(target_os = "linux") {
        assert_eq!(body("c1")["pid"], std::process::id());
    }
}

/// The heartbeat records the payload's repository on the lease: a cwd
/// below a root resolves to it; the same root again moves the mtime
/// and leaves the body byte-identical; another root appends; an
/// earlier one moves to the end; a callsign written between two
/// heartbeats survives them, and the store's root and the trigger's
/// agree on one spelling; a cwd with no repository above it records
/// nothing; the end takes the lock sidecar with the lease.
#[test]
fn the_heartbeat_records_the_repository() {
    use atc_testsupport::as_reported;

    let repo_a = repo();
    let home = root(repo_a.path());
    let repo_b = Repo::new();
    let elsewhere = tempfile::TempDir::new().unwrap();
    let a = as_reported(repo_a.path()).display().to_string();
    let b = as_reported(repo_b.path()).display().to_string();
    let lease_path = lease(home, "c1");
    let body = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(&lease_path).unwrap()).unwrap()
    };
    let repos = || body()["repos"].clone();
    let activity = |cwd: &Path| {
        let activity = payload(cwd, Some("PreToolUse"), None);
        silent(
            &trigger(
                elsewhere.path(),
                home,
                "claude",
                Some("c1"),
                Some(&activity),
            ),
            "activity",
        );
    };

    let sub = repo_a.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    activity(&sub);
    assert_eq!(
        repos(),
        serde_json::json!([a]),
        "a cwd below the root resolves to it"
    );

    // The same root again: the mtime moves, the bytes do not.
    let before = age(&lease_path);
    let bytes = std::fs::read(&lease_path).unwrap();
    activity(&sub);
    assert!(
        mtime(&lease_path) > before,
        "a heartbeat under the last root still renews"
    );
    assert_eq!(
        std::fs::read(&lease_path).unwrap(),
        bytes,
        "and never rewrites"
    );

    activity(repo_b.path());
    assert_eq!(repos(), serde_json::json!([a, b]), "another root appends");
    activity(repo_a.path());
    assert_eq!(
        repos(),
        serde_json::json!([b, a]),
        "an earlier root moves to the end"
    );

    // A word written between two heartbeats survives the rewrite the
    // second one makes, and the store's root is the trigger's spelling.
    stdout(
        &command(repo_a.path(), home, &["callsign", "alpha"])
            .env("CLAUDE_CODE_SESSION_ID", "c1")
            .output()
            .unwrap(),
    );
    assert_eq!(body()["callsign"], "alpha");
    assert_eq!(
        repos(),
        serde_json::json!([b, a]),
        "the store's root is the same entry"
    );
    assert!(
        lease(home, "c1.lock").is_file(),
        "the callsign wrote under the sidecar"
    );
    activity(repo_b.path());
    assert_eq!(
        body()["callsign"],
        "alpha",
        "the word survives the heartbeat's rewrite"
    );
    assert_eq!(repos(), serde_json::json!([a, b]));

    // No repository above the cwd: nothing recorded, still a heartbeat.
    let before = age(&lease_path);
    activity(elsewhere.path());
    assert!(mtime(&lease_path) > before);
    assert_eq!(
        repos(),
        serde_json::json!([a, b]),
        "a bare cwd records nothing"
    );

    let end = payload(repo_a.path(), Some("SessionEnd"), None);
    silent(
        &trigger(elsewhere.path(), home, "claude", Some("c1"), Some(&end)),
        "the end",
    );
    assert!(!lease_path.exists(), "the end releases the lease");
    assert!(!lease(home, "c1.lock").exists(), "and its sidecar");
}

// ---- the shell source ------------------------------------------------------

/// `atc trigger shell` under the terminal's variables creates the lease
/// with the shell's pid and says nothing; a second call moves the mtime;
/// `--end` removes it; with no session variable nothing is touched.
#[test]
fn the_shell_source_is_the_terminals_heartbeat_and_release() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();
    let own = std::process::id().to_string();
    let shell = |args: &[&str], vars: &[(&str, &str)], stdin: Option<&str>| -> Output {
        let mut argv = vec!["trigger", "shell"];
        argv.extend_from_slice(args);
        let mut command = command(elsewhere.path(), home, &argv);
        for (name, value) in vars {
            command.env(name, value);
        }
        command
            .stdin(match stdin {
                Some(_) => Stdio::piped(),
                None => Stdio::null(),
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        if let Some(text) = stdin {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
        }
        child.wait_with_output().unwrap()
    };
    let terminal: [(&str, &str); 2] = [("ATC_SHELL_SESSION", "t1"), ("ATC_SHELL_PID", &own)];
    let lease = lease(home, "t1");

    silent(&shell(&[], &terminal, None), "the prompt");
    assert!(lease.is_file(), "the prompt creates the lease");
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&lease).unwrap()).unwrap();
    assert_eq!(stored["session"], "t1");
    assert_eq!(stored["client"], serde_json::Value::Null);
    if cfg!(target_os = "linux") {
        assert_eq!(
            stored["pid"],
            std::process::id(),
            "the shell's pid: {stored}"
        );
    }

    let before = age(&lease);
    silent(&shell(&[], &terminal, None), "the next prompt");
    assert!(mtime(&lease) > before, "a prompt renews the lease");

    silent(&shell(&["--end"], &terminal, None), "the exit");
    assert!(!lease.exists(), "the exit releases the lease");
    silent(&shell(&["--end"], &terminal, None), "the exit again");

    // No session variable: nothing to lease, nothing said.
    silent(&shell(&[], &[], None), "no session");
    silent(&shell(&["--end"], &[], None), "no session, the end");
    assert!(lease_names(home).is_empty(), "nothing was leased");

    // The shell source reads no stdin: a payload naming a session keys
    // nothing, and the script it would have eaten is left alone.
    let piped = payload(repo.path(), Some("SessionStart"), Some("p1"));
    silent(&shell(&[], &[], Some(&piped)), "a payload on stdin");
    assert!(
        !crate::lease(home, "p1").exists(),
        "the shell source reads no payload"
    );

    // `--end` on a client source is nothing: no class, no lease touched.
    let mut command = command(elsewhere.path(), home, &["trigger", "claude", "--end"]);
    command.env("CLAUDE_CODE_SESSION_ID", "c9");
    command.stdin(Stdio::null());
    silent(&command.output().unwrap(), "a client given --end");
    assert!(!crate::lease(home, "c9").exists());
}

/// An agent under a wired terminal is its own session: with Claude's
/// variable and the terminal's both set, `atc trigger shell` renews
/// Claude's lease — the row walk is the store's, and the terminal's
/// row is last.
#[test]
fn under_an_agent_the_shell_source_renews_the_agents_lease() {
    let repo = repo();
    let home = root(repo.path());
    let elsewhere = tempfile::TempDir::new().unwrap();
    let own = std::process::id().to_string();
    let mut command = command(elsewhere.path(), home, &["trigger", "shell"]);
    command
        .env("CLAUDE_CODE_SESSION_ID", "c1")
        .env("CLAUDE_PID", &own)
        .env("ATC_SHELL_SESSION", "t1")
        .env("ATC_SHELL_PID", "1")
        .stdin(Stdio::null());
    silent(&command.output().unwrap(), "the prompt under an agent");
    assert!(lease(home, "c1").is_file(), "Claude's lease");
    assert!(!lease(home, "t1").exists(), "not the terminal's");
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(lease(home, "c1")).unwrap()).unwrap();
    if cfg!(target_os = "linux") {
        assert_eq!(
            stored["pid"],
            std::process::id(),
            "CLAUDE_PID, never the shell's"
        );
    }
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
        for source in ["claude", "codex", "qwen", "cursor", "gemini"] {
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

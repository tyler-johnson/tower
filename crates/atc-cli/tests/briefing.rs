//! `atc briefing` against real repositories: the alias `atc trigger`
//! replaced, kept forever for the configs that still spell it. The
//! notice and its status line, the failure outside a repository, and the
//! client form — wrapped the way each client reads it, silent where
//! there is nothing to say, and the notice whatever event the payload
//! names, because the alias reads no event and touches no lease.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

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
        .env("ATC_FF", "/nonexistent");
    scrub(&mut command);
    command
}

fn atc(repo: &Path, args: &[&str]) -> Output {
    command(repo, root(repo), args).output().expect("spawn atc")
}

/// The client form, fed a payload on stdin (or nothing at all), from a
/// directory that is not a repository — the payload's `cwd` is what
/// names the session's repository, not where the hook happens to run.
fn hook(cwd: &Path, home: &Path, client: &str, stdin: Option<&str>) -> Output {
    let mut child = command(cwd, home, &["briefing", client])
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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

#[test]
fn the_notice_leads_and_the_status_line_closes() {
    let repo = repo();
    let text = stdout(&atc(repo.path(), &["briefing"]));
    assert!(text.starts_with("tower (`atc`) keeps"), "{text}");
    assert!(
        text.trim_end()
            .ends_with("Nothing filed here yet. Run `atc`."),
        "{text}"
    );

    stdout(&atc(repo.path(), &["file", "one", "--status", "ready"]));
    stdout(&atc(repo.path(), &["file", "two", "--status", "ready"]));
    stdout(&atc(repo.path(), &["file", "three", "--status", "backlog"]));
    let text = stdout(&atc(repo.path(), &["briefing"]));
    assert!(
        text.trim_end().ends_with("2 flights ready. Run `atc`."),
        "{text}"
    );

    let out = atc(repo.path(), &["briefing", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("an envelope");
    assert_eq!(v["atc"], 1);
    assert_eq!(v["cmd"], "briefing");
    assert_eq!(v["data"]["text"].as_str().unwrap(), text.trim_end());
    assert_eq!(v["data"]["ready"], 2);
    assert_eq!(v["data"]["filed"], 3);
    assert_eq!(v["data"]["on"], serde_json::json!([]));
    assert!(
        v["data"]["callsign"].is_null(),
        "no callsign under the runner"
    );
}

#[test]
fn a_callsign_on_a_flight_gets_the_resume_line() {
    // The pilot's own In Progress flights, ahead of the count: what a
    // session that compacted mid-flight needs first. Another callsign's
    // pull, and a pull with no callsign, are not this session's.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "one", "--status", "ready"]));
    stdout(&atc(repo.path(), &["file", "two", "--status", "ready"]));
    stdout(&atc(repo.path(), &["file", "three", "--status", "ready"]));
    let as_claude = |args: &[&str]| {
        let mut command = command(repo.path(), root(repo.path()), args);
        command.env("ATC_CALLSIGN", "claude");
        command.output().expect("spawn atc")
    };
    stdout(&as_claude(&["status", "2", "in_progress"]));
    stdout(&atc(repo.path(), &["status", "3", "in_progress"]));

    let text = stdout(&as_claude(&["briefing"]));
    assert!(
        text.trim_end()
            .ends_with("You are on #2. Run `atc brief 2`."),
        "{text}"
    );
    let out = as_claude(&["briefing", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("an envelope");
    assert_eq!(v["data"]["on"], serde_json::json!(["#2"]));
    assert_eq!(v["data"]["callsign"], serde_json::json!("claude"));
    assert_eq!(v["data"]["ready"], 1);

    // Without the callsign, the count line as before.
    let text = stdout(&atc(repo.path(), &["briefing"]));
    assert!(
        text.trim_end().ends_with("1 flight ready. Run `atc`."),
        "{text}"
    );

    // The client form under the variable carries the line too.
    stdout(&as_claude(&["status", "1", "in_progress"]));
    let mut hooked = command(
        root(repo.path()),
        root(repo.path()),
        &["briefing", "claude"],
    );
    hooked.env("ATC_CALLSIGN", "claude");
    hooked.stdin(Stdio::piped());
    let mut child = hooked
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn atc");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(
            format!(
                r#"{{"cwd":{}}}"#,
                serde_json::Value::String(repo.path().display().to_string())
            )
            .as_bytes(),
        )
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for atc");
    let text = stdout(&out);
    assert!(
        text.contains("You are on #1 and #2. Run `atc brief 1`."),
        "{text}"
    );
}

#[test]
fn outside_a_repository_the_failure_is_the_ordinary_one() {
    let dir = tempfile::TempDir::new().unwrap();
    let spawn = |args: &[&str]| {
        command(dir.path(), dir.path(), args)
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

/// The client form: the payload names the repository, the text goes out
/// wrapped the way that client reads injected context.
#[test]
fn a_client_source_is_wrapped_the_way_each_client_reads_it() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "one", "--status", "ready"]));
    let elsewhere = tempfile::TempDir::new().unwrap();
    let payload = format!(
        r#"{{"hook_event_name":"SessionStart","session_id":"s","cwd":{}}}"#,
        serde_json::Value::String(repo.path().display().to_string())
    );

    let plain = |client: &str| -> String {
        let out = hook(elsewhere.path(), root(repo.path()), client, Some(&payload));
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        stdout(&out)
    };
    for client in ["claude", "codex"] {
        let text = plain(client);
        assert!(text.starts_with("tower (`atc`) keeps"), "{client}: {text}");
        assert!(
            text.trim_end().ends_with("1 flight ready. Run `atc`."),
            "{client}: {text}"
        );
    }
    let qwen: serde_json::Value = serde_json::from_str(&plain("qwen")).unwrap();
    let carried = qwen["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(carried.starts_with("tower (`atc`) keeps"), "{carried}");
    let gemini: serde_json::Value = serde_json::from_str(&plain("gemini")).unwrap();
    let carried = gemini["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(carried.starts_with("tower (`atc`) keeps"), "{carried}");
    assert!(carried.ends_with("1 flight ready. Run `atc`."), "{carried}");
    let cursor: serde_json::Value = serde_json::from_str(&plain("cursor")).unwrap();
    let carried = cursor["additional_context"].as_str().unwrap();
    assert!(carried.starts_with("tower (`atc`) keeps"), "{carried}");
    assert!(carried.ends_with("1 flight ready. Run `atc`."), "{carried}");

    // The alias reads no event: an activity payload still gets the
    // notice, which is what the entry it sits in was wired for.
    let activity = payload.replace("SessionStart", "PreToolUse");
    let out = hook(
        elsewhere.path(),
        root(repo.path()),
        "claude",
        Some(&activity),
    );
    let text = stdout(&out);
    assert!(text.starts_with("tower (`atc`) keeps"), "{text}");
    assert!(
        !root(repo.path()).join(".local/state/atc/leases").exists(),
        "the alias touches no lease"
    );

    // A name nothing wrote is the verb's own refusal, not a silence.
    let out = hook(elsewhere.path(), root(repo.path()), "tcsh", Some(&payload));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("usage/unknown-slug")
            || String::from_utf8_lossy(&out.stderr).contains("unknown client"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A hook's stderr is noise in someone else's terminal, and a hook that
/// fails gets uninstalled: outside a repository, with garbage on stdin,
/// or with no stdin at all, the client form exits 0 and says nothing.
#[test]
fn a_client_source_outside_a_repository_says_nothing() {
    let elsewhere = tempfile::TempDir::new().unwrap();
    let silent = |stdin: Option<&str>| {
        // The live clients and the retired spellings alike.
        for client in ["claude", "codex", "qwen", "opencode", "cursor", "gemini"] {
            let out = hook(elsewhere.path(), elsewhere.path(), client, stdin);
            assert_eq!(out.status.code(), Some(0), "{client}: {stdin:?}");
            assert!(
                out.stdout.is_empty(),
                "{client}: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            assert!(
                out.stderr.is_empty(),
                "{client}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    };
    silent(Some(&format!(
        r#"{{"hook_event_name":"SessionStart","cwd":{}}}"#,
        serde_json::Value::String(elsewhere.path().display().to_string())
    )));
    silent(Some("this is not json"));
    silent(Some(""));
    silent(None);
}

/// The printed text spells live verbs and never a retired one.
#[test]
fn the_briefing_teaches_only_live_spellings() {
    let repo = repo();
    let text = stdout(&atc(repo.path(), &["briefing"]));
    assert!(text.contains("`atc next`"), "{text}");
    assert!(text.contains("`atc hold"), "{text}");
    assert!(!text.contains("atc requeue"), "{text}");
}

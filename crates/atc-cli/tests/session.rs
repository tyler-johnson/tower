//! `atc session` against a scratch state directory: `--mint` prints
//! v7 ids that sort in order and writes nothing, in a repository or
//! out of one; bare lists the machine's leases with the client, the
//! callsign, fresh or stale, the pid alive or dead, and marks this
//! process's own row; `leaseWindow` moves the line between fresh and
//! stale.
//!
//! Every spawn gets a scratch HOME, so leases land under the fixture's
//! `.local/state` and never under the developer's.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

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

fn leases(home: &Path) -> PathBuf {
    home.join(".local/state/atc/leases")
}

/// Write a lease by hand: the client, the word, an optional pid with a
/// start time, and an mtime `age` seconds into the past.
fn plant(
    home: &Path,
    session: &str,
    client: Option<&str>,
    word: Option<&str>,
    pid: Option<(u32, u64)>,
    age: u64,
) {
    let path = leases(home).join(session);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let body = serde_json::json!({
        "session": session,
        "client": client,
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

/// The start time of this process, from `/proc`, so a planted pid reads
/// alive; none where there is no reader.
fn own_start() -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", std::process::id())).ok()?;
    let after = &stat[stat.rfind(')')? + 1..];
    after.split_whitespace().nth(19)?.parse().ok()
}

// ---- --mint ----------------------------------------------------------------

/// Two mints are two v7 ids that sort in order, no lease is written,
/// and it works with no repository in sight.
#[test]
fn mint_prints_ordered_v7_ids_and_touches_nothing() {
    let repo = repo();
    let path = repo.path();
    let first = stdout(&atc(path, &[], &["session", "--mint"]));
    let second = stdout(&atc(path, &[], &["session", "--mint"]));
    let first = first.trim_end().to_string();
    let second = second.trim_end().to_string();
    for id in [&first, &second] {
        assert_eq!(id.len(), 36, "{id}");
        assert_eq!(id.as_bytes()[14], b'7', "version 7: {id}");
        assert!(
            matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
            "variant: {id}"
        );
    }
    assert!(first < second, "{first} then {second}");
    assert!(!leases(root(path)).exists(), "no lease written");

    let v = envelope(&atc(path, &[], &["session", "--mint", "--json"]));
    assert_eq!(v["cmd"], "session");
    assert_eq!(v["data"]["session"].as_str().unwrap().len(), 36);

    // Outside a repository, and under a terminal's variables: the same,
    // and still no lease — a mint is not a heartbeat.
    let elsewhere = tempfile::TempDir::new().unwrap();
    let home = tempfile::TempDir::new().unwrap();
    let out = command(
        elsewhere.path(),
        home.path(),
        &[("ATC_SHELL_SESSION", "t1")],
        &["session", "--mint"],
    )
    .output()
    .unwrap();
    assert_eq!(stdout(&out).trim_end().len(), 36);
    assert!(!leases(home.path()).exists());
}

// ---- the listing -----------------------------------------------------------

/// Planted leases list with their client, callsign, freshness, and pid,
/// sorted by session, the own row marked; an empty state directory is
/// one line.
#[test]
fn the_listing_is_every_lease_on_the_machine() {
    let repo = repo();
    let path = repo.path();
    let home = root(path);

    let text = stdout(&atc(path, &[], &["session"]));
    assert_eq!(text, "no sessions on this machine\n");
    let v = envelope(&atc(path, &[], &["session", "--json"]));
    assert_eq!(v["cmd"], "session");
    assert_eq!(v["data"]["sessions"], serde_json::json!([]));

    let alive = own_start().map(|start| (std::process::id(), start));
    plant(home, "b-old", None, Some("beta"), Some((4_000_000, 1)), 400);
    plant(home, "a-new", Some("claude"), None, alive, 3);
    plant(home, "c-own", None, Some("gamma"), None, 30);

    let text = stdout(&atc(path, &[("ATC_SHELL_SESSION", "c-own")], &["session"]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert_eq!(
        lines[0].split_whitespace().collect::<Vec<_>>(),
        ["session", "client", "callsign", "lease", "pid"]
    );
    let fields = |line: &str| -> Vec<String> {
        line.split("  ")
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(str::to_string)
            .collect()
    };
    let a = fields(lines[1]);
    assert_eq!(a[..3], ["a-new", "claude", "-"], "{text}");
    assert!(a[3].starts_with("fresh "), "{text}");
    match alive {
        Some((pid, _)) => assert_eq!(a[4], format!("{pid} alive"), "{text}"),
        None => assert_eq!(a[4], "-", "{text}"),
    }
    let b = fields(lines[2]);
    assert_eq!(b[..3], ["b-old", "-", "beta"], "{text}");
    assert!(b[3].starts_with("stale 6m"), "{text}");
    assert_eq!(b[4], "4000000 dead", "{text}");
    let c = fields(lines[3]);
    assert_eq!(
        c,
        ["c-own", "-", "gamma", "fresh 30s", "-", "this session"],
        "{text}"
    );
    assert!(
        !lines[1].contains("this session") && !lines[2].contains("this session"),
        "{text}"
    );

    let v = envelope(&atc(
        path,
        &[("ATC_SHELL_SESSION", "c-own")],
        &["session", "--json"],
    ));
    let sessions = v["data"]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions[0]["session"], "a-new");
    assert_eq!(sessions[0]["client"], "claude");
    assert_eq!(sessions[0]["callsign"], serde_json::Value::Null);
    assert_eq!(sessions[0]["lease"]["fresh"], true);
    assert_eq!(sessions[0]["this"], false);
    assert_eq!(sessions[1]["lease"]["fresh"], false);
    assert_eq!(sessions[1]["lease"]["pid_alive"], false);
    assert_eq!(sessions[1]["pid"], 4_000_000);
    assert_eq!(sessions[2]["this"], true);
    assert_eq!(sessions[2]["pid"], serde_json::Value::Null);
    let mut keys: Vec<&str> = sessions[2]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["callsign", "client", "lease", "pid", "session", "this"]
    );

    // Not a heartbeat, and inside the expiry not a sweep either: the
    // stale lease is still there, and the listing wrote none of its own.
    assert!(leases(home).join("b-old").is_file());
    assert_eq!(std::fs::read_dir(leases(home)).unwrap().count(), 3);
}

/// The listing sweeps first, by mtime alone: a pidless lease past
/// `leaseExpiry` is gone, one with a live pid past it is gone all the
/// same, a stale one inside it is listed stale and kept, and a fresh
/// one is never touched. The repository's own `leaseExpiry` moves the
/// line.
#[test]
fn a_lease_past_the_expiry_is_gone_after_the_listing() {
    let repo = repo();
    let path = repo.path();
    let home = root(path);
    let alive = own_start().map(|start| (std::process::id(), start));
    plant(home, "dead", None, None, None, 90_000);
    plant(home, "old", None, None, None, 400);
    plant(home, "ghost", None, None, alive, 90_000);
    plant(home, "young", None, None, None, 3);

    let v = envelope(&atc(path, &[], &["session", "--json"]));
    let sessions = v["data"]["sessions"].as_array().unwrap();
    let names: Vec<&str> = sessions
        .iter()
        .map(|row| row["session"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["old", "young"], "{v}");
    assert_eq!(sessions[0]["lease"]["fresh"], false, "old is stale, kept");
    assert_eq!(sessions[1]["lease"]["fresh"], true);
    assert!(!leases(home).join("dead").exists(), "past the expiry, gone");
    assert!(
        !leases(home).join("ghost").exists(),
        "a live pid past the expiry is gone all the same"
    );
    assert!(leases(home).join("old").is_file());
    assert!(leases(home).join("young").is_file());

    repo.git(&["config", "tower.leaseExpiry", "1h"]);
    plant(home, "hour", None, None, None, 4_000);
    let text = stdout(&atc(path, &[], &["session"]));
    assert!(!text.contains("hour"), "{text}");
    assert!(!leases(home).join("hour").exists(), "past a shorter expiry");
    assert!(leases(home).join("old").is_file(), "inside it, kept");
}

/// The heartbeat's `sweep` marker lives in the lease directory and is
/// not a session: the listing never names it.
#[test]
fn the_marker_is_not_a_session() {
    let repo = repo();
    let path = repo.path();
    let home = root(path);
    plant(home, "s1", None, None, None, 3);
    std::fs::write(leases(home).join("sweep"), "0").unwrap();

    let v = envelope(&atc(path, &[], &["session", "--json"]));
    let sessions = v["data"]["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1, "{v}");
    assert_eq!(sessions[0]["session"], "s1");
    let text = stdout(&atc(path, &[], &["session"]));
    assert!(!text.contains("sweep"), "{text}");
}

/// `leaseWindow` from the repository you are in decides fresh: a 45s
/// lease is fresh under the default and stale under 30s, and outside a
/// repository the default rules.
#[test]
fn the_window_is_the_repositorys_setting() {
    let repo = repo();
    let path = repo.path();
    let home = root(path);
    plant(home, "s1", None, None, None, 45);

    let text = stdout(&atc(path, &[], &["session"]));
    assert!(text.contains("fresh 45s"), "{text}");
    repo.git(&["config", "tower.leaseWindow", "30s"]);
    let text = stdout(&atc(path, &[], &["session"]));
    assert!(text.contains("stale 45s"), "{text}");

    let elsewhere = tempfile::TempDir::new().unwrap();
    let out = command(elsewhere.path(), home, &[], &["session"])
        .output()
        .unwrap();
    assert!(
        stdout(&out).contains("fresh 45s"),
        "no repository, the default"
    );
}

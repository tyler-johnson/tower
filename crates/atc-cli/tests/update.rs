//! `atc update` against the real binary. Test builds are never
//! official (ATC_OFFICIAL_BUILD unset), and classification precedes the
//! API call, so source instructions and refusals touch no network.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::{Repo, scrub};

fn atc(repo: &Path, args: &[&str]) -> Output {
    let root = repo.parent().expect("the fixture nests the repository");
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    command
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", root.join("xdg"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        // The update cache root forks to `LOCALAPPDATA` on Windows.
        .env("LOCALAPPDATA", root.join("cache"))
        .env("HOME", root)
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", root)
        .output()
        .expect("spawn atc")
}

#[test]
fn an_unofficial_build_prints_the_cargo_line_and_succeeds() {
    let repo = Repo::new();
    let out = atc(repo.path(), &["update"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    let err = String::from_utf8_lossy(&out.stdout);
    assert!(err.contains("atc was built from source"), "{err}");
    assert!(
        err.contains("cargo install --git https://github.com/tyler-johnson/tower atc-cli"),
        "{err}"
    );
}

#[test]
fn the_refusal_envelope_names_the_id_and_the_exit() {
    let repo = Repo::new();
    let out = atc(repo.path(), &["update", "--json", "-y"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("an envelope");
    assert_eq!(envelope["cmd"], serde_json::json!("update"));
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("update/source-build")
    );
    let message = envelope["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("cargo install --git https://github.com/tyler-johnson/tower atc-cli"),
        "{message}"
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["cargo install --git https://github.com/tyler-johnson/tower atc-cli"])
    );
}

#[test]
fn the_background_check_is_silent_and_exits_zero() {
    // The detached child's contract: whatever happens — no repo, no
    // network, nothing to say — it exits 0 with no output, because
    // nothing is attached to hear it.
    let repo = Repo::new();
    // A failing local proxy keeps the explicit check offline and immediate.
    let root = repo.path().parent().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    let out = command
        .args(["update", "--check"])
        .current_dir(repo.path())
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("XDG_CONFIG_HOME", root.join("xdg"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("LOCALAPPDATA", root.join("cache"))
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("https_proxy", "http://127.0.0.1:1")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

#[test]
fn source_instructions_are_one_success_envelope() {
    let repo = Repo::new();
    let out = atc(repo.path(), &["update", "--json"]);
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(envelope["cmd"], "update");
    assert_eq!(envelope["data"]["channel"], "source");
    assert_eq!(envelope["data"]["status"], "instructions");
    assert_eq!(
        envelope["data"]["command"],
        "cargo install --git https://github.com/tyler-johnson/tower atc-cli"
    );
}

#[test]
fn yes_and_check_conflict() {
    let repo = Repo::new();
    let out = atc(repo.path(), &["update", "--check", "-y"]);
    assert_eq!(out.status.code(), Some(2));
}

//! `ff tower update` against the real binary. Test builds are never
//! official (TOWER_OFFICIAL_BUILD unset), and classification precedes the
//! API call, so the refusal below is hermetic — no network is touched.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    let root = repo.parent().expect("the fixture nests the repository");
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", root.join("xdg"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        // The update cache root forks to `LOCALAPPDATA` on Windows.
        .env("LOCALAPPDATA", root.join("cache"))
        .env("HOME", root)
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", root)
        .output()
        .expect("spawn ff-tower")
}

#[test]
fn an_unofficial_build_refuses_with_the_cargo_line() {
    let repo = Repo::new();
    let out = ff_tower(repo.path(), &["update"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ff-tower was built from source"), "{err}");
    assert!(
        err.contains("cargo install --git https://github.com/tyler-johnson/tower ff-tower-cli"),
        "{err}"
    );
}

#[test]
fn the_refusal_envelope_names_the_id_and_the_exit() {
    let repo = Repo::new();
    let out = ff_tower(repo.path(), &["update", "--json"]);
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
        message.contains("cargo install --git https://github.com/tyler-johnson/tower ff-tower-cli"),
        "{message}"
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!([
            "cargo install --git https://github.com/tyler-johnson/tower ff-tower-cli"
        ])
    );
}

#[test]
fn the_background_check_is_silent_and_exits_zero() {
    // The detached child's contract: whatever happens — no repo, no
    // network, nothing to say — it exits 0 with no output, because
    // nothing is attached to hear it.
    let repo = Repo::new();
    let out = ff_tower(repo.path(), &["update", "--check"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

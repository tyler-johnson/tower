//! Shared numbering through the production CLI and HTTP boundaries.
mod support;

use atc_testsupport::Repo;
use serde_json::{Value, json};
use support::{Server, atc, envelope, post};

fn run(repo: &Repo, args: &[&str]) -> Value {
    let out = atc(repo.path(), args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    envelope(&out)
}

fn configure(repo: &Repo, bare: &std::path::Path, writer: &str) {
    repo.pin_writer(writer);
    repo.git(&["remote", "add", "shared", bare.to_str().unwrap()]);
    repo.git(&["config", "tower.syncInterval", "1s"]);
    repo.git(&["config", "tower.numberTimeout", "1s"]);
}

#[test]
fn config_gates_enrollment_and_the_cli_keeps_wire_references() {
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("board.git");
    a.git(&["init", "--bare", bare.to_str().unwrap()]);
    configure(&a, &bare, "alpha");
    configure(&b, &bare, "beta");
    run(&a, &["config", "remote", "shared", "--json"]);
    run(&a, &["file", "remote one", "--json"]);
    let own = run(&b, &["file", "local one", "--json"])["data"]["flights"][0].clone();
    let id = own["id"].as_str().unwrap();
    run(&b, &["comment", id, "-m", "see #1", "--json"]);
    let out = atc(b.path(), &["config", "remote", "shared", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(envelope(&out)["error"]["id"], "sync/renumber-required");
    assert!(
        envelope(&out)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("1 local flights")
    );
    let refs = a.git(&[
        "--git-dir",
        bare.to_str().unwrap(),
        "for-each-ref",
        "--format=%(refname)",
        "refs/tower/log",
    ]);
    assert!(
        !refs.contains("beta"),
        "unconfirmed enrollment published its log"
    );
    run(&b, &["config", "remote", "shared", "--renumber", "--json"]);
    let brief = run(&b, &["brief", "2", "--json"]);
    assert_eq!(brief["data"]["id"], id);
    assert_eq!(brief["data"]["display"], "#2");
    assert_eq!(brief["data"]["comments"][0]["text"], format!("see #{id}"));
    for reference in [id, "beta#1", "2", "#2"] {
        assert_eq!(run(&b, &["brief", reference, "--json"])["data"]["id"], id);
    }
}

#[test]
fn http_mints_return_confirmed_rows_and_provisional_brief_inputs_stay_pinned() {
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("board.git");
    a.git(&["init", "--bare", bare.to_str().unwrap()]);
    configure(&a, &bare, "alpha");
    configure(&b, &bare, "beta");
    run(&a, &["config", "remote", "shared", "--json"]);
    run(&b, &["config", "remote", "shared", "--json"]);
    let server = Server::start(a.path(), &["--port", "0"], &[]);
    let (status, _, body) = post(&server.addr, "/api/file", r#"{"subject":"parent"}"#);
    assert_eq!(status, 200, "{body}");
    let filed: Value = serde_json::from_str(&body).unwrap();
    let parent = filed["data"]["flights"][0].clone();
    assert_eq!(parent["display"], "#1");
    let (status, _, body) = post(
        &server.addr,
        "/api/decompose",
        &json!({"flight": parent["id"], "parts": ["first", "second"]}).to_string(),
    );
    assert_eq!(status, 200, "{body}");
    let split: Value = serde_json::from_str(&body).unwrap();
    let rows = split["data"]["flights"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["display"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["#1", "#2", "#3"]
    );
    run(&b, &["config", "remote", "shared", "--json"]);
    b.git(&["remote", "set-url", "shared", "/missing-tower-test-remote"]);
    let offline = run(&b, &["file", "offline", "--json"])["data"]["flights"][0].clone();
    assert_eq!(offline["display"], "~4");
    run(&a, &["file", "takes four", "--json"]);
    b.git(&["remote", "set-url", "shared", bare.to_str().unwrap()]);
    let brief = run(&b, &["brief", "~4", "--json"]);
    assert_eq!(brief["data"]["id"], offline["id"]);
    assert_eq!(brief["data"]["display"], "#5");
}

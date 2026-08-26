//! `ff tower bay` and bare `done` against real repositories: the pool's
//! three paths, the occupancy gate, and the derivation the pool unlocks.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::ff::Ff;
use ff_tower_testsupport::Repo;

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .output()
        .expect("spawn ff-tower")
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

/// Assert a refusal: the exit code, and the envelope's error id.
fn refusal(output: &Output, code: i32, id: &str) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let envelope = envelope(output);
    assert_eq!(envelope["error"]["id"], serde_json::json!(id));
    envelope
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

/// Warm a bay through the binary and hand back its path as a string.
fn warm(repo: &Repo, name: &str, branch: &str) -> String {
    let bay = repo.bay_path(name);
    let bay = bay.to_str().expect("utf8");
    stdout(&ff_tower(repo.path(), &["bay", "warm", bay, branch]));
    bay.to_string()
}

/// Dirty a bay's tree and capture it under the flight's session tag —
/// what a claimed flight working in its bay does.
fn occupy(bay: &str, flight: &str) {
    std::fs::write(Path::new(bay).join("work.txt"), "an agent in a bay\n").expect("write");
    Ff::at(bay).session(flight).status().expect("status");
}

#[test]
fn warm_creates_a_bay_and_bare_bay_lists_it_free() {
    let repo = repo();
    let bay = repo.bay_path("bay1");
    let out = ff_tower(
        repo.path(),
        &[
            "bay",
            "warm",
            bay.to_str().expect("utf8"),
            "feather",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let warmed = envelope(&out);
    assert_eq!(warmed["cmd"], serde_json::json!("bay warm"));
    let added = &warmed["data"]["added"];
    assert_eq!(added["branch"], serde_json::json!("feather"));
    assert!(added["id"].is_string() && added["path"].is_string());

    // Bare `bay` and `bay list` are one verb: same render, same envelope.
    let bare = stdout(&ff_tower(repo.path(), &["bay"]));
    assert_eq!(bare, stdout(&ff_tower(repo.path(), &["bay", "list"])));
    assert!(bare.contains("feather"), "{bare}");
    assert!(bare.contains("free"), "{bare}");
    assert!(bare.contains("here"), "main is the invoking row: {bare}");
    assert!(bare.contains("2 bays"), "{bare}");

    let out = ff_tower(repo.path(), &["bay", "--json"]);
    let listed = envelope(&out);
    assert_eq!(listed["cmd"], serde_json::json!("bay list"));
    let bays = listed["data"]["bays"].as_array().expect("bays");
    assert_eq!(bays.len(), 2);
    assert_eq!(bays[0]["id"], serde_json::json!("main"));
    assert_eq!(bays[0]["current"], serde_json::json!(true));
    let bay = &bays[1];
    assert_eq!(bay["branch"], serde_json::json!("feather"));
    assert!(bay["flight"].is_null() && bay["subject"].is_null());
}

#[test]
fn a_bay_capture_occupies_the_list_and_the_board_sees_the_flight() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "bay work"]));
    let bay = warm(&repo, "bay1", "feather");
    occupy(&bay, "pi.1");

    let list = stdout(&ff_tower(repo.path(), &["bay"]));
    assert!(list.contains("#1  bay work"), "{list}");
    assert!(
        !list.contains("free") || list.matches("free").count() == 1,
        "{list}"
    );

    // The widened reads, end to end: the board rendered from main sees
    // the flight in the air on the bay's branch.
    let out = ff_tower(repo.path(), &["--json"]);
    let board = envelope(&out);
    let air = board["data"]["in_the_air"].as_array().expect("in_the_air");
    let flown = air
        .iter()
        .find(|view| view["id"] == serde_json::json!("pi.1"))
        .expect("pi.1 is in the air");
    assert_eq!(flown["branch"], serde_json::json!("feather"));
}

#[test]
fn release_of_an_occupied_bay_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "bay work"]));
    let bay = warm(&repo, "bay1", "feather");
    occupy(&bay, "pi.1");

    let out = ff_tower(repo.path(), &["bay", "release", &bay, "--json"]);
    let envelope = refusal(&out, 1, "bay/occupied");
    let message = envelope["error"]["message"].as_str().expect("message");
    assert!(message.contains("#1"), "{message}");
    assert!(message.contains("bay work"), "{message}");
    let exits = envelope["error"]["exits"].as_array().expect("exits");
    assert!(
        exits.iter().any(|exit| exit
            .as_str()
            .is_some_and(|e| e.starts_with("ff tower done"))),
        "{exits:?}"
    );
}

#[test]
fn release_of_a_free_bay_removes_it() {
    let repo = repo();
    let first = warm(&repo, "bay1", "feather");
    warm(&repo, "bay2", "quill");

    let text = stdout(&ff_tower(repo.path(), &["bay", "release", &first]));
    assert!(text.contains("released"), "{text}");
    assert!(text.contains(&first), "{text}");

    let out = ff_tower(repo.path(), &["bay", "release", "bay2", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let released = envelope(&out);
    assert_eq!(released["cmd"], serde_json::json!("bay release"));
    let removed = &released["data"]["removed"];
    assert_eq!(removed["id"], serde_json::json!("bay2"));
    assert_eq!(removed["branch"], serde_json::json!("quill"));
    assert!(removed["path"].is_string());

    let list = stdout(&ff_tower(repo.path(), &["bay"]));
    assert!(
        !list.contains("feather") && !list.contains("quill"),
        "{list}"
    );
    assert!(list.contains("1 bay "), "{list}");
}

#[test]
fn releasing_main_forwards_fufus_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["bay", "release", "main", "--json"]);
    refusal(&out, 1, "worktree/is-main");
}

#[test]
fn a_bare_done_inside_a_bay_finishes_its_flight() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "bay work"]));
    let bay = warm(&repo, "bay1", "feather");
    occupy(&bay, "pi.1");

    // Spawned with `FF_REPO=<bay>`, exactly what fufu's dispatch exports
    // when `ff tower done` runs inside the bay.
    let out = ff_tower(Path::new(&bay), &["done"]);
    let text = stdout(&out);
    assert!(text.contains("done #1: bay work"), "{text}");

    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(!board.contains("in the air"), "{board}");
}

#[test]
fn a_bare_done_with_no_session_work_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "untouched"]));
    let out = ff_tower(repo.path(), &["done", "--json"]);
    let envelope = refusal(&out, 2, "usage/needs-flight");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("no session-tagged work on this worktree — name the flight")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower done <flight>"])
    );
}

#[test]
fn a_bare_done_on_a_done_flight_is_already_done() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));
    repo.write("work.txt", "work on main\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let out = ff_tower(repo.path(), &["done", "--json"]);
    refusal(&out, 1, "flight/done");
}

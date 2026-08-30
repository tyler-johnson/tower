//! The two names of a flight: the dense human number against the sparse
//! wire id, and every spelling a verb accepts.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .output()
        .expect("spawn ff-tower")
}

/// The fixture's own config root, beside the repository inside the
/// tempdir and never created — an empty user layer. A suite that read the
/// developer's real `~/.config/tower/procedures` would pass or fail by
/// whose machine it is running on.
fn xdg(repo: &Path) -> std::path::PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .join("xdg")
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

/// File, comment, file: the comment consumes seq 2, so the second filing
/// is event `pi.3` — and flight #2.
fn repo_with_a_seq_gap() -> Repo {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the first flight"]));
    stdout(&ff_tower(repo.path(), &["comment", "1", "-m", "a note"]));
    stdout(&ff_tower(repo.path(), &["file", "the second flight"]));
    repo
}

#[test]
fn the_flight_number_is_dense_while_the_event_seq_is_not() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the first flight"]));
    stdout(&ff_tower(repo.path(), &["comment", "1", "-m", "a note"]));
    let out = stdout(&ff_tower(repo.path(), &["file", "the second flight"]));
    assert_eq!(
        out,
        "filed #2 in triage: the second flight\nboard: ff tower\n"
    );

    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(rendered.contains("#2"), "got {rendered}");

    // The split, visible on one view: flight #2 rides event pi.3.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let second = &board["data"]["open"][1];
    assert_eq!(second["id"], serde_json::json!("pi.3"));
    assert_eq!(second["number"], serde_json::json!(2));
}

#[test]
fn both_names_brief_the_same_flight() {
    let repo = repo_with_a_seq_gap();
    for reference in ["2", "pi.3"] {
        let brief = envelope(&ff_tower(repo.path(), &["brief", reference, "--json"]));
        assert_eq!(
            brief["data"]["id"],
            serde_json::json!("pi.3"),
            "`{reference}`"
        );
        assert_eq!(
            brief["data"]["number"],
            serde_json::json!(2),
            "`{reference}`"
        );
    }
}

#[test]
fn done_by_number_finishes_the_second_filed_flight_not_event_two() {
    let repo = repo_with_a_seq_gap();
    let out = stdout(&ff_tower(repo.path(), &["done", "2"]));
    assert_eq!(out, "done #2: the second flight\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let ids: Vec<&str> = open.iter().filter_map(|view| view["id"].as_str()).collect();
    assert_eq!(ids, ["pi.1"], "the first flight stays");
}

#[test]
fn two_writers_bind_with_hash_and_a_bare_number_is_ambiguous() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "from the pi"]));
    repo.pin_writer("qi");
    stdout(&ff_tower(repo.path(), &["file", "from the qi"]));

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("pi#1"), "long form binds with `#`: {out}");
    assert!(out.contains("qi#1"), "long form binds with `#`: {out}");
    assert!(!out.contains("pi.1"), "the wire form never renders: {out}");

    let out = ff_tower(repo.path(), &["comment", "1", "-m", "x", "--json"]);
    let refused = refusal(&out, 1, "flight/ambiguous");
    let message = refused["error"]["message"].as_str().expect("a message");
    assert!(message.contains("`pi#1`"), "got {message:?}");
    assert!(message.contains("`qi#1`"), "got {message:?}");

    // `writer#n` resolves exactly; the pasted `#`-prefixed form too.
    stdout(&ff_tower(repo.path(), &["comment", "qi#1", "-m", "one"]));
    stdout(&ff_tower(repo.path(), &["comment", "#qi#1", "-m", "two"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let qi = open
        .iter()
        .find(|view| view["id"] == serde_json::json!("qi.1"))
        .expect("qi.1 in open");
    assert_eq!(qi["comments"], serde_json::json!(2));

    let out = ff_tower(repo.path(), &["comment", "qi#9", "-m", "x", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn a_bad_reference_names_the_three_spellings() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["comment", "not-an-id", "-m", "x", "--json"]);
    let envelope = refusal(&out, 2, "usage/bad-flight");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!(
            "`not-an-id` is not a flight — `<n>`, `<writer>#<n>`, or `<writer>.<seq>`"
        )
    );
}

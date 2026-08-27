//! `ff tower next` against real repositories: the greedy admission, the
//! peek, the crew gate, and the exits — 1 drained, 3 needs-you.
//!
//! The crew gate makes bare `open` filings unclaimable — their stamp says
//! `you` — so every claimable fixture files under a two-part repo-layer
//! procedure whose `pass` part is agent-crewed; the flight `next` hands
//! out is that part, and the parent and `verdict` sit in your lane.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::ff::Ff;
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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    install_pipeline(&repo);
    repo
}

/// The minimal claimable shape principle 12 admits: an agent-crewed part
/// with a you-crewed end. Filing under it mints a parent and two parts —
/// three flights and three edges, so each filing consumes six event seqs
/// and three flight numbers.
fn install_pipeline(repo: &Repo) {
    repo.write(
        ".tower/procedures/pipeline.toml",
        concat!(
            "name = \"pipeline\"\n\n",
            "[[part]]\nid   = \"pass\"\ncrew = \"agent\"\n\n",
            "[[part]]\nid    = \"verdict\"\ncrew  = \"you\"\nafter = [\"pass\"]\n",
        ),
    );
}

fn file_pipeline(repo: &Repo, subject: &str) {
    stdout(&ff_tower(repo.path(), &["file", subject, "-p", "pipeline"]));
}

#[test]
fn next_claims_the_agent_part_and_the_second_ask_is_the_exit_3_needs_you() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #2: the one flight · pass"), "{text}");
    assert!(text.contains("board: ff tower"), "{text}");

    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(board.contains("in the air"), "{board}");
    assert!(board.contains("claimed"), "{board}");

    // The pool is empty but the parent and `verdict` are yours — work
    // exists and it needs you, which is 3, not fufu's 1.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(3));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready — two flights need you"), "{text}");
    assert!(text.contains("ff tower triage"), "{text}");
}

#[test]
fn the_needs_you_count_rides_the_envelope() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "needs a look"]));

    let out = ff_tower(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(1));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(3));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready — one flight needs you"), "{text}");
}

#[test]
fn the_parent_is_never_picked_and_exit_1_needs_a_truly_drained_board() {
    let repo = repo();
    file_pipeline(&repo, "a broad task");

    let text = stdout(&ff_tower(repo.path(), &["next"]));
    assert!(text.contains("claimed #2: a broad task · pass"), "{text}");
    stdout(&ff_tower(repo.path(), &["done", "2"]));

    // `verdict` is ready now, but you-crewed: never picked, counted.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(3));
    stdout(&ff_tower(repo.path(), &["done", "3"]));

    // Every part done leaves the parent live and permanently un-nextable
    // — principle 12 at the flight level. Only `done` ends it.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(3));
    let envelope = envelope(&ff_tower(repo.path(), &["next", "--json"]));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(1));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1), "drained at last");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(!text.contains("need you"), "{text}");
}

#[test]
fn peek_reads_without_claiming() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = ff_tower(repo.path(), &["next", "--peek"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("ready #2: the one flight · pass"), "{text}");
    assert!(!text.contains("claimed"), "{text}");

    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(board.contains("open"), "{board}");
    assert!(
        !board.contains("in the air"),
        "the peek wrote nothing: {board}"
    );

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    assert_eq!(
        envelope["data"]["picked"][0]["flight"],
        serde_json::json!("pi.2")
    );
}

#[test]
fn an_undone_dependency_is_passed_as_waiting_and_the_dependency_is_claimed() {
    let repo = repo();
    file_pipeline(&repo, "the dependent");
    file_pipeline(&repo, "the dependency");
    // The agent parts: the first filing's is #2, the second's is #5.
    stdout(&ff_tower(repo.path(), &["link", "2", "5"]));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #5: the dependency · pass"), "{text}");
    assert!(text.contains("passed #2 · waiting on #5"), "{text}");

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    let envelope = envelope(&out);
    let passed = &envelope["data"]["passed"];
    assert_eq!(passed[0]["flight"], serde_json::json!("pi.2"));
    assert_eq!(passed[0]["reason"], serde_json::json!("waiting"));
    assert_eq!(passed[0]["on"], serde_json::json!(["pi.8"]));
}

#[test]
fn deconfliction_passes_the_collider_claims_the_clear_one_and_pins_the_json() {
    let repo = repo();
    file_pipeline(&repo, "left work");
    file_pipeline(&repo, "right work");
    file_pipeline(&repo, "third work");
    // Claim the first filing's agent part and put its work on a branch.
    stdout(&ff_tower(repo.path(), &["claim", "2"]));

    repo.ff(&["start", "-b", "left"]);
    repo.write("shared.txt", "left side\n");
    Ff::at(repo.path())
        .session("pi.2")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "left: touch shared"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("shared.txt", "right side\n");
    Ff::at(repo.path())
        .session("pi.8")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "right: touch shared"]);

    // The peek first, so the JSON pins the same pick the claiming run
    // takes a line below — after the claim the pool would be different.
    let out = ff_tower(repo.path(), &["next", "-n", "2", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    let picked = envelope["data"]["picked"].as_array().expect("picked");
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0]["flight"], serde_json::json!("pi.14"));
    let passed = &envelope["data"]["passed"];
    assert_eq!(passed[0]["flight"], serde_json::json!("pi.8"));
    assert_eq!(passed[0]["reason"], serde_json::json!("collides"));
    assert_eq!(passed[0]["with"], serde_json::json!("pi.2"));
    assert_eq!(passed[0]["paths"], serde_json::json!(["shared.txt"]));

    let out = ff_tower(repo.path(), &["next", "-n", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #8: third work · pass"), "{text}");
    assert!(
        text.contains("passed #5 · collides with #2 on shared.txt"),
        "{text}"
    );

    let board = stdout(&ff_tower(repo.path(), &["--json"]));
    let board: serde_json::Value = serde_json::from_str(&board).expect("an envelope");
    let air = board["data"]["in_the_air"].as_array().expect("in_the_air");
    let picked = air
        .iter()
        .find(|view| view["id"] == serde_json::json!("pi.14"))
        .expect("pi.14 is in the air");
    assert!(picked["claimed_by"].is_string(), "{picked}");
}

#[test]
fn a_zero_count_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["next", "-n", "0", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("usage/bad-count")
    );
}

#[test]
fn an_empty_pick_under_json_is_a_data_envelope_not_an_error() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("next"));
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["claimed"], serde_json::json!(false));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(0));
    assert!(
        envelope.get("error").is_none(),
        "data and error, never both"
    );
}

//! `ff tower explain` against real repositories: the standing line, the
//! routing, and the beat rows — one flight's slice of `next`'s walk.
//!
//! Same fixture grammar as `next.rs`: the crew gate makes bare `open`
//! filings unclaimable, so pool fixtures file under the two-part
//! `pipeline` procedure whose `pass` part is agent-crewed. Each filing
//! mints six event seqs and three flight numbers, so the agent parts are
//! `pi.2` (#2) and `pi.8` (#5).

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
/// with a you-crewed end.
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

/// Two agent parts on branches that edit the same file — the walk admits
/// #2 and passes #5 naming it.
fn colliding(repo: &Repo) {
    file_pipeline(repo, "left work");
    file_pipeline(repo, "right work");

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
}

#[test]
fn a_ready_flight_reports_ready_and_what_it_beat() {
    let repo = repo();
    colliding(&repo);

    let out = ff_tower(repo.path(), &["explain", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("#2  left work · pass"), "{text}");
    assert!(text.contains("ready"), "{text}");
    assert!(text.contains("beat #5 · collides on shared.txt"), "{text}");
    assert!(text.contains("board: ff tower"), "{text}");

    let out = ff_tower(repo.path(), &["explain", "5"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("collides with #2 on shared.txt"), "{text}");
    assert!(
        !text.contains("beat"),
        "a passed flight blocks nothing: {text}"
    );
}

#[test]
fn a_claimed_flights_beat_is_what_its_branch_blocks() {
    let repo = repo();
    colliding(&repo);
    stdout(&ff_tower(repo.path(), &["next"]));

    let out = ff_tower(repo.path(), &["explain", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed by"), "{text}");
    assert!(text.contains("beat #5 · collides on shared.txt"), "{text}");
}

#[test]
fn a_bare_filing_explains_as_yours() {
    // Bare `file` lands under `open`, whose single part is you-crewed —
    // the stamp is what keeps it out of the pool, and the explain says so.
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "needs a look"]));

    let out = ff_tower(repo.path(), &["explain", "1"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("yours — crewed you"), "{text}");
}

#[test]
fn a_stampless_parent_explains_as_yours_with_no_stamp() {
    let repo = repo();
    file_pipeline(&repo, "a broad task");

    let out = ff_tower(repo.path(), &["explain", "1"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("yours — no part stamp"), "{text}");
}

#[test]
fn a_held_flight_carries_the_question_text() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");
    // `hold` exits 3 by design — the fixture tolerates it.
    let held = ff_tower(repo.path(), &["hold", "2", "-m", "which half?"]);
    assert_eq!(held.status.code(), Some(3));

    let out = ff_tower(repo.path(), &["explain", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("which half?"), "{text}");
    assert!(text.contains("asked by"), "{text}");
}

#[test]
fn a_done_flight_explains_with_its_mark() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");
    stdout(&ff_tower(repo.path(), &["next"]));
    stdout(&ff_tower(repo.path(), &["done", "2"]));

    let out = ff_tower(repo.path(), &["explain", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("done by"), "{text}");
}

#[test]
fn the_json_pins_the_flattened_standing() {
    let repo = repo();
    colliding(&repo);

    let out = ff_tower(repo.path(), &["explain", "5", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("explain"));
    let data = &envelope["data"];
    assert_eq!(data["id"], serde_json::json!("pi.8"));
    assert_eq!(data["number"], serde_json::json!(5));
    assert_eq!(data["standing"], serde_json::json!("collides"));
    assert_eq!(data["with"], serde_json::json!("pi.2"));
    assert_eq!(data["paths"], serde_json::json!(["shared.txt"]));
    assert_eq!(data["beat"], serde_json::json!([]));
    assert_eq!(data["procedure"], serde_json::json!("pipeline"));
    let data = data.as_object().expect("data is an object");
    for key in ["routed_by", "routed_at", "because", "part"] {
        assert!(data.contains_key(key), "data is missing `{key}`");
    }

    let out = ff_tower(repo.path(), &["explain", "2", "--json"]);
    let envelope = serde_json::from_str::<serde_json::Value>(&stdout(&out)).expect("an envelope");
    let data = &envelope["data"];
    assert_eq!(data["standing"], serde_json::json!("ready"));
    let beat = data["beat"].as_array().expect("beat");
    assert_eq!(beat.len(), 1);
    assert_eq!(beat[0]["flight"], serde_json::json!("pi.8"));
    assert_eq!(beat[0]["reason"], serde_json::json!("collides"));
    assert_eq!(beat[0]["with"], serde_json::json!("pi.2"));
}

#[test]
fn an_unknown_flight_is_the_not_found_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["explain", "99", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["cmd"], serde_json::json!("explain"));
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("flight/not-found")
    );
    assert!(envelope.get("data").is_none(), "data and error, never both");
}

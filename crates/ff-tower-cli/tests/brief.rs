//! `ff tower brief` against real repositories: the full record in one
//! read, the standing and beat rows — one flight's slice of `next`'s
//! walk — the JSON round-trip, the lazy probes, and the refusals.
//!
//! The pool fixtures share `next.rs`'s grammar: the pool is Ready
//! flights in the agent lane, so bare filings — born Triage — are never
//! candidates, and the pullable ones file under the two-flight
//! `pipeline` procedure whose `pass` is agent-assigned and born Ready.
//! Each filing mints six event seqs and three flight numbers, so the
//! agent flights are `pi.2` (#2) and `pi.8` (#5).

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::{FakeFf, Repo};

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    ff_tower_via(repo, args, None)
}

/// The spawn, with the `TOWER_FF` seam: a named program reaches the
/// binary's `ff()` through the environment, the way `cmd/mod.rs` reads
/// it back.
fn ff_tower_via(repo: &Path, args: &[&str], program: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ff-tower"));
    command
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo));
    if let Some(program) = program {
        command.env("TOWER_FF", program);
    }
    command.output().expect("spawn ff-tower")
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
    install_pipeline(&repo);
    repo
}

/// The minimal pullable shape principle 12 admits: an agent-assigned
/// flight with a me-assigned end.
fn install_pipeline(repo: &Repo) {
    repo.write(
        ".tower/procedures/pipeline.toml",
        concat!(
            "name = \"pipeline\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );
}

/// `docs/procedures/review.toml`'s shape, for the one test that needs a
/// flight born with a skill and another born with a bay.
fn install_review(repo: &Repo) {
    repo.write(
        ".tower/procedures/review.toml",
        concat!(
            "name    = \"review\"\nsubject = \"branch\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nskill    = \"review\"\n\n",
            "[[flight]]\nid       = \"smoke\"\nassignee = \"me\"\nbay      = \"warm\"\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\", \"smoke\"]\n",
        ),
    );
}

fn file_pipeline(repo: &Repo, subject: &str) {
    stdout(&ff_tower(repo.path(), &["file", "pipeline", subject]));
}

/// Two agent flights on branches that edit the same file — the walk admits
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

/// A fake `ff` that records every argv it was asked before answering with
/// the real one — the probe-laziness assertions read the log back.
fn logging_ff() -> FakeFf {
    FakeFf::script(concat!(
        "printf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/calls.log\"\n",
        "exec ff \"$@\"\n",
    ))
}

fn calls(fake: &FakeFf) -> String {
    std::fs::read_to_string(fake.dir().join("calls.log")).unwrap_or_default()
}

/// Two linked flights, a body on the first, a comment on the first.
fn repo_with_a_record() -> Repo {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the dependent", "-m", "the body of the work"],
    ));
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["link", "1", "2"]));
    stdout(&ff_tower(
        repo.path(),
        &["comment", "1", "-m", "a note on the record"],
    ));
    repo
}

#[test]
fn brief_renders_the_full_record_both_link_directions() {
    let repo = repo_with_a_record();

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("#1  the dependent"), "{text}");
    assert!(text.contains("the body of the work"), "{text}");
    assert!(text.contains("children\n· #2  the dependency"), "{text}");
    assert!(text.contains("comments\n"), "{text}");
    // The header leads with the comment's wire id — its only name, and
    // what `edit` takes.
    assert!(text.contains("pi.4 · tests@tower.invalid · "), "{text}");
    assert!(text.contains("a note on the record"), "{text}");
    assert!(text.contains("filed "), "{text}");
    assert!(text.contains("board: ff tower"), "{text}");
    assert!(
        !text.contains('\x1b'),
        "piped output has escape bytes: {text:?}"
    );

    let text = stdout(&ff_tower(repo.path(), &["brief", "2"]));
    assert!(text.contains("#2  the dependency"), "{text}");
    assert!(text.contains("parents\n· #1  the dependent"), "{text}");
    assert!(!text.contains("children"), "{text}");
}

#[test]
fn the_stored_fields_get_their_own_line_under_the_head() {
    let repo = repo();
    install_review(&repo);
    stdout(&ff_tower(
        repo.path(),
        &["file", "review", "the retry test"],
    ));

    let text = stdout(&ff_tower(repo.path(), &["brief", "2"]));
    assert!(text.contains("#2  the retry test · pass\n"), "{text}");
    assert!(
        text.contains("    assignee agent · skill review · under review\n"),
        "{text}"
    );

    let text = stdout(&ff_tower(repo.path(), &["brief", "3"]));
    assert!(
        text.contains("    assignee me · bay warm · under review\n"),
        "{text}"
    );

    let envelope = envelope(&ff_tower(repo.path(), &["brief", "2", "--json"]));
    assert_eq!(envelope["data"]["assignee"], serde_json::json!("agent"));
    assert_eq!(envelope["data"]["skill"], serde_json::json!("review"));
    assert_eq!(envelope["data"]["status"], serde_json::json!("ready"));
    assert!(
        envelope["data"]["bay"].is_null(),
        "absent facts are null: {envelope}"
    );
}

#[test]
fn json_round_trips_the_brief() {
    let repo = repo_with_a_record();

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("brief"));
    let data = &envelope["data"];
    assert_eq!(data["id"], serde_json::json!("pi.1"));
    assert_eq!(data["subject"], serde_json::json!("the dependent"));
    assert_eq!(data["body"], serde_json::json!("the body of the work"));
    assert_eq!(
        data["comments"][0]["text"],
        serde_json::json!("a note on the record")
    );
    assert_eq!(data["depends_on"][0]["flight"], serde_json::json!("pi.2"));
    assert_eq!(
        data["depends_on"][0]["subject"],
        serde_json::json!("the dependency")
    );
    assert_eq!(data["depends_on"][0]["closed"], serde_json::json!(false));
    assert_eq!(data["status"], serde_json::json!("triage"));
    // Absent facts are null, never missing keys.
    assert!(data["branch"].is_null(), "{data}");
    assert!(data["status_by"].is_null(), "{data}");
    assert!(data["question"].is_null(), "{data}");
}

#[test]
fn a_session_tagged_flight_carries_its_branch_and_tip() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "branch work"]));

    repo.ff(&["start", "-b", "work"]);
    repo.write("work.txt", "work\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "work: a commit"]);

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["branch"], serde_json::json!("work"));
    let tip = envelope["data"]["tip"].as_str().expect("a tip").to_string();

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains(&format!("on work {}", &tip[..8])), "{text}");
}

#[test]
fn a_held_flight_renders_its_question() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck work"]));
    let out = ff_tower(repo.path(), &["hold", "1", "-m", "which color?"]);
    assert_eq!(out.status.code(), Some(3));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("which color?"), "{text}");
    assert!(text.contains("asked "), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["question"], serde_json::json!("which color?"));
    assert_eq!(data["asked_by"], serde_json::json!("tests@tower.invalid"));
}

#[test]
fn a_closed_flight_still_briefs_with_its_move() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "finished work", "-m", "what it was about"],
    ));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("done — tests@tower.invalid"), "{text}");
    assert!(text.contains("what it was about"), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["status"], serde_json::json!("done"));
    assert_eq!(data["status_by"], serde_json::json!("tests@tower.invalid"));
    assert!(data["status_at"].is_i64(), "{data}");
}

#[test]
fn a_closed_dependency_marks_its_link_row() {
    let repo = repo_with_a_record();
    stdout(&ff_tower(repo.path(), &["done", "2"]));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("· #2  the dependency  done"), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["depends_on"][0]["closed"], serde_json::json!(true));
    assert_eq!(data["depends_on"][0]["status"], serde_json::json!("done"));
}

#[test]
fn the_history_lists_every_gesture_in_log_order() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the work", "-m", "the body of the work"],
    ));
    stdout(&ff_tower(repo.path(), &["comment", "1", "-m", "a note"]));
    stdout(&ff_tower(repo.path(), &["status", "1", "in_progress"]));
    let held = ff_tower(repo.path(), &["hold", "1", "-m", "which way?"]);
    assert_eq!(held.status.code(), Some(3));
    stdout(&ff_tower(repo.path(), &["answer", "1", "-m", "that way"]));
    stdout(&ff_tower(
        repo.path(),
        &["edit", "1", "-s", "the reworded work"],
    ));
    // A reword of the comment, targeting its event id rather than the
    // flight's — a gesture on this flight all the same.
    stdout(&ff_tower(
        repo.path(),
        &["edit", "pi.2", "-m", "a fuller note"],
    ));

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    // The filing leads, and each row is the wire id, the kind's own
    // name, the author, and the age.
    assert!(
        text.contains("history\n  pi.1 · filed · tests@tower.invalid · "),
        "{text}"
    );
    assert!(
        text.contains("pi.7 · edited · tests@tower.invalid · "),
        "{text}"
    );

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    let history = data["history"].as_array().expect("a history");
    let what: Vec<&str> = history
        .iter()
        .map(|moment| moment["what"].as_str().expect("a kind name"))
        .collect();
    assert_eq!(
        what,
        [
            "filed",
            "commented",
            "status",
            "held",
            "answered",
            "edited",
            "edited",
        ]
    );
    assert_eq!(history[0]["id"], serde_json::json!("pi.1"));
    assert_eq!(history[0]["by"], serde_json::json!("tests@tower.invalid"));
    assert!(history[0]["at"].is_i64(), "{data}");
}

#[test]
fn an_unknown_kind_naming_the_flight_lands_under_its_own_name() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "work worth promoting"]));

    // A gesture from a newer tower. The fold cannot route it — it lands
    // in `unrouted` — but its body names this flight, so the history
    // carries it under the kind's own string instead of dropping it.
    Store::open(repo.path())
        .expect("open")
        .append(vec![Kind::Unknown {
            kind: "promoted".to_string(),
            body: serde_json::value::RawValue::from_string(
                r#"{"flight":"pi.1","upstream":"LIN-123"}"#.to_string(),
            )
            .expect("raw"),
        }])
        .expect("append");

    let text = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(text.contains("pi.2 · promoted · "), "{text}");

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let history = envelope(&out)["data"]["history"]
        .as_array()
        .expect("a history")
        .clone();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1]["what"], serde_json::json!("promoted"));

    // A body naming some other flight stays off this one's history.
    Store::open(repo.path())
        .expect("open")
        .append(vec![Kind::Unknown {
            kind: "promoted".to_string(),
            body: serde_json::value::RawValue::from_string(r#"{"flight":"pi.99"}"#.to_string())
                .expect("raw"),
        }])
        .expect("append");
    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let history = envelope(&out)["data"]["history"]
        .as_array()
        .expect("a history")
        .clone();
    assert_eq!(history.len(), 2);
}

#[test]
fn bare_seq_and_hash_prefixed_and_full_references_all_resolve() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the flight"]));
    for reference in ["1", "#1", "pi.1"] {
        let text = stdout(&ff_tower(repo.path(), &["brief", reference]));
        assert!(text.contains("#1  the flight"), "`{reference}`: {text}");
    }
}

#[test]
fn an_unknown_flight_refuses_not_found() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the only flight"]));
    let out = ff_tower(repo.path(), &["brief", "9", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn a_malformed_reference_refuses_bad_flight() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["brief", "not-an-id", "--json"]);
    refusal(&out, 2, "usage/bad-flight");
}

#[test]
fn a_ready_flight_briefs_ready_and_what_it_beat() {
    let repo = repo();
    colliding(&repo);

    let out = ff_tower(repo.path(), &["brief", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("#2  left work · pass"), "{text}");
    assert!(text.contains("ready"), "{text}");
    assert!(text.contains("beat #5 · collides on shared.txt"), "{text}");
    assert!(text.contains("board: ff tower"), "{text}");

    let out = ff_tower(repo.path(), &["brief", "5"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("collides with #2 on shared.txt"), "{text}");
    assert!(
        !text.contains("beat"),
        "a passed flight blocks nothing: {text}"
    );
}

#[test]
fn a_pulled_flights_beat_is_what_its_branch_blocks() {
    let repo = repo();
    colliding(&repo);
    stdout(&ff_tower(repo.path(), &["next"]));

    let out = ff_tower(repo.path(), &["brief", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("in progress — tests@tower.invalid"), "{text}");
    assert!(text.contains("beat #5 · collides on shared.txt"), "{text}");
}

#[test]
fn a_bare_filing_briefs_as_yours() {
    // Bare `file` lands in Triage with no lane — the stored fields are
    // what keep it out of the pool, and the brief says so.
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "needs a look"]));

    let out = ff_tower(repo.path(), &["brief", "1"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("triage"), "{text}");
    assert!(text.contains("yours — unassigned"), "{text}");
}

#[test]
fn a_me_laned_ready_flight_briefs_as_yours_with_its_lane() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "needs a look", "--assignee", "me"],
    ));
    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));

    let out = ff_tower(repo.path(), &["brief", "1"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("yours — assigned me"), "{text}");
}

#[test]
fn the_json_pins_the_merged_envelope() {
    let repo = repo();
    colliding(&repo);

    // The flattened standing rides beside the brief's own record — one
    // envelope, no inner nesting, and the walk payload keys only where
    // the variant carries them.
    let out = ff_tower(repo.path(), &["brief", "5", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("brief"));
    let data = &envelope["data"];
    assert_eq!(data["id"], serde_json::json!("pi.8"));
    assert_eq!(data["number"], serde_json::json!(5));
    assert_eq!(data["standing"], serde_json::json!("collides"));
    assert_eq!(data["with"], serde_json::json!("pi.2"));
    assert_eq!(data["paths"], serde_json::json!(["shared.txt"]));
    assert_eq!(data["beat"], serde_json::json!([]));
    assert_eq!(data["procedure"], serde_json::json!("pipeline"));
    assert_eq!(data["subject"], serde_json::json!("right work · pass"));
    let data = data.as_object().expect("data is an object");
    for key in [
        "status",
        "status_by",
        "assignee",
        "priority",
        "labels",
        "skill",
        "bay",
        "body",
        "comments",
    ] {
        assert!(data.contains_key(key), "data is missing `{key}`");
    }

    let out = ff_tower(repo.path(), &["brief", "2", "--json"]);
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
fn a_closed_flights_standing_carries_no_duplicate_payload() {
    // The slimmed variants flatten to the tag alone: `status_by` appears
    // once, from the brief's own field — serde flatten would otherwise
    // emit the key twice and the envelope would stop being an object a
    // strict parser accepts.
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished work"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let out = ff_tower(repo.path(), &["brief", "1", "--json"]);
    let raw = stdout(&out);
    assert_eq!(raw.matches("\"status_by\"").count(), 1, "{raw}");
    let envelope = serde_json::from_str::<serde_json::Value>(&raw).expect("an envelope");
    let data = &envelope["data"];
    assert_eq!(data["standing"], serde_json::json!("done"));
    assert_eq!(data["status_by"], serde_json::json!("tests@tower.invalid"));
    let data = data.as_object().expect("data is an object");
    for key in ["with", "paths", "on"] {
        assert!(
            !data.contains_key(key),
            "`{key}` belongs to the walk variants alone"
        );
    }
}

#[test]
fn a_live_on_branch_flight_probes_and_a_closed_one_does_not() {
    // The laziness seam, end to end: `wants_verdicts` decides whether
    // the render spawns `ff collide` at all. Two live branches collide,
    // so a live candidate's brief must probe — and a done flight on the
    // same board must not, because probes cannot change its answer.
    let repo = repo();
    colliding(&repo);

    let probing = logging_ff();
    let out = ff_tower_via(repo.path(), &["brief", "2"], Some(probing.path()));
    let text = stdout(&out);
    assert!(text.contains("beat #5 · collides on shared.txt"), "{text}");
    assert!(
        calls(&probing).lines().any(|line| line.contains("collide")),
        "a live on-branch brief rides the probes: {}",
        calls(&probing)
    );

    stdout(&ff_tower(repo.path(), &["file", "extra"]));
    stdout(&ff_tower(repo.path(), &["done", "7"]));

    let lazy = logging_ff();
    let out = ff_tower_via(repo.path(), &["brief", "7"], Some(lazy.path()));
    let text = stdout(&out);
    assert!(text.contains("done — tests@tower.invalid"), "{text}");
    assert!(
        !calls(&lazy).lines().any(|line| line.contains("collide")),
        "a done flight briefs with zero probes: {}",
        calls(&lazy)
    );
}

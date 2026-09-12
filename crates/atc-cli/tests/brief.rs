//! `atc brief` against real repositories: the full record in one
//! read, the standing, the JSON round-trip, and the refusals.
//!
//! The pool fixtures share `next.rs`'s grammar: the pool is Ready
//! flights in the agent lane, so bare filings — born Ready but laned to
//! no one — are never candidates, and the pullable ones file under the two-flight
//! `pipeline` procedure whose `pass` is agent-assigned and born Ready.
//! Each filing mints six event seqs and three flight numbers, so the
//! agent flights are `pi.2` (#2) and `pi.8` (#5).

use std::path::Path;
use std::process::{Command, Output};

use atc_core::log::{Kind, Store};
use atc_testsupport::{Repo, scrub};

fn atc(repo: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    // A developer's own Claude Code session must not tag or stamp the
    // fixture's events: the bylines below assert the bare email.
    scrub(&mut command);
    command
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .output()
        .expect("spawn atc")
}

/// The spawn under a session tag, the way Claude Code hands one down.
fn atc_tagged(repo: &Path, args: &[&str], tag: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    command
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .env("CLAUDE_CODE_SESSION_ID", tag)
        .output()
        .expect("spawn atc")
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
/// flight born with a skill.
fn install_review(repo: &Repo) {
    repo.write(
        ".tower/procedures/review.toml",
        concat!(
            "name    = \"review\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nskill    = \"review\"\n\n",
            "[[flight]]\nid       = \"smoke\"\nassignee = \"me\"\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\", \"smoke\"]\n",
        ),
    );
}

fn file_pipeline(repo: &Repo, subject: &str) {
    stdout(&atc(repo.path(), &["file", "pipeline", subject]));
}

/// Two linked flights, a body on the first, a comment on the first.
fn repo_with_a_record() -> Repo {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "the dependent", "-m", "the body of the work"],
    ));
    stdout(&atc(repo.path(), &["file", "the dependency"]));
    stdout(&atc(repo.path(), &["link", "1", "2"]));
    stdout(&atc(
        repo.path(),
        &["comment", "1", "-m", "a note on the record"],
    ));
    repo
}

#[test]
fn brief_renders_the_full_record_both_link_directions() {
    let repo = repo_with_a_record();

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("#1  the dependent"), "{text}");
    assert!(text.contains("the body of the work"), "{text}");
    assert!(text.contains("children\n· #2  the dependency"), "{text}");
    assert!(text.contains("comments\n"), "{text}");
    // The header leads with the comment's wire id — its only name, and
    // what `edit` takes.
    assert!(text.contains("pi.4 · tests@tower.invalid · "), "{text}");
    assert!(text.contains("a note on the record"), "{text}");
    assert!(text.contains("filed "), "{text}");
    assert!(text.contains("board: atc"), "{text}");
    assert!(
        !text.contains('\x1b'),
        "piped output has escape bytes: {text:?}"
    );

    let text = stdout(&atc(repo.path(), &["brief", "2"]));
    assert!(text.contains("#2  the dependency"), "{text}");
    assert!(text.contains("parents\n· #1  the dependent"), "{text}");
    assert!(!text.contains("children"), "{text}");
}

#[test]
fn the_stored_fields_get_their_own_line_under_the_head() {
    let repo = repo();
    install_review(&repo);
    stdout(&atc(repo.path(), &["file", "review", "the retry test"]));

    let text = stdout(&atc(repo.path(), &["brief", "2"]));
    assert!(text.contains("#2  the retry test · pass\n"), "{text}");
    assert!(
        text.contains("    assignee agent · skill review · under review\n"),
        "{text}"
    );

    let text = stdout(&atc(repo.path(), &["brief", "3"]));
    assert!(text.contains("    assignee me · under review\n"), "{text}");

    let envelope = envelope(&atc(repo.path(), &["brief", "2", "--json"]));
    assert_eq!(envelope["data"]["assignee"], serde_json::json!("agent"));
    assert_eq!(envelope["data"]["skill"], serde_json::json!("review"));
    assert_eq!(envelope["data"]["status"], serde_json::json!("ready"));
}

#[test]
fn json_round_trips_the_brief() {
    let repo = repo_with_a_record();

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["atc"], serde_json::json!(1));
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
    assert_eq!(
        data["status"],
        serde_json::json!("waiting"),
        "born Ready, gated by the edge"
    );
    // Absent facts are null, never missing keys.
    assert!(data["status_by"].is_null(), "{data}");
    assert!(data["question"].is_null(), "{data}");
}

#[test]
fn show_and_brief_agree_byte_for_byte() {
    let repo = repo_with_a_record();
    let brief = stdout(&atc(repo.path(), &["brief", "1"]));
    let show = stdout(&atc(repo.path(), &["show", "1"]));
    assert_eq!(show, brief);

    // A spelling, not a verb: the envelope names `brief` under either.
    let brief = atc(repo.path(), &["brief", "1", "--json"]);
    let show = atc(repo.path(), &["show", "1", "--json"]);
    assert_eq!(stdout(&show), stdout(&brief));
    assert_eq!(envelope(&show)["cmd"], serde_json::json!("brief"));
}

#[test]
fn a_held_flight_renders_its_question() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "stuck work"]));
    let out = atc(repo.path(), &["hold", "1", "-m", "which color?"]);
    assert_eq!(out.status.code(), Some(3));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("which color?"), "{text}");
    assert!(text.contains("asked "), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["question"], serde_json::json!("which color?"));
    assert_eq!(data["asked_by"], serde_json::json!("tests@tower.invalid"));
}

#[test]
fn a_cancel_over_a_question_briefs_the_reason_and_keeps_the_hold_in_history() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "stuck work"]));
    let out = atc(repo.path(), &["hold", "1", "-m", "which color?"]);
    assert_eq!(out.status.code(), Some(3));
    stdout(&atc(repo.path(), &["cancel", "1", "-m", "superseded"]));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    let note = text
        .lines()
        .find(|line| line.contains("canceled — tests@tower.invalid"))
        .expect("the note line");
    assert!(note.contains("superseded"), "{note}");
    assert!(!note.contains("which color?"), "{note}");
    assert!(!note.contains("asked "), "{note}");
    // The hold is still a moment of the record.
    assert!(text.contains("pi.2 · held · "), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert!(data["question"].is_null(), "{data}");
    assert!(data["asked_by"].is_null(), "{data}");
    assert!(data["asked_at"].is_null(), "{data}");
    assert_eq!(data["closed_reason"], serde_json::json!("superseded"));
    assert_eq!(data["status"], serde_json::json!("canceled"));
    let history = data["history"].as_array().expect("a history");
    assert!(
        history
            .iter()
            .any(|moment| moment["what"] == "held" && moment["question"] == "which color?"),
        "{history:?}"
    );
}

#[test]
fn a_closed_flight_still_briefs_with_its_move() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "finished work", "-m", "what it was about"],
    ));
    stdout(&atc(repo.path(), &["done", "1"]));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("done — tests@tower.invalid"), "{text}");
    assert!(text.contains("what it was about"), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["status"], serde_json::json!("done"));
    assert_eq!(data["status_by"], serde_json::json!("tests@tower.invalid"));
    assert!(data["status_at"].is_i64(), "{data}");
}

#[test]
fn a_closed_dependency_marks_its_link_row() {
    let repo = repo_with_a_record();
    stdout(&atc(repo.path(), &["done", "2"]));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("· #2  the dependency  done"), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["depends_on"][0]["closed"], serde_json::json!(true));
    assert_eq!(data["depends_on"][0]["status"], serde_json::json!("done"));
}

#[test]
fn the_byline_carries_the_session() {
    // The wire keeps the full id, so it cross-references fufu's own
    // `ff op log 'session(<id>)'`; the human row shortens a UUID to its
    // first eight characters in brackets, and anything else renders
    // verbatim. The author stays the email underneath.
    let repo = repo();
    let uuid = "95b36d9d-efdc-4564-9b06-91842f51ef6b";
    stdout(&atc_tagged(repo.path(), &["file", "tagged"], uuid));
    stdout(&atc_tagged(
        repo.path(),
        &["status", "1", "in_progress"],
        "hand-typed",
    ));
    stdout(&atc(repo.path(), &["comment", "1", "-m", "a note"]));

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let data = &envelope(&out)["data"];
    assert_eq!(data["filed_by"], serde_json::json!("tests@tower.invalid"));
    assert_eq!(data["filed_session"], serde_json::json!(uuid));
    assert_eq!(data["status_by"], serde_json::json!("tests@tower.invalid"));
    assert_eq!(data["status_session"], serde_json::json!("hand-typed"));
    let history = data["history"].as_array().expect("a history");
    assert_eq!(history[0]["session"], serde_json::json!(uuid));
    assert_eq!(history[1]["session"], serde_json::json!("hand-typed"));
    assert!(history[2]["session"].is_null(), "{data}");
    assert_eq!(
        data["comments"][0]["author"],
        serde_json::json!("tests@tower.invalid")
    );
    assert!(data["comments"][0]["session"].is_null(), "{data}");

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("in progress — hand-typed "), "{text}");
    assert!(
        text.contains("history\n  pi.1 · filed · [95b36d9d] · "),
        "{text}"
    );
    assert!(
        text.contains("pi.2 · status in_progress · hand-typed · "),
        "{text}"
    );
    assert!(
        text.contains("pi.3 · commented · tests@tower.invalid · "),
        "{text}"
    );

    let board = envelope(&atc(repo.path(), &["--json"]));
    let flown = &board["data"]["in_progress"][0];
    assert_eq!(flown["filed_session"], serde_json::json!(uuid));
    assert_eq!(flown["status_session"], serde_json::json!("hand-typed"));
}

#[test]
fn the_byline_is_the_callsign_and_the_session_follows_under_it() {
    // A move under both a callsign and a session: the callsign is the
    // byline, and the session prints on its own dim line under the
    // entry, the way a reason does. The wire carries all three.
    let repo = repo();
    let uuid = "95b36d9d-efdc-4564-9b06-91842f51ef6b";
    stdout(&atc(repo.path(), &["file", "flown"]));
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    let out = command
        .args(["status", "1", "in_progress"])
        .current_dir(repo.path())
        .env("XDG_CONFIG_HOME", xdg(repo.path()))
        .env("CLAUDE_CODE_SESSION_ID", uuid)
        .env("ATC_CALLSIGN", "claude")
        .output()
        .expect("spawn atc");
    stdout(&out);
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    let out = command
        .args(["comment", "1", "-m", "a note"])
        .current_dir(repo.path())
        .env("XDG_CONFIG_HOME", xdg(repo.path()))
        .env("ATC_CALLSIGN", "tyler")
        .output()
        .expect("spawn atc");
    stdout(&out);

    let data = &envelope(&atc(repo.path(), &["brief", "1", "--json"]))["data"];
    assert!(data["filed_callsign"].is_null(), "{data}");
    assert_eq!(data["status_callsign"], serde_json::json!("claude"));
    assert_eq!(data["status_session"], serde_json::json!(uuid));
    assert_eq!(data["status_by"], serde_json::json!("tests@tower.invalid"));
    assert_eq!(data["comments"][0]["callsign"], serde_json::json!("tyler"));
    let history = data["history"].as_array().expect("a history");
    assert!(history[0]["callsign"].is_null(), "{data}");
    assert_eq!(history[1]["callsign"], serde_json::json!("claude"));
    assert_eq!(history[1]["session"], serde_json::json!(uuid));
    assert_eq!(history[2]["callsign"], serde_json::json!("tyler"));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("in progress — claude "), "{text}");
    assert!(
        text.contains("pi.2 · status in_progress · claude · "),
        "{text}"
    );
    assert!(
        text.contains("\n    session [95b36d9d]\n"),
        "the session follows the entry: {text}"
    );
    assert!(text.contains("pi.3 · commented · tyler · "), "{text}");
    assert!(
        !text.contains("session tyler"),
        "no session, no follow line: {text}"
    );
    assert!(
        text.contains("pi.1 · filed · tests@tower.invalid · "),
        "{text}"
    );
}

#[test]
fn the_history_lists_every_gesture_in_log_order() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "the work", "-m", "the body of the work"],
    ));
    stdout(&atc(repo.path(), &["comment", "1", "-m", "a note"]));
    stdout(&atc(repo.path(), &["status", "1", "in_progress"]));
    let held = atc(repo.path(), &["hold", "1", "-m", "which way?"]);
    assert_eq!(held.status.code(), Some(3));
    stdout(&atc(repo.path(), &["answer", "1", "-m", "that way"]));
    stdout(&atc(repo.path(), &["edit", "1", "-s", "the reworded work"]));
    // A reword of the comment, targeting its event id rather than the
    // flight's — a gesture on this flight all the same.
    stdout(&atc(repo.path(), &["edit", "pi.2", "-m", "a fuller note"]));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    // The filing leads, and each row is the wire id, the kind's own
    // name and the words it took, the author, and the age.
    assert!(
        text.contains("history\n  pi.1 · filed · tests@tower.invalid · "),
        "{text}"
    );
    assert!(
        text.contains("pi.3 · status in_progress · tests@tower.invalid · "),
        "{text}"
    );
    assert!(
        text.contains("pi.6 · edited subject · tests@tower.invalid · "),
        "{text}"
    );
    assert!(
        text.contains("pi.7 · edited comment pi.2 · tests@tower.invalid · "),
        "{text}"
    );

    let out = atc(repo.path(), &["brief", "1", "--json"]);
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
    // No tag and no terminal under the runner: the session is null, and
    // the key is on the row regardless.
    let filing = history[0].as_object().expect("an object");
    assert!(filing.contains_key("session"), "{data}");
    assert!(history[0]["session"].is_null(), "{data}");
    assert!(history[0]["at"].is_i64(), "{data}");
    // The words sit flat beside `what`, only where the kind carries them.
    assert_eq!(history[2]["status"], serde_json::json!("in_progress"));
    assert_eq!(history[5]["fields"], serde_json::json!(["subject"]));
    assert_eq!(history[6]["fields"], serde_json::json!(["body"]));
    assert_eq!(history[6]["comment"], serde_json::json!("pi.2"));
    let filing = history[0].as_object().expect("an object");
    for key in ["status", "fields", "from"] {
        assert!(!filing.contains_key(key), "a filing carries no `{key}`");
    }
}

#[test]
fn the_history_says_the_lane_and_the_edge() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the dependent"]));
    stdout(&atc(repo.path(), &["file", "the dependency"]));
    stdout(&atc(repo.path(), &["assign", "1", "agent"]));
    stdout(&atc(repo.path(), &["assign", "1", "none"]));
    stdout(&atc(repo.path(), &["link", "1", "2"]));
    stdout(&atc(repo.path(), &["unlink", "1", "2"]));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("pi.3 · assigned agent · "), "{text}");
    assert!(text.contains("pi.4 · assigned none · "), "{text}");
    assert!(text.contains("pi.5 · linked depends on #2 · "), "{text}");
    assert!(text.contains("pi.6 · unlinked depends on #2 · "), "{text}");

    // The same edge, read from the other end.
    let text = stdout(&atc(repo.path(), &["brief", "2"]));
    assert!(text.contains("pi.5 · linked blocks #1 · "), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let history = envelope(&out)["data"]["history"]
        .as_array()
        .expect("a history")
        .clone();
    // The second filing is not on this flight's record, so the rows are
    // the filing, the two lanes, the link, and the unlink.
    assert_eq!(history.len(), 5);
    assert_eq!(history[1]["assignee"], serde_json::json!("agent"));
    let cleared = history[2].as_object().expect("an object");
    assert!(cleared.contains_key("assignee"), "{}", history[2]);
    assert!(cleared["assignee"].is_null(), "{}", history[2]);
    assert_eq!(history[3]["from"], serde_json::json!("pi.1"));
    assert_eq!(history[3]["to"], serde_json::json!("pi.2"));
    assert_eq!(history[4]["from"], serde_json::json!("pi.1"));
    assert_eq!(history[4]["to"], serde_json::json!("pi.2"));
}

#[test]
fn a_cancel_reason_rides_its_moment() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the work"]));
    stdout(&atc(repo.path(), &["status", "1", "in_progress"]));
    stdout(&atc(repo.path(), &["cancel", "1", "-m", "superseded"]));

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("pi.2 · status in_progress · "), "{text}");
    assert!(
        text.contains("pi.3 · status canceled · tests@tower.invalid · "),
        "{text}"
    );
    // The reason follows on its own indented line, the comments' grammar.
    let row = text
        .lines()
        .position(|line| line.starts_with("  pi.3 · status canceled"))
        .expect("the cancel row");
    assert_eq!(text.lines().nth(row + 1), Some("    superseded"), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let history = envelope(&out)["data"]["history"]
        .as_array()
        .expect("a history")
        .clone();
    assert_eq!(history[2]["status"], serde_json::json!("canceled"));
    assert_eq!(history[2]["reason"], serde_json::json!("superseded"));
    let plain = history[1].as_object().expect("an object");
    assert_eq!(plain["status"], serde_json::json!("in_progress"));
    assert!(!plain.contains_key("reason"), "{}", history[1]);
}

#[test]
fn an_unknown_kind_naming_the_flight_lands_under_its_own_name() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "work worth promoting"]));

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

    let text = stdout(&atc(repo.path(), &["brief", "1"]));
    assert!(text.contains("pi.2 · promoted · "), "{text}");

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let history = envelope(&out)["data"]["history"]
        .as_array()
        .expect("a history")
        .clone();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1]["what"], serde_json::json!("promoted"));
    // Its words are unknowable, so the row is the six keys alone.
    let mut keys: Vec<&str> = history[1]
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["at", "by", "callsign", "id", "session", "what"]);

    // A body naming some other flight stays off this one's history.
    Store::open(repo.path())
        .expect("open")
        .append(vec![Kind::Unknown {
            kind: "promoted".to_string(),
            body: serde_json::value::RawValue::from_string(r#"{"flight":"pi.99"}"#.to_string())
                .expect("raw"),
        }])
        .expect("append");
    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let history = envelope(&out)["data"]["history"]
        .as_array()
        .expect("a history")
        .clone();
    assert_eq!(history.len(), 2);
}

#[test]
fn bare_seq_and_hash_prefixed_and_full_references_all_resolve() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the flight"]));
    for reference in ["1", "#1", "pi.1"] {
        let text = stdout(&atc(repo.path(), &["brief", reference]));
        assert!(text.contains("#1  the flight"), "`{reference}`: {text}");
    }
}

#[test]
fn an_unknown_flight_refuses_not_found() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the only flight"]));
    let out = atc(repo.path(), &["brief", "9", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn a_malformed_reference_refuses_bad_flight() {
    let repo = repo();
    let out = atc(repo.path(), &["brief", "not-an-id", "--json"]);
    refusal(&out, 2, "usage/bad-flight");
}

#[test]
fn a_bare_filing_briefs_as_yours() {
    // Bare `file` lands Ready with no lane — the lane is what keeps it
    // out of the pool, and the brief says so.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "needs a look"]));

    let out = atc(repo.path(), &["brief", "1"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("ready"), "{text}");
    assert!(text.contains("yours — unassigned"), "{text}");
}

#[test]
fn a_me_laned_ready_flight_briefs_as_yours_with_its_lane() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "needs a look", "--assignee", "me"],
    ));

    let out = atc(repo.path(), &["brief", "1"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("yours — assigned me"), "{text}");
}

#[test]
fn the_json_pins_the_merged_envelope() {
    let repo = repo();
    file_pipeline(&repo, "left work");
    file_pipeline(&repo, "right work");

    // The flattened standing rides beside the brief's own record — one
    // envelope, no inner nesting, and a bare tag with no payload keys.
    let out = atc(repo.path(), &["brief", "5", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["atc"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("brief"));
    let data = &envelope["data"];
    assert_eq!(data["id"], serde_json::json!("pi.8"));
    assert_eq!(data["number"], serde_json::json!(5));
    assert_eq!(data["standing"], serde_json::json!("ready"));
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
        "body",
        "comments",
    ] {
        assert!(data.contains_key(key), "data is missing `{key}`");
    }
    for key in ["beat", "with", "paths"] {
        assert!(!data.contains_key(key), "`{key}` is gone: {envelope}");
    }

    let out = atc(repo.path(), &["brief", "2", "--json"]);
    let envelope = serde_json::from_str::<serde_json::Value>(&stdout(&out)).expect("an envelope");
    assert_eq!(envelope["data"]["standing"], serde_json::json!("ready"));
}

#[test]
fn a_closed_flights_standing_carries_no_duplicate_payload() {
    // The slimmed variants flatten to the tag alone: `status_by` appears
    // once, from the brief's own field — serde flatten would otherwise
    // emit the key twice and the envelope would stop being an object a
    // strict parser accepts.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "finished work"]));
    stdout(&atc(repo.path(), &["done", "1"]));

    let out = atc(repo.path(), &["brief", "1", "--json"]);
    let raw = stdout(&out);
    assert_eq!(raw.matches("\"status_by\"").count(), 1, "{raw}");
    let envelope = serde_json::from_str::<serde_json::Value>(&raw).expect("an envelope");
    let data = &envelope["data"];
    assert_eq!(data["standing"], serde_json::json!("done"));
    assert_eq!(data["status_by"], serde_json::json!("tests@tower.invalid"));
    let data = data.as_object().expect("data is an object");
    for key in ["with", "paths", "on"] {
        assert!(!data.contains_key(key), "`{key}` is not a brief key");
    }
}

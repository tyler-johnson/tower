//! `atc next` against real repositories: the filed-order walk, the
//! peek, the lane gate, the outcomes — work, drained, yours — and the
//! exit, 0 on a pick and 1 otherwise.
//!
//! The pool is Ready flights in the agent lane, so bare filings — born
//! Ready but laned to no one — are never handed out, and every pullable fixture files under
//! a two-flight repo-layer procedure whose `pass` is agent-assigned and
//! born Ready; the flight `next` hands out is that one, and the parent
//! and `verdict` fold Waiting by their edges. The fold derives the
//! release, so a Waiting flight whose dependencies close reads Ready on
//! the following invocation — the `yours` outcome when the released
//! flight is off the agent lane, which in this shape it always is.
//!
//! A pull writes to tower's log and nothing to the repository: no
//! branch, no worktree, no op row. The picked row is the flight, its
//! number, its subject, and its skill when it names one, and the tree it
//! flies in is the agent's own to choose.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::Repo;

fn atc(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atc"))
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        // A developer's own fufu session must not tag the fixture's
        // events: the bylines below assert the bare email.
        .env_remove("FF_SESSION")
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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    install_pipeline(&repo);
    repo
}

/// The minimal pullable shape principle 12 admits: an agent-assigned
/// flight with a me-assigned end. Filing under it mints a parent and two
/// flights — three flights and three edges, so each filing consumes six
/// event seqs and three flight numbers, and only `pass` is born Ready.
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

fn file_pipeline(repo: &Repo, subject: &str) {
    stdout(&atc(repo.path(), &["file", "pipeline", subject]));
}

#[test]
fn next_pulls_the_agent_flight_and_sets_in_progress() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.contains("in progress #2: the one flight · pass"),
        "{text}"
    );
    assert!(text.contains("board: atc"), "{text}");

    // The pull is a status move, and the board is flat: the
    // pulled sub-flight is a row under In Progress, and its parent keeps
    // the mark. The brief is still where one flight's pilot and branch
    // are read.
    let board = stdout(&atc(repo.path(), &[]));
    assert!(board.contains("the one flight (0/2)"), "{board}");
    let json = envelope(&atc(repo.path(), &["--json"]));
    assert_eq!(
        json["data"]["in_progress"][0]["id"],
        serde_json::json!("pi.2")
    );
    assert_eq!(
        json["data"]["in_progress"][0]["status_by"],
        serde_json::json!("tests@tower.invalid"),
        "the byline on the pull is the pilot"
    );

    let brief = stdout(&atc(repo.path(), &["brief", "2"]));
    assert!(
        brief.contains("in progress — tests@tower.invalid"),
        "{brief}"
    );

    // The pool is empty and the parent and `verdict` are born Waiting —
    // no Ready work off the lane, so the outcome is drained, not yours.
    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
}

#[test]
fn a_ready_flight_off_the_agent_lane_is_the_yours_outcome_at_exit_1() {
    let repo = repo();
    // Born Ready, but never assigned to the lane.
    stdout(&atc(repo.path(), &["file", "needs a look"]));

    let out = atc(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("yours"));
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(1));

    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("nothing ready — one flight needs you"),
        "{text}"
    );
}

#[test]
fn a_backlog_filing_is_never_pulled_and_the_board_drains_to_1() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "unclassified work", "--status", "backlog"],
    ));
    // Even in the agent lane: Backlog is not Ready, and nothing leaves
    // Backlog but a person's gesture.
    stdout(&atc(repo.path(), &["assign", "1", "agent"]));

    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(
        !text.contains("need you"),
        "Backlog is not yours-Ready: {text}"
    );

    // The person's gesture is what clears it.
    stdout(&atc(repo.path(), &["status", "1", "ready"]));
    let text = stdout(&atc(repo.path(), &["next"]));
    assert!(text.contains("in progress #1: unclassified work"), "{text}");
}

#[test]
fn the_parent_is_never_pulled_and_the_fold_releases_a_satisfied_waiter() {
    let repo = repo();
    file_pipeline(&repo, "a broad task");

    let text = stdout(&atc(repo.path(), &["next"]));
    assert!(
        text.contains("in progress #2: a broad task · pass"),
        "{text}"
    );
    stdout(&atc(repo.path(), &["done", "2"]));

    // `verdict` waited on `pass`; this invocation's fold derives it
    // Ready — off the agent lane, so exit 1 with the yours outcome.
    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = self::envelope(&atc(repo.path(), &["next", "--json"]));
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("yours"));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(1));
    stdout(&atc(repo.path(), &["done", "3"]));

    // Every child closed releases the parent the same way — Ready, not
    // finished: whether the broad task is over stays a judgment.
    let out = atc(repo.path(), &["next"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "the parent is yours, never pulled"
    );

    // The two empty picks share the code; the word is what diverges.
    stdout(&atc(repo.path(), &["done", "1"]));
    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1), "drained at last");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(!text.contains("need you"), "{text}");
    let envelope = self::envelope(&atc(repo.path(), &["next", "--json"]));
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("drained"));
}

#[test]
fn peek_reads_without_pulling() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = atc(repo.path(), &["next", "--peek"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("ready #2: the one flight · pass"), "{text}");
    assert!(!text.contains("in progress"), "{text}");

    let board = stdout(&atc(repo.path(), &[]));
    assert!(board.contains("ready\n"), "{board}");
    assert!(
        !board.contains("in progress\n"),
        "the peek wrote nothing: {board}"
    );

    let out = atc(repo.path(), &["next", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["data"]["outcome"],
        serde_json::json!("work"),
        "a peek is still work — the outcome says what was found, `pulled` what was written"
    );
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    let row = &envelope["data"]["picked"][0];
    assert_eq!(row["flight"], serde_json::json!("pi.2"));
    assert_eq!(keys(row), ["flight", "number", "subject"]);
}

/// The keys on one picked row, in emitted order.
fn keys(row: &serde_json::Value) -> Vec<&str> {
    row.as_object()
        .expect("a row is an object")
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn a_pull_writes_nothing_to_the_repository() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let envelope = envelope(&atc(repo.path(), &["next", "--json"]));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(true));
    let row = &envelope["data"]["picked"][0];
    assert_eq!(row["flight"], serde_json::json!("pi.2"));
    assert_eq!(row["number"], serde_json::json!(2));
    assert_eq!(row["subject"], serde_json::json!("the one flight · pass"));
    assert_eq!(
        keys(row),
        ["flight", "number", "subject"],
        "no branch, no skill the flight never named: {row}"
    );

    // The pull is a log event and nothing else: no branch minted, and
    // the operator's own tree still stands where it was.
    let branches = repo.ff(&["branch", "list"]);
    assert!(!branches.contains("flight/"), "{branches}");
    let status: serde_json::Value =
        serde_json::from_str(&repo.ff(&["status", "--json"])).expect("a status envelope");
    assert_eq!(status["data"]["head"]["name"], serde_json::json!("main"));
}

#[test]
fn an_unclosed_dependency_keeps_the_dependent_out_of_the_pool() {
    let repo = repo();
    file_pipeline(&repo, "the dependent");
    file_pipeline(&repo, "the dependency");
    // The agent flights: the first filing's is #2, the second's is #5.
    stdout(&atc(repo.path(), &["link", "2", "5"]));

    // The dependent is Waiting, not a candidate, and the dependency is
    // what the walk hands out.
    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.contains("in progress #5: the dependency · pass"),
        "{text}"
    );
    assert!(!text.contains("#2"), "{text}");

    let board = envelope(&atc(repo.path(), &["--json"]));
    assert_eq!(board["data"]["waiting"][0]["id"], serde_json::json!("pi.1"));
    assert!(
        board["data"]["waiting"]
            .as_array()
            .expect("waiting")
            .iter()
            .any(|view| view["id"] == serde_json::json!("pi.2")),
        "{board}"
    );
}

#[test]
fn a_count_picks_in_filed_order_and_the_envelope_has_no_passed_key() {
    let repo = repo();
    file_pipeline(&repo, "first work");
    file_pipeline(&repo, "second work");
    file_pipeline(&repo, "third work");

    // The agent flights are #2, #5 and #8; two of three, in filed order,
    // and nothing about a tree decides it.
    let out = atc(repo.path(), &["next", "-n", "2", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("work"));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    let picked = envelope["data"]["picked"].as_array().expect("picked");
    assert_eq!(picked.len(), 2);
    assert_eq!(picked[0]["flight"], serde_json::json!("pi.2"));
    assert_eq!(picked[1]["flight"], serde_json::json!("pi.8"));
    let data = envelope["data"].as_object().expect("data");
    assert!(!data.contains_key("passed"), "{envelope}");
    for key in ["outcome", "picked", "pulled", "yours"] {
        assert!(data.contains_key(key), "{key} missing: {envelope}");
    }

    let out = atc(repo.path(), &["next", "-n", "2"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("in progress #2: first work · pass"), "{text}");
    assert!(
        text.contains("in progress #5: second work · pass"),
        "{text}"
    );
    assert!(!text.contains("#8"), "{text}");
    assert!(!text.contains("passed"), "{text}");
}

#[test]
fn a_zero_count_is_a_usage_refusal() {
    let repo = repo();
    let out = atc(repo.path(), &["next", "-n", "0", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("tower/usage/bad-count")
    );
}

#[test]
fn an_empty_pick_under_json_is_a_data_envelope_not_an_error() {
    let repo = repo();
    let out = atc(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["ff"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("tower next"));
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("drained"));
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(0));
    assert!(
        envelope.get("error").is_none(),
        "data and error, never both"
    );
}

#[test]
fn next_never_exits_three() {
    // fufu's served-extension contract reserves 3 for `held/*` error
    // envelopes, and the MCP relay reads `isError` off the status alone.
    // All three outcomes ride a data envelope at 0 or 1, and the word —
    // not the code — says which empty pick it was.
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let mut seen = Vec::new();
    for expected in ["work", "yours", "drained"] {
        if expected == "drained" {
            // `verdict` was released Ready by the pull's `done`; close
            // it and the parent so nothing Ready is left off the lane.
            stdout(&atc(repo.path(), &["done", "3"]));
            stdout(&atc(repo.path(), &["done", "1"]));
        }
        let out = atc(repo.path(), &["next", "--json"]);
        let code = out.status.code().expect("a code");
        assert!(code == 0 || code == 1, "{expected}: exit {code}, never 3");
        let envelope = envelope(&out);
        assert!(envelope.get("error").is_none(), "{expected}: {envelope}");
        assert_eq!(envelope["data"]["outcome"], serde_json::json!(expected));
        seen.push((expected, code));
        if expected == "work" {
            assert_eq!(envelope["data"]["pulled"], serde_json::json!(true));
            stdout(&atc(repo.path(), &["done", "2"]));
        }
    }
    assert_eq!(seen, [("work", 0), ("yours", 1), ("drained", 1)]);
}

#[test]
fn a_picked_flight_hands_out_its_skill() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(
        ".tower/procedures/skilled.toml",
        concat!(
            "name = \"skilled\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nskill    = \"review\"\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );
    stdout(&atc(repo.path(), &["file", "skilled", "look this over"]));

    let envelope = envelope(&atc(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(
        envelope["data"]["picked"][0]["skill"],
        serde_json::json!("review")
    );

    // The human render says it on the picked line, where the loop reads.
    let text = stdout(&atc(repo.path(), &["next"]));
    assert!(
        text.contains("in progress #2: look this over · pass · skill review"),
        "{text}"
    );
}

#[test]
fn a_flight_naming_no_skill_omits_the_field() {
    let repo = repo();
    file_pipeline(&repo, "plain work");

    let envelope = envelope(&atc(repo.path(), &["next", "--peek", "--json"]));
    let row = &envelope["data"]["picked"][0];
    assert_eq!(row["flight"], serde_json::json!("pi.2"));
    assert!(
        row.get("skill").is_none(),
        "absent, not null, when the flight names none: {row}"
    );

    let text = stdout(&atc(repo.path(), &["next"]));
    assert!(!text.contains("skill"), "{text}");
}

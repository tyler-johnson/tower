//! `atc next` against real repositories: the lane walk, the `none`
//! overflow, the peek, the re-lane, the outcomes — work, drained,
//! elsewhere — and the exit, 0 on a pick and 1 otherwise.
//!
//! The walk is the lanes named in order then the unassigned lane, and
//! `me` when unsaid — the harness scrubs the callsign, so bare `next`
//! here walks the literal `me` lane and then `none`, and a pull lands
//! in the literal `me` lane. Bare filings are born Ready
//! and laned to no one, so they are the overflow of every walk. The
//! agent fixtures file under a two-flight repo-layer procedure whose
//! `pass` is agent-assigned and born Ready; `next agent` hands that one
//! out, and the parent and `verdict` fold Waiting by their edges. The
//! fold derives the release, so a Waiting flight whose dependencies
//! close reads Ready on the following invocation — `verdict` in the
//! `me` lane, `elsewhere` to the agent walk, and the parent unassigned,
//! the overflow of any walk.
//!
//! A pull writes to tower's log and nothing to the repository: no
//! branch, no worktree, no op row. The picked row is the flight, its
//! number, its subject, and its skill when it names one, and the tree it
//! flies in is the agent's own to choose.

use std::path::Path;
use std::process::{Command, Output};

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

/// The spawn under a callsign, the way a harness exports one.
fn atc_as(repo: &Path, args: &[&str], callsign: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    command
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .env("ATC_CALLSIGN", callsign)
        .output()
        .expect("spawn atc")
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    install_pipeline(&repo);
    repo
}

/// The minimal agent-walk shape principle 12 admits: an agent-assigned
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

/// The picked ids off an envelope, in walk order.
fn picked(envelope: &serde_json::Value) -> Vec<&str> {
    envelope["data"]["picked"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["flight"].as_str().expect("flight"))
        .collect()
}

/// How many commits every tower chain holds — one per append.
fn commits(repo: &Repo) -> usize {
    repo.git(&["rev-list", "--count", "--glob=refs/tower/log/*"])
        .trim()
        .parse()
        .expect("a count")
}

#[test]
fn next_pulls_the_agent_flight_and_sets_in_progress() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = atc(repo.path(), &["next", "agent"]);
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

    // The lane is empty and the parent and `verdict` are born Waiting —
    // no Ready work anywhere, so the outcome is drained, not elsewhere.
    let out = atc(repo.path(), &["next", "agent"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
}

#[test]
fn a_ready_flight_in_another_lane_is_the_elsewhere_outcome_at_exit_1() {
    let repo = repo();
    // Born Ready in the agent lane; the bare walk is `me` then `none`,
    // and both are empty.
    stdout(&atc(
        repo.path(),
        &["file", "needs a look", "--assignee", "agent"],
    ));

    let out = atc(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("elsewhere"));
    assert_eq!(envelope["data"]["lanes"], serde_json::json!(["me", "none"]));
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(1));

    let out = atc(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("nothing ready here — one flight in another lane"),
        "{text}"
    );

    // The plural, with a second one alongside.
    stdout(&atc(
        repo.path(),
        &["file", "needs another", "--assignee", "agent"],
    ));
    let out = atc(repo.path(), &["next"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("nothing ready here — two flights in other lanes"),
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

    let out = atc(repo.path(), &["next", "agent"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(
        !text.contains("another lane"),
        "Backlog is not elsewhere: {text}"
    );

    // The person's gesture is what clears it.
    stdout(&atc(repo.path(), &["status", "1", "ready"]));
    let text = stdout(&atc(repo.path(), &["next", "agent"]));
    assert!(text.contains("in progress #1: unclassified work"), "{text}");
}

#[test]
fn the_fold_releases_a_satisfied_waiter_into_its_lane() {
    let repo = repo();
    file_pipeline(&repo, "a broad task");

    let text = stdout(&atc(repo.path(), &["next", "agent"]));
    assert!(
        text.contains("in progress #2: a broad task · pass"),
        "{text}"
    );
    stdout(&atc(repo.path(), &["done", "2"]));

    // `verdict` waited on `pass`; this invocation's fold derives it
    // Ready — in the `me` lane, so the agent walk is elsewhere at exit
    // 1, and the bare walk finds it.
    let out = atc(repo.path(), &["next", "agent"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = self::envelope(&atc(repo.path(), &["next", "agent", "--json"]));
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("elsewhere"));
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(1));
    let mine = self::envelope(&atc(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(picked(&mine), ["pi.3"], "the `me` lane is the bare walk");
    stdout(&atc(repo.path(), &["done", "3"]));

    // Every child closed releases the parent the same way — Ready, not
    // finished: whether the broad task is over stays a judgment. It is
    // laned to no one, so it is the overflow of every walk.
    let envelope = self::envelope(&atc(repo.path(), &["next", "agent", "--peek", "--json"]));
    assert_eq!(picked(&envelope), ["pi.1"], "the unassigned overflow");
    let envelope = self::envelope(&atc(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(picked(&envelope), ["pi.1"]);

    // The two empty picks share the code; the word is what diverges.
    stdout(&atc(repo.path(), &["done", "1"]));
    let out = atc(repo.path(), &["next", "agent"]);
    assert_eq!(out.status.code(), Some(1), "drained at last");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(!text.contains("another lane"), "{text}");
    let envelope = self::envelope(&atc(repo.path(), &["next", "agent", "--json"]));
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("drained"));
}

#[test]
fn peek_reads_without_pulling() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = atc(repo.path(), &["next", "agent", "--peek"]);
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

    let out = atc(repo.path(), &["next", "agent", "--peek", "--json"]);
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

/// The keys on one JSON object, in emitted order.
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

    let envelope = envelope(&atc(repo.path(), &["next", "agent", "--json"]));
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
fn an_unclosed_dependency_keeps_the_dependent_out_of_the_walk() {
    let repo = repo();
    file_pipeline(&repo, "the dependent");
    file_pipeline(&repo, "the dependency");
    // The agent flights: the first filing's is #2, the second's is #5.
    stdout(&atc(repo.path(), &["link", "2", "5"]));

    // The dependent is Waiting, not a candidate, and the dependency is
    // what the walk hands out.
    let out = atc(repo.path(), &["next", "agent"]);
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
    let out = atc(
        repo.path(),
        &["next", "agent", "-n", "2", "--peek", "--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("work"));
    assert_eq!(
        envelope["data"]["lanes"],
        serde_json::json!(["agent", "none"])
    );
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(picked(&envelope), ["pi.2", "pi.8"]);
    assert_eq!(
        keys(&envelope["data"]),
        [
            "outcome",
            "lanes",
            "assignee",
            "picked",
            "pulled",
            "elsewhere"
        ],
        "no `passed`, and the re-lane said even when unsaid: {envelope}"
    );

    let out = atc(repo.path(), &["next", "agent", "-n", "2"]);
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
        serde_json::json!("usage/bad-count")
    );
}

#[test]
fn an_empty_pick_under_json_is_a_data_envelope_not_an_error() {
    let repo = repo();
    let out = atc(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(envelope["atc"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("next"));
    assert_eq!(envelope["data"]["outcome"], serde_json::json!("drained"));
    assert_eq!(envelope["data"]["lanes"], serde_json::json!(["me", "none"]));
    assert_eq!(
        envelope["data"]["assignee"],
        serde_json::json!("me"),
        "no callsign under the harness, so the literal is stored"
    );
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(0));
    assert!(
        envelope.get("error").is_none(),
        "data and error, never both"
    );
}

#[test]
fn next_never_exits_three() {
    // 3 is `hold`'s outcome and the `held/*` namespace's, so an empty
    // pick never borrows it. All three outcomes ride a data envelope at 0
    // or 1, and the word — not the code — says which empty pick it was.
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let mut seen = Vec::new();
    for expected in ["work", "elsewhere", "drained"] {
        if expected == "drained" {
            // `verdict` was released Ready by the pull's `done`; close
            // it and the parent so nothing Ready is left in any lane.
            stdout(&atc(repo.path(), &["done", "3"]));
            stdout(&atc(repo.path(), &["done", "1"]));
        }
        let out = atc(repo.path(), &["next", "agent", "--json"]);
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
    assert_eq!(seen, [("work", 0), ("elsewhere", 1), ("drained", 1)]);
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

    let envelope = envelope(&atc(repo.path(), &["next", "agent", "--peek", "--json"]));
    assert_eq!(
        envelope["data"]["picked"][0]["skill"],
        serde_json::json!("review")
    );

    // The human render says it on the picked line, where the loop reads.
    let text = stdout(&atc(repo.path(), &["next", "agent"]));
    assert!(
        text.contains("in progress #2: look this over · pass · skill review"),
        "{text}"
    );
}

#[test]
fn a_flight_naming_no_skill_omits_the_field() {
    let repo = repo();
    file_pipeline(&repo, "plain work");

    let envelope = envelope(&atc(repo.path(), &["next", "agent", "--peek", "--json"]));
    let row = &envelope["data"]["picked"][0];
    assert_eq!(row["flight"], serde_json::json!("pi.2"));
    assert!(
        row.get("skill").is_none(),
        "absent, not null, when the flight names none: {row}"
    );

    let text = stdout(&atc(repo.path(), &["next", "agent"]));
    assert!(!text.contains("skill"), "{text}");
}

#[test]
fn the_default_pull_is_me_then_none() {
    // Bare `next` walks the `me` lane first — ahead of an earlier bare
    // filing — then the unassigned lane, and never the agent lane. No
    // callsign under the harness, so `me` is the literal lane.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "nobody's"]));
    stdout(&atc(repo.path(), &["file", "mine", "--assignee", "me"]));
    stdout(&atc(
        repo.path(),
        &["file", "pooled", "--assignee", "agent"],
    ));

    let envelope = envelope(&atc(repo.path(), &["next", "-n", "3", "--json"]));
    assert_eq!(envelope["data"]["lanes"], serde_json::json!(["me", "none"]));
    assert_eq!(picked(&envelope), ["pi.2", "pi.1"]);
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(1));

    let board = self::envelope(&atc(repo.path(), &["--json"]));
    assert_eq!(board["data"]["ready"][0]["id"], serde_json::json!("pi.3"));
}

#[test]
fn a_callsign_pulls_its_own_queue_the_literal_me_and_then_none() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "nobody's"]));
    stdout(&atc(
        repo.path(),
        &["file", "qwen's", "--assignee", "qwen-review"],
    ));
    stdout(&atc(
        repo.path(),
        &["file", "the literal", "--assignee", "me"],
    ));
    stdout(&atc(
        repo.path(),
        &["file", "claude's", "--assignee", "claude"],
    ));

    // Filed order within the lane, then the overflow; claude's queue is
    // elsewhere.
    let envelope = envelope(&atc_as(
        repo.path(),
        &["next", "-n", "4", "--json"],
        "qwen-review",
    ));
    assert_eq!(envelope["data"]["lanes"], serde_json::json!(["me", "none"]));
    assert_eq!(picked(&envelope), ["pi.2", "pi.3", "pi.1"]);
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(1));
}

#[test]
fn agent_pulls_the_literal_lane_and_then_none() {
    // The lane is the argument: the caller's own queue is not in the
    // agent walk, whatever the callsign.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "nobody's"]));
    stdout(&atc(
        repo.path(),
        &["file", "claude's", "--assignee", "claude"],
    ));
    stdout(&atc(
        repo.path(),
        &["file", "pooled", "--assignee", "agent"],
    ));

    let envelope = envelope(&atc_as(
        repo.path(),
        &["next", "agent", "-n", "3", "--json"],
        "claude",
    ));
    assert_eq!(
        envelope["data"]["lanes"],
        serde_json::json!(["agent", "none"])
    );
    assert_eq!(picked(&envelope), ["pi.3", "pi.1"]);
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(1));
}

#[test]
fn none_named_outright_walks_the_unassigned() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "pooled", "--assignee", "agent"],
    ));
    stdout(&atc(repo.path(), &["file", "nobody's first"]));
    stdout(&atc(repo.path(), &["file", "nobody's second"]));

    let envelope = envelope(&atc(repo.path(), &["next", "none", "-n", "3", "--json"]));
    assert_eq!(envelope["data"]["lanes"], serde_json::json!(["none"]));
    assert_eq!(picked(&envelope), ["pi.2", "pi.3"], "once, not twice");
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(1));
}

#[test]
fn a_pick_is_relaned_to_you_by_default_in_the_same_append() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "the review", "--assignee", "agent"],
    ));
    stdout(&atc(
        repo.path(),
        &["file", "the other", "--assignee", "agent"],
    ));
    stdout(&atc(
        repo.path(),
        &["file", "the third", "--assignee", "agent"],
    ));
    let before = commits(&repo);

    // No flag: the pull is yours. `me` stores the callsign, the way
    // `file` stores it, and the envelope carries the stored word.
    let out = atc_as(repo.path(), &["next", "agent", "--json"], "claude");
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["assignee"], serde_json::json!("claude"));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(true));
    assert_eq!(picked(&envelope), ["pi.1"]);
    assert_eq!(
        commits(&repo),
        before + 1,
        "the pick and the re-lane are one append"
    );

    let board = self::envelope(&atc(repo.path(), &["--json"]));
    let flown = &board["data"]["in_progress"][0];
    assert_eq!(flown["id"], serde_json::json!("pi.1"));
    assert_eq!(flown["assignee"], serde_json::json!("claude"));
    assert_eq!(flown["status_callsign"], serde_json::json!("claude"));
    let brief = self::envelope(&atc(repo.path(), &["brief", "1", "--json"]));
    let history = brief["data"]["history"].as_array().expect("a history");
    assert_eq!(history.len(), 3, "{history:?}");
    assert_eq!(history[0]["what"], serde_json::json!("filed"));
    assert_eq!(history[1]["what"], serde_json::json!("status"));
    assert_eq!(history[1]["status"], serde_json::json!("in_progress"));
    assert_eq!(history[2]["what"], serde_json::json!("assigned"));
    assert_eq!(history[2]["assignee"], serde_json::json!("claude"));

    // The human line says where the pick went, and `none` clears the
    // lane and says so.
    let text = stdout(&atc_as(
        repo.path(),
        &["next", "agent", "--assignee", "none"],
        "claude",
    ));
    assert!(
        text.contains("in progress #2: the other · assigned none"),
        "{text}"
    );
    let board = self::envelope(&atc(repo.path(), &["--json"]));
    let cleared = board["data"]["in_progress"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["id"] == serde_json::json!("pi.2"))
        .expect("pulled")
        .clone();
    assert!(cleared["assignee"].is_null(), "{cleared}");

    // `--assignee agent` is how a pick stays in the pool: the lane is
    // the one it was found in, so nothing about the lane is written.
    let before = commits(&repo);
    let out = atc_as(
        repo.path(),
        &["next", "agent", "--assignee", "agent", "--json"],
        "claude",
    );
    let envelope = self::envelope(&out);
    assert_eq!(envelope["data"]["assignee"], serde_json::json!("agent"));
    assert_eq!(picked(&envelope), ["pi.3"]);
    assert_eq!(commits(&repo), before + 1);
    let board = self::envelope(&atc(repo.path(), &["--json"]));
    let kept = board["data"]["in_progress"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["id"] == serde_json::json!("pi.3"))
        .expect("pulled")
        .clone();
    assert_eq!(kept["assignee"], serde_json::json!("agent"), "{kept}");
    let brief = self::envelope(&atc(repo.path(), &["brief", "3", "--json"]));
    let history = brief["data"]["history"].as_array().expect("a history");
    assert_eq!(history.len(), 2, "filing and status only: {history:?}");

    let envelope = self::envelope(&atc(repo.path(), &["next", "--json"]));
    assert_eq!(
        envelope["data"]["outcome"],
        serde_json::json!("drained"),
        "nothing Ready anywhere once all three are pulled"
    );
}

#[test]
fn a_pick_already_in_the_lane_writes_no_assigned_event() {
    // A flight pulled from its own queue lands where it already is:
    // one `status` moment, no no-op `assigned`, and no phrase on the
    // row.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "mine", "--assignee", "me"]));
    let before = commits(&repo);

    let text = stdout(&atc(repo.path(), &["next"]));
    assert!(text.contains("in progress #1: mine"), "{text}");
    assert!(!text.contains(" · assigned"), "{text}");
    assert_eq!(commits(&repo), before + 1);

    let brief = envelope(&atc(repo.path(), &["brief", "1", "--json"]));
    let history = brief["data"]["history"].as_array().expect("a history");
    assert_eq!(history.len(), 2, "{history:?}");
    assert_eq!(history[0]["what"], serde_json::json!("filed"));
    assert_eq!(history[1]["what"], serde_json::json!("status"));
    let board = envelope(&atc(repo.path(), &["--json"]));
    assert_eq!(
        board["data"]["in_progress"][0]["assignee"],
        serde_json::json!("me")
    );
}

#[test]
fn lanes_walk_in_the_order_given_and_none_once() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "nobody's"]));
    stdout(&atc(
        repo.path(),
        &["file", "pooled", "--assignee", "agent"],
    ));
    stdout(&atc(repo.path(), &["file", "mine", "--assignee", "me"]));

    let envelope = envelope(&atc(
        repo.path(),
        &["next", "agent", "me", "-n", "3", "--peek", "--json"],
    ));
    assert_eq!(
        envelope["data"]["lanes"],
        serde_json::json!(["agent", "me", "none"])
    );
    assert_eq!(picked(&envelope), ["pi.2", "pi.3", "pi.1"]);
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(0));

    let envelope = self::envelope(&atc(
        repo.path(),
        &["next", "none", "agent", "-n", "3", "--peek", "--json"],
    ));
    assert_eq!(
        envelope["data"]["lanes"],
        serde_json::json!(["none", "agent"]),
        "`none` walks where named and is not appended again"
    );
    assert_eq!(picked(&envelope), ["pi.1", "pi.2"]);
    assert_eq!(
        envelope["data"]["elsewhere"],
        serde_json::json!(1),
        "the `me` lane is outside this walk"
    );

    let envelope = self::envelope(&atc(
        repo.path(),
        &["next", "me", "me", "agent", "-n", "3", "--peek", "--json"],
    ));
    assert_eq!(
        envelope["data"]["lanes"],
        serde_json::json!(["me", "agent", "none"]),
        "a lane named twice walks once"
    );
    assert_eq!(picked(&envelope), ["pi.3", "pi.2", "pi.1"]);
}

#[test]
fn a_callsign_and_me_named_together_pick_the_flight_once() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "qwen's", "--assignee", "qwen-review"],
    ));

    let envelope = envelope(&atc_as(
        repo.path(),
        &["next", "me", "qwen-review", "-n", "2", "--peek", "--json"],
        "qwen-review",
    ));
    assert_eq!(
        envelope["data"]["lanes"],
        serde_json::json!(["me", "qwen-review", "none"])
    );
    assert_eq!(picked(&envelope), ["pi.1"], "once, in the first lane");
    assert_eq!(envelope["data"]["elsewhere"], serde_json::json!(0));
}

#[test]
fn assignee_under_peek_writes_nothing() {
    let repo = repo();
    stdout(&atc(
        repo.path(),
        &["file", "the review", "--assignee", "agent"],
    ));
    let before = commits(&repo);

    // No flag, so a pull would re-lane to the callsign; the envelope
    // says so, and the peek still writes nothing.
    let envelope = envelope(&atc_as(
        repo.path(),
        &["next", "agent", "--peek", "--json"],
        "claude",
    ));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["assignee"], serde_json::json!("claude"));
    assert_eq!(picked(&envelope), ["pi.1"]);
    assert_eq!(commits(&repo), before, "a peek appends nothing");

    let board = self::envelope(&atc(repo.path(), &["--json"]));
    assert_eq!(board["data"]["ready"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        board["data"]["ready"][0]["assignee"],
        serde_json::json!("agent"),
        "the lane stands"
    );
}

#[test]
fn a_bad_lane_on_either_side_is_bad_assignee() {
    let repo = repo();
    for args in [
        vec!["next", "two words"],
        vec!["next", "agent", "two words"],
        vec!["next", "--assignee", "two words"],
    ] {
        let mut args = args;
        args.push("--json");
        let out = atc(repo.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let envelope = envelope(&out);
        assert_eq!(
            envelope["error"]["id"],
            serde_json::json!("usage/bad-assignee"),
            "{args:?}"
        );
    }
}

#[test]
fn a_callsigns_own_queue_is_pulled_by_that_callsign_alone() {
    // The brief's verify: `atc assign 5 qwen-review`, then the pull under
    // that callsign picks it and a pull under another does not — and the
    // pick's In Progress carries the callsign onto the board.
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the review"]));
    stdout(&atc(repo.path(), &["assign", "1", "qwen-review"]));

    let out = atc_as(repo.path(), &["next"], "claude");
    assert_eq!(out.status.code(), Some(1), "another callsign's queue");
    let json = envelope(&atc_as(repo.path(), &["next", "--json"], "claude"));
    assert_eq!(json["data"]["outcome"], serde_json::json!("elsewhere"));

    let out = atc_as(repo.path(), &["next"], "qwen-review");
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("in progress #1: the review"), "{text}");

    let board = envelope(&atc(repo.path(), &["--json"]));
    let flown = &board["data"]["in_progress"][0];
    assert_eq!(flown["id"], serde_json::json!("pi.1"));
    assert_eq!(flown["status_callsign"], serde_json::json!("qwen-review"));
    assert_eq!(flown["status_by"], serde_json::json!("tests@tower.invalid"));
    let rendered = stdout(&atc(repo.path(), &[]));
    assert!(rendered.contains("in progress — qwen-review"), "{rendered}");
}

#[test]
fn a_callsign_pulls_its_own_queue_and_never_the_pool() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "own first"]));
    stdout(&atc(repo.path(), &["assign", "1", "claude"]));
    file_pipeline(&repo, "the pool");
    stdout(&atc(repo.path(), &["file", "own last"]));
    stdout(&atc(repo.path(), &["assign", "5", "claude"]));

    // The agent lane's `pass` is elsewhere to a bare pull, and the
    // pipeline's parent is Waiting, so nothing overflows.
    let json = envelope(&atc_as(
        repo.path(),
        &["next", "-n", "3", "--json"],
        "claude",
    ));
    assert_eq!(picked(&json), ["pi.1", "pi.9"], "own queue, filed order");
    assert_eq!(json["data"]["elsewhere"], serde_json::json!(1));
}

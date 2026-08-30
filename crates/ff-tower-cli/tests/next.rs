//! `ff tower next` against real repositories: the greedy admission, the
//! peek, the lane gate, and the exits — 1 drained, 3 needs-you.
//!
//! The pool is Ready flights in the agent lane, so bare filings — born
//! Triage — are never handed out, and every pullable fixture files under
//! a two-flight repo-layer procedure whose `pass` is agent-assigned and
//! born Ready; the flight `next` hands out is that one, and the parent
//! and `verdict` are born Waiting. The lazy pass rides every `next`, so
//! a Waiting flight whose dependencies close advances to Ready on the
//! following invocation — exit 3 when the advanced flight is off the
//! agent lane, which in this shape it always is.
//!
//! The assignment half rides the same fixtures. Main is a bay and it is
//! first in survey order, so the solo norm's single pick binds *there*;
//! a warmed slot is what the second pick takes. Every pull ends with a
//! branch and an op row tagged with the flight, and a warm or bind that
//! fufu refuses lands on the row rather than ending the walk.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::ff::Ff;
use ff_tower_testsupport::{Repo, canonicalized};

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
    stdout(&ff_tower(repo.path(), &["file", "pipeline", subject]));
}

#[test]
fn next_pulls_the_agent_flight_and_sets_in_progress() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.contains("in progress #2: the one flight · pass"),
        "{text}"
    );
    assert!(text.contains("board: ff tower"), "{text}");

    // The pull is a stored status move. What was pulled is a sub-flight
    // and needs nobody now, so the board keeps it under its parent's row
    // — the brief is where one flight's pilot and branch are read.
    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(board.contains("the one flight (0/2)"), "{board}");
    let json = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        json["data"]["in_progress"],
        serde_json::json!([]),
        "a flight under way competes with nothing"
    );

    let brief = stdout(&ff_tower(repo.path(), &["brief", "2"]));
    assert!(
        brief.contains("in progress — tests@tower.invalid"),
        "{brief}"
    );
    assert!(brief.contains("on flight/pi.2"), "{brief}");

    // The pool is empty and the parent and `verdict` are born Waiting —
    // no Ready work off the lane, so the drained code is 1, not 3.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
}

#[test]
fn a_ready_flight_off_the_agent_lane_is_the_exit_3_needs_you() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "needs a look"]));
    // Born Triage — cleared by hand, but never assigned to the lane.
    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));

    let out = ff_tower(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(1));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(3));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("nothing ready — one flight needs you"),
        "{text}"
    );
}

#[test]
fn a_triage_filing_is_never_pulled_and_the_board_drains_to_1() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "unclassified work"]));
    // Even in the agent lane: Triage is not Ready, and nothing leaves
    // Triage but a person's gesture.
    stdout(&ff_tower(repo.path(), &["assign", "1", "agent"]));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(
        !text.contains("need you"),
        "Triage is not yours-Ready: {text}"
    );

    // The person's gesture is what clears it.
    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));
    let text = stdout(&ff_tower(repo.path(), &["next"]));
    assert!(text.contains("in progress #1: unclassified work"), "{text}");
}

#[test]
fn the_parent_is_never_pulled_and_the_pass_advances_a_satisfied_waiter() {
    let repo = repo();
    file_pipeline(&repo, "a broad task");

    let text = stdout(&ff_tower(repo.path(), &["next"]));
    assert!(
        text.contains("in progress #2: a broad task · pass"),
        "{text}"
    );
    stdout(&ff_tower(repo.path(), &["done", "2"]));

    // `verdict` was born Waiting on `pass`; this invocation's pass
    // advances it — Ready off the agent lane, so exit 3, yours.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(3));
    let envelope = envelope(&ff_tower(repo.path(), &["next", "--json"]));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(1));
    stdout(&ff_tower(repo.path(), &["done", "3"]));

    // Every child closed advances the parent the same way — Ready, not
    // finished: whether the broad task is over stays a judgment.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "the parent is yours, never pulled"
    );

    stdout(&ff_tower(repo.path(), &["done", "1"]));
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(1), "drained at last");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("nothing ready\n"), "{text}");
    assert!(!text.contains("need you"), "{text}");
}

#[test]
fn peek_reads_without_pulling() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = ff_tower(repo.path(), &["next", "--peek"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("ready #2: the one flight · pass"), "{text}");
    assert!(!text.contains("in progress"), "{text}");

    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(board.contains("ready\n"), "{board}");
    assert!(
        !board.contains("in progress\n"),
        "the peek wrote nothing: {board}"
    );

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(
        envelope["data"]["picked"][0]["flight"],
        serde_json::json!("pi.2")
    );
}

#[test]
fn an_unclosed_dependency_is_passed_as_waiting_and_the_dependency_is_pulled() {
    let repo = repo();
    file_pipeline(&repo, "the dependent");
    file_pipeline(&repo, "the dependency");
    // The agent flights: the first filing's is #2, the second's is #5.
    stdout(&ff_tower(repo.path(), &["link", "2", "5"]));

    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(
        text.contains("in progress #5: the dependency · pass"),
        "{text}"
    );
    assert!(text.contains("passed #2 · waiting on #5"), "{text}");

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    let envelope = envelope(&out);
    let passed = &envelope["data"]["passed"];
    assert_eq!(passed[0]["flight"], serde_json::json!("pi.2"));
    assert_eq!(passed[0]["reason"], serde_json::json!("waiting"));
    assert_eq!(passed[0]["on"], serde_json::json!(["pi.8"]));
}

#[test]
fn deconfliction_passes_the_collider_pulls_the_clear_one_and_pins_the_json() {
    let repo = repo();
    file_pipeline(&repo, "left work");
    file_pipeline(&repo, "right work");
    file_pipeline(&repo, "third work");
    // Pull the first filing's agent flight by hand and put its work on a
    // branch.
    stdout(&ff_tower(repo.path(), &["status", "2", "in_progress"]));

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

    // The peek first, so the JSON pins the same pick the pulling run
    // takes a line below — after the pull the pool would be different.
    let out = ff_tower(repo.path(), &["next", "-n", "2", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
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
    assert!(text.contains("in progress #8: third work · pass"), "{text}");
    assert!(
        text.contains("passed #5 · collides with #2 on shared.txt"),
        "{text}"
    );

    // A pulled sub-flight sits under its parent on the board, so the
    // move is read back off its own record.
    let brief: serde_json::Value = serde_json::from_str(&stdout(&ff_tower(
        repo.path(),
        &["brief", "pi.14", "--json"],
    )))
    .expect("an envelope");
    assert_eq!(brief["data"]["status"], serde_json::json!("in_progress"));
    assert!(brief["data"]["status_by"].is_string(), "{brief}");
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
    assert_eq!(envelope["data"]["pulled"], serde_json::json!(false));
    assert_eq!(envelope["data"]["yours"], serde_json::json!(0));
    assert!(
        envelope.get("error").is_none(),
        "data and error, never both"
    );
}

/// A procedure whose subject resolves against a branch — `next` binds by
/// switching to the flight's own subject rather than minting a name.
fn install_branchy(repo: &Repo) {
    repo.write(
        ".tower/procedures/branchy.toml",
        concat!(
            "name = \"branchy\"\nsubject = \"branch\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );
}

/// `pipeline`, with the agent flight asking the pool for a tree.
fn install_warmly(repo: &Repo) {
    repo.write(
        ".tower/procedures/warmly.toml",
        concat!(
            "name = \"warmly\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nbay      = \"warm\"\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );
}

/// A fufu verb against an arbitrary worktree — the bays live outside the
/// repository, so `Repo::ff` cannot reach them.
fn ff_at(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("ff")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("FF_NONINTERACTIVE", "1")
        .env_remove("FF_SESSION")
        .output()
        .expect("spawn ff");
    assert!(
        output.status.success(),
        "`ff {}` exited {:?}: {}",
        args.join(" "),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_pull_binds_the_flight_to_a_branch_in_a_bay_and_tags_the_op_row() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = ff_tower(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let row = &envelope(&out)["data"]["picked"][0];
    // Main is first in survey order and free, and in the solo norm main
    // *is* the bay — DESIGN's `bays: 1` case.
    assert_eq!(row["bay_id"], serde_json::json!("main"));
    assert_eq!(row["branch"], serde_json::json!("flight/pi.2"));
    assert_eq!(row["warmed"], serde_json::json!(false));
    assert_eq!(row["refused"], serde_json::Value::Null);
    let bay = row["bay"].as_str().expect("a bay path");

    // The branch exists in that tree, and the op row that made it carries
    // the flight's name — which is what every later derivation reads.
    assert!(ff_at(Path::new(bay), &["branch", "list"]).contains("flight/pi.2"));
    let ops = ff_at(Path::new(bay), &["op", "log", "session(pi.2)", "-n", "0"]);
    assert!(ops.contains("flight/pi.2"), "{ops}");

    let out = ff_tower(repo.path(), &["next", "--peek"]);
    assert_eq!(out.status.code(), Some(1), "the flight is pulled");
}

#[test]
fn the_second_pick_takes_the_warm_bay_and_the_two_never_share_a_tree() {
    let repo = repo();
    file_pipeline(&repo, "left work");
    file_pipeline(&repo, "right work");
    let slot = repo.bay_path("bay-a");
    stdout(&ff_tower(
        repo.path(),
        &["bay", "warm", slot.to_str().expect("utf8")],
    ));

    let out = ff_tower(repo.path(), &["next", "-n", "2", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let picked = envelope(&out)["data"]["picked"].clone();
    assert_eq!(picked[0]["bay_id"], serde_json::json!("main"));
    assert_eq!(picked[0]["branch"], serde_json::json!("flight/pi.2"));
    assert_eq!(picked[1]["bay_id"], serde_json::json!("bay-a"));
    assert_eq!(picked[1]["branch"], serde_json::json!("flight/pi.8"));

    // The bay's own chain carries the second flight's tag, not the
    // first's — the fan-out reads each bay separately for exactly this.
    let ops = ff_at(&slot, &["op", "log", "session(pi.8)", "-n", "0"]);
    assert!(ops.contains("flight/pi.8"), "{ops}");
    assert!(ff_at(&slot, &["branch", "list"]).contains("flight/pi.8"));

    // And the human render puts the tree under the pull.
    let text = stdout(&ff_tower(repo.path(), &["bay"]));
    assert!(
        text.contains("flight/pi.2") && text.contains("flight/pi.8"),
        "{text}"
    );
}

#[test]
fn a_subject_that_is_a_branch_switches_rather_than_minting_one() {
    let repo = repo();
    install_branchy(&repo);
    repo.ff(&["start", "-b", "feature"]);
    repo.write("feature.txt", "work\n");
    repo.ff(&["commit", "-m", "feature: a file"]);
    repo.ff(&["switch", "main"]);
    stdout(&ff_tower(repo.path(), &["file", "branchy", "feature"]));

    let out = ff_tower(repo.path(), &["next", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let row = &envelope(&out)["data"]["picked"][0];
    assert_eq!(row["branch"], serde_json::json!("feature"));
    assert_eq!(row["refused"], serde_json::Value::Null);

    // Nothing was minted: the stamped branch is the one it flies on.
    let branches = ff_at(repo.path(), &["branch", "list"]);
    assert!(!branches.contains("flight/pi."), "{branches}");
    let out = ff_tower(repo.path(), &["next", "--peek"]);
    assert_eq!(out.status.code(), Some(1), "the flight is pulled");
}

#[test]
fn peek_reports_the_bay_and_the_branch_and_binds_neither() {
    let repo = repo();
    file_pipeline(&repo, "the one flight");

    let out = ff_tower(repo.path(), &["next", "--peek", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let row = &envelope(&out)["data"]["picked"][0];
    assert_eq!(row["bay_id"], serde_json::json!("main"));
    assert_eq!(row["branch"], serde_json::json!("flight/pi.2"));
    assert_eq!(row["warmed"], serde_json::json!(false));

    let text = stdout(&ff_tower(repo.path(), &["next", "--peek"]));
    assert!(text.contains("ready #2: the one flight · pass"), "{text}");
    assert!(text.contains("flight/pi.2"), "{text}");

    // The tree never moved and the log never grew.
    let branches = ff_at(repo.path(), &["branch", "list"]);
    assert!(!branches.contains("flight/pi.2"), "{branches}");
    let board = stdout(&ff_tower(repo.path(), &[]));
    assert!(!board.contains("in progress\n"), "{board}");
}

#[test]
fn a_full_pool_with_no_stamp_pulls_anyway_and_says_where_the_next_bay_comes_from() {
    let repo = repo();
    file_pipeline(&repo, "the first");
    file_pipeline(&repo, "the second");
    stdout(&ff_tower(repo.path(), &["next"]));

    // Main is the only bay and the first flight is live in it, so the
    // second pull gets no tree — which is a hint, not a refusal.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("in progress #5: the second · pass"), "{text}");
    assert!(text.contains("no free bay · ff tower bay warm"), "{text}");

    let envelope = envelope(&ff_tower(repo.path(), &["next", "--json"]));
    let row = &envelope["data"]["picked"][0];
    assert!(row.is_null(), "both flights are pulled now");
}

#[test]
fn a_warm_stamp_with_a_pool_root_mints_the_slot_it_needs() {
    let repo = repo();
    install_warmly(&repo);
    let root = repo.bay_path("bays");
    repo.git(&["config", "tower.bays", root.to_str().expect("utf8")]);
    stdout(&ff_tower(repo.path(), &["file", "warmly", "the first"]));
    stdout(&ff_tower(repo.path(), &["file", "warmly", "the second"]));

    let out = ff_tower(repo.path(), &["next", "-n", "2", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let picked = envelope(&out)["data"]["picked"].clone();
    assert_eq!(picked[0]["bay_id"], serde_json::json!("main"));
    assert_eq!(picked[0]["warmed"], serde_json::json!(false));
    assert_eq!(picked[1]["bay_id"], serde_json::json!("bay-1"));
    assert_eq!(picked[1]["warmed"], serde_json::json!(true));
    assert_eq!(picked[1]["branch"], serde_json::json!("flight/pi.8"));
    assert!(canonicalized(&root).join("bay-1").is_dir());
}

#[test]
fn a_warm_stamp_with_no_pool_root_lands_the_refusal_on_the_row_and_still_exits_zero() {
    let repo = repo();
    install_warmly(&repo);
    stdout(&ff_tower(repo.path(), &["file", "warmly", "the first"]));
    stdout(&ff_tower(repo.path(), &["file", "warmly", "the second"]));

    let out = ff_tower(repo.path(), &["next", "-n", "2"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a refusal on a row is not fatal"
    );
    let text = stdout(&out);
    assert!(text.contains("in progress #5: the second · pass"), "{text}");
    assert!(text.contains("needs a pool root"), "{text}");

    let envelope = envelope(&ff_tower(repo.path(), &["next", "--json"]));
    assert_eq!(envelope["data"]["picked"], serde_json::json!([]));
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
    stdout(&ff_tower(
        repo.path(),
        &["file", "skilled", "look this over"],
    ));

    let envelope = envelope(&ff_tower(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(
        envelope["data"]["picked"][0]["skill"],
        serde_json::json!("review")
    );

    // The human render says it on the picked line, where the loop reads.
    let text = stdout(&ff_tower(repo.path(), &["next"]));
    assert!(
        text.contains("in progress #2: look this over · pass · skill review"),
        "{text}"
    );
}

#[test]
fn a_flight_naming_no_skill_omits_the_field() {
    let repo = repo();
    file_pipeline(&repo, "plain work");

    let envelope = envelope(&ff_tower(repo.path(), &["next", "--peek", "--json"]));
    let row = &envelope["data"]["picked"][0];
    assert_eq!(row["flight"], serde_json::json!("pi.2"));
    assert!(
        row.get("skill").is_none(),
        "absent, not null, when the flight names none: {row}"
    );

    let text = stdout(&ff_tower(repo.path(), &["next"]));
    assert!(!text.contains("skill"), "{text}");
}

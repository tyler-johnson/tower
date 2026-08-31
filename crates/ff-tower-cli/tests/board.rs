//! The binary, spawned the way fufu's dispatch spawns it: `FF_REPO` in the
//! environment — the production handshake, not a test-only door.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::log::{Kind, Store};
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

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).expect("an envelope")
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

/// Block until the wall clock's second changes — under a second, and the
/// only way an event and a capture taken back to back land on distinct
/// stamps.
fn next_second() {
    let second = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs()
    };
    let started = second();
    while second() == started {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// A repository with one filed flight on its log.
fn repo_with_a_filing() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    Store::open(repo.path())
        .expect("open")
        .append(vec![Kind::Filed {
            procedure: None,
            subject: "write the doctor verb".to_string(),
            body: String::new(),
            status: "triage".to_string(),
            assignee: None,
            priority: "none".to_string(),
            labels: Vec::new(),
            skill: None,
            bay: None,
            done: "asserted".to_string(),
            branch: None,
        }])
        .expect("append");
    repo
}

#[test]
fn json_emits_towers_envelope_and_round_trips() {
    let repo = repo_with_a_filing();
    let out = stdout(&ff_tower(repo.path(), &["--json"]));

    let envelope: serde_json::Value = serde_json::from_str(&out).expect("an envelope");
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("board"));
    let data = envelope["data"].as_object().expect("data is an object");
    for key in [
        "waiting_on_you",
        "triage",
        "waiting",
        "ready",
        "in_progress",
        "held",
        "closed",
        "unrouted",
    ] {
        assert!(data.contains_key(key), "data is missing `{key}`");
    }
    let inbox = data["waiting_on_you"]
        .as_object()
        .expect("the inbox is an object");
    for key in ["questions", "yours"] {
        assert!(inbox.contains_key(key), "the inbox is missing `{key}`");
    }
    assert_eq!(data["triage"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(data["triage"][0]["stale"], serde_json::json!(false));
    assert_eq!(
        data["triage"][0]["changed_since_ready"],
        serde_json::json!(false)
    );
    assert!(data["triage"][0]["last_change"].is_null());
    assert!(data["triage"][0]["progress"].is_null());
}

#[test]
fn bare_and_the_board_alias_agree_byte_for_byte() {
    let repo = repo_with_a_filing();
    let bare = stdout(&ff_tower(repo.path(), &["--json"]));
    let alias = stdout(&ff_tower(repo.path(), &["board", "--json"]));
    assert_eq!(bare, alias);
}

#[test]
fn a_piped_render_is_plain_text_and_names_its_groups() {
    let repo = repo_with_a_filing();
    let out = stdout(&ff_tower(repo.path(), &[]));

    assert!(
        !out.contains('\x1b'),
        "piped output has escape bytes: {out:?}"
    );
    assert!(
        out.contains("triage\n"),
        "the group header is the stored status: {out}"
    );
    assert!(out.contains("#1"));
    assert!(out.contains("write the doctor verb"));
    assert!(out.contains("1 flight · ff tower file to add one"));
}

#[test]
fn the_groups_are_the_stored_statuses_in_lifecycle_order() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    for subject in ["one", "two", "three", "four"] {
        stdout(&ff_tower(repo.path(), &["file", subject]));
    }
    stdout(&ff_tower(repo.path(), &["status", "2", "waiting"]));
    stdout(&ff_tower(repo.path(), &["status", "3", "ready"]));
    stdout(&ff_tower(repo.path(), &["status", "4", "in_progress"]));

    let out = stdout(&ff_tower(repo.path(), &[]));
    let order: Vec<usize> = ["triage\n", "waiting\n", "ready\n", "in progress\n"]
        .iter()
        .map(|title| {
            out.find(title)
                .unwrap_or_else(|| panic!("{title} in {out}"))
        })
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "lifecycle order: {out}"
    );

    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    let data = &envelope["data"];
    assert_eq!(data["triage"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(data["waiting"][0]["id"], serde_json::json!("pi.2"));
    assert_eq!(data["ready"][0]["id"], serde_json::json!("pi.3"));
    assert_eq!(data["in_progress"][0]["id"], serde_json::json!("pi.4"));
}

#[test]
fn a_closed_flight_lands_in_the_closed_group_and_out_of_the_count() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["file", "still going"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("closed\n"), "{out}");
    assert!(
        out.contains("1 flight · ff tower file to add one"),
        "closed is on the record, not in the count: {out}"
    );

    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        envelope["data"]["closed"][0]["id"],
        serde_json::json!("pi.1")
    );
    assert_eq!(
        envelope["data"]["closed"][0]["status"],
        serde_json::json!("done")
    );
    assert_eq!(
        envelope["data"]["triage"][0]["id"],
        serde_json::json!("pi.2")
    );
}

#[test]
fn a_held_flight_is_pinned_in_the_inbox_and_keeps_its_group() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which flow wins?"]);

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("questions\n"), "{out}");
    assert!(out.contains("held\n"), "{out}");
    assert!(out.contains("which flow wins?"), "{out}");

    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        envelope["data"]["waiting_on_you"]["questions"][0]["id"],
        serde_json::json!("pi.1")
    );
    assert_eq!(envelope["data"]["held"][0]["id"], serde_json::json!("pi.1"));
}

#[test]
fn a_ready_flight_in_the_me_lane_is_the_inboxs_second_group() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "mine to do"]));
    stdout(&ff_tower(repo.path(), &["assign", "1", "me"]));
    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("yours\n"), "{out}");

    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        envelope["data"]["waiting_on_you"]["yours"][0]["id"],
        serde_json::json!("pi.1")
    );
    assert_eq!(
        envelope["data"]["ready"][0]["id"],
        serde_json::json!("pi.1")
    );
}

#[test]
fn an_empty_repository_renders_the_empty_board_hint() {
    let repo = Repo::new();
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert_eq!(out, "nothing on the board · ff tower file to add one\n");
}

#[test]
fn a_missing_ff_exits_one_and_names_fufu() {
    let repo = Repo::new();
    let output = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .env("FF_REPO", repo.path())
        .env("TOWER_FF", "/nonexistent/ff")
        .output()
        .expect("spawn ff-tower");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("ff-tower: "), "got {stderr:?}");
    assert!(stderr.contains("fufu"), "the dependency is named: {stderr}");

    // The same failure under --json is an error envelope on stdout — slice
    // 3's scoped omission, closed.
    let output = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(["--json"])
        .env("FF_REPO", repo.path())
        .env("TOWER_FF", "/nonexistent/ff")
        .output()
        .expect("spawn ff-tower");

    assert_eq!(output.status.code(), Some(1));
    let envelope: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("an envelope");
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("board"));
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("ff/not-installed")
    );
    // The raise site says nothing, so `exits_for` falls back to naming
    // the registry lookup — a coded failure always has prose behind it.
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower explain ff/not-installed"])
    );
    assert!(envelope.get("data").is_none(), "data and error, never both");
}

#[test]
fn colliding_flights_carry_the_warn_phrase_and_the_json_verdicts() {
    // Built through the binary where a verb exists for it, and through the
    // core `Ff` for the session-tagged captures an agent would take.
    use ff_tower_core::ff::Ff;

    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "left work"]));
    stdout(&ff_tower(repo.path(), &["file", "right work"]));

    repo.ff(&["start", "-b", "left"]);
    repo.write("shared.txt", "left side\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "left: touch shared"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("shared.txt", "right side\n");
    Ff::at(repo.path())
        .session("pi.2")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "right: touch shared"]);

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("collides #2 on shared.txt"), "{out}");
    assert!(out.contains("collides #1 on shared.txt"), "{out}");
    assert!(!out.contains("no verdict"), "{out}");

    let out = stdout(&ff_tower(repo.path(), &["--json"]));
    let envelope: serde_json::Value = serde_json::from_str(&out).expect("an envelope");
    let rows = envelope["data"]["triage"].as_array().expect("triage");
    assert_eq!(rows.len(), 2);
    for view in rows {
        let other = if view["id"] == serde_json::json!("pi.1") {
            "pi.2"
        } else {
            "pi.1"
        };
        assert_eq!(view["collides"][0]["with"], serde_json::json!(other));
        assert_eq!(
            view["collides"][0]["paths"],
            serde_json::json!(["shared.txt"])
        );
        assert_eq!(view["unanswered"], serde_json::json!([]));
    }
}

#[test]
fn a_ready_flight_whose_branch_moved_carries_the_audit_line() {
    use ff_tower_core::ff::Ff;

    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "cleared work"]));
    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));

    // The capture has to land after the move, which is the whole test —
    // a branch that moved *before* the flight was set Ready is the
    // resumed hold, and says nothing. Epoch seconds are the log's
    // granularity, so wait out the current one first.
    next_second();
    repo.write("work.txt", "an agent was here\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(
        out.contains("changes on the branch since it was set ready"),
        "{out}"
    );
    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        envelope["data"]["ready"][0]["changed_since_ready"],
        serde_json::json!(true)
    );
    assert_eq!(
        envelope["data"]["ready"][0]["stale"],
        serde_json::json!(false),
        "the two audits are independent facts"
    );
}

#[test]
fn a_decomposed_parent_carries_its_familys_progress_and_the_children_are_rows() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    stdout(&ff_tower(
        repo.path(),
        &["decompose", "1", "part one", "part two"],
    ));

    // Decomposing creates three things and the board says so: the
    // parent's mark is what marks it a family, and the children are
    // rows of their own beside it.
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("a broad task (0/2)"), "{out}");
    assert!(out.contains("part one"), "{out}");
    assert!(out.contains("part two"), "{out}");
    assert!(
        out.contains("3 flights · ff tower file to add one"),
        "the count counts every live flight: {out}"
    );

    stdout(&ff_tower(repo.path(), &["done", "2"]));
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("a broad task (1/2)"), "{out}");

    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        envelope["data"]["triage"][0]["progress"],
        serde_json::json!([1, 2])
    );
    assert_eq!(
        envelope["data"]["closed"][0]["subject"],
        serde_json::json!("part one"),
        "a closed child is a closed row like any other"
    );
}

/// The flat board: a sub-flight files into the group its own status
/// names whether or not it needs anyone, and it carries the subject it
/// was filed with — nothing is prefixed onto it.
#[test]
fn a_sub_flight_is_a_row_in_its_own_status_group() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    stdout(&ff_tower(
        repo.path(),
        &["decompose", "1", "part one", "part two"],
    ));
    stdout(&ff_tower(repo.path(), &["assign", "2", "agent"]));
    stdout(&ff_tower(repo.path(), &["status", "2", "ready"]));

    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("part one"), "{out}");
    assert!(out.contains("part two"), "{out}");
    assert!(
        !out.contains("\u{203a}"),
        "no crumb rides the subject: {out}"
    );

    let envelope = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        envelope["data"]["ready"][0]["subject"],
        serde_json::json!("part one")
    );
    assert_eq!(
        envelope["data"]["triage"][0]["subject"],
        serde_json::json!("a broad task")
    );
    assert_eq!(
        envelope["data"]["triage"][1]["subject"],
        serde_json::json!("part two")
    );
}

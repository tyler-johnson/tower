//! The write verbs, spawned the way fufu's dispatch spawns them: `FF_REPO`
//! in the environment, argv carrying the verb. file/comment/link never
//! spawn fufu, so no `TOWER_FF` appears here — a wrong `ff` on PATH could
//! not break these even if it tried.

use std::path::Path;
use std::process::{Command, Output};

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

#[test]
fn file_echoes_the_filing_and_the_tail() {
    let repo = repo();
    let out = stdout(&ff_tower(
        repo.path(),
        &["file", "fix the flaky retry test"],
    ));
    assert_eq!(
        out,
        "filed #1 under open: fix the flaky retry test\nboard: ff tower\n"
    );
}

#[test]
fn file_json_round_trips_body_and_procedure() {
    let repo = repo();
    let out = ff_tower(
        repo.path(),
        &[
            "file",
            "try the verbs",
            "-m",
            "body text",
            "-p",
            "review",
            "--json",
        ],
    );
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("file"));
    let filed = &envelope["data"]["filed"];
    assert_eq!(filed["id"], serde_json::json!("pi.1"));
    assert_eq!(filed["kind"], serde_json::json!("filed"));
    assert_eq!(filed["body"]["subject"], serde_json::json!("try the verbs"));
    assert_eq!(filed["body"]["body"], serde_json::json!("body text"));
    assert_eq!(filed["body"]["procedure"], serde_json::json!("review"));
}

#[test]
fn a_filed_flight_lands_in_the_open_section() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "land on the board"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        board["data"]["open"][0]["subject"],
        serde_json::json!("land on the board")
    );
}

#[test]
fn an_empty_subject_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "   ", "--json"]);
    refusal(&out, 2, "usage/empty-subject");
}

#[test]
fn an_empty_procedure_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "a subject", "-p", "", "--json"]);
    refusal(&out, 2, "usage/empty-procedure");
}

#[test]
fn a_comment_counts_on_the_board() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    let out = stdout(&ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note"]));
    assert_eq!(out, "commented on #1\nboard: ff tower\n");
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"][0]["comments"], serde_json::json!(1));
}

#[test]
fn comment_json_carries_the_appended_event() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    let out = ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("comment"));
    let commented = &envelope["data"]["commented"];
    assert_eq!(commented["id"], serde_json::json!("pi.2"));
    assert_eq!(commented["body"]["flight"], serde_json::json!("pi.1"));
    assert_eq!(commented["body"]["text"], serde_json::json!("a note"));
}

#[test]
fn a_comment_on_a_missing_flight_is_refused() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["comment", "pi.99", "-m", "x", "--json"]);
    let envelope = refusal(&out, 1, "flight/not-found");
    assert_eq!(envelope["error"]["exits"], serde_json::json!(["ff tower"]));

    // The human rendering of the same refusal: stderr, with the try block.
    let out = ff_tower(repo.path(), &["comment", "pi.99", "-m", "x"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with("ff-tower: "), "got {stderr:?}");
    assert!(stderr.contains("  try:\n    ff tower\n"), "got {stderr:?}");
}

#[test]
fn a_comment_without_a_note_is_refused() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["comment", "pi.1", "--json"]);
    let envelope = refusal(&out, 2, "usage/needs-message");
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower comment <flight> -m <note>"])
    );
}

#[test]
fn link_declares_the_edge_both_ways() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["file", "the dependent"]));
    let out = stdout(&ff_tower(repo.path(), &["link", "pi.2", "pi.1"]));
    assert_eq!(out, "linked #2: depends on #1\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let by_id = |id: &str| {
        open.iter()
            .find(|view| view["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no {id} in open"))
    };
    assert_eq!(by_id("pi.2")["depends_on"], serde_json::json!(["pi.1"]));
    assert_eq!(by_id("pi.1")["blocks"], serde_json::json!(["pi.2"]));
}

#[test]
fn a_self_link_is_a_usage_refusal() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "itself"]));
    let out = ff_tower(repo.path(), &["link", "pi.1", "pi.1", "--json"]);
    refusal(&out, 2, "usage/self-link");
    // A bare seq naming the same flight is a self-link only resolution
    // can see — the check fires on resolved ids.
    let out = ff_tower(repo.path(), &["link", "1", "1", "--json"]);
    refusal(&out, 2, "usage/self-link");
}

#[test]
fn a_link_to_a_missing_flight_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the only flight"]));
    let out = ff_tower(repo.path(), &["link", "pi.1", "pi.9", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn the_same_edge_twice_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["file", "the dependent"]));
    stdout(&ff_tower(repo.path(), &["link", "pi.2", "pi.1"]));
    let out = ff_tower(repo.path(), &["link", "pi.2", "pi.1", "--json"]);
    refusal(&out, 1, "link/exists");
}

#[test]
fn a_comment_shows_in_the_rendered_note_line() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    stdout(&ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note"]));
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("1 comment"), "the note line counts: {out}");
}

#[test]
fn a_missing_identity_is_a_coded_envelope() {
    let repo = repo();
    repo.git(&["config", "--unset", "user.email"]);
    let output = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(["file", "a subject", "--json"])
        .env("FF_REPO", repo.path())
        // The machine's own git config must not answer for the fixture.
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("EMAIL")
        .output()
        .expect("spawn ff-tower");
    let envelope = refusal(&output, 1, "identity/missing");
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["git config user.email <email>"])
    );
}

#[test]
fn a_bare_seq_resolves_against_the_board() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "take a bare seq"]));
    stdout(&ff_tower(repo.path(), &["comment", "1", "-m", "note"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"][0]["comments"], serde_json::json!(1));
}

#[test]
fn a_hash_prefixed_reference_is_accepted() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "take a pasted ref"]));
    stdout(&ff_tower(repo.path(), &["comment", "#1", "-m", "note"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"][0]["comments"], serde_json::json!(1));
}

#[test]
fn a_bare_seq_with_no_match_is_not_found() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the only flight"]));
    let out = ff_tower(repo.path(), &["comment", "9", "-m", "x", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn a_bare_seq_links_and_the_wire_stays_full() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the dependency"]));
    stdout(&ff_tower(repo.path(), &["file", "the dependent"]));
    stdout(&ff_tower(repo.path(), &["link", "2", "1"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let dependent = open
        .iter()
        .find(|view| view["id"] == serde_json::json!("pi.2"))
        .expect("pi.2 in open");
    assert_eq!(dependent["depends_on"], serde_json::json!(["pi.1"]));
}

#[test]
fn claim_puts_the_flight_in_the_air_on_the_claim_alone() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "take a claim"]));
    let out = stdout(&ff_tower(repo.path(), &["claim", "1"]));
    assert_eq!(out, "claimed #1: take a claim\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let flown = &board["data"]["in_the_air"][0];
    assert_eq!(flown["id"], serde_json::json!("pi.1"));
    assert_eq!(
        flown["claimed_by"],
        serde_json::json!("tests@tower.invalid")
    );
    assert!(flown["branch"].is_null());

    // No branch yet, so the note line carries the dim `claimed` phrase.
    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(rendered.contains("in the air"), "got {rendered}");
    assert!(rendered.contains("claimed"), "got {rendered}");
}

#[test]
fn a_second_claim_is_refused_naming_the_claimant() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "claimed once"]));
    stdout(&ff_tower(repo.path(), &["claim", "1"]));
    let out = ff_tower(repo.path(), &["claim", "1", "--json"]);
    let envelope = refusal(&out, 1, "claim/taken");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is already claimed by tests@tower.invalid")
    );
    assert_eq!(envelope["error"]["exits"], serde_json::json!([]));
}

#[test]
fn hold_exits_three_with_a_data_envelope() {
    // Never through the stdout() helper — 3 is a success this helper
    // would read as failure. The envelope is the data form, not an error.
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    let out = ff_tower(
        repo.path(),
        &["hold", "1", "-m", "which retry path?", "--json"],
    );
    assert_eq!(out.status.code(), Some(3), "hold's success exits 3");
    let envelope = envelope(&out);
    assert_eq!(envelope["cmd"], serde_json::json!("hold"));
    assert!(envelope["error"].is_null());
    let held = &envelope["data"]["held"];
    assert_eq!(held["body"]["flight"], serde_json::json!("pi.1"));
    assert_eq!(
        held["body"]["question"],
        serde_json::json!("which retry path?")
    );
}

#[test]
fn hold_echoes_on_stdout_and_still_exits_three() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    let out = ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "held #1: which retry path?\nboard: ff tower\n"
    );
}

#[test]
fn a_hold_without_a_question_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    let out = ff_tower(repo.path(), &["hold", "1", "--json"]);
    let envelope = refusal(&out, 2, "usage/needs-message");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("no question given")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower hold <flight> -m <question>"])
    );
}

#[test]
fn a_second_hold_is_refused_quoting_the_open_question() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);
    let out = ff_tower(repo.path(), &["hold", "1", "-m", "another?", "--json"]);
    let envelope = refusal(&out, 1, "hold/exists");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is already held: which retry path?")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower answer <flight> -m <answer>"])
    );
}

#[test]
fn a_held_flight_renders_under_waiting_on_you() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("waiting on you"), "got {out}");
    assert!(out.contains("? "), "the waiting glyph: {out}");
    assert!(out.contains("which retry path?"), "got {out}");
    assert!(out.contains("asked "), "got {out}");
}

#[test]
fn answer_releases_the_hold_with_the_answer_as_motion() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    stdout(&ff_tower(repo.path(), &["claim", "1"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);
    let out = stdout(&ff_tower(
        repo.path(),
        &["answer", "1", "-m", "the outer one"],
    ));
    assert_eq!(out, "answered #1: the outer one\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["waiting_on_you"], serde_json::json!([]));
    let flown = &board["data"]["in_the_air"][0];
    assert_eq!(flown["id"], serde_json::json!("pi.1"));
    assert!(flown["question"].is_null());
    assert!(flown["last_motion"].is_number());
}

#[test]
fn an_answer_with_no_open_question_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "never held"]));
    let out = ff_tower(repo.path(), &["answer", "1", "-m", "to what?", "--json"]);
    let envelope = refusal(&out, 1, "answer/not-held");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` has no open question")
    );
    assert_eq!(envelope["error"]["exits"], serde_json::json!(["ff tower"]));
}

#[test]
fn done_takes_the_flight_off_the_board_and_out_of_the_count() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["file", "still open"]));
    let out = stdout(&ff_tower(repo.path(), &["done", "1"]));
    assert_eq!(out, "done #1: finished\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    for section in ["waiting_on_you", "in_the_air", "holding", "open"] {
        let views = board["data"][section].as_array().expect(section);
        assert!(
            views
                .iter()
                .all(|view| view["id"] != serde_json::json!("pi.1")),
            "pi.1 must leave {section}"
        );
    }
    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(
        rendered.contains("1 flight ·"),
        "the footer drops: {rendered}"
    );
}

#[test]
fn done_twice_is_refused_as_already_done() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));
    let out = ff_tower(repo.path(), &["done", "1", "--json"]);
    let envelope = refusal(&out, 1, "flight/done");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is already done")
    );
    assert_eq!(envelope["error"]["exits"], serde_json::json!(["ff tower"]));
}

#[test]
fn the_lifecycle_verbs_refuse_a_done_flight_but_a_comment_lands() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let out = ff_tower(repo.path(), &["claim", "1", "--json"]);
    refusal(&out, 1, "flight/done");
    let out = ff_tower(repo.path(), &["hold", "1", "-m", "q?", "--json"]);
    refusal(&out, 1, "flight/done");
    let out = ff_tower(repo.path(), &["answer", "1", "-m", "a", "--json"]);
    refusal(&out, 1, "flight/done");

    // A note on the record is fine — only the lifecycle verbs refuse.
    stdout(&ff_tower(
        repo.path(),
        &["comment", "1", "-m", "postscript"],
    ));
}

#[test]
fn decompose_files_the_parts_and_echoes_them() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    let out = stdout(&ff_tower(
        repo.path(),
        &[
            "decompose",
            "1",
            "the fold: parent-child edges",
            "the cli: decompose verb",
        ],
    ));
    assert_eq!(
        out,
        "decomposed #1 into two parts\n\
         · #2  the fold: parent-child edges\n\
         · #3  the cli: decompose verb\n\
         board: ff tower\n"
    );
}

#[test]
fn decompose_json_carries_the_parent_the_parts_and_the_edges() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "a broad task", "-p", "review"],
    ));
    let out = ff_tower(
        repo.path(),
        &["decompose", "1", "part one", "part two", "--json"],
    );
    let decomposed = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(decomposed["cmd"], serde_json::json!("decompose"));
    let data = &decomposed["data"];
    assert_eq!(data["parent"], serde_json::json!("pi.1"));

    let filed = data["filed"].as_array().expect("filed");
    assert_eq!(filed.len(), 2);
    for (event, subject) in filed.iter().zip(["part one", "part two"]) {
        assert_eq!(event["kind"], serde_json::json!("filed"));
        assert_eq!(event["body"]["subject"], serde_json::json!(subject));
        assert_eq!(event["body"]["body"], serde_json::json!(""));
        // The parts inherit the parent's procedure stamp.
        assert_eq!(event["body"]["procedure"], serde_json::json!("review"));
    }
    assert_eq!(filed[0]["id"], serde_json::json!("pi.2"));
    assert_eq!(filed[1]["id"], serde_json::json!("pi.3"));

    let linked = data["linked"].as_array().expect("linked");
    assert_eq!(linked.len(), 2);
    for (event, part) in linked.iter().zip(["pi.2", "pi.3"]) {
        assert_eq!(event["kind"], serde_json::json!("linked"));
        assert_eq!(event["body"]["from"], serde_json::json!("pi.1"));
        assert_eq!(event["body"]["to"], serde_json::json!(part));
    }

    // The parent depends on both parts; each part blocks the parent.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let by_id = |id: &str| {
        open.iter()
            .find(|view| view["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no {id} in open"))
    };
    assert_eq!(
        by_id("pi.1")["depends_on"],
        serde_json::json!(["pi.2", "pi.3"])
    );
    assert_eq!(by_id("pi.2")["blocks"], serde_json::json!(["pi.1"]));
    assert_eq!(by_id("pi.3")["blocks"], serde_json::json!(["pi.1"]));
}

#[test]
fn decompose_with_no_parts_is_a_usage_refusal() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    let out = ff_tower(repo.path(), &["decompose", "1", "--json"]);
    refusal(&out, 2, "usage/no-parts");
}

#[test]
fn a_part_trimmed_to_nothing_is_a_usage_refusal() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    let out = ff_tower(repo.path(), &["decompose", "1", "part one", "  ", "--json"]);
    refusal(&out, 2, "usage/empty-subject");

    // Nothing was filed: the refusal lands before the append.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"].as_array().expect("open").len(), 1);
}

#[test]
fn decomposing_a_done_flight_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));
    let out = ff_tower(repo.path(), &["decompose", "1", "too late", "--json"]);
    refusal(&out, 1, "flight/done");
}

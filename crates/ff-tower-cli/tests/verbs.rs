//! The write verbs, spawned the way fufu's dispatch spawns them: `FF_REPO`
//! in the environment, argv carrying the verb. file/comment/link never
//! spawn fufu, so no `TOWER_FF` appears here — a wrong `ff` on PATH could
//! not break these even if it tried. `file` does read the procedure
//! registry, so every spawn points `XDG_CONFIG_HOME` at the fixture's own
//! tempdir.

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

/// One flight's record through `brief` — the surface that answers for a
/// flight's family, which the board's own rows say nothing about.
fn brief_of(repo: &Path, flight: &str) -> serde_json::Value {
    envelope(&ff_tower(repo, &["brief", flight, "--json"]))["data"].clone()
}

/// The flight ids on one of a board's groups, in the group's own order.
fn row_ids(group: &serde_json::Value) -> Vec<&str> {
    group
        .as_array()
        .expect("a group")
        .iter()
        .map(|view| view["id"].as_str().expect("id"))
        .collect()
}

/// The flight ids on one of a brief's family lists, in the fold's order.
fn family(brief: &serde_json::Value, key: &str) -> Vec<String> {
    brief[key]
        .as_array()
        .expect(key)
        .iter()
        .map(|link| link["flight"].as_str().expect("flight").to_string())
        .collect()
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

/// A single-flight procedure in the repository layer, so a test can file
/// one flight under a name that is not built in.
/// `docs/procedures/review.toml`'s shape in the repository layer. The
/// engine ships empty, so the procedure forms need a definition
/// installed before they have a name to take.
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

fn install_chore(repo: &Repo) {
    repo.write(
        ".tower/procedures/chore.toml",
        "name = \"chore\"\n\n[[flight]]\nid       = \"do\"\nassignee = \"me\"\n",
    );
}

#[test]
fn a_bare_file_echoes_the_triage_filing_and_the_tail() {
    let repo = repo();
    let out = stdout(&ff_tower(
        repo.path(),
        &["file", "fix the flaky retry test"],
    ));
    assert_eq!(
        out,
        "filed #1 in triage: fix the flaky retry test\nboard: ff tower\n"
    );
}

#[test]
fn file_json_round_trips_body_and_procedure() {
    let repo = repo();
    install_chore(&repo);
    let out = ff_tower(
        repo.path(),
        &[
            "file",
            "chore",
            "try the verbs",
            "-m",
            "body text",
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
    assert_eq!(filed["body"]["procedure"], serde_json::json!("chore"));
    // One flight collapses onto the filing, born Ready with the
    // definition's fields.
    assert_eq!(filed["body"]["status"], serde_json::json!("ready"));
    assert_eq!(filed["body"]["assignee"], serde_json::json!("me"));
    assert_eq!(envelope["data"]["parts"], serde_json::json!([]));
    assert_eq!(envelope["data"]["linked"], serde_json::json!([]));
}

#[test]
fn a_bare_file_is_one_triage_flight_and_one_event() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "a plain one", "--json"]);
    let filing = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(filing["data"]["filed"]["id"], serde_json::json!("pi.1"));
    assert!(
        filing["data"]["filed"]["body"]["procedure"].is_null(),
        "a bare filing carries no procedure"
    );
    assert_eq!(
        filing["data"]["filed"]["body"]["status"],
        serde_json::json!("triage")
    );
    assert!(filing["data"]["filed"]["body"]["assignee"].is_null());
    assert_eq!(
        filing["data"]["filed"]["body"]["priority"],
        serde_json::json!("none")
    );
    assert_eq!(filing["data"]["parts"], serde_json::json!([]));
    assert_eq!(filing["data"]["linked"], serde_json::json!([]));

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"].as_array().expect("open").len(), 1);
}

#[test]
fn the_field_flags_ride_the_filing_and_the_board_reads_them_back() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &[
            "file",
            "laned work",
            "-p",
            "high",
            "--label",
            "chore",
            "--label",
            "web",
            "--skill",
            "review",
            "--assignee",
            "agent",
            "--bay",
            "warm",
        ],
    ));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let view = &board["data"]["triage"][0];
    assert_eq!(view["status"], serde_json::json!("triage"));
    assert_eq!(view["priority"], serde_json::json!("high"));
    assert_eq!(view["labels"], serde_json::json!(["chore", "web"]));
    assert_eq!(view["skill"], serde_json::json!("review"));
    assert_eq!(view["assignee"], serde_json::json!("agent"));

    let brief = envelope(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["bay"], serde_json::json!("warm"));
}

#[test]
fn a_bad_lane_at_filing_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(
        repo.path(),
        &["file", "laned work", "--assignee", "pair", "--json"],
    );
    let envelope = refusal(&out, 2, "usage/bad-assignee");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`pair` is not a lane — me, agent, or none")
    );

    // Nothing was filed: the refusal lands before the append.
    let board = self::envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"], serde_json::json!([]));
}

#[test]
fn file_under_review_echoes_the_parent_and_its_flights() {
    let repo = repo();
    install_review(&repo);
    let out = stdout(&ff_tower(
        repo.path(),
        &["file", "review", "the retry test"],
    ));
    assert_eq!(
        out,
        "filed #1 under review: the retry test\n\
         · #2  the retry test · pass     ready · agent\n\
         · #3  the retry test · smoke    ready · me\n\
         · #4  the retry test · verdict  waiting · me\n\
         board: ff tower\n"
    );
}

#[test]
fn file_under_review_json_carries_the_parent_three_flights_and_five_edges() {
    let repo = repo();
    install_review(&repo);
    let out = ff_tower(
        repo.path(),
        &[
            "file",
            "review",
            "the retry test",
            "-m",
            "body text",
            "--json",
        ],
    );
    let filing = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(filing["cmd"], serde_json::json!("file"));
    let data = &filing["data"];

    // The parent keeps the procedure stamp and the body, and is born
    // Waiting on its children.
    let filed = &data["filed"];
    assert_eq!(filed["id"], serde_json::json!("pi.1"));
    assert_eq!(filed["body"]["procedure"], serde_json::json!("review"));
    assert_eq!(
        filed["body"]["subject"],
        serde_json::json!("the retry test")
    );
    assert_eq!(filed["body"]["body"], serde_json::json!("body text"));
    assert_eq!(filed["body"]["status"], serde_json::json!("waiting"));

    let parts = data["parts"].as_array().expect("parts");
    assert_eq!(parts.len(), 3);
    let ids: Vec<&str> = parts
        .iter()
        .map(|event| event["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(ids, ["pi.2", "pi.3", "pi.4"]);
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached — and the born statuses fall out of the edges.
    assert_eq!(
        parts[0]["body"],
        serde_json::json!({
            "procedure": "review",
            "subject": "the retry test · pass",
            "body": "",
            "status": "ready",
            "assignee": "agent",
            "priority": "none",
            "skill": "review",
            "done": "asserted",
            // `review` resolves its subject against a branch, so file
            // time stamps the branch each flight flies on.
            "branch": "the retry test",
        })
    );
    assert_eq!(
        parts[1]["body"],
        serde_json::json!({
            "procedure": "review",
            "subject": "the retry test · smoke",
            "body": "",
            "status": "ready",
            "assignee": "me",
            "priority": "none",
            "bay": "warm",
            "done": "asserted",
            "branch": "the retry test",
        })
    );
    assert_eq!(
        parts[2]["body"],
        serde_json::json!({
            "procedure": "review",
            "subject": "the retry test · verdict",
            "body": "",
            "status": "waiting",
            "assignee": "me",
            "priority": "none",
            "done": "asserted",
            "branch": "the retry test",
        })
    );

    // Three edges from the parent, then the `after` DAG: `verdict` waits
    // on both, `pass` and `smoke` on neither.
    let linked = data["linked"].as_array().expect("linked");
    let edges: Vec<(&str, &str)> = linked
        .iter()
        .map(|event| {
            (
                event["body"]["from"].as_str().expect("from"),
                event["body"]["to"].as_str().expect("to"),
            )
        })
        .collect();
    assert_eq!(
        edges,
        [
            ("pi.1", "pi.2"),
            ("pi.1", "pi.3"),
            ("pi.1", "pi.4"),
            ("pi.4", "pi.2"),
            ("pi.4", "pi.3"),
        ]
    );

    // And the briefs read the same DAG back off the log — one level of
    // links each, which is the family a brief answers for.
    let parent = brief_of(repo.path(), "pi.1");
    assert_eq!(family(&parent, "depends_on"), ["pi.2", "pi.3", "pi.4"]);
    assert_eq!(parent["progress"], serde_json::json!([0, 3]));
    let verdict = brief_of(repo.path(), "pi.4");
    assert_eq!(family(&verdict, "depends_on"), ["pi.2", "pi.3"]);
    let pass = brief_of(repo.path(), "pi.2");
    assert_eq!(pass["assignee"], serde_json::json!("agent"));
    assert_eq!(pass["status"], serde_json::json!("ready"));

    // The board is flat: the parent, born Waiting on its children, with
    // the mark, and every sub-flight a row in the group its own status
    // names — `pass` and `smoke` Ready, `verdict` Waiting behind them.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["waiting"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        board["data"]["waiting"][0]["progress"],
        serde_json::json!([0, 3])
    );
    assert_eq!(row_ids(&board["data"]["waiting"]), ["pi.1", "pi.4"]);
    assert_eq!(row_ids(&board["data"]["ready"]), ["pi.2", "pi.3"]);
    assert_eq!(
        board["data"]["waiting_on_you"]["yours"][0]["id"],
        serde_json::json!("pi.3")
    );
}

#[test]
fn caller_flags_win_on_a_collapsed_filing() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(
        repo.path(),
        &["file", "chore", "swap the lane", "--assignee", "agent"],
    ));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let view = &board["data"]["ready"][0];
    assert_eq!(view["status"], serde_json::json!("ready"));
    assert_eq!(
        view["assignee"],
        serde_json::json!("agent"),
        "the caller's lane beats the definition's `me`"
    );
}

#[test]
fn a_procedure_that_is_not_installed_is_refused() {
    let repo = repo();
    install_chore(&repo);
    let out = ff_tower(repo.path(), &["file", "ghost", "a subject", "--json"]);
    let refused = refusal(&out, 1, "procedure/not-found");
    assert_eq!(
        refused["error"]["message"],
        serde_json::json!("no procedure `ghost` — installed: chore")
    );
    assert_eq!(
        refused["error"]["exits"],
        serde_json::json!(["ff tower procedures"])
    );

    // Nothing was filed: the refusal lands before the append. One word
    // is never guessed as a procedure name, so the subject spelling of
    // the same word files fine.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"], serde_json::json!([]));
}

#[test]
fn a_filed_flight_lands_in_the_triage_group() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "land on the board"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"][0]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        board["data"]["triage"][0]["subject"],
        serde_json::json!("land on the board")
    );
}

#[test]
fn an_empty_subject_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "   ", "--json"]);
    refusal(&out, 2, "usage/empty-subject");
    // No argument at all is the same refusal — clap holds no `required`.
    let out = ff_tower(repo.path(), &["file", "--json"]);
    refusal(&out, 2, "usage/empty-subject");
}

#[test]
fn an_empty_procedure_is_a_usage_refusal() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "", "a subject", "--json"]);
    refusal(&out, 2, "usage/empty-procedure");
}

#[test]
fn a_comment_counts_on_the_board() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "carry a note"]));
    let out = stdout(&ff_tower(repo.path(), &["comment", "pi.1", "-m", "a note"]));
    assert_eq!(out, "commented on #1\nboard: ff tower\n");
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"][0]["comments"], serde_json::json!(1));
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

    assert_eq!(
        family(&brief_of(repo.path(), "pi.2"), "depends_on"),
        ["pi.1"]
    );
    assert_eq!(family(&brief_of(repo.path(), "pi.1"), "blocks"), ["pi.2"]);
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
        .env("GIT_CONFIG_GLOBAL", ff_tower_testsupport::null_device())
        .env("GIT_CONFIG_SYSTEM", ff_tower_testsupport::null_device())
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
    assert_eq!(board["data"]["triage"][0]["comments"], serde_json::json!(1));
}

#[test]
fn a_hash_prefixed_reference_is_accepted() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "take a pasted ref"]));
    stdout(&ff_tower(repo.path(), &["comment", "#1", "-m", "note"]));
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"][0]["comments"], serde_json::json!(1));
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
    let open = board["data"]["triage"].as_array().expect("open");
    let dependent = open
        .iter()
        .find(|view| view["id"] == serde_json::json!("pi.2"))
        .expect("pi.2 in open");
    assert_eq!(dependent["depends_on"], serde_json::json!(["pi.1"]));
}

#[test]
fn a_status_move_puts_the_flight_in_progress_with_the_pilots_byline() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "take a pull"]));
    let out = stdout(&ff_tower(repo.path(), &["status", "1", "in_progress"]));
    assert_eq!(
        out,
        "moved #1 to in progress: take a pull\nboard: ff tower\n"
    );

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let flown = &board["data"]["in_progress"][0];
    assert_eq!(flown["id"], serde_json::json!("pi.1"));
    assert_eq!(flown["status"], serde_json::json!("in_progress"));
    assert_eq!(flown["status_by"], serde_json::json!("tests@tower.invalid"));
    assert!(flown["branch"].is_null());

    // The note line carries the pilot phrase.
    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(rendered.contains("in progress\n"), "got {rendered}");
    assert!(
        rendered.contains("in progress — tests@tower.invalid"),
        "got {rendered}"
    );
}

#[test]
fn status_json_carries_the_appended_move() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "moved work"]));
    let out = ff_tower(repo.path(), &["status", "1", "ready", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("status"));
    let moved = &envelope["data"]["status"];
    assert_eq!(moved["id"], serde_json::json!("pi.2"));
    assert_eq!(moved["kind"], serde_json::json!("status"));
    assert_eq!(moved["body"]["flight"], serde_json::json!("pi.1"));
    assert_eq!(moved["body"]["status"], serde_json::json!("ready"));
    assert!(moved["body"].get("reason").is_none());
}

#[test]
fn a_word_outside_the_status_vocabulary_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "standing work"]));
    let out = ff_tower(repo.path(), &["status", "1", "claimed", "--json"]);
    let envelope = refusal(&out, 2, "usage/bad-status");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!(
            "`claimed` is not a status — triage, waiting, ready, in_progress, held, done, or canceled"
        )
    );
}

#[test]
fn assign_moves_the_lane_and_none_clears_it() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "laned work"]));
    let out = stdout(&ff_tower(repo.path(), &["assign", "1", "agent"]));
    assert_eq!(out, "assigned #1 to agent: laned work\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        board["data"]["triage"][0]["assignee"],
        serde_json::json!("agent")
    );

    let out = stdout(&ff_tower(repo.path(), &["assign", "1", "none"]));
    assert_eq!(out, "cleared the lane on #1: laned work\nboard: ff tower\n");
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert!(board["data"]["triage"][0]["assignee"].is_null());
}

#[test]
fn assign_json_carries_the_appended_event() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "laned work"]));
    let out = ff_tower(repo.path(), &["assign", "1", "me", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("assign"));
    let assigned = &envelope["data"]["assigned"];
    assert_eq!(assigned["kind"], serde_json::json!("assigned"));
    assert_eq!(assigned["body"]["flight"], serde_json::json!("pi.1"));
    assert_eq!(assigned["body"]["assignee"], serde_json::json!("me"));
}

#[test]
fn a_word_outside_the_lanes_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "laned work"]));
    let out = ff_tower(repo.path(), &["assign", "1", "you", "--json"]);
    let envelope = refusal(&out, 2, "usage/bad-assignee");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`you` is not a lane — me, agent, or none")
    );
}

#[test]
fn a_release_back_to_ready_rejoins_the_pool() {
    let repo = repo();
    stdout(&ff_tower(
        repo.path(),
        &["file", "the recovery", "--assignee", "agent"],
    ));
    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));
    let envelope = envelope(&ff_tower(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(
        envelope["data"]["picked"][0]["flight"],
        serde_json::json!("pi.1")
    );

    stdout(&ff_tower(repo.path(), &["status", "1", "in_progress"]));
    let peeked = self::envelope(&ff_tower(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(
        peeked["data"]["picked"],
        serde_json::json!([]),
        "the pull holds it out of the pool"
    );

    stdout(&ff_tower(repo.path(), &["status", "1", "ready"]));
    let peeked = self::envelope(&ff_tower(repo.path(), &["next", "--peek", "--json"]));
    assert_eq!(
        peeked["data"]["picked"][0]["flight"],
        serde_json::json!("pi.1"),
        "the release is the recovery path"
    );
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
fn a_held_flight_is_pinned_in_the_inbox_with_the_held_status() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);
    let out = stdout(&ff_tower(repo.path(), &[]));
    assert!(out.contains("questions"), "got {out}");
    assert!(out.contains("? "), "the question glyph: {out}");
    assert!(out.contains("which retry path?"), "got {out}");
    assert!(out.contains("asked "), "got {out}");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        board["data"]["waiting_on_you"]["questions"][0]["status"],
        serde_json::json!("held"),
        "the hold is a status move too"
    );
}

#[test]
fn an_open_question_refuses_a_move_short_of_closing() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);

    let out = ff_tower(repo.path(), &["status", "1", "ready", "--json"]);
    let envelope = refusal(&out, 1, "status/held");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is held on a question: which retry path?")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower answer <flight> -m <answer>"])
    );

    // The two exceptions: closing the flight abandons the question
    // deliberately.
    stdout(&ff_tower(repo.path(), &["done", "1"]));
}

#[test]
fn answer_releases_the_hold_to_ready() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "stuck"]));
    ff_tower(repo.path(), &["hold", "1", "-m", "which retry path?"]);
    let out = stdout(&ff_tower(
        repo.path(),
        &["answer", "1", "-m", "the outer one"],
    ));
    assert_eq!(out, "answered #1: the outer one\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(
        board["data"]["waiting_on_you"]["questions"],
        serde_json::json!([])
    );
    let released = &board["data"]["ready"][0];
    assert_eq!(released["id"], serde_json::json!("pi.1"));
    assert_eq!(
        released["status"],
        serde_json::json!("ready"),
        "the answer releases to Ready"
    );
    assert!(released["question"].is_null());
    assert!(
        released["last_change"].is_null(),
        "an answer is a record gesture, not a change on the branch"
    );
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
    for group in ["triage", "waiting", "ready", "in_progress", "held"] {
        let views = board["data"][group].as_array().expect(group);
        assert!(
            views
                .iter()
                .all(|view| view["id"] != serde_json::json!("pi.1")),
            "pi.1 must leave {group}"
        );
    }
    assert_eq!(
        board["data"]["closed"][0]["id"],
        serde_json::json!("pi.1"),
        "on the record, in the closed group"
    );
    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(
        rendered.contains("1 flight ·"),
        "the footer drops: {rendered}"
    );
}

#[test]
fn done_json_lands_under_its_own_cmd_name() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    let out = ff_tower(repo.path(), &["done", "1", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("done"));
    let moved = &envelope["data"]["status"];
    assert_eq!(moved["kind"], serde_json::json!("status"));
    assert_eq!(moved["body"]["status"], serde_json::json!("done"));
}

#[test]
fn cancel_closes_with_the_reason_on_the_move() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "abandoned"]));
    let out = stdout(&ff_tower(
        repo.path(),
        &["cancel", "1", "-m", "superseded by #2"],
    ));
    assert_eq!(out, "canceled #1: abandoned\nboard: ff tower\n");

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"], serde_json::json!([]));

    let brief = envelope(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["status"], serde_json::json!("canceled"));
    assert_eq!(brief["data"]["standing"], serde_json::json!("done"));

    let out = ff_tower(repo.path(), &["cancel", "1", "--json"]);
    let envelope = refusal(&out, 1, "flight/done");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is done — the log keeps its record")
    );
}

#[test]
fn cancel_json_lands_under_its_own_cmd_name_with_the_reason() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "abandoned"]));
    let out = ff_tower(
        repo.path(),
        &["cancel", "1", "-m", "not worth it", "--json"],
    );
    let envelope = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(envelope["cmd"], serde_json::json!("cancel"));
    let moved = &envelope["data"]["status"];
    assert_eq!(moved["body"]["status"], serde_json::json!("canceled"));
    assert_eq!(moved["body"]["reason"], serde_json::json!("not worth it"));
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
fn the_lifecycle_verbs_refuse_a_closed_flight_but_a_comment_lands() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));

    let out = ff_tower(repo.path(), &["status", "1", "ready", "--json"]);
    refusal(&out, 1, "flight/done");
    let out = ff_tower(repo.path(), &["assign", "1", "agent", "--json"]);
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
fn decompose_files_the_sub_flights_and_echoes_them() {
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
        "decomposed #1 into two sub-flights\n\
         · #2  the fold: parent-child edges  triage\n\
         · #3  the cli: decompose verb  triage\n\
         board: ff tower\n"
    );
}

#[test]
fn decompose_json_carries_the_parent_the_children_and_the_edges() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(repo.path(), &["file", "chore", "a broad task"]));
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
        // The children inherit the parent's procedure as provenance, and
        // are otherwise bare filings — Triage, no lane.
        assert_eq!(event["body"]["procedure"], serde_json::json!("chore"));
        assert_eq!(event["body"]["status"], serde_json::json!("triage"));
        assert!(event["body"]["assignee"].is_null());
    }
    assert_eq!(filed[0]["id"], serde_json::json!("pi.2"));
    assert_eq!(filed[1]["id"], serde_json::json!("pi.3"));

    let linked = data["linked"].as_array().expect("linked");
    assert_eq!(linked.len(), 2);
    for (event, child) in linked.iter().zip(["pi.2", "pi.3"]) {
        assert_eq!(event["kind"], serde_json::json!("linked"));
        assert_eq!(event["body"]["from"], serde_json::json!("pi.1"));
        assert_eq!(event["body"]["to"], serde_json::json!(child));
    }

    // The parent depends on both children; each child blocks the parent.
    assert_eq!(
        family(&brief_of(repo.path(), "pi.1"), "depends_on"),
        ["pi.2", "pi.3"]
    );
    assert_eq!(family(&brief_of(repo.path(), "pi.2"), "blocks"), ["pi.1"]);
    assert_eq!(family(&brief_of(repo.path(), "pi.3"), "blocks"), ["pi.1"]);
}

#[test]
fn decompose_under_a_procedure_mints_the_definitions_flights() {
    let repo = repo();
    install_review(&repo);
    stdout(&ff_tower(repo.path(), &["file", "look this over"]));
    let out = stdout(&ff_tower(repo.path(), &["decompose", "1", "review"]));
    assert!(
        out.contains("decomposed #1 into three sub-flights"),
        "{out}"
    );

    // The children keep the parent's subject in theirs, ride the same
    // edges, and are born by their own `after` — Ready or Waiting.
    let parent = brief_of(repo.path(), "pi.1");
    assert_eq!(family(&parent, "depends_on"), ["pi.2", "pi.3", "pi.4"]);
    let pass = brief_of(repo.path(), "pi.2");
    assert_eq!(pass["subject"], serde_json::json!("look this over · pass"));
    assert_eq!(pass["status"], serde_json::json!("ready"));
    assert_eq!(pass["assignee"], serde_json::json!("agent"));
    assert_eq!(
        brief_of(repo.path(), "pi.4")["status"],
        serde_json::json!("waiting")
    );
    // The parent's own fields are untouched — the mint adds children,
    // never re-stamps.
    assert_eq!(parent["status"], serde_json::json!("triage"));
}

#[test]
fn decompose_with_nothing_to_split_into_is_a_usage_refusal() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    let out = ff_tower(repo.path(), &["decompose", "1", "--json"]);
    refusal(&out, 2, "usage/no-parts");
}

#[test]
fn a_sub_flight_trimmed_to_nothing_is_a_usage_refusal() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "a broad task"]));
    let out = ff_tower(repo.path(), &["decompose", "1", "part one", "  ", "--json"]);
    refusal(&out, 2, "usage/empty-subject");

    // Nothing was filed: the refusal lands before the append.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["triage"].as_array().expect("open").len(), 1);
}

#[test]
fn decomposing_a_closed_flight_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "1"]));
    let out = ff_tower(repo.path(), &["decompose", "1", "too late", "--json"]);
    refusal(&out, 1, "flight/done");
}

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

/// A single-part procedure in the repository layer, so a test can file one
/// flight under a name that is not the default.
fn install_chore(repo: &Repo) {
    repo.write(
        ".tower/procedures/chore.toml",
        "name = \"chore\"\n\n[[part]]\nid   = \"do\"\ncrew = \"you\"\n",
    );
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
    install_chore(&repo);
    let out = ff_tower(
        repo.path(),
        &[
            "file",
            "try the verbs",
            "-m",
            "body text",
            "-p",
            "chore",
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
    // One part collapses onto the flight, so there is nothing beside it.
    assert_eq!(envelope["data"]["parts"], serde_json::json!([]));
    assert_eq!(envelope["data"]["linked"], serde_json::json!([]));
}

#[test]
fn a_plain_file_is_still_one_flight_and_one_event() {
    // `open` is a real procedure now, and its single part collapses onto
    // the flight: the echo, the count, and the chain are what they were.
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "a plain one", "--json"]);
    let filing = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(filing["data"]["filed"]["id"], serde_json::json!("pi.1"));
    assert_eq!(
        filing["data"]["filed"]["body"]["procedure"],
        serde_json::json!("open")
    );
    assert_eq!(
        filing["data"]["filed"]["body"]["part"],
        serde_json::json!({"id": "work", "crew": "you", "done": "asserted"})
    );
    assert_eq!(filing["data"]["parts"], serde_json::json!([]));
    assert_eq!(filing["data"]["linked"], serde_json::json!([]));

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"].as_array().expect("open").len(), 1);
}

#[test]
fn file_under_review_echoes_the_parent_and_its_parts() {
    let repo = repo();
    let out = stdout(&ff_tower(
        repo.path(),
        &["file", "the retry test", "-p", "review"],
    ));
    assert_eq!(
        out,
        "filed #1 under review: the retry test\n\
         · #2  the retry test · pass     agent\n\
         · #3  the retry test · smoke    you\n\
         · #4  the retry test · verdict  you\n\
         board: ff tower\n"
    );
}

#[test]
fn file_under_review_json_carries_the_parent_three_parts_and_five_edges() {
    let repo = repo();
    let out = ff_tower(
        repo.path(),
        &[
            "file",
            "the retry test",
            "-m",
            "body text",
            "-p",
            "review",
            "--json",
        ],
    );
    let filing = envelope(&out);
    assert!(out.status.success(), "exit {:?}", out.status.code());
    assert_eq!(filing["cmd"], serde_json::json!("file"));
    let data = &filing["data"];

    // The parent keeps the procedure stamp and the body, and is no part.
    let filed = &data["filed"];
    assert_eq!(filed["id"], serde_json::json!("pi.1"));
    assert_eq!(filed["body"]["procedure"], serde_json::json!("review"));
    assert_eq!(
        filed["body"]["subject"],
        serde_json::json!("the retry test")
    );
    assert_eq!(filed["body"]["body"], serde_json::json!("body text"));
    assert!(filed["body"]["part"].is_null());

    let parts = data["parts"].as_array().expect("parts");
    assert_eq!(parts.len(), 3);
    let ids: Vec<&str> = parts
        .iter()
        .map(|event| event["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(ids, ["pi.2", "pi.3", "pi.4"]);
    // Every row stands alone: `next` hands one to an agent with no parent
    // context attached.
    assert_eq!(
        parts[0]["body"]["subject"],
        serde_json::json!("the retry test · pass")
    );
    assert_eq!(parts[0]["body"]["body"], serde_json::json!(""));
    assert_eq!(
        parts[0]["body"]["part"],
        serde_json::json!({
            "id": "pass",
            "crew": "agent",
            "skill": "review",
            "done": "asserted",
        })
    );
    assert_eq!(
        parts[1]["body"]["part"],
        serde_json::json!({"id": "smoke", "crew": "you", "done": "asserted", "bay": "warm"})
    );
    assert_eq!(
        parts[2]["body"]["part"],
        serde_json::json!({"id": "verdict", "crew": "you", "done": "asserted"})
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

    // And the board reads the same DAG back off the log.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let open = board["data"]["open"].as_array().expect("open");
    let by_id = |id: &str| {
        open.iter()
            .find(|view| view["id"] == serde_json::json!(id))
            .unwrap_or_else(|| panic!("no {id} in open"))
    };
    assert_eq!(
        by_id("pi.1")["depends_on"],
        serde_json::json!(["pi.2", "pi.3", "pi.4"])
    );
    assert_eq!(
        by_id("pi.4")["depends_on"],
        serde_json::json!(["pi.2", "pi.3"])
    );
    assert_eq!(by_id("pi.2")["part"]["crew"], serde_json::json!("agent"));
}

#[test]
fn a_procedure_that_is_not_installed_is_refused() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["file", "a subject", "-p", "ghost", "--json"]);
    let refused = refusal(&out, 1, "procedure/not-found");
    assert_eq!(
        refused["error"]["message"],
        serde_json::json!("no procedure `ghost` — installed: open, review")
    );
    assert_eq!(
        refused["error"]["exits"],
        serde_json::json!(["ff tower procedures"])
    );

    // Nothing was filed: the refusal lands before the append.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["open"], serde_json::json!([]));
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
    // The raise site says nothing, so the exits are the registry's.
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower", "ff tower next"])
    );
}

/// The minimal claimable shape principle 12 admits: an agent-crewed part
/// with a you-crewed end. Filing under it mints a parent and two parts, so
/// `#2` is the flight the pool is about.
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

/// One pipeline filing: the parent `#1`, the agent part `#2`, and the
/// you-crewed `#3`.
fn file_pipeline(repo: &Repo, subject: &str) {
    stdout(&ff_tower(repo.path(), &["file", subject, "-p", "pipeline"]));
}

/// The wire ids `next` would hand out right now, written by nothing — the
/// membership both new verbs exist to move.
fn peeked(repo: &Repo) -> Vec<String> {
    let envelope = envelope(&ff_tower(repo.path(), &["next", "--peek", "--json"]));
    envelope["data"]["picked"]
        .as_array()
        .expect("a picked list")
        .iter()
        .map(|pick| pick["flight"].as_str().expect("a wire id").to_string())
        .collect()
}

#[test]
fn requeue_hands_a_claimed_flight_back_and_next_picks_it_again() {
    let repo = repo();
    install_pipeline(&repo);
    file_pipeline(&repo, "the recovery");
    stdout(&ff_tower(repo.path(), &["claim", "2"]));
    assert!(
        peeked(&repo).is_empty(),
        "the claim holds it out of the pool"
    );

    let out = stdout(&ff_tower(repo.path(), &["requeue", "2"]));
    assert_eq!(out, "requeued #2: the recovery · pass\nboard: ff tower\n");

    // The recovery path end to end: the claim is gone and the pool has
    // the flight back, with no second edit anywhere.
    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    assert_eq!(board["data"]["in_the_air"], serde_json::json!([]));
    let requeued = board["data"]["open"]
        .as_array()
        .expect("the open section")
        .iter()
        .find(|view| view["id"] == serde_json::json!("pi.2"))
        .expect("pi.2 back in open");
    assert!(requeued["claimed_by"].is_null());
    assert_eq!(requeued["taken"], serde_json::json!(false));
    assert!(requeued["requeued_at"].is_number());
    // The filed stamp is untouched — the requeue restored nothing
    // because nothing was destroyed.
    assert_eq!(requeued["part"]["crew"], serde_json::json!("agent"));
    assert_eq!(peeked(&repo), ["pi.2"]);
}

#[test]
fn a_requeue_with_nothing_claimed_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "never claimed"]));
    let out = ff_tower(repo.path(), &["requeue", "1", "--json"]);
    let envelope = refusal(&out, 1, "requeue/unclaimed");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is not claimed — nothing to hand back")
    );
    // The raise site says nothing, so the exits are the registry's.
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower", "ff tower next"])
    );
}

#[test]
fn take_over_a_claim_names_where_it_came_from_and_closes_the_pool() {
    let repo = repo();
    install_pipeline(&repo);
    file_pipeline(&repo, "the override");
    stdout(&ff_tower(repo.path(), &["claim", "2"]));

    let out = stdout(&ff_tower(repo.path(), &["take", "2"]));
    assert_eq!(
        out,
        "took #2 from tests@tower.invalid: the override · pass\nboard: ff tower\n"
    );

    let board = envelope(&ff_tower(repo.path(), &["--json"]));
    let flown = &board["data"]["in_the_air"][0];
    assert_eq!(flown["id"], serde_json::json!("pi.2"));
    assert_eq!(flown["taken"], serde_json::json!(true));
    assert_eq!(
        flown["claimed_by"],
        serde_json::json!("tests@tower.invalid")
    );
    // The stamp still says `agent`; the take is an overlay, not an edit.
    assert_eq!(flown["part"]["crew"], serde_json::json!("agent"));
    assert!(peeked(&repo).is_empty(), "agent off");

    let brief = envelope(&ff_tower(repo.path(), &["brief", "2", "--json"]));
    assert_eq!(
        brief["data"]["taken_by"],
        serde_json::json!("tests@tower.invalid")
    );
    assert!(brief["data"]["taken_at"].is_number());
    assert_eq!(brief["data"]["standing"], serde_json::json!("claimed"));
    let rendered = stdout(&ff_tower(repo.path(), &["brief", "2"]));
    assert!(
        rendered.contains("taken by tests@tower.invalid"),
        "{rendered}"
    );
}

#[test]
fn a_take_with_no_claim_standing_names_nobody() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "unclaimed"]));
    let out = stdout(&ff_tower(repo.path(), &["take", "1"]));
    assert_eq!(out, "took #1: unclaimed\nboard: ff tower\n");
}

#[test]
fn a_second_take_is_refused() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "taken once"]));
    stdout(&ff_tower(repo.path(), &["take", "1"]));
    let out = ff_tower(repo.path(), &["take", "1", "--json"]);
    let envelope = refusal(&out, 1, "take/taken");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("`#1` is already yours")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower requeue <flight>", "ff tower brief <flight>"])
    );
}

#[test]
fn the_note_line_carries_taken_and_then_requeued() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "the ownership phrase"]));
    stdout(&ff_tower(repo.path(), &["take", "1"]));
    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(rendered.contains("in the air"), "{rendered}");
    assert!(rendered.contains("taken"), "{rendered}");
    assert!(
        !rendered.contains("claimed"),
        "one phrase, not two: {rendered}"
    );

    stdout(&ff_tower(repo.path(), &["requeue", "1"]));
    let rendered = stdout(&ff_tower(repo.path(), &[]));
    assert!(rendered.contains("requeued"), "{rendered}");
    assert!(!rendered.contains("taken"), "{rendered}");
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
    install_chore(&repo);
    stdout(&ff_tower(
        repo.path(),
        &["file", "a broad task", "-p", "chore"],
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
        // The parts inherit the parent's procedure stamp, and carry no
        // part stamp of their own — a part named on the command line is
        // not a part of a definition.
        assert_eq!(event["body"]["procedure"], serde_json::json!("chore"));
        assert!(event["body"]["part"].is_null());
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

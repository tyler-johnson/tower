//! The verb API against the real binary: byte parity with the CLI.
//!
//! The read suite's discipline applied to writes. Every success answers
//! 200 with the exact envelope the verb emits under `--json` — asserted
//! by rebuilding it around the event the POST appended, read back off
//! the chain, so the store-assigned time cannot race the comparison —
//! and the next `GET /api/board` reads the write back as a fresh CLI run
//! would. Every refusal answers the byte-for-byte CLI error envelope
//! under the status the id maps to: 400 for what could never run, 404
//! for a reference naming nothing, 409 where the board's standing state
//! refuses the write. Refusals append nothing, so running the same verb
//! through the CLI afterwards reproduces the exact envelope the route
//! answered.

use std::path::Path;

use ff_tower_core::log::{Event, Kind, Store};
use ff_tower_core::{machine, verb};
use ff_tower_testsupport::Repo;
use serde_json::json;

mod support;
use support::{Server, ff_tower, free_port, http, post, refusal, request};

/// A repository with one filed flight on its log, and a server on it.
fn served() -> (Repo, Server) {
    let repo = Repo::new();
    repo.pin_writer("pi");
    file(repo.path(), "write the doctor verb");
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);
    (repo, server)
}

fn file(repo: &Path, subject: &str) {
    Store::open(repo)
        .expect("open")
        .append(vec![Kind::Filed {
            procedure: "open".to_string(),
            subject: subject.to_string(),
            body: String::new(),
            part: None,
        }])
        .expect("append");
}

/// The newest event on the writer's chain matching `wanted` — what the
/// POST appended, read back the way the verb itself reads it.
fn appended(repo: &Path, wanted: impl Fn(&Kind) -> bool) -> Event {
    chain(repo)
        .into_iter()
        .rev()
        .find(|event| wanted(&event.kind))
        .expect("the appended event is on the chain")
}

fn chain(repo: &Path) -> Vec<Event> {
    Store::open(repo).expect("open").read().expect("read")
}

/// Every success wears the same dress: 200, the JSON content type, one
/// envelope line with its trailing newline.
fn ok(path: &str, status: u16, head: &str, body: &str, envelope: String) {
    assert_eq!(status, 200, "{path}: {body}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{path}: {head}"
    );
    assert_eq!(body, format!("{envelope}\n"), "{path}");
}

/// Read-after-write: the next board GET carries the write, and equals a
/// fresh CLI run byte for byte.
fn read_after_write(server: &Server, repo: &Path) {
    let (status, _, board) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{board}");
    let out = ff_tower(repo, &["--json"]);
    assert!(out.status.success(), "the CLI board run failed");
    assert_eq!(board, String::from_utf8_lossy(&out.stdout));
}

/// The refusal half: the status differs by design — the exit code's job
/// on the CLI — and the envelope does not. The CLI runs after the POST,
/// which is fair because a refusal appends nothing.
fn error_parity(
    server: &Server,
    repo: &Path,
    path: &str,
    body: &str,
    status: u16,
    cli: &[&str],
    id: &str,
) {
    let (got, head, answered) = post(&server.addr, path, body);
    assert_eq!(got, status, "{path}: {answered}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{path}: {head}"
    );
    let out = ff_tower(repo, cli);
    refusal(&out, if id.starts_with("usage/") { 2 } else { 1 }, id);
    assert_eq!(answered, String::from_utf8_lossy(&out.stdout), "{path}");
}

#[test]
fn claim_appends_and_answers_the_verbs_envelope() {
    let (repo, server) = served();
    let (status, head, body) = post(&server.addr, "/api/claim", r#"{"flight":"1"}"#);
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Claimed { .. }));
    ok(
        "/api/claim",
        status,
        &head,
        &body,
        machine::emit("claim", &verb::Claimed { claimed: event }),
    );
    read_after_write(&server, repo.path());
}

#[test]
fn take_and_requeue_round_trip_over_http() {
    let (repo, server) = served();
    let (status, head, body) = post(&server.addr, "/api/take", r#"{"flight":"1"}"#);
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Taken { .. }));
    ok(
        "/api/take",
        status,
        &head,
        &body,
        machine::emit("take", &verb::Taken { taken: event }),
    );

    let (status, head, body) = post(&server.addr, "/api/requeue", r#"{"flight":"1"}"#);
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Requeued { .. }));
    ok(
        "/api/requeue",
        status,
        &head,
        &body,
        machine::emit("requeue", &verb::Requeued { requeued: event }),
    );
    read_after_write(&server, repo.path());
}

#[test]
fn hold_answers_two_hundred_with_the_data_envelope() {
    // The CLI's hold exits 3; HTTP has no exit-code channel, so the held
    // outcome is a 200 carrying the same full data envelope.
    let (repo, server) = served();
    let (status, head, body) = post(
        &server.addr,
        "/api/hold",
        r#"{"flight":"1","message":"which retry path?"}"#,
    );
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Held { .. }));
    ok(
        "/api/hold",
        status,
        &head,
        &body,
        machine::emit("hold", &verb::Held { held: event }),
    );

    let (status, head, body) = post(
        &server.addr,
        "/api/answer",
        r#"{"flight":"1","message":"the outer one"}"#,
    );
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Answered { .. }));
    ok(
        "/api/answer",
        status,
        &head,
        &body,
        machine::emit("answer", &verb::Answered { answered: event }),
    );
    read_after_write(&server, repo.path());
}

#[test]
fn comment_and_done_land_on_the_record() {
    let (repo, server) = served();
    let (status, head, body) = post(
        &server.addr,
        "/api/comment",
        r#"{"flight":"1","message":"a note"}"#,
    );
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Commented { .. }));
    ok(
        "/api/comment",
        status,
        &head,
        &body,
        machine::emit("comment", &verb::Commented { commented: event }),
    );

    let (status, head, body) = post(&server.addr, "/api/done", r#"{"flight":"1"}"#);
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Done { .. }));
    ok(
        "/api/done",
        status,
        &head,
        &body,
        machine::emit("done", &verb::Finished { done: event }),
    );
    read_after_write(&server, repo.path());
}

#[test]
fn file_appends_the_whole_batch_and_answers_it() {
    let (repo, server) = served();
    let (status, head, body) = post(
        &server.addr,
        "/api/file",
        r#"{"subject":"the retry test","message":"body text","procedure":"review"}"#,
    );
    // The batch rides the chain in mint order behind the seeded filing:
    // the parent, `review`'s three parts, then its five edges.
    let chain = chain(repo.path());
    let batch = &chain[1..];
    assert_eq!(batch.len(), 9, "the parent, three parts, five edges");
    let (filed, rest) = batch.split_first().expect("the parent");
    let (parts, linked) = rest.split_at(3);
    ok(
        "/api/file",
        status,
        &head,
        &body,
        machine::emit(
            "file",
            &verb::Filed {
                filed: filed.clone(),
                linked: linked.to_vec(),
                parts: parts.to_vec(),
            },
        ),
    );
    read_after_write(&server, repo.path());
}

/// The route: the third writing surface gaining a verb the other two
/// already have. `review` is a three-part built-in, so the batch is the
/// routed head, its parts, and the edges that hang them off the flight —
/// the same shape `file` appends, minus the parent it does not mint.
#[test]
fn triage_routes_over_http_and_the_board_reads_the_stamp_back() {
    let (repo, server) = served();
    let (status, head, body) = post(
        &server.addr,
        "/api/triage",
        r#"{"flight":"1","procedure":"review","message":"it is a review"}"#,
    );
    let chain = chain(repo.path());
    let batch = &chain[1..];
    assert_eq!(batch.len(), 9, "the routed head, three parts, five edges");
    let (routed, rest) = batch.split_first().expect("the routed head");
    let (parts, linked) = rest.split_at(3);
    ok(
        "/api/triage",
        status,
        &head,
        &body,
        machine::emit(
            "triage",
            &verb::Routed {
                routed: routed.clone(),
                parts: parts.to_vec(),
                linked: linked.to_vec(),
            },
        ),
    );

    // The stamp is on the board the next GET folds: the flight left the
    // open pile for `review`.
    let (status, _, board) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{board}");
    let board: serde_json::Value = serde_json::from_str(&board).expect("the board envelope");
    let flights = board["data"]["open"].as_array().expect("open");
    let parent = flights
        .iter()
        .find(|view| view["subject"] == json!("write the doctor verb"))
        .expect("the routed flight is still open — a route is not a claim");
    assert_eq!(parent["procedure"], json!("review"));
    read_after_write(&server, repo.path());
}

#[test]
fn the_guard_refusals_are_conflicts_with_the_clis_envelope() {
    let (repo, server) = served();

    // Nothing claimed yet: a requeue has nothing to hand back.
    error_parity(
        &server,
        repo.path(),
        "/api/requeue",
        r#"{"flight":"1"}"#,
        409,
        &["requeue", "1", "--json"],
        "requeue/unclaimed",
    );
    // And an answer has no question to release.
    error_parity(
        &server,
        repo.path(),
        "/api/answer",
        r#"{"flight":"1","message":"to what?"}"#,
        409,
        &["answer", "1", "-m", "to what?", "--json"],
        "answer/not-held",
    );

    let (status, _, _) = post(&server.addr, "/api/claim", r#"{"flight":"1"}"#);
    assert_eq!(status, 200);
    error_parity(
        &server,
        repo.path(),
        "/api/claim",
        r#"{"flight":"1"}"#,
        409,
        &["claim", "1", "--json"],
        "claim/taken",
    );

    let (status, _, _) = post(&server.addr, "/api/take", r#"{"flight":"1"}"#);
    assert_eq!(status, 200);
    error_parity(
        &server,
        repo.path(),
        "/api/take",
        r#"{"flight":"1"}"#,
        409,
        &["take", "1", "--json"],
        "take/taken",
    );

    let (status, _, _) = post(
        &server.addr,
        "/api/hold",
        r#"{"flight":"1","message":"which retry path?"}"#,
    );
    assert_eq!(status, 200);
    error_parity(
        &server,
        repo.path(),
        "/api/hold",
        r#"{"flight":"1","message":"another?"}"#,
        409,
        &["hold", "1", "-m", "another?", "--json"],
        "hold/exists",
    );

    let (status, _, _) = post(&server.addr, "/api/done", r#"{"flight":"1"}"#);
    assert_eq!(status, 200);
    error_parity(
        &server,
        repo.path(),
        "/api/done",
        r#"{"flight":"1"}"#,
        409,
        &["done", "1", "--json"],
        "flight/done",
    );
    error_parity(
        &server,
        repo.path(),
        "/api/claim",
        r#"{"flight":"1"}"#,
        409,
        &["claim", "1", "--json"],
        "flight/done",
    );
}

#[test]
fn the_usage_refusals_are_four_hundreds_with_the_clis_envelope() {
    let (repo, server) = served();
    // A missing message parses fine and refuses in core — the same
    // vocabulary the CLI's missing `-m` lands in.
    error_parity(
        &server,
        repo.path(),
        "/api/hold",
        r#"{"flight":"1"}"#,
        400,
        &["hold", "1", "--json"],
        "usage/needs-message",
    );
    error_parity(
        &server,
        repo.path(),
        "/api/file",
        r#"{"subject":"   "}"#,
        400,
        &["file", "   ", "--json"],
        "usage/empty-subject",
    );
    error_parity(
        &server,
        repo.path(),
        "/api/comment",
        r#"{"flight":"banana","message":"x"}"#,
        400,
        &["comment", "banana", "-m", "x", "--json"],
        "usage/bad-flight",
    );
}

#[test]
fn a_reference_naming_nothing_is_a_four_oh_four() {
    let (repo, server) = served();
    error_parity(
        &server,
        repo.path(),
        "/api/comment",
        r#"{"flight":"pi.99","message":"x"}"#,
        404,
        &["comment", "pi.99", "-m", "x", "--json"],
        "flight/not-found",
    );
    error_parity(
        &server,
        repo.path(),
        "/api/file",
        r#"{"subject":"a subject","procedure":"ghost"}"#,
        404,
        &["file", "a subject", "-p", "ghost", "--json"],
        "procedure/not-found",
    );
    error_parity(
        &server,
        repo.path(),
        "/api/triage",
        r#"{"flight":"1","procedure":"ghost"}"#,
        404,
        &["triage", "1", "-p", "ghost", "--json"],
        "procedure/not-found",
    );
}

/// A body the verb cannot read is `usage/bad-body` — the one id only the
/// server raises. Not JSON at all, a missing required field, and an
/// unknown field all land there; `done` with no flight is the missing
/// field, deliberately, because the server has no worktree to derive one
/// from.
#[test]
fn a_body_that_is_not_the_verbs_json_is_bad_body() {
    let (repo, server) = served();
    for (path, body) in [
        ("/api/claim", "not json"),
        ("/api/claim", r#"{}"#),
        ("/api/claim", r#"{"flight":"1","extra":true}"#),
        ("/api/done", r#"{}"#),
        ("/api/file", r#"{"flight":"1"}"#),
        // `procedure` is required here where `-p` is optional on the
        // CLI: a missing field is the body's refusal, not the verb's.
        ("/api/triage", r#"{"flight":"1"}"#),
        (
            "/api/triage",
            r#"{"flight":"1","procedure":"review","extra":true}"#,
        ),
    ] {
        let (status, _, answered) = post(&server.addr, path, body);
        assert_eq!(status, 400, "{path} {body}: {answered}");
        let envelope: serde_json::Value =
            serde_json::from_str(&answered).expect("an error envelope");
        assert_eq!(envelope["error"]["id"], json!("usage/bad-body"), "{body}");
        assert_eq!(
            envelope["error"]["exits"],
            json!(["ff tower explain usage/bad-body"]),
            "{body}"
        );
    }
    // Nothing was appended by any of them: the board still holds one
    // untouched flight.
    let out = ff_tower(repo.path(), &["--json"]);
    let board: serde_json::Value = serde_json::from_slice(&out.stdout).expect("the board envelope");
    assert_eq!(board["data"]["open"].as_array().expect("open").len(), 1);
}

#[test]
fn a_read_method_on_a_write_route_is_405() {
    let (_repo, server) = served();
    assert_eq!(request(&server.addr, "GET", "/api/claim").0, 405);
    assert_eq!(request(&server.addr, "GET", "/api/triage").0, 405);
}

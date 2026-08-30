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
            procedure: None,
            subject: subject.to_string(),
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
fn assign_appends_and_answers_the_verbs_envelope() {
    let (repo, server) = served();
    let (status, head, body) = post(
        &server.addr,
        "/api/assign",
        r#"{"flight":"1","assignee":"agent"}"#,
    );
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Assigned { .. }));
    ok(
        "/api/assign",
        status,
        &head,
        &body,
        machine::emit("assign", &verb::Assigned { assigned: event }),
    );
    read_after_write(&server, repo.path());
}

#[test]
fn status_moves_round_trip_over_http() {
    let (repo, server) = served();
    let (status, head, body) = post(
        &server.addr,
        "/api/status",
        r#"{"flight":"1","status":"ready"}"#,
    );
    let event = appended(repo.path(), |kind| matches!(kind, Kind::Status { .. }));
    ok(
        "/api/status",
        status,
        &head,
        &body,
        machine::emit("status", &verb::Moved { status: event }),
    );

    let (status, head, body) = post(
        &server.addr,
        "/api/status",
        r#"{"flight":"1","status":"in_progress"}"#,
    );
    let event = appended(
        repo.path(),
        |kind| matches!(kind, Kind::Status { status, .. } if status == "in_progress"),
    );
    ok(
        "/api/status",
        status,
        &head,
        &body,
        machine::emit("status", &verb::Moved { status: event }),
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
fn comment_done_and_cancel_land_on_the_record() {
    let (repo, server) = served();
    file(repo.path(), "a second flight");
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
    let event = appended(
        repo.path(),
        |kind| matches!(kind, Kind::Status { status, .. } if status == "done"),
    );
    ok(
        "/api/done",
        status,
        &head,
        &body,
        machine::emit("done", &verb::Moved { status: event }),
    );

    let (status, head, body) = post(
        &server.addr,
        "/api/cancel",
        r#"{"flight":"2","message":"superseded"}"#,
    );
    let event = appended(
        repo.path(),
        |kind| matches!(kind, Kind::Status { status, .. } if status == "canceled"),
    );
    ok(
        "/api/cancel",
        status,
        &head,
        &body,
        machine::emit("cancel", &verb::Moved { status: event }),
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
    // the parent, `review`'s three flights, then its five edges.
    let chain = chain(repo.path());
    let batch = &chain[1..];
    assert_eq!(batch.len(), 9, "the parent, three flights, five edges");
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

/// The field flags ride the body: a filing can carry every stored field,
/// and the board reads them back on the next GET.
#[test]
fn file_carries_the_field_flags_over_http() {
    let (repo, server) = served();
    let (status, _, _) = post(
        &server.addr,
        "/api/file",
        r#"{"subject":"laned work","priority":"high","labels":["chore"],"assignee":"agent","skill":"review"}"#,
    );
    assert_eq!(status, 200);
    let (status, _, board) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{board}");
    let board: serde_json::Value = serde_json::from_str(&board).expect("the board envelope");
    let flights = board["data"]["open"].as_array().expect("open");
    let filed = flights
        .iter()
        .find(|view| view["subject"] == json!("laned work"))
        .expect("the filing is on the board");
    assert_eq!(filed["status"], json!("triage"));
    assert_eq!(filed["priority"], json!("high"));
    assert_eq!(filed["labels"], json!(["chore"]));
    assert_eq!(filed["assignee"], json!("agent"));
    assert_eq!(filed["skill"], json!("review"));
    read_after_write(&server, repo.path());
}

#[test]
fn the_guard_refusals_are_conflicts_with_the_clis_envelope() {
    let (repo, server) = served();

    // No question standing: an answer has nothing to release.
    error_parity(
        &server,
        repo.path(),
        "/api/answer",
        r#"{"flight":"1","message":"to what?"}"#,
        409,
        &["answer", "1", "-m", "to what?", "--json"],
        "answer/not-held",
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
    // An open question blocks every move short of closing the flight.
    error_parity(
        &server,
        repo.path(),
        "/api/status",
        r#"{"flight":"1","status":"ready"}"#,
        409,
        &["status", "1", "ready", "--json"],
        "status/held",
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
        "/api/assign",
        r#"{"flight":"1","assignee":"agent"}"#,
        409,
        &["assign", "1", "agent", "--json"],
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
    // The closed vocabularies refuse at the verb, one wording on both
    // surfaces.
    error_parity(
        &server,
        repo.path(),
        "/api/status",
        r#"{"flight":"1","status":"claimed"}"#,
        400,
        &["status", "1", "claimed", "--json"],
        "usage/bad-status",
    );
    error_parity(
        &server,
        repo.path(),
        "/api/assign",
        r#"{"flight":"1","assignee":"you"}"#,
        400,
        &["assign", "1", "you", "--json"],
        "usage/bad-assignee",
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
        &["file", "ghost", "a subject", "--json"],
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
        ("/api/assign", "not json"),
        ("/api/assign", r#"{"flight":"1"}"#),
        ("/api/status", r#"{"flight":"1"}"#),
        (
            "/api/status",
            r#"{"flight":"1","status":"ready","extra":true}"#,
        ),
        ("/api/done", r#"{}"#),
        ("/api/file", r#"{"flight":"1"}"#),
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
    assert_eq!(request(&server.addr, "GET", "/api/assign").0, 405);
    assert_eq!(request(&server.addr, "GET", "/api/status").0, 405);
}

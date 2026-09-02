//! The view routes against the real binary.
//!
//! The verb suite's discipline without a CLI verb to run for parity:
//! every success answers 200 with the envelope `machine::emit` builds
//! around the folded view or the appended event, read back off the
//! chain so the store-assigned time cannot race the comparison, and
//! every refusal is the exact error envelope under the status the id
//! maps to. A view is not a row, so the board after a save is the board
//! before it, byte for byte, and still the CLI's `--json` — view events
//! are routed, never warned about.

use std::path::Path;

use ff_tower_core::log::{Event, Kind, Store};
use ff_tower_core::{board, machine, verb};
use ff_tower_testsupport::Repo;
use serde_json::json;

mod support;
use support::{Server, ff_tower, free_port, http, post};

fn served() -> (Repo, Server) {
    let repo = Repo::new();
    repo.pin_writer("pi");
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

fn chain(repo: &Path) -> Vec<Event> {
    Store::open(repo).expect("open").read().expect("read")
}

/// The view as the fold holds it — what the verb answers with.
fn folded(repo: &Path, id: &str) -> board::View {
    let events = Store::open(repo).expect("open").read_all().expect("read");
    board::fold(&events)
        .views
        .into_iter()
        .find(|view| view.id.to_string() == id)
        .expect("the view is on the chain")
}

fn ok(path: &str, status: u16, head: &str, body: &str, envelope: String) {
    assert_eq!(status, 200, "{path}: {body}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{path}: {head}"
    );
    assert_eq!(body, format!("{envelope}\n"), "{path}");
}

/// A refusal: the status the id maps to, and the exact error envelope.
fn refused(path: &str, got: (u16, String, String), status: u16, envelope: String) {
    let (code, head, body) = got;
    assert_eq!(code, status, "{path}: {body}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{path}: {head}"
    );
    assert_eq!(body, format!("{envelope}\n"), "{path}");
}

fn save(server: &Server, body: &str) -> (u16, String, String) {
    post(&server.addr, "/api/views/save", body)
}

#[test]
fn an_empty_list_answers_the_view_list_envelope() {
    let (_repo, server) = served();
    let (status, head, body) = http(&server.addr, "/api/views");
    ok(
        "/api/views",
        status,
        &head,
        &body,
        r#"{"tower":1,"cmd":"view list","data":{"views":[]}}"#.to_string(),
    );
}

#[test]
fn save_answers_the_view_and_the_log_holds_the_canonical_query() {
    let (repo, server) = served();
    let (status, head, body) = save(
        &server,
        r#"{"name":"mine","query":"?assignee=me&group=assignee&closed=3"}"#,
    );
    let view = folded(repo.path(), "pi.1");
    ok(
        "/api/views/save",
        status,
        &head,
        &body,
        machine::emit("view save", &verb::Saved { view: view.clone() }),
    );
    let envelope: serde_json::Value = serde_json::from_str(&body).expect("an envelope");
    assert_eq!(envelope["data"]["view"]["id"], json!("pi.1"));
    assert_eq!(envelope["data"]["view"]["name"], json!("mine"));
    assert_eq!(
        envelope["data"]["view"]["query"],
        json!("assignee=me&group=assignee"),
        "the `?` and the default window are not spelled"
    );
    assert_eq!(envelope["data"]["view"]["shared"], json!(false));
    assert_eq!(
        envelope["data"]["view"]["author"],
        json!("tests@tower.invalid")
    );

    let last = chain(repo.path()).pop().expect("the appended event");
    let Kind::ViewSaved {
        view: target,
        query,
        ..
    } = last.kind
    else {
        panic!("the chain ends in the save");
    };
    assert!(target.is_none(), "the mint names no view");
    assert_eq!(query, "assignee=me&group=assignee");

    let (status, head, body) = http(&server.addr, "/api/views");
    ok(
        "/api/views",
        status,
        &head,
        &body,
        machine::emit("view list", &verb::Views { views: vec![view] }),
    );
}

#[test]
fn edit_renames_and_keeps_the_query_and_delete_empties_the_list() {
    let (repo, server) = served();
    let (status, _, body) = save(&server, r#"{"name":"mine","query":"assignee=me"}"#);
    assert_eq!(status, 200, "{body}");

    let (status, head, body) = post(
        &server.addr,
        "/api/views/edit",
        r#"{"view":"pi.1","name":"renamed","shared":true}"#,
    );
    let view = folded(repo.path(), "pi.1");
    ok(
        "/api/views/edit",
        status,
        &head,
        &body,
        machine::emit("view edit", &verb::Saved { view: view.clone() }),
    );
    assert_eq!(view.name, "renamed");
    assert_eq!(view.query, "assignee=me", "the query left unsaid stands");
    assert!(view.shared);

    let (status, head, body) = post(&server.addr, "/api/views/delete", r#"{"view":"pi.1"}"#);
    let deleted = chain(repo.path()).pop().expect("the appended event");
    assert!(matches!(deleted.kind, Kind::ViewDeleted { .. }));
    ok(
        "/api/views/delete",
        status,
        &head,
        &body,
        machine::emit("view delete", &verb::Deleted { deleted }),
    );

    let (status, _, body) = http(&server.addr, "/api/views");
    assert_eq!(status, 200);
    assert_eq!(
        body,
        format!(
            "{}\n",
            r#"{"tower":1,"cmd":"view list","data":{"views":[]}}"#
        )
    );
}

#[test]
fn the_refusals_answer_their_ids_under_their_statuses() {
    let (repo, server) = served();

    let (status, _, body) = save(&server, r#"{"name":"mine"}"#);
    assert_eq!(status, 400, "{body}");
    let envelope: serde_json::Value = serde_json::from_str(&body).expect("an envelope");
    assert_eq!(envelope["cmd"], json!("view save"));
    assert_eq!(envelope["error"]["id"], json!("usage/bad-body"));

    refused(
        "/api/views/save",
        save(&server, r#"{"name":"  ","query":""}"#),
        400,
        machine::emit_error(
            "view save",
            "usage/empty-name",
            "the view name is empty",
            &["ff tower explain usage/empty-name".to_string()],
        ),
    );

    let unknown = board::QueryError::UnknownField {
        text: "bogus".to_string(),
    };
    refused(
        "/api/views/save",
        save(&server, r#"{"name":"mine","query":"bogus=1"}"#),
        400,
        machine::emit_error(
            "view save",
            "usage/unknown-field",
            &unknown.to_string(),
            &["ff tower".to_string()],
        ),
    );

    refused(
        "/api/views/edit",
        post(
            &server.addr,
            "/api/views/edit",
            r#"{"view":"pi.9","name":"x"}"#,
        ),
        404,
        machine::emit_error(
            "view edit",
            "view/not-found",
            "no view `pi.9` is visible to you",
            &["ff tower".to_string()],
        ),
    );

    refused(
        "/api/views/delete",
        post(&server.addr, "/api/views/delete", r#"{"view":"1"}"#),
        400,
        machine::emit_error(
            "view delete",
            "usage/bad-view",
            "`1` is not a view id — `<writer>.<seq>`",
            &["ff tower explain usage/bad-view".to_string()],
        ),
    );

    assert!(chain(repo.path()).is_empty(), "a refusal appends nothing");
}

#[test]
fn the_board_after_a_save_is_unchanged_and_still_the_clis() {
    let (repo, server) = served();
    file(repo.path(), "a flight to have a board about");
    let (status, _, before) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{before}");

    let (status, _, body) = save(&server, r#"{"name":"mine","query":"status=triage"}"#);
    assert_eq!(status, 200, "{body}");

    let (status, _, after) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{after}");
    assert_eq!(before, after, "a view is not a row");
    let envelope: serde_json::Value = serde_json::from_str(&after).expect("an envelope");
    assert_eq!(
        envelope["data"]["unrouted"],
        json!([]),
        "a view event is routed, never warned about"
    );

    let out = ff_tower(repo.path(), &["--json"]);
    assert!(out.status.success(), "the CLI board run failed");
    assert_eq!(after, String::from_utf8_lossy(&out.stdout));
}

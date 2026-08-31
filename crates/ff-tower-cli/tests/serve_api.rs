//! The read API against the real binary: byte parity with the CLI.
//!
//! The flight's own acceptance criterion, asserted literally: for every
//! route, success and refusal, the HTTP body equals the bytes the
//! matching verb writes to stdout under `--json` — same fold, same
//! serializer, same trailing newline. The CLI runs ride the same
//! `command` env as the server spawn, so the two answers come from one
//! fixture and can only differ if the surfaces themselves do.

use std::path::Path;
use std::process::Output;

use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::Repo;

mod support;
use support::{Server, command, free_port, http, refusal, request};

/// A repository with one filed flight on its log, and a server on it.
fn served() -> (Repo, Server) {
    let repo = repo_with_a_filing();
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);
    (repo, server)
}

fn repo_with_a_filing() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    file(repo.path(), "write the doctor verb");
    repo
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

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    command(repo).args(args).output().expect("spawn ff-tower")
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

/// The parity assertion: the route's body is the verb's stdout, byte for
/// byte, under the status and content type every success carries.
fn parity(server: &Server, repo: &Path, path: &str, cli: &[&str]) {
    let (status, head, body) = http(&server.addr, path);
    assert_eq!(status, 200, "{path}: {body}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{path}: {head}"
    );
    assert_eq!(body, stdout(&ff_tower(repo, cli)), "{path}");
}

/// The refusal half of the same assertion: the status differs by design
/// — the exit code's job on the CLI — and the envelope does not.
fn error_parity(server: &Server, repo: &Path, path: &str, status: u16, cli: &[&str], id: &str) {
    let (got, head, body) = http(&server.addr, path);
    assert_eq!(got, status, "{path}: {body}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{path}: {head}"
    );
    let out = ff_tower(repo, cli);
    refusal(&out, if id.starts_with("usage/") { 2 } else { 1 }, id);
    assert_eq!(body, String::from_utf8_lossy(&out.stdout), "{path}");
}

#[test]
fn the_board_route_is_the_board_verb_byte_for_byte() {
    let (repo, server) = served();
    parity(&server, repo.path(), "/api/board", &["--json"]);
}

#[test]
fn the_brief_route_answers_every_reference_form() {
    let (repo, server) = served();
    // The bare number, the percent-encoded `writer#n`, and the wire form
    // — the extractor decodes `%23` back to the `#` the grammar wants.
    for reference in ["1", "pi%231", "pi.1"] {
        parity(
            &server,
            repo.path(),
            &format!("/api/brief/{reference}"),
            &["brief", "1", "--json"],
        );
    }
}

#[test]
fn the_bays_route_is_the_bay_list_verb_byte_for_byte() {
    let (repo, server) = served();
    parity(&server, repo.path(), "/api/bays", &["bay", "--json"]);
}

#[test]
fn the_procedures_routes_are_the_verb_bare_and_named() {
    let (repo, server) = served();
    // The engine ships empty, so the named route needs a definition to
    // name: `docs/procedures/ticket.toml`'s shape, in the repository
    // layer the server reads through `Store::main_worktree`.
    repo.write(
        ".tower/procedures/ticket.toml",
        "name = \"ticket\"\n\n[[flight]]\nid       = \"work\"\nassignee = \"me\"\n",
    );
    parity(
        &server,
        repo.path(),
        "/api/procedures",
        &["procedures", "--json"],
    );
    parity(
        &server,
        repo.path(),
        "/api/procedures/ticket",
        &["procedures", "ticket", "--json"],
    );
}

#[test]
fn a_reference_that_does_not_parse_is_400_and_the_verbs_envelope() {
    let (repo, server) = served();
    error_parity(
        &server,
        repo.path(),
        "/api/brief/banana",
        400,
        &["brief", "banana", "--json"],
        "usage/bad-flight",
    );
}

#[test]
fn a_reference_naming_nothing_is_404_and_the_verbs_envelope() {
    let (repo, server) = served();
    error_parity(
        &server,
        repo.path(),
        "/api/brief/999",
        404,
        &["brief", "999", "--json"],
        "flight/not-found",
    );
}

#[test]
fn a_bare_number_two_writers_hold_is_404_ambiguous() {
    let repo = repo_with_a_filing();
    repo.pin_writer("qi");
    file(repo.path(), "from the qi");
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);
    error_parity(
        &server,
        repo.path(),
        "/api/brief/1",
        404,
        &["brief", "1", "--json"],
        "flight/ambiguous",
    );
}

#[test]
fn a_procedure_name_nothing_installed_answers_is_404() {
    let (repo, server) = served();
    error_parity(
        &server,
        repo.path(),
        "/api/procedures/nope",
        404,
        &["procedures", "nope", "--json"],
        "procedure/not-found",
    );
}

#[test]
fn an_unmounted_api_path_is_the_plain_fallback_not_an_envelope() {
    let (_repo, server) = served();
    // An envelope needs a `cmd`, and an unmounted path has no verb — so
    // the fallback stays plain text under `/api/` too.
    let (status, _, body) = http(&server.addr, "/api/nothing");
    assert_eq!(status, 404);
    assert_eq!(body, "nothing is mounted here yet\n");
}

#[test]
fn a_write_method_on_a_read_route_is_405() {
    let (_repo, server) = served();
    assert_eq!(request(&server.addr, "POST", "/api/board").0, 405);
}

#[test]
fn every_get_is_a_fresh_fold() {
    let (repo, server) = served();
    let (_, _, before) = http(&server.addr, "/api/board");
    assert!(before.contains("write the doctor verb"), "{before}");
    assert!(!before.contains("filed while serving"), "{before}");

    // A filing lands while the server runs; the next GET carries it with
    // no restart, and still equals a fresh CLI run.
    file(repo.path(), "filed while serving");
    let (_, _, after) = http(&server.addr, "/api/board");
    assert!(after.contains("filed while serving"), "{after}");
    assert_eq!(after, stdout(&ff_tower(repo.path(), &["--json"])));
}

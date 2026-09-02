//! The change feed against the real binary.
//!
//! The suite's spine is the never-short-circuit rule: every board
//! update rides the one stamp loop, whoever wrote. The first event
//! equals the pulled board byte for byte, an out-of-process write
//! reaches a stream that was already open, a POST verb's board arrives
//! through the watcher rather than from its handler, a subscriber with
//! a query gets its own fold, and the server outlives its watch child —
//! the filesystem lane alone still feeds.
//!
//! The debounce itself is not pinned here: end to end it races the
//! fold, so its honest test is the paused-clock unit test beside
//! `settle` in ff-tower-serve.

use std::path::Path;
use std::process::Output;

use ff_tower_core::log::Store;
use ff_tower_testsupport::{FakeFf, Repo};

mod support;
use support::{Server, Sse, command, free_port, http, matches_cli, post, sse};

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

/// File a flight the way every other writer would: the binary, in its
/// own process, so the server can only learn of it by watching.
fn file(repo: &Path, subject: &str) {
    stdout(&support::ff_tower(repo, &["file", subject, "--json"]));
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

/// Read events until one carries the subject. Intermediate boards are
/// legitimate — the repository moves for more reasons than the one
/// write under test, and every motion folds — so the assertion is that
/// the subject arrives, not that it arrives first. The socket's read
/// timeout bounds every step, and the count bounds the whole wait.
fn await_subject(feed: &mut Sse, subject: &str) -> String {
    for _ in 0..20 {
        let data = feed.next_data();
        if data.contains(subject) {
            return data;
        }
    }
    panic!("the feed never carried {subject:?}");
}

#[test]
fn the_feed_opens_with_the_current_board() {
    let (repo, server) = served();
    let mut feed = sse(&server.addr, "/api/feed");
    let event = feed.next_data();

    // The event plus the trailing newline is the pulled board byte for
    // byte — the newline is `println!` framing, and SSE frames itself —
    // and its groups are the CLI's rows. Folds are time-free, so
    // equality holds.
    let (status, _, body) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{body}");
    assert_eq!(format!("{event}\n"), body);
    matches_cli(
        &event,
        &stdout(&support::ff_tower(repo.path(), &["--json"])),
    );
}

#[test]
fn an_out_of_process_write_reaches_the_feed() {
    let (repo, server) = served();
    let mut feed = sse(&server.addr, "/api/feed");
    let first = feed.next_data();
    assert!(first.contains("write the doctor verb"), "{first}");

    // A separate process appends to the log; the server's only path to
    // knowing is the watcher.
    file(repo.path(), "filed while the stream was open");
    await_subject(&mut feed, "filed while the stream was open");
}

#[test]
fn a_post_verb_rides_the_same_loop() {
    let (_repo, server) = served();
    let mut feed = sse(&server.addr, "/api/feed");
    let _initial = feed.next_data();

    // The handler publishes nothing. If the feed updates, the write
    // reached it through the watcher — the same loop every writer's
    // does.
    let (status, _, body) = post(
        &server.addr,
        "/api/file",
        r#"{"subject": "filed over http"}"#,
    );
    assert_eq!(status, 200, "{body}");
    await_subject(&mut feed, "filed over http");
}

#[test]
fn a_status_move_posted_over_http_arrives_as_a_fresh_frame() {
    let (_repo, server) = served();
    let mut feed = sse(&server.addr, "/api/feed");
    let _initial = feed.next_data();

    // The move is a board change like any other, and the handler
    // publishes none of it: the frame carrying the In Progress row got
    // there through the watcher.
    let (status, _, body) = post(
        &server.addr,
        "/api/status",
        r#"{"flight":"1","status":"in_progress"}"#,
    );
    assert_eq!(status, 200, "{body}");
    await_subject(&mut feed, "\"status\":\"in_progress\"");
}

/// One flight's status, read out of a board envelope wherever its
/// group put it.
fn flight_status(envelope: &serde_json::Value, id: &str) -> Option<String> {
    let groups = envelope["data"]["groups"].as_array()?;
    for group in groups {
        for row in group["rows"].as_array().into_iter().flatten() {
            if row["id"] == serde_json::json!(id) {
                return row["status"].as_str().map(str::to_string);
            }
        }
    }
    None
}

#[test]
fn a_subscriber_with_a_query_gets_its_own_fold() {
    let (repo, server) = served();
    let mut high = sse(&server.addr, "/api/feed?priority=high");
    let mut plain = sse(&server.addr, "/api/feed");
    let _ = plain.next_data();
    let first = high.next_data();
    assert!(!first.contains("write the doctor verb"), "{first}");

    stdout(&support::ff_tower(
        repo.path(),
        &["file", "the urgent one", "--priority", "high", "--json"],
    ));
    file(repo.path(), "the ordinary one");

    // The plain feed carries both subjects.
    let both = await_subject(&mut plain, "the ordinary one");
    assert!(both.contains("the urgent one"), "{both}");

    // The filtered feed carries the high one, never the bare one, and
    // its filtered count reaches the fixture flight plus the bare one.
    for _ in 0..20 {
        let data = high.next_data();
        assert!(!data.contains("the ordinary one"), "{data}");
        let envelope: serde_json::Value = serde_json::from_str(&data).expect("an envelope");
        if envelope["data"]["filtered"] == serde_json::json!(2) {
            assert!(data.contains("the urgent one"), "{data}");
            return;
        }
    }
    panic!("the filtered feed never counted the bare flight out");
}

#[test]
fn a_feed_query_that_does_not_parse_is_400() {
    let (_repo, server) = served();
    let (status, head, body) = http(&server.addr, "/api/feed?bogus=1");
    assert_eq!(status, 400, "{body}");
    assert!(
        head.to_lowercase()
            .contains("content-type: application/json"),
        "{head}"
    );
    let envelope: serde_json::Value = serde_json::from_str(body.trim_end()).expect("an envelope");
    assert_eq!(envelope["cmd"], serde_json::json!("board"));
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("usage/unknown-field")
    );
}

#[test]
fn a_concluding_repo_serves_a_settled_board_and_the_loop_terminates() {
    // A rule the pass will fire: the hand-filed labeled flight below
    // sits in Triage when the server starts — `file`'s own preflight
    // pass runs before its append — so the server's first refold is what
    // routes it.
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(
        ".tower/procedures/chores.toml",
        "name = \"chores\"\n\
         [[match]]\n\
         name  = \"chore-label\"\n\
         label = \"chore\"\n\
         [[flight]]\n\
         id       = \"work\"\n\
         assignee = \"me\"\n",
    );
    stdout(&support::ff_tower(
        repo.path(),
        &["file", "sweep the logs", "--label", "chore", "--json"],
    ));
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);

    // The first pull is already settled: the pass ran ahead of the read.
    let (status, _, first) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{first}");
    let envelope: serde_json::Value = serde_json::from_str(first.trim_end()).expect("an envelope");
    assert_eq!(flight_status(&envelope, "pi.1").as_deref(), Some("ready"));
    assert!(first.contains(r#""procedure":"chores""#), "{first}");

    // The pass's own append re-trips the watcher, the next refold
    // concludes nothing, and the loop publishes identical bytes and
    // stops — so a second pull and the feed's frame are the same board.
    let (_, _, second) = http(&server.addr, "/api/board");
    assert_eq!(first, second, "a second fold concludes nothing");
    let mut feed = sse(&server.addr, "/api/feed");
    assert_eq!(format!("{}\n", feed.next_data()), second);
}

#[test]
fn done_on_the_last_dependency_releases_the_waiter_into_a_frame() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    Store::open(repo.path())
        .expect("open")
        .append(vec![
            filed_kind("the dep", "ready"),
            filed_kind("the waiter", "ready"),
            ff_tower_core::log::Kind::Linked {
                from: "pi.2".parse().expect("id"),
                to: "pi.1".parse().expect("id"),
            },
        ])
        .expect("append");
    let server = Server::start(repo.path(), &["--port", &free_port().to_string()], &[]);
    let mut feed = sse(&server.addr, "/api/feed");
    let _initial = feed.next_data();

    // The POST appends the finish; the watcher refolds, and that fold
    // derives the waiter Ready — no advance appended — so the frame
    // carrying it reaches the stream through the same loop as
    // everything else.
    let (status, _, body) = post(&server.addr, "/api/done", r#"{"flight":"pi.1"}"#);
    assert_eq!(status, 200, "{body}");
    for _ in 0..20 {
        let data = feed.next_data();
        let envelope: serde_json::Value = serde_json::from_str(&data).expect("an envelope");
        if flight_status(&envelope, "pi.2").as_deref() == Some("ready") {
            return;
        }
    }
    panic!("the feed never carried the released waiter");
}

fn filed_kind(subject: &str, status: &str) -> ff_tower_core::log::Kind {
    ff_tower_core::log::Kind::Filed {
        procedure: None,
        subject: subject.to_string(),
        body: String::new(),
        status: status.to_string(),
        assignee: None,
        priority: "none".to_string(),
        labels: Vec::new(),
        skill: None,
        bay: None,
        done: "asserted".to_string(),
        branch: None,
    }
}

#[test]
fn the_server_survives_its_watch_child_dying() {
    let repo = repo_with_a_filing();

    // An ff whose watch always dies at once, with every other verb
    // handed to the real one — the respawn loop spins harmlessly while
    // the folds keep working.
    let fake = FakeFf::script(
        "for arg in \"$@\"; do\n  if [ \"$arg\" = watch ]; then exit 1; fi\ndone\nexec ff \"$@\"\n",
    );
    let server = Server::start(
        repo.path(),
        &["--port", &free_port().to_string()],
        &[("TOWER_FF", &fake.path().to_string_lossy())],
    );

    let mut feed = sse(&server.addr, "/api/feed");
    let _initial = feed.next_data();

    // The pull side still answers...
    let (status, _, body) = http(&server.addr, "/api/board");
    assert_eq!(status, 200, "{body}");

    // ...and the filesystem lane alone still feeds: a tower append is a
    // ref write, and the watcher sees it with no watch child alive.
    let mut command = command(repo.path());
    command.env("TOWER_FF", fake.path());
    let output = command
        .args(["file", "the fs lane suffices", "--json"])
        .output()
        .expect("spawn ff-tower");
    stdout(&output);
    await_subject(&mut feed, "the fs lane suffices");
}

//! The lazy pass against a real store: what only a repository can prove.
//!
//! The routing rule itself is pinned in `verb/pass.rs`'s unit tests
//! over hand-built events; what runs here is the store seam — the quiet
//! pass touching neither writer nor lock, a concluding batch landing as
//! one commit, idempotence over real appends, and the release of a
//! waiter being the fold's work rather than the pass's.
//!
//! The registry is built through `procedure::layered` with the fixture's
//! own directories rather than `procedure::registry`: environment is
//! process-global, and the user layer must be the test's, not the
//! machine's.

use ff_tower_core::board;
use ff_tower_core::log::{Event, Kind, Store};
use ff_tower_core::procedure::{self, Registry};
use ff_tower_core::verb;
use ff_tower_testsupport::Repo;

/// The repository layer's registry alone — exactly what the production
/// pass loads, minus the machine's user layer.
fn registry(repo: &Repo) -> Registry {
    procedure::layered(None, Some(&procedure::repo_dir(repo.path()))).expect("the registry loads")
}

fn filed(subject: &str, status: &str, labels: &[&str]) -> Kind {
    Kind::Filed {
        procedure: None,
        subject: subject.to_string(),
        body: String::new(),
        status: status.to_string(),
        assignee: None,
        priority: "none".to_string(),
        labels: labels.iter().map(|label| label.to_string()).collect(),
        skill: None,
        bay: None,
        done: "asserted".to_string(),
        branch: None,
    }
}

fn moved(flight: &str, to: &str) -> Kind {
    Kind::Status {
        flight: flight.parse().expect("id"),
        status: to.to_string(),
        reason: None,
    }
}

fn linked(from: &str, to: &str) -> Kind {
    Kind::Linked {
        from: from.parse().expect("id"),
        to: to.parse().expect("id"),
    }
}

/// Commits on the pinned writer's chain — the count that proves a batch
/// landed atomically.
fn commits(repo: &Repo) -> usize {
    repo.git(&[
        "rev-list",
        "--count",
        "refs/tower/log/tests@tower.invalid/pi",
    ])
    .trim()
    .parse()
    .expect("a count")
}

const CHORES: &str = r#"
name = "chores"
[[match]]
name  = "chore-label"
label = "chore"
[[flight]]
id       = "work"
assignee = "me"
skill    = "tidy"
"#;

const REVIEWISH: &str = r#"
name = "reviewish"
[[match]]
name  = "review-label"
label = "review"
[[flight]]
id       = "pass"
assignee = "agent"
[[flight]]
id       = "verdict"
assignee = "me"
after    = ["pass"]
"#;

#[test]
fn a_quiet_pass_mints_no_writer_and_takes_no_lock() {
    // The `append_with` regression: it mints the writer and takes the
    // lock before its plan runs, so a pass that concludes nothing must
    // return before reaching it.
    let repo = Repo::new();
    let store = Store::open(repo.path()).expect("open");
    let appended = verb::pass(&store, &registry(&repo)).expect("pass");
    assert!(appended.is_empty());
    assert!(store.writer().is_none(), "no writer minted on this handle");

    let reopened = Store::open(repo.path()).expect("open");
    assert!(reopened.writer().is_none(), "no writer reached config");
    assert!(reopened.read_all().expect("read").is_empty());
}

#[test]
fn a_board_no_rule_covers_passes_without_appending() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    // Triage with no matching rule, ready, and an old log's hand-set
    // `waiting` word with no edges, which folds as cleared: nothing
    // concludes.
    store
        .append(vec![
            filed("unmatched", "triage", &["ops"]),
            filed("cleared", "ready", &[]),
            filed("parked", "waiting", &[]),
        ])
        .expect("append");
    let appended = verb::pass(&store, &registry(&repo)).expect("pass");
    assert!(appended.is_empty());
    assert_eq!(store.read_all().expect("read").len(), 3);
}

#[test]
fn a_single_flight_route_lands_in_one_commit_and_a_second_pass_appends_nothing() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/chores.toml", CHORES);
    let store = Store::open(repo.path()).expect("open");
    store
        .append(vec![filed("sweep the logs", "triage", &["chore"])])
        .expect("append");
    let before = commits(&repo);

    let installed = registry(&repo);
    let appended = verb::pass(&store, &installed).expect("pass");
    assert_eq!(appended.len(), 1, "one routed event");
    assert_eq!(commits(&repo), before + 1, "one commit");

    let events = store.read_all().expect("read");
    let fold = board::fold(&events);
    let flight = &fold.flights[0];
    assert_eq!(flight.status, "ready", "the collapse is born Ready");
    assert_eq!(flight.procedure.as_deref(), Some("chores"));
    assert_eq!(flight.assignee.as_deref(), Some("me"));
    assert_eq!(flight.skill.as_deref(), Some("tidy"));
    assert!(fold.unrouted.is_empty());

    let again = verb::pass(&store, &installed).expect("pass");
    assert!(again.is_empty(), "a second pass concludes nothing");
    assert_eq!(commits(&repo), before + 1);
}

#[test]
fn a_multi_flight_route_mints_the_family_in_one_commit() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/reviewish.toml", REVIEWISH);
    let store = Store::open(repo.path()).expect("open");
    store
        .append(vec![filed("feather", "triage", &["review"])])
        .expect("append");
    let before = commits(&repo);

    let appended = verb::pass(&store, &registry(&repo)).expect("pass");
    // One routed parent, two children, the parent's two edges, and the
    // after edge.
    assert_eq!(appended.len(), 6);
    assert_eq!(commits(&repo), before + 1, "the family lands atomically");

    let events = store.read_all().expect("read");
    let fold = board::fold(&events);
    let parent = &fold.flights[0];
    assert_eq!(parent.status, "waiting", "the parent waits on them all");
    assert_eq!(parent.procedure.as_deref(), Some("reviewish"));
    assert_eq!(parent.depends_on.len(), 2);
    let child = |tail: &str| {
        fold.flights
            .iter()
            .find(|flight| flight.subject.ends_with(tail))
            .expect("minted")
    };
    assert_eq!(child("· pass").status, "ready");
    assert_eq!(child("· pass").assignee.as_deref(), Some("agent"));
    assert_eq!(child("· verdict").status, "waiting");
    assert!(fold.unrouted.is_empty());

    // The routing event itself carries which rule fired.
    let routed = events
        .iter()
        .find(|event| matches!(event.kind, Kind::Routed { .. }))
        .expect("a routed event");
    let Kind::Routed { rule, because, .. } = &routed.kind else {
        unreachable!()
    };
    assert_eq!(rule, "review-label");
    assert_eq!(because, "matched label review");
}

#[test]
fn done_on_the_last_dependency_releases_the_waiter_with_no_event() {
    // The fold's work, not the pass's: the closing is the waiter's Ready
    // mark, and no history moment is minted for it.
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store
        .append(vec![
            filed("the dep", "ready", &[]),
            filed("the waiter", "ready", &[]),
            linked("pi.2", "pi.1"),
        ])
        .expect("append");
    assert_eq!(
        board::fold(&store.read_all().expect("read")).flights[1].status,
        "waiting",
        "the edge alone gates it"
    );
    store.append(vec![moved("pi.1", "done")]).expect("append");

    let appended = verb::pass(&store, &registry(&repo)).expect("pass");
    assert!(appended.is_empty(), "nothing to conclude");

    let events = store.read_all().expect("read");
    assert_eq!(events.len(), 4, "no advance on the record");
    let fold = board::fold(&events);
    let waiter = &fold.flights[1];
    assert_eq!(waiter.status, "ready");
    let mark = waiter.status_mark.as_ref().expect("released");
    assert_eq!(mark.by, "tests@tower.invalid", "the closer's byline");
    let closing: &Event = events.last().expect("the closing");
    assert!(matches!(&closing.kind, Kind::Status { status, .. } if status == "done"));
    assert_eq!(mark.at, closing.time, "the closing's time");
    assert_eq!(waiter.status_dep, Some("pi.1".parse().expect("id")));
}

#[test]
fn a_canceled_dependency_releases_its_waiter_too() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store
        .append(vec![
            filed("the dep", "ready", &[]),
            filed("the waiter", "ready", &[]),
            linked("pi.2", "pi.1"),
            moved("pi.1", "canceled"),
        ])
        .expect("append");

    let appended = verb::pass(&store, &registry(&repo)).expect("pass");
    assert!(appended.is_empty());
    let fold = board::fold(&store.read_all().expect("read"));
    assert_eq!(fold.flights[1].status, "ready", "closed is closed");
}

//! Intake matching against a real store: what only a repository can
//! prove.
//!
//! The matcher itself is pinned in `verb/file.rs`'s unit tests; what
//! runs here is the store seam — a routed filing landing as one commit
//! of one `filed` plus one `routed`, a multi-flight rule's family and
//! its routing in one commit, a filing no rule covers as one plain
//! `filed`, a named filing never re-matched, and a broken rule file
//! refusing the bare filing with the loader's own id — and, beside it,
//! the release of a waiter being the fold's work with nothing appended.
//!
//! `file` reads `procedure::registry`'s layers: the machine's user layer
//! under the fixture's repository layer. Every rule under test lives in
//! the repository layer, which wins whole by name, the same footing the
//! named-procedure unit tests already stand on.

use ff_tower_core::board;
use ff_tower_core::log::{Event, Kind, Store};
use ff_tower_core::verb::{self, Fields};
use ff_tower_testsupport::Repo;

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

fn labeled(label: &str) -> Fields {
    Fields {
        labels: vec![label.to_string()],
        ..Fields::default()
    }
}

/// Commits on the pinned writer's chain — the count that proves a batch
/// landed atomically. The ref must exist: read it after the first append.
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

const TICKET: &str = r#"
name = "ticket"
[[flight]]
id       = "work"
assignee = "me"
"#;

#[test]
fn a_routed_filing_lands_in_one_commit_as_one_filed_plus_one_routed() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/chores.toml", CHORES);
    let store = Store::open(repo.path()).expect("open");

    let outcome = verb::file(&store, "sweep the logs", labeled("chore"), None).expect("files");
    assert_eq!(commits(&repo), 1, "one commit on a fresh chain");
    assert!(outcome.payload.routed.is_some());

    let events = store.read_all().expect("read");
    let kinds: Vec<&str> = events.iter().map(|event| event.kind.name()).collect();
    assert_eq!(kinds, ["filed", "routed"]);
    let fold = board::fold(&events);
    let flight = &fold.flights[0];
    assert_eq!(flight.status, "ready", "the collapse is born Ready");
    assert_eq!(flight.procedure.as_deref(), Some("chores"));
    assert_eq!(flight.assignee.as_deref(), Some("me"));
    assert_eq!(flight.skill.as_deref(), Some("tidy"));
    assert!(fold.unrouted.is_empty());
}

#[test]
fn a_multi_flight_rule_mints_the_family_and_the_routing_in_one_commit() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/reviewish.toml", REVIEWISH);
    let store = Store::open(repo.path()).expect("open");

    let outcome = verb::file(&store, "feather", labeled("review"), None).expect("files");
    assert_eq!(outcome.part_ids.len(), 2);
    assert_eq!(commits(&repo), 1, "the family lands atomically");

    // The parent, two children, the parent's two edges, the after edge,
    // and the routing on the tail.
    let events = store.read_all().expect("read");
    assert_eq!(events.len(), 7);
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

    // The routing names the parent and carries which rule fired.
    let routed: &Event = events.last().expect("the routing");
    let Kind::Routed {
        flight,
        rule,
        because,
        ..
    } = &routed.kind
    else {
        panic!("the routing is the last event, got {:?}", routed.kind);
    };
    assert_eq!(flight, &outcome.parent);
    assert_eq!(rule, "review-label");
    assert_eq!(because, "matched label review");
}

#[test]
fn a_filing_no_rule_covers_is_one_plain_filed() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/chores.toml", CHORES);
    let store = Store::open(repo.path()).expect("open");

    let outcome = verb::file(&store, "unmatched", labeled("ops"), None).expect("files");
    assert!(outcome.payload.routed.is_none());
    let events = store.read_all().expect("read");
    assert_eq!(events.len(), 1);
    let fold = board::fold(&events);
    assert!(fold.flights[0].procedure.is_none());
    assert_eq!(fold.flights[0].status, "ready");
}

#[test]
fn a_named_procedure_is_never_re_matched() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/chores.toml", CHORES);
    repo.write(".tower/procedures/ticket.toml", TICKET);
    let store = Store::open(repo.path()).expect("open");

    let outcome = verb::file(&store, "typed", labeled("chore"), Some("ticket")).expect("files");
    assert!(outcome.payload.routed.is_none(), "the name was typed");
    let events = store.read_all().expect("read");
    assert_eq!(events.len(), 1, "one filing, no routing");
    let fold = board::fold(&events);
    assert_eq!(fold.flights[0].procedure.as_deref(), Some("ticket"));
    assert_eq!(fold.flights[0].labels, ["chore"]);
}

#[test]
fn a_broken_rule_file_refuses_the_bare_filing_with_the_loaders_id() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/broken.toml", "name = \"broken\"\n");
    let store = Store::open(repo.path()).expect("open");

    let err = verb::file(&store, "anything", Fields::default(), None)
        .err()
        .expect("the registry refuses");
    assert_eq!(err.id(), "procedure/no-parts");
    assert!(
        err.to_string().contains("broken.toml"),
        "named by path: {err}"
    );
    assert!(
        store.read_all().expect("read").is_empty(),
        "nothing written"
    );
}

#[test]
fn done_on_the_last_dependency_releases_the_waiter_with_no_event() {
    // The fold's work: the closing is the waiter's Ready mark, and no
    // history moment is minted for it.
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

    let fold = board::fold(&store.read_all().expect("read"));
    assert_eq!(fold.flights[1].status, "ready", "closed is closed");
}

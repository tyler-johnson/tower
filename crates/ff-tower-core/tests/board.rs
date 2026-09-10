//! The board's repository-facing half, against a real fufu.
//!
//! Status grouping lives in `board/model.rs`'s unit tests over
//! hand-built rows — the authoritative held/resolving coverage, cheaper
//! and more precise than coercing a real repository into a held state.
//! (A real-held-branch fixture is deliberately skipped for that reason.)
//! What runs here is what only a real repository can prove: the wrappers
//! parse what fufu actually emits, and the whole pipeline connects.

use ff_tower_core::board;
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::Repo;

/// The suite's board: the real clock, and the audit off — a stale line
/// is `model.rs`'s to prove, and a wall clock is not a fixture.
fn assemble(repo: &std::path::Path, events: &[ff_tower_core::log::Event]) -> board::Board {
    board::assemble(
        &Ff::at(repo),
        events,
        board::now(),
        0,
        board::ClosedWindow::default(),
    )
    .expect("assemble")
}

fn filed(subject: &str) -> Kind {
    Kind::Filed {
        procedure: None,
        subject: subject.to_string(),
        body: String::new(),
        status: "backlog".to_string(),
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
fn op_log_reads_a_tagged_capture_from_a_real_repository() {
    // The argv gate: `op log --json session(glob:*) -n 0` puts a bare
    // global flag before a positional, which nothing else exercises. This
    // lands before anything in board/ depends on the wrapper.
    let repo = Repo::new();
    repo.write("work.txt", "an agent was here\n");
    Ff::at(repo.path())
        .session("flight-1")
        .status()
        .expect("status");

    let ops = Ff::at(repo.path())
        .op_log("session(glob:*)")
        .expect("op log");

    assert_eq!(ops.len(), 1, "one tagged capture, got {ops:?}");
    assert_eq!(ops[0].session.as_deref(), Some("flight-1"));
    assert_eq!(ops[0].branch.as_deref(), Some("main"));
    assert!(ops[0].time > 0);
}

#[test]
fn branch_list_reads_a_real_repository() {
    let repo = Repo::new();
    let branches = Ff::at(repo.path()).branch_list().expect("branch list");

    let main = branches
        .named
        .iter()
        .find(|branch| branch.name == "main")
        .expect("main is listed");
    assert!(main.tip.is_some());
    assert!(!main.held && !main.resolving);
    assert!(branches.anonymous.is_empty());
}

#[test]
fn a_tagged_flight_carries_its_branch_and_an_untouched_one_carries_nothing() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    let ids = store
        .append(vec![filed("flown"), filed("untouched")])
        .expect("append");
    assert_eq!(ids[0].to_string(), "pi.1");

    // Standing in for an agent: dirty the tree and take a capture tagged
    // with the flight's id, which is exactly what a claimed flight does.
    repo.write("work.txt", "an agent was here\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");

    let events = store.read_all().expect("read_all");
    let board = assemble(repo.path(), &events);

    // Both were filed into Backlog and the capture does not move them:
    // the group is the stored field, and the branch is a fact beside it.
    assert_eq!(board.backlog.len(), 2);
    let flown = board.backlog.iter().find(|v| v.id == "pi.1").expect("pi.1");
    assert_eq!(flown.branch.as_deref(), Some("main"));
    assert!(flown.tip.is_some());
    assert!(flown.current, "the fixture's worktree sits on main");
    assert!(flown.last_change.is_some());

    let untouched = board.backlog.iter().find(|v| v.id == "pi.2").expect("pi.2");
    assert!(untouched.branch.is_none() && untouched.last_change.is_none());
    assert!(board.waiting_on_you.questions.is_empty());
    assert!(board.unrouted.is_empty());
}

#[test]
fn a_held_flight_assembles_into_waiting_on_you() {
    // Serde, fold, and enrich end to end: the lifecycle kinds go through a
    // real store and come back out as the waiting section.
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    let ids = store
        .append(vec![filed("stuck on a question")])
        .expect("append");
    let flight = ids[0].clone();
    store
        .append(vec![
            Kind::Status {
                flight: flight.clone(),
                status: "in_progress".to_string(),
                reason: None,
            },
            Kind::Held {
                flight,
                question: "which retry path?".to_string(),
            },
        ])
        .expect("append");

    let events = store.read_all().expect("read_all");
    let board = assemble(repo.path(), &events);

    assert_eq!(board.waiting_on_you.questions.len(), 1);
    let view = &board.waiting_on_you.questions[0];
    assert_eq!(view.id, "pi.1");
    assert_eq!(view.question.as_deref(), Some("which retry path?"));
    assert!(view.asked_at.is_some());
    assert_eq!(view.status, "held", "the hold is a status move too");
    assert_eq!(board.held.len(), 1, "the inbox is a view of the same row");
    assert!(board.in_progress.is_empty() && board.backlog.is_empty());
}

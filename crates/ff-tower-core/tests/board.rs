//! The board's repository-facing half, against a real fufu.
//!
//! Section classification lives in `board/model.rs`'s unit tests over
//! hand-built rows — the authoritative held/resolving coverage, cheaper
//! and more precise than coercing a real repository into a held state.
//! (A real-held-branch fixture is deliberately skipped for that reason.)
//! What runs here is what only a real repository can prove: the wrappers
//! parse what fufu actually emits, and the whole pipeline connects.

use ff_tower_core::board;
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::Repo;

fn filed(subject: &str) -> Kind {
    Kind::Filed {
        procedure: "open".to_string(),
        subject: subject.to_string(),
        body: String::new(),
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
fn a_tagged_flight_assembles_into_the_air_and_an_untouched_one_stays_open() {
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
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.in_the_air.len(), 1);
    let flown = &board.in_the_air[0];
    assert_eq!(flown.id, "pi.1");
    assert_eq!(flown.branch.as_deref(), Some("main"));
    assert!(flown.tip.is_some());
    assert!(flown.current, "the fixture's worktree sits on main");
    assert!(flown.last_motion.is_some());

    assert_eq!(board.open.len(), 1);
    assert_eq!(board.open[0].id, "pi.2");
    assert!(board.holding.is_empty());
    assert!(board.waiting_on_you.is_empty());
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
            Kind::Claimed {
                flight: flight.clone(),
            },
            Kind::Held {
                flight,
                question: "which retry path?".to_string(),
            },
        ])
        .expect("append");

    let events = store.read_all().expect("read_all");
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.waiting_on_you.len(), 1);
    let view = &board.waiting_on_you[0];
    assert_eq!(view.id, "pi.1");
    assert_eq!(view.question.as_deref(), Some("which retry path?"));
    assert!(view.asked_at.is_some());
    assert!(view.claimed_by.is_some());
    assert!(board.in_the_air.is_empty() && board.holding.is_empty() && board.open.is_empty());
}

#[test]
fn colliding_branches_carry_their_verdicts_through_the_board() {
    // The probe step end to end: two flights on branches that both edit
    // `shared.txt` carry mirrored `collides` entries, and an untouched
    // flight carries none.
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store
        .append(vec![
            filed("left work"),
            filed("right work"),
            filed("untouched"),
        ])
        .expect("append");

    repo.ff(&["start", "-b", "left"]);
    repo.write("shared.txt", "left side\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "left: touch shared"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("shared.txt", "right side\n");
    Ff::at(repo.path())
        .session("pi.2")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "right: touch shared"]);

    let events = store.read_all().expect("read_all");
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.in_the_air.len(), 2);
    let one = board
        .in_the_air
        .iter()
        .find(|v| v.id == "pi.1")
        .expect("pi.1");
    let two = board
        .in_the_air
        .iter()
        .find(|v| v.id == "pi.2")
        .expect("pi.2");
    assert_eq!(one.collides.len(), 1, "{one:?}");
    assert_eq!(one.collides[0].with, "pi.2");
    assert_eq!(one.collides[0].paths, ["shared.txt"]);
    assert_eq!(two.collides.len(), 1, "{two:?}");
    assert_eq!(two.collides[0].with, "pi.1");
    assert_eq!(two.collides[0].paths, ["shared.txt"]);
    assert!(one.unanswered.is_empty() && two.unanswered.is_empty());

    assert_eq!(board.open.len(), 1);
    let untouched = &board.open[0];
    assert_eq!(untouched.id, "pi.3");
    assert!(untouched.collides.is_empty() && untouched.unanswered.is_empty());
}

//! The board over a real store.
//!
//! Status grouping lives in `board/model.rs`'s unit tests over
//! hand-built rows. What runs here is what only a real repository can
//! prove: the lifecycle kinds round-trip through a store and the whole
//! pipeline connects.

use ff_tower_core::board;
use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::Repo;

/// The suite's board: the real clock, because a wall clock is not a
/// fixture and nothing here reads an age.
fn assemble(events: &[ff_tower_core::log::Event]) -> board::Board {
    board::assemble(events, board::now(), board::ClosedWindow::default())
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
    let board = assemble(&events);

    assert_eq!(board.waiting_on_you.questions.len(), 1);
    let view = &board.waiting_on_you.questions[0];
    assert_eq!(view.id, "pi.1");
    assert_eq!(view.question.as_deref(), Some("which retry path?"));
    assert!(view.asked_at.is_some());
    assert_eq!(view.status, "held", "the hold is a status move too");
    assert_eq!(board.held.len(), 1, "the inbox is a view of the same row");
    assert!(board.in_progress.is_empty() && board.backlog.is_empty());
}

//! The pick: `next`'s walk over the same reads as the board.
//!
//! Pure like `flight.rs` — no `crate::ff` spawns, no `std::process`; the
//! walk runs over a [`Fold`] and a [`Reads`] the caller already fetched,
//! so the pool is unit-testable with hand-built rows.
//!
//! The pool is every Ready flight in the agent lane: the derived status
//! and the stored assignee, read off the fold — never the registry
//! (principle 11). The gate is `Flight::pullable`, the one the brief
//! reads too: exact string compares, so an unknown status or lane never
//! rounds down into the pool. Ready is derived, so a pool candidate has
//! no live dependency by construction — a dependent sits in Waiting
//! until its last dependency closes, done or canceled, and never
//! reaches the walk. An open question or a fufu hold takes a flight out
//! on top of it. Ready flights *not* in the agent lane are counted in
//! `yours`, the count behind the `yours` outcome; the flights themselves
//! are silent here because the board is their surface, not this one's.
//!
//! Candidates walk in filed order and the first `want` of them are the
//! pick. Nothing deconflicts here: which branches can fly together is
//! not tower's question.

use serde::Serialize;

use super::flight::Fold;
use super::reads::Reads;

/// What the walk produced: the picked set, and the count of Ready work
/// the lane kept out of the pool.
#[derive(Debug, Serialize)]
pub struct Picks {
    pub picked: Vec<Pick>,
    /// Ready, unquestioned, not fufu-held — excluded from the pool by
    /// the lane alone. Work that exists and needs you.
    pub yours: usize,
}

/// Which of `next`'s three things happened. The word rides the envelope
/// and the CLI's exit code is its rendering — `work` 0, the other two 1,
/// fufu's "no" — so a server derives the same word from the same fold
/// (principle 9) and a harness that needs to know *why* an empty pick
/// was empty reads the field, not the status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// Something was picked.
    Work,
    /// Nothing picked and nothing Ready off the lane either: the board
    /// has nothing left.
    Drained,
    /// Nothing picked, but Ready work exists that the lane alone kept
    /// out of the pool. It needs you.
    Yours,
}

impl Picks {
    /// The outcome the walk arrived at: a pick is `Work` whatever
    /// `yours` says, and an empty pick is `Yours` or `Drained` by it.
    pub fn outcome(&self) -> Outcome {
        if !self.picked.is_empty() {
            Outcome::Work
        } else if self.yours > 0 {
            Outcome::Yours
        } else {
            Outcome::Drained
        }
    }
}

/// One picked flight, in wire form.
#[derive(Debug, Serialize)]
pub struct Pick {
    pub flight: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    pub subject: String,
}

/// Walk the candidates in filed order and pick the first `want` of them.
pub fn pick(fold: &Fold, reads: &Reads, want: usize) -> Picks {
    let freshest = reads.freshest();
    let index = reads.branch_index();

    // Per live flight: whether it is in the pool — Ready, agent lane, no
    // open question, and its branch row (non-`@detached`, from the
    // freshest op row) not held or resolving. A branch of `None`,
    // `@detached`, or a name absent from the index cannot be held, the
    // existing idiom.
    let mut picked: Vec<Pick> = Vec::new();
    let mut yours = 0;
    for flight in &fold.flights {
        if flight.closed() {
            continue;
        }
        let id = flight.id.to_string();
        let fufu_held = freshest
            .get(id.as_str())
            .and_then(|op| op.branch.as_deref())
            .filter(|name| *name != "@detached")
            .and_then(|name| index.get(name))
            .is_some_and(|row| row.held || row.resolving);
        let unheld = flight.question.is_none() && !fufu_held;
        if unheld && flight.pullable() {
            if picked.len() < want {
                picked.push(Pick {
                    flight: id,
                    number: flight.number,
                    subject: flight.subject.clone(),
                });
            }
        } else if unheld && flight.status == "ready" {
            yours += 1;
        }
    }

    Picks { picked, yours }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry};
    use crate::log::{Event, EventId, Kind};

    /// A filing with the given status and lane stored — the shape the
    /// pool gate is about.
    fn stored(id: &str, time: i64, status: &str, assignee: Option<&str>) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            session: None,
            id,
            kind: Kind::Filed {
                procedure: Some("review".to_string()),
                subject: format!("subject of {time}"),
                body: String::new(),
                status: status.to_string(),
                assignee: assignee.map(str::to_string),
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        }
    }

    /// The pool's norm: Ready, agent lane.
    fn filed(id: &str, time: i64) -> Event {
        stored(id, time, "ready", Some("agent"))
    }

    fn lifecycle(id: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            session: None,
            id,
            kind,
        }
    }

    fn moved(id: &str, time: i64, flight: &str, to: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
            },
        )
    }

    fn assigned(id: &str, time: i64, flight: &str, lane: Option<&str>) -> Event {
        lifecycle(
            id,
            time,
            Kind::Assigned {
                flight: flight.parse().expect("id"),
                assignee: lane.map(str::to_string),
            },
        )
    }

    fn held(id: &str, time: i64, flight: &str, question: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Held {
                flight: flight.parse().expect("id"),
                question: question.to_string(),
            },
        )
    }

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
            },
        )
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        moved(id, time, flight, "done")
    }

    fn op(session: &str, branch: Option<&str>, time: i64) -> OpEntry {
        OpEntry {
            branch: branch.map(str::to_string),
            session: Some(session.to_string()),
            time,
        }
    }

    fn branch(name: &str, held: bool, resolving: bool) -> BranchInfo {
        BranchInfo {
            name: name.to_string(),
            tip: Some("3c8f91686a9e35a10ae8ebb6f0d6f9bbbfdd6940".to_string()),
            held,
            resolving,
        }
    }

    fn reads(ops: Vec<OpEntry>, named: Vec<BranchInfo>) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named,
                anonymous: Vec::new(),
            },
            current_branch: None,
        }
    }

    #[test]
    fn an_unclosed_dependency_keeps_the_dependent_out_of_the_pool() {
        // The fold derives the dependent Waiting, so it is never a
        // candidate.
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ]),
            &reads(Vec::new(), Vec::new()),
            2,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.2");
        assert_eq!(picks.picked[0].number, 2);
        assert_eq!(picks.yours, 0, "waiting is not yours either");
    }

    #[test]
    fn a_done_dependency_admits_the_dependent() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
                done("pi.4", 40, "pi.2"),
            ]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn a_canceled_dependency_releases_its_dependent() {
        // Closed is closed: a canceled part still shows on the parent's
        // brief, and the dependent is Ready for whoever reconsiders it.
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
                moved("pi.4", 40, "pi.2", "canceled"),
            ]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn pulled_questioned_and_fufu_held_flights_are_not_picked() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                filed("pi.3", 30),
                moved("pi.4", 40, "pi.1", "in_progress"),
                held("pi.5", 50, "pi.2", "which?"),
            ]),
            &reads(
                vec![op("pi.3", Some("work"), 60)],
                vec![branch("work", true, false)],
            ),
            3,
        );
        assert!(picks.picked.is_empty());
    }

    #[test]
    fn branchless_candidates_all_admit_in_filed_order() {
        let picks = pick(
            &fold(&[filed("pi.2", 20), filed("pi.1", 10), filed("pi.3", 30)]),
            &reads(Vec::new(), Vec::new()),
            3,
        );
        let ids: Vec<&str> = picks.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(
            ids,
            ["pi.2", "pi.1", "pi.3"],
            "fold order, which is filed order"
        );
    }

    #[test]
    fn a_flying_flights_branchmate_still_admits() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                moved("pi.3", 30, "pi.1", "in_progress"),
            ]),
            &reads(
                vec![op("pi.1", Some("work"), 40), op("pi.2", Some("work"), 50)],
                vec![branch("work", false, false)],
            ),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.2");
    }

    #[test]
    fn the_walk_stops_at_want() {
        let picks = pick(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn only_ready_agent_flights_enter_the_pool_and_ready_rest_are_yours() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", None),
                stored("pi.2", 20, "ready", Some("me")),
                stored("pi.3", 30, "ready", Some("pair")),
                stored("pi.4", 40, "ready", Some("agent")),
                stored("pi.5", 50, "backlog", Some("agent")),
                stored("pi.6", 60, "ready", Some("agent")),
                linked("pi.7", 70, "pi.6", "pi.5"),
            ]),
            &reads(Vec::new(), Vec::new()),
            6,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.4");
        assert_eq!(
            picks.yours, 3,
            "Ready off the agent lane counts; Backlog and Waiting do not"
        );
    }

    #[test]
    fn the_outcome_is_work_then_yours_then_drained() {
        let work = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                stored("pi.2", 20, "ready", Some("agent")),
            ]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert_eq!(work.picked.len(), 1);
        assert_eq!(work.yours, 1);
        assert_eq!(
            work.outcome(),
            Outcome::Work,
            "a pick is work whatever `yours` counts"
        );

        let yours = pick(
            &fold(&[stored("pi.1", 10, "ready", Some("me"))]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert!(yours.picked.is_empty());
        assert_eq!(yours.yours, 1);
        assert_eq!(yours.outcome(), Outcome::Yours);

        let drained = pick(
            &fold(&[stored("pi.1", 10, "backlog", Some("agent"))]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert!(drained.picked.is_empty());
        assert_eq!(drained.yours, 0);
        assert_eq!(drained.outcome(), Outcome::Drained);

        assert_eq!(
            serde_json::to_string(&Outcome::Yours).expect("serializes"),
            "\"yours\"",
            "the wire word is lowercase"
        );
    }

    #[test]
    fn an_unknown_status_or_lane_never_rounds_into_the_pool() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "parked", Some("agent")),
                stored("pi.2", 20, "ready", Some("pair")),
            ]),
            &reads(Vec::new(), Vec::new()),
            2,
        );
        assert!(picks.picked.is_empty());
        assert_eq!(picks.yours, 1, "the unknown lane's Ready flight is yours");
    }

    #[test]
    fn questioned_and_pulled_flights_are_not_yours() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                stored("pi.2", 20, "ready", None),
                moved("pi.3", 30, "pi.1", "in_progress"),
                held("pi.4", 40, "pi.2", "which?"),
            ]),
            &reads(Vec::new(), Vec::new()),
            2,
        );
        assert!(picks.picked.is_empty());
        assert_eq!(picks.yours, 0, "a pull or a question already has an owner");
    }

    #[test]
    fn a_reassignment_moves_a_flight_across_the_gate() {
        // Both directions: the agent lane opened by hand, and closed by
        // hand — the stored field is the whole gate.
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                assigned("pi.2", 20, "pi.1", Some("agent")),
                filed("pi.3", 30),
                assigned("pi.4", 40, "pi.3", Some("me")),
            ]),
            &reads(Vec::new(), Vec::new()),
            2,
        );
        let ids: Vec<&str> = picks.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(ids, ["pi.1"]);
        assert_eq!(picks.yours, 1);
    }

    #[test]
    fn a_release_back_to_ready_rejoins_the_pool() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                moved("pi.2", 20, "pi.1", "in_progress"),
                moved("pi.3", 30, "pi.1", "ready"),
            ]),
            &reads(Vec::new(), Vec::new()),
            1,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn an_empty_fold_picks_nothing() {
        let picks = pick(&fold(&[]), &reads(Vec::new(), Vec::new()), 1);
        assert!(picks.picked.is_empty());
    }
}

//! The pick: `next`'s walk over the same fold as the board.
//!
//! Pure like `flight.rs` — no `crate::ff` spawns, no `std::process`; the
//! walk runs over a [`Fold`], so the pool is unit-testable with
//! hand-built rows.
//!
//! The pool is every Ready flight in the agent lane, plus every Ready
//! flight laned to the caller's own callsign: the derived status and
//! the stored assignee, read off the fold — never the registry
//! (principle 11). The gate is `Flight::pullable`, the one the brief
//! reads too: exact string compares, so an unknown status or lane never
//! rounds down into the pool, and a caller with no callsign pulls the
//! agent lane alone. Ready is derived, so a pool candidate has
//! no live dependency by construction — a dependent sits in Waiting
//! until its last dependency closes, done or canceled, and never
//! reaches the walk. An open question takes a flight out on top of it.
//! Ready flights *not* in the pool are counted in `yours`, the count
//! behind the `yours` outcome; the flights themselves are silent here
//! because the board is their surface, not this one's.
//!
//! Candidates walk in filed order and the first `want` of them are the
//! pick. Nothing deconflicts here: which branches can fly together is
//! not tower's question.

use serde::Serialize;

use super::flight::Fold;

/// What the walk produced: the picked set, and the count of Ready work
/// the lane kept out of the pool.
#[derive(Debug, Serialize)]
pub struct Picks {
    pub picked: Vec<Pick>,
    /// Ready and unquestioned — excluded from the pool by the lane
    /// alone. Work that exists and needs you.
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
/// `caller` is the puller's callsign: its own queue and the pool walk
/// together, because the walk is filed order and the gate is one
/// predicate.
pub fn pick(fold: &Fold, want: usize, caller: Option<&str>) -> Picks {
    // Per live flight: whether it is in the pool — Ready, agent lane or
    // the caller's own, no open question.
    let mut picked: Vec<Pick> = Vec::new();
    let mut yours = 0;
    for flight in &fold.flights {
        if flight.closed() {
            continue;
        }
        let id = flight.id.to_string();
        let unheld = flight.question.is_none();
        if unheld && flight.pullable(caller) {
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
            callsign: None,
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
            callsign: None,
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
            2,
            None,
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
            1,
            None,
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
            1,
            None,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn pulled_and_questioned_flights_are_not_picked() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                moved("pi.3", 30, "pi.1", "in_progress"),
                held("pi.4", 40, "pi.2", "which?"),
            ]),
            2,
            None,
        );
        assert!(picks.picked.is_empty());
    }

    #[test]
    fn branchless_candidates_all_admit_in_filed_order() {
        let picks = pick(
            &fold(&[filed("pi.2", 20), filed("pi.1", 10), filed("pi.3", 30)]),
            3,
            None,
        );
        let ids: Vec<&str> = picks.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(
            ids,
            ["pi.2", "pi.1", "pi.3"],
            "fold order, which is filed order"
        );
    }

    #[test]
    fn the_walk_stops_at_want() {
        let picks = pick(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            1,
            None,
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
            6,
            None,
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
            1,
            None,
        );
        assert_eq!(work.picked.len(), 1);
        assert_eq!(work.yours, 1);
        assert_eq!(
            work.outcome(),
            Outcome::Work,
            "a pick is work whatever `yours` counts"
        );

        let yours = pick(&fold(&[stored("pi.1", 10, "ready", Some("me"))]), 1, None);
        assert!(yours.picked.is_empty());
        assert_eq!(yours.yours, 1);
        assert_eq!(yours.outcome(), Outcome::Yours);

        let drained = pick(
            &fold(&[stored("pi.1", 10, "backlog", Some("agent"))]),
            1,
            None,
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
            2,
            None,
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
            2,
            None,
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
            2,
            None,
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
            1,
            None,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn an_empty_fold_picks_nothing() {
        let picks = pick(&fold(&[]), 1, None);
        assert!(picks.picked.is_empty());
    }

    #[test]
    fn a_callers_own_queue_walks_with_the_pool_in_filed_order() {
        // The brief's verify: `assign 5 qwen-review`, then the pull under
        // that callsign picks it and a pull under another does not. Own
        // queue and pool interleave by filed order, not queue first.
        let events = [
            stored("pi.1", 10, "ready", Some("qwen-review")),
            filed("pi.2", 20),
            stored("pi.3", 30, "ready", Some("claude")),
            stored("pi.4", 40, "ready", Some("qwen-review")),
        ];
        let qwen = pick(&fold(&events), 4, Some("qwen-review"));
        let ids: Vec<&str> = qwen.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(ids, ["pi.1", "pi.2", "pi.4"]);
        assert_eq!(qwen.yours, 1, "claude's queue is yours to qwen");

        let claude = pick(&fold(&events), 4, Some("claude"));
        let ids: Vec<&str> = claude.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(ids, ["pi.2", "pi.3"]);
        assert_eq!(claude.yours, 2);

        let nobody = pick(&fold(&events), 4, None);
        let ids: Vec<&str> = nobody.picked.iter().map(|p| p.flight.as_str()).collect();
        assert_eq!(ids, ["pi.2"], "no callsign pulls the agent lane alone");
        assert_eq!(nobody.yours, 3);
    }
}

//! The pick: `next`'s walk over the same fold as the board.
//!
//! Pure like `flight.rs` — no `crate::ff` spawns, no `std::process`; the
//! walk runs over a [`Fold`], so the lane walk is unit-testable with
//! hand-built rows.
//!
//! The walk is a list of lanes. Each named [`Lane`] — the caller's own
//! queue, the literal `agent` lane, the unassigned lane, or one pilot's
//! callsign — walks in the order given, each in filed order, and the
//! unassigned lane walks last unless it was named, in which case it
//! walks where it was named: everyone falls through to `none` once
//! their own lanes are drained, and `none` walks once wherever it
//! lands. [`walk`] builds that list from the words; an empty list is
//! `me` alone. A flight in two named lanes — the literal `me` and the
//! caller's callsign, say — is picked once, in the first lane that
//! admits it. Membership is the derived status and the stored
//! assignee, read off the fold — never the registry (principle 11) —
//! through `Flight::in_lane`: exact string compares, so an unknown
//! status or lane never rounds into a walk. Ready is derived, so a
//! candidate has no live dependency by construction — a dependent sits
//! in Waiting until its last dependency closes, done or canceled, and
//! never reaches the walk. An open question takes a flight out on top
//! of it. Ready flights outside the walk — other lanes — are counted in
//! `elsewhere`, the count behind the `elsewhere` outcome; the flights
//! themselves are silent here because the board is their surface, not
//! this one's.
//!
//! The first `want` of the walk are the pick. Nothing deconflicts here:
//! which branches can fly together is not tower's question.

use std::fmt;

use serde::Serialize;

use super::flight::Fold;

/// The lane a walk names: the same four shapes `assign` and `file`
/// take, read against the caller at the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lane {
    /// The caller's own queue: the literal `me` lane and the caller's
    /// callsign. A caller with no callsign walks the literal alone.
    Me,
    /// The literal `agent` lane alone — the shared pool.
    Agent,
    /// The unassigned lane — everyone's overflow, or the walk itself
    /// when named outright.
    None,
    /// One pilot's own queue.
    Callsign(String),
}

impl Lane {
    /// The lane `verb::lane_word` validated: `None` is the absent lane
    /// `none` spelled out, and the words map to their variants.
    pub fn from_word(word: Option<String>) -> Lane {
        match word.as_deref() {
            None => Lane::None,
            Some("me") => Lane::Me,
            Some("agent") => Lane::Agent,
            Some(callsign) => Lane::Callsign(callsign.to_string()),
        }
    }
}

/// The word back, for the envelope: `none` for the unassigned lane.
impl fmt::Display for Lane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Lane::Me => f.write_str("me"),
            Lane::Agent => f.write_str("agent"),
            Lane::None => f.write_str("none"),
            Lane::Callsign(callsign) => f.write_str(callsign),
        }
    }
}

/// What the walk produced: the picked set, and the count of Ready work
/// in lanes the walk never entered.
#[derive(Debug, Serialize)]
pub struct Picks {
    pub picked: Vec<Pick>,
    /// Ready and unquestioned — outside the walk by the lane alone.
    /// Work that exists, in another lane.
    pub elsewhere: usize,
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
    /// Nothing picked and nothing Ready in any other lane either: the
    /// board has nothing left.
    Drained,
    /// Nothing picked, but Ready work exists in a lane the walk never
    /// entered.
    Elsewhere,
}

impl Picks {
    /// The outcome the walk arrived at: a pick is `Work` whatever
    /// `elsewhere` says, and an empty pick is `Elsewhere` or `Drained`
    /// by it.
    pub fn outcome(&self) -> Outcome {
        if !self.picked.is_empty() {
            Outcome::Work
        } else if self.elsewhere > 0 {
            Outcome::Elsewhere
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
    /// The lane the flight was in when picked, as the log stores it —
    /// `None` for the unassigned lane — so a caller deciding a re-lane
    /// needs no second look at the fold.
    pub assignee: Option<String>,
}

/// The walk for the lanes named: each once, in the order given, with
/// the unassigned lane appended unless it was named. No lane named is
/// the caller's own queue, then the overflow.
pub fn walk(named: &[Lane]) -> Vec<Lane> {
    let mut lanes: Vec<Lane> = Vec::with_capacity(named.len() + 1);
    if named.is_empty() {
        lanes.push(Lane::Me);
    }
    for lane in named {
        if !lanes.contains(lane) {
            lanes.push(lane.clone());
        }
    }
    if !lanes.contains(&Lane::None) {
        lanes.push(Lane::None);
    }
    lanes
}

/// Walk `lanes` in order, each in filed order, and pick the first `want`
/// of them. The list is walked exactly as given — [`walk`] is where the
/// overflow is appended. `caller` is the puller's callsign, which only
/// `Lane::Me` reads.
pub fn pick(fold: &Fold, want: usize, lanes: &[Lane], caller: Option<&str>) -> Picks {
    // A candidate: live, unheld, Ready. The first lane admitting it
    // decides which pass it joins, and a candidate in none is elsewhere.
    let candidates = fold
        .flights
        .iter()
        .filter(|flight| !flight.closed() && flight.question.is_none() && flight.status == "ready");
    let mut buckets: Vec<Vec<Pick>> = lanes.iter().map(|_| Vec::new()).collect();
    let mut elsewhere = 0;
    for flight in candidates {
        match lanes.iter().position(|lane| flight.in_lane(lane, caller)) {
            Some(index) => buckets[index].push(Pick {
                flight: flight.id.to_string(),
                number: flight.number,
                subject: flight.subject.clone(),
                assignee: flight.assignee.clone(),
            }),
            None => elsewhere += 1,
        }
    }
    let mut picked: Vec<Pick> = buckets.into_iter().flatten().collect();
    picked.truncate(want);

    Picks { picked, elsewhere }
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

    /// The picked ids, in walk order.
    fn ids(picks: &Picks) -> Vec<&str> {
        picks.picked.iter().map(|p| p.flight.as_str()).collect()
    }

    #[test]
    fn an_unclosed_dependency_keeps_the_dependent_out_of_the_walk() {
        // The fold derives the dependent Waiting, so it is never a
        // candidate.
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ]),
            2,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.2");
        assert_eq!(picks.picked[0].number, 2);
        assert_eq!(picks.elsewhere, 0, "waiting is not elsewhere either");
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
            &[Lane::Agent, Lane::None],
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
            &[Lane::Agent, Lane::None],
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
            &[Lane::Agent, Lane::None],
            None,
        );
        assert!(picks.picked.is_empty());
    }

    #[test]
    fn branchless_candidates_all_admit_in_filed_order() {
        let picks = pick(
            &fold(&[filed("pi.2", 20), filed("pi.1", 10), filed("pi.3", 30)]),
            3,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(
            ids(&picks),
            ["pi.2", "pi.1", "pi.3"],
            "fold order, which is filed order"
        );
    }

    #[test]
    fn the_walk_stops_at_want() {
        let picks = pick(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            1,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(picks.picked.len(), 1);
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn only_ready_flights_in_the_walk_are_picked_and_ready_rest_are_elsewhere() {
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
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(
            ids(&picks),
            ["pi.4", "pi.1"],
            "the agent lane, then the unassigned overflow"
        );
        assert_eq!(
            picks.elsewhere, 2,
            "Ready in other lanes counts; Backlog and Waiting do not"
        );
    }

    #[test]
    fn the_outcome_is_work_then_elsewhere_then_drained() {
        let work = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                stored("pi.2", 20, "ready", Some("agent")),
            ]),
            1,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(work.picked.len(), 1);
        assert_eq!(work.elsewhere, 1);
        assert_eq!(
            work.outcome(),
            Outcome::Work,
            "a pick is work whatever `elsewhere` counts"
        );

        let elsewhere = pick(
            &fold(&[stored("pi.1", 10, "ready", Some("me"))]),
            1,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert!(elsewhere.picked.is_empty());
        assert_eq!(elsewhere.elsewhere, 1);
        assert_eq!(elsewhere.outcome(), Outcome::Elsewhere);

        let drained = pick(
            &fold(&[stored("pi.1", 10, "backlog", Some("agent"))]),
            1,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert!(drained.picked.is_empty());
        assert_eq!(drained.elsewhere, 0);
        assert_eq!(drained.outcome(), Outcome::Drained);

        assert_eq!(
            serde_json::to_string(&Outcome::Elsewhere).expect("serializes"),
            "\"elsewhere\"",
            "the wire word is lowercase"
        );
    }

    #[test]
    fn an_unknown_status_or_lane_never_rounds_into_the_walk() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "parked", Some("agent")),
                stored("pi.2", 20, "ready", Some("pair")),
            ]),
            2,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert!(picks.picked.is_empty());
        assert_eq!(
            picks.elsewhere, 1,
            "the unknown lane's Ready flight is elsewhere"
        );
    }

    #[test]
    fn questioned_and_pulled_flights_are_not_elsewhere() {
        let picks = pick(
            &fold(&[
                stored("pi.1", 10, "ready", Some("me")),
                stored("pi.2", 20, "ready", Some("pair")),
                moved("pi.3", 30, "pi.1", "in_progress"),
                held("pi.4", 40, "pi.2", "which?"),
            ]),
            2,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert!(picks.picked.is_empty());
        assert_eq!(
            picks.elsewhere, 0,
            "a pull or a question already has an owner"
        );
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
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(ids(&picks), ["pi.1"]);
        assert_eq!(picks.elsewhere, 1);
    }

    #[test]
    fn a_release_back_to_ready_rejoins_the_walk() {
        let picks = pick(
            &fold(&[
                filed("pi.1", 10),
                moved("pi.2", 20, "pi.1", "in_progress"),
                moved("pi.3", 30, "pi.1", "ready"),
            ]),
            1,
            &[Lane::Agent, Lane::None],
            None,
        );
        assert_eq!(picks.picked[0].flight, "pi.1");
    }

    #[test]
    fn an_empty_fold_picks_nothing() {
        let picks = pick(&fold(&[]), 1, &walk(&[]), None);
        assert!(picks.picked.is_empty());
        assert_eq!(picks.outcome(), Outcome::Drained);
    }

    #[test]
    fn the_lanes_walk_in_order_and_none_is_the_overflow() {
        // Each lane in filed order, lanes in the order given, then the
        // unassigned in filed order — never interleaved, whatever the
        // filing times say.
        let events = [
            stored("pi.1", 10, "ready", None),
            stored("pi.2", 20, "ready", Some("qwen-review")),
            stored("pi.3", 30, "ready", None),
            stored("pi.4", 40, "ready", Some("qwen-review")),
            stored("pi.5", 50, "ready", Some("claude")),
            stored("pi.6", 60, "ready", Some("agent")),
        ];
        let qwen = Lane::Callsign("qwen-review".to_string());
        let one = pick(&fold(&events), 6, &walk(std::slice::from_ref(&qwen)), None);
        assert_eq!(ids(&one), ["pi.2", "pi.4", "pi.1", "pi.3"]);
        assert_eq!(
            one.elsewhere, 2,
            "claude's queue and the pool are elsewhere"
        );
        assert_eq!(
            one.picked[0].assignee.as_deref(),
            Some("qwen-review"),
            "the pick carries the lane it was found in"
        );
        assert_eq!(one.picked[2].assignee, None);

        let two = pick(&fold(&events), 6, &walk(&[qwen.clone(), Lane::Agent]), None);
        assert_eq!(ids(&two), ["pi.2", "pi.4", "pi.6", "pi.1", "pi.3"]);
        assert_eq!(two.elsewhere, 1);

        let reversed = pick(&fold(&events), 6, &walk(&[Lane::Agent, qwen.clone()]), None);
        assert_eq!(ids(&reversed), ["pi.6", "pi.2", "pi.4", "pi.1", "pi.3"]);

        // `want` counts across the walk: the first lane fills first.
        let three = pick(&fold(&events), 3, &walk(&[qwen]), None);
        assert_eq!(ids(&three), ["pi.2", "pi.4", "pi.1"]);
    }

    #[test]
    fn walk_dedupes_and_appends_none_once() {
        assert_eq!(walk(&[Lane::Agent]), [Lane::Agent, Lane::None]);
        assert_eq!(
            walk(&[Lane::Me, Lane::Me, Lane::Agent, Lane::Me]),
            [Lane::Me, Lane::Agent, Lane::None],
            "a lane named twice walks once, where first named"
        );
        assert_eq!(
            walk(&[Lane::None, Lane::Agent, Lane::None]),
            [Lane::None, Lane::Agent],
            "`none` named walks where named and is not appended again"
        );
        assert_eq!(walk(&[Lane::None]), [Lane::None]);
    }

    #[test]
    fn an_empty_walk_defaults_to_me() {
        assert_eq!(walk(&[]), [Lane::Me, Lane::None]);
    }

    #[test]
    fn none_named_in_the_middle_walks_there() {
        let events = [
            stored("pi.1", 10, "ready", None),
            stored("pi.2", 20, "ready", Some("agent")),
            stored("pi.3", 30, "ready", Some("claude")),
            stored("pi.4", 40, "ready", None),
        ];
        let picks = pick(
            &fold(&events),
            4,
            &walk(&[Lane::Agent, Lane::None, Lane::Me]),
            Some("claude"),
        );
        assert_eq!(ids(&picks), ["pi.2", "pi.1", "pi.4", "pi.3"]);
        assert_eq!(picks.elsewhere, 0);
    }

    #[test]
    fn a_flight_in_two_named_lanes_is_picked_once_in_the_first() {
        // `me` under the caller's callsign and the callsign named
        // outright admit the same flight; it joins the first bucket only.
        let events = [
            stored("pi.1", 10, "ready", Some("qwen-review")),
            stored("pi.2", 20, "ready", Some("me")),
            stored("pi.3", 30, "ready", None),
        ];
        let qwen = Lane::Callsign("qwen-review".to_string());
        let me_first = pick(
            &fold(&events),
            5,
            &walk(&[Lane::Me, qwen.clone()]),
            Some("qwen-review"),
        );
        assert_eq!(ids(&me_first), ["pi.1", "pi.2", "pi.3"]);
        assert_eq!(me_first.elsewhere, 0);

        let callsign_first = pick(
            &fold(&events),
            5,
            &walk(&[qwen, Lane::Me]),
            Some("qwen-review"),
        );
        assert_eq!(
            ids(&callsign_first),
            ["pi.1", "pi.2", "pi.3"],
            "the callsign bucket takes pi.1, the `me` bucket pi.2, the overflow pi.3"
        );
    }

    #[test]
    fn me_is_the_literal_and_the_callsign() {
        // The brief's verify: `assign 5 qwen-review`, then the pull under
        // that callsign picks it and a pull under another does not.
        let events = [
            stored("pi.1", 10, "ready", Some("qwen-review")),
            stored("pi.2", 20, "ready", Some("me")),
            stored("pi.3", 30, "ready", Some("claude")),
            stored("pi.4", 40, "ready", Some("agent")),
            stored("pi.5", 50, "ready", None),
        ];
        let qwen = pick(&fold(&events), 5, &walk(&[Lane::Me]), Some("qwen-review"));
        assert_eq!(
            ids(&qwen),
            ["pi.1", "pi.2", "pi.5"],
            "own queue and the literal, then the overflow; never agent"
        );
        assert_eq!(qwen.elsewhere, 2, "claude's queue and the agent lane");

        let claude = pick(&fold(&events), 5, &walk(&[Lane::Me]), Some("claude"));
        assert_eq!(ids(&claude), ["pi.2", "pi.3", "pi.5"]);
        assert_eq!(claude.elsewhere, 2);

        let nobody = pick(&fold(&events), 5, &walk(&[Lane::Me]), None);
        assert_eq!(
            ids(&nobody),
            ["pi.2", "pi.5"],
            "no callsign walks the literal `me` lane alone"
        );
        assert_eq!(nobody.elsewhere, 3);
    }

    #[test]
    fn agent_is_the_literal_lane_alone() {
        // The caller's own queue is not in the agent walk: the lane is
        // the argument, and the callsign does not widen it.
        let events = [
            stored("pi.1", 10, "ready", Some("claude")),
            stored("pi.2", 20, "ready", Some("me")),
            stored("pi.3", 30, "ready", Some("agent")),
            stored("pi.4", 40, "ready", None),
        ];
        let picks = pick(&fold(&events), 4, &walk(&[Lane::Agent]), Some("claude"));
        assert_eq!(ids(&picks), ["pi.3", "pi.4"]);
        assert_eq!(picks.elsewhere, 2);
    }

    #[test]
    fn none_named_outright_walks_the_unassigned_once() {
        let events = [
            stored("pi.1", 10, "ready", Some("agent")),
            stored("pi.2", 20, "ready", None),
            stored("pi.3", 30, "ready", Some("me")),
            stored("pi.4", 40, "ready", None),
        ];
        let picks = pick(&fold(&events), 4, &walk(&[Lane::None]), Some("claude"));
        assert_eq!(ids(&picks), ["pi.2", "pi.4"], "once, not twice");
        assert_eq!(picks.elsewhere, 2);
    }

    #[test]
    fn a_lane_walked_empty_with_work_elsewhere_is_elsewhere() {
        let events = [
            stored("pi.1", 10, "ready", Some("agent")),
            stored("pi.2", 20, "backlog", None),
        ];
        let picks = pick(&fold(&events), 1, &walk(&[Lane::Me]), Some("claude"));
        assert!(picks.picked.is_empty());
        assert_eq!(picks.elsewhere, 1);
        assert_eq!(picks.outcome(), Outcome::Elsewhere);
    }

    #[test]
    fn the_lane_word_round_trips() {
        assert_eq!(Lane::from_word(None), Lane::None);
        assert_eq!(Lane::from_word(Some("me".to_string())), Lane::Me);
        assert_eq!(Lane::from_word(Some("agent".to_string())), Lane::Agent);
        assert_eq!(
            Lane::from_word(Some("qwen-review".to_string())),
            Lane::Callsign("qwen-review".to_string())
        );
        for word in ["me", "agent", "none", "qwen-review"] {
            let lane = Lane::from_word(if word == "none" {
                None
            } else {
                Some(word.to_string())
            });
            assert_eq!(lane.to_string(), word);
        }
    }
}

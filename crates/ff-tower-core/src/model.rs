//! The stored model's closed vocabularies: status and assignee.
//!
//! Closed here, free strings in the log — the crew precedent, kept. A
//! verb or a loader refuses an unknown word at the boundary where a
//! person can fix it; the fold and the wire carry whatever got written,
//! so a newer tower's value costs one flight's fidelity rather than the
//! whole board.

use serde::{Deserialize, Serialize};

/// Where a flight stands. The wire spelling is lowercase with
/// `in_progress` — what `Status` events store and the board reads back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Triage,
    Waiting,
    Ready,
    InProgress,
    Held,
    Done,
    Canceled,
}

impl Status {
    /// The wire name.
    pub fn name(&self) -> &'static str {
        match self {
            Status::Triage => "triage",
            Status::Waiting => "waiting",
            Status::Ready => "ready",
            Status::InProgress => "in_progress",
            Status::Held => "held",
            Status::Done => "done",
            Status::Canceled => "canceled",
        }
    }

    /// The wire name back to the value; `None` for a word this binary
    /// does not know.
    pub fn parse(text: &str) -> Option<Status> {
        Some(match text {
            "triage" => Status::Triage,
            "waiting" => Status::Waiting,
            "ready" => Status::Ready,
            "in_progress" => Status::InProgress,
            "held" => Status::Held,
            "done" => Status::Done,
            "canceled" => Status::Canceled,
            _ => return None,
        })
    }

    /// Every wire name, in lifecycle order — what a bad-status refusal
    /// lists.
    pub const NAMES: [&'static str; 7] = [
        "triage",
        "waiting",
        "ready",
        "in_progress",
        "held",
        "done",
        "canceled",
    ];
}

/// Whose queue a flight is in. Deliberately coarse — the routing
/// decision is all the field carries; *which* agent flew it is the
/// event byline's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Assignee {
    Me,
    Agent,
}

impl Assignee {
    /// The wire name.
    pub fn name(&self) -> &'static str {
        match self {
            Assignee::Me => "me",
            Assignee::Agent => "agent",
        }
    }

    /// The wire name back to the value; `None` for anything else.
    pub fn parse(text: &str) -> Option<Assignee> {
        Some(match text {
            "me" => Assignee::Me,
            "agent" => Assignee::Agent,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_round_trips_through_its_name() {
        for name in Status::NAMES {
            let status = Status::parse(name).expect("a listed name parses");
            assert_eq!(status.name(), name);
        }
        assert!(Status::parse("claimed").is_none());
        assert!(Status::parse("In Progress").is_none(), "wire form only");
    }

    #[test]
    fn the_assignee_lanes_round_trip_and_the_rest_refuse() {
        assert_eq!(Assignee::parse("me"), Some(Assignee::Me));
        assert_eq!(Assignee::parse("agent"), Some(Assignee::Agent));
        assert_eq!(Assignee::Me.name(), "me");
        assert_eq!(Assignee::Agent.name(), "agent");
        assert!(Assignee::parse("you").is_none());
        assert!(
            Assignee::parse("none").is_none(),
            "none is absence, not a lane"
        );
    }
}

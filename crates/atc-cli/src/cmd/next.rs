//! `atc next [<lane>] [--assignee <lane>] [-n <k>] [--peek]` — pull the
//! next Ready flight from a lane, or the next `k` in filed order. The
//! walk is the named lane in filed order, then the unassigned lane in
//! filed order: `me` — your own queue, the literal `me` lane and your
//! callsign's — when unsaid, `agent` for the shared pool alone, `none`
//! for the unassigned lane by itself, or a callsign for one pilot's
//! queue. The pull is the Ready check and the In Progress move in one
//! command: unless `--peek` the picked set becomes one In Progress
//! `status` event per flight in a single append, the store's callsign
//! stamp the pilot, and `--assignee` re-lanes each pick in that same
//! append with `file`'s lane words. The verb writes to tower's log and
//! nothing to the repository: no branch, no worktree, no op row.
//!
//! The verb's success code is 0 on a pick and 1 on an empty one, fufu's
//! "no." An empty pick rides the success path with a full data envelope,
//! and `outcome` on it says which empty pick it was — `drained`, a board
//! with nothing left, or `elsewhere`, Ready work in a lane the walk
//! never entered. The code never says 3: 3 belongs to `held/*`, which
//! an empty walk is not. `while atc next` terminates on the code alone;
//! a harness that needs to know why reads the field, not the status.
//! The pipeline is the board's — store, fold — with `pick` in place of
//! `enrich`. `--peek` is the same computation with the append left out,
//! and reports the pick alone. A bad lane word on either side refuses
//! before the read, the way `assign` refuses it.

use serde::Serialize;

use crate::error::CliError;
use crate::{machine, render};
use atc_core::board::{self, Fold, Lane, Outcome};
use atc_core::log::{EventId, Kind};
use atc_core::verb;

/// One shape either way: `pulled` is `false` under `--peek`, so the
/// envelope never lies about whether the write happened.
#[derive(Serialize)]
struct Data<'a> {
    /// Which of the three things happened; the exit code is its
    /// rendering.
    outcome: Outcome,
    /// The lane the walk named — `me` when unsaid.
    lane: String,
    /// The lane each pick was moved to, as the log stores it — absent
    /// when `--assignee` was not given, `null` when it cleared the lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee: Option<Option<String>>,
    picked: &'a [Row],
    pulled: bool,
    /// Ready, in a lane the walk never entered — the count behind the
    /// `elsewhere` outcome.
    elsewhere: usize,
}

/// One pulled flight.
#[derive(Serialize)]
struct Row {
    flight: String,
    number: u64,
    subject: String,
    /// The skill the flight is flown with, for the harness to resolve
    /// through `atc skills <name>`. Absent, not null, when the
    /// flight names none.
    #[serde(skip_serializing_if = "Option::is_none")]
    skill: Option<String>,
}

pub fn run(
    json: bool,
    lane: Option<&str>,
    assignee: Option<&str>,
    count: usize,
    peek: bool,
) -> Result<i32, CliError> {
    if count == 0 {
        return Err(CliError::coded(
            "usage/bad-count",
            "`-n 0` asks for no flights — the count starts at 1",
            Vec::new(),
        ));
    }

    // Both lane words before the read, so a typo refuses before the
    // fold. The walk's lane defaults to `me`; the re-lane resolves the
    // way `file` stores it, so `--assignee me` writes the callsign.
    let lane = match lane {
        Some(word) => Lane::from_word(verb::lane_word(word)?),
        None => Lane::Me,
    };
    let assignee = assignee.map(verb::lane_word).transpose()?;

    let store = super::store()?;
    let assignee = assignee.map(|word| verb::stored_lane(word, store.callsign()));
    let events = store.read_all()?;
    let fold = board::fold(&events);
    let picks = board::pick(&fold, count, &lane, store.callsign());
    let outcome = picks.outcome();

    let pulled = !peek && !picks.picked.is_empty();
    if pulled {
        let mut batch = Vec::with_capacity(picks.picked.len() * 2);
        for pick in &picks.picked {
            let flight: EventId = pick.flight.parse().expect("the fold's ids parse");
            batch.push(Kind::Status {
                flight: flight.clone(),
                status: "in_progress".to_string(),
                reason: None,
            });
            if let Some(assignee) = &assignee {
                batch.push(Kind::Assigned {
                    flight,
                    assignee: assignee.clone(),
                });
            }
        }
        store.append(batch)?;
    }

    let rows: Vec<Row> = picks
        .picked
        .iter()
        .map(|pick| Row {
            flight: pick.flight.clone(),
            number: pick.number,
            subject: pick.subject.clone(),
            skill: fold
                .flights
                .iter()
                .find(|flight| flight.id.to_string() == pick.flight)
                .and_then(|flight| flight.skill.clone()),
        })
        .collect();

    if json {
        println!(
            "{}",
            machine::emit(
                "next",
                &Data {
                    outcome,
                    lane: lane.to_string(),
                    assignee: assignee.clone(),
                    picked: &rows,
                    pulled,
                    elsewhere: picks.elsewhere,
                }
            )
        );
    } else {
        let colored = render::colored();
        let verb = if peek { "ready" } else { "in progress" };
        for row in &rows {
            let mut line = format!(
                "{verb} {}: {}",
                render::paint_id(&show(&fold, &row.flight), colored),
                row.subject
            );
            if let Some(skill) = &row.skill {
                line.push_str(&render::paint_dim(&format!(" · skill {skill}"), colored));
            }
            if let Some(assignee) = &assignee {
                let lane = assignee.as_deref().unwrap_or("none");
                line.push_str(&render::paint_dim(&format!(" · assigned {lane}"), colored));
            }
            println!("{line}");
        }
        if picks.picked.is_empty() {
            if picks.elsewhere > 0 {
                let noun = if picks.elsewhere == 1 {
                    "flight in another lane"
                } else {
                    "flights in other lanes"
                };
                println!(
                    "nothing ready here — {} {noun}",
                    super::count(picks.elsewhere)
                );
            } else {
                println!("nothing ready");
            }
        }
        println!("{}", super::tail(colored));
    }
    Ok(match outcome {
        Outcome::Work => 0,
        Outcome::Drained | Outcome::Elsewhere => 1,
    })
}

/// A wire id from the pick, in the board's display form. Infallible — the
/// ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

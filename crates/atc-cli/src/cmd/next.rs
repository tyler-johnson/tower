//! `atc next [-n <k>] [--peek]` — pull the next Ready flight from
//! the agent lane, or the next `k` in filed order. The pull is the Ready
//! check and the In Progress move in one command: the pool is every
//! Ready flight assigned to the agent lane, and unless `--peek` the
//! picked set becomes one In Progress `status` event per flight in a
//! single append, the byline the pilot. The verb writes to tower's log
//! and nothing to the repository: no branch, no worktree, no op row.
//!
//! The verb's success code is 0 on a pick and 1 on an empty one, fufu's
//! "no." An empty pick rides the success path with a full data envelope,
//! and `outcome` on it says which empty pick it was — `drained`, a board
//! with nothing left, or `yours`, Ready work the lane kept out of the
//! pool that needs you. The code never says 3: 3 belongs to `held/*`,
//! which an empty pool is not. `while atc next` terminates on the
//! code alone; a harness that needs to know why reads the field, not the
//! status. The pipeline is the board's — store, fold — with `pick` in
//! place of `enrich`. `--peek` is the same computation
//! with the append left out, and reports the pick alone.

use serde::Serialize;

use crate::error::CliError;
use crate::{machine, render};
use atc_core::board::{self, Fold, Outcome};
use atc_core::log::Kind;

/// One shape either way: `pulled` is `false` under `--peek`, so the
/// envelope never lies about whether the write happened.
#[derive(Serialize)]
struct Data<'a> {
    /// Which of the three things happened; the exit code is its
    /// rendering.
    outcome: Outcome,
    picked: &'a [Row],
    pulled: bool,
    /// Ready, kept out of the pool by the lane alone — the count behind
    /// the `yours` outcome.
    yours: usize,
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

pub fn run(json: bool, count: usize, peek: bool) -> Result<i32, CliError> {
    if count == 0 {
        return Err(CliError::coded(
            "usage/bad-count",
            "`-n 0` asks for no flights — the count starts at 1",
            Vec::new(),
        ));
    }

    let store = super::store()?;
    let events = store.read_all()?;
    let fold = board::fold(&events);
    let picks = board::pick(&fold, count);
    let outcome = picks.outcome();

    let pulled = !peek && !picks.picked.is_empty();
    if pulled {
        store.append(
            picks
                .picked
                .iter()
                .map(|pick| Kind::Status {
                    flight: pick.flight.parse().expect("the fold's ids parse"),
                    status: "in_progress".to_string(),
                    reason: None,
                })
                .collect(),
        )?;
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
                    picked: &rows,
                    pulled,
                    yours: picks.yours,
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
            println!("{line}");
        }
        if picks.picked.is_empty() {
            if picks.yours > 0 {
                let (count, noun, verb) = if picks.yours == 1 {
                    ("one".to_string(), "flight", "needs")
                } else {
                    (super::count(picks.yours), "flights", "need")
                };
                println!("nothing ready — {count} {noun} {verb} you");
            } else {
                println!("nothing ready");
            }
        }
        println!("{}", super::tail(colored));
    }
    Ok(match outcome {
        Outcome::Work => 0,
        Outcome::Drained | Outcome::Yours => 1,
    })
}

/// A wire id from the pick, in the board's display form. Infallible — the
/// ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

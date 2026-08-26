//! `ff tower next [-n <k>] [--peek]` — claim the next ready flight, or a
//! set of `k` that collide with neither each other nor anything already
//! flying.
//!
//! The one verb whose success code varies: 0 when anything was picked, 1
//! when nothing — fufu's "no," on the hold precedent. An empty pick rides
//! the success path with a full data envelope and only the code says it,
//! so `while ff tower next` terminates on the code alone. The pipeline is
//! the board's — store, fold, gather, probe — with `pick` in place of
//! `enrich`, and unless `--peek` the picked set becomes one `claimed`
//! event per flight in a single atomic append.

use serde::Serialize;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Fold, Passed, Pick, Skip};
use ff_tower_core::log::{Kind, Store};

/// One shape either way: `claimed` is `false` under `--peek`, so the
/// envelope never lies about whether the write happened.
#[derive(Serialize)]
struct Data<'a> {
    picked: &'a [Pick],
    claimed: bool,
    passed: &'a [Passed],
}

pub fn run(json: bool, count: usize, peek: bool) -> Result<i32, CliError> {
    if count == 0 {
        return Err(CliError::coded(
            "usage/bad-count",
            "`-n 0` asks for no flights — the count starts at 1",
            Vec::new(),
        ));
    }

    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let events = store.read_all()?;
    let fold = board::fold(&events);
    let reads = board::gather(&ff)?;
    let verdicts = board::probe(&ff, &fold, &reads)?;
    let picks = board::pick(&fold, &reads, &verdicts, count);

    let claimed = !peek && !picks.picked.is_empty();
    if claimed {
        store.append(
            picks
                .picked
                .iter()
                .map(|pick| Kind::Claimed {
                    flight: pick.flight.parse().expect("the fold's ids parse"),
                })
                .collect(),
        )?;
    }

    if json {
        println!(
            "{}",
            machine::emit(
                "next",
                &Data {
                    picked: &picks.picked,
                    claimed,
                    passed: &picks.passed,
                }
            )
        );
    } else {
        let colored = render::colored();
        let verb = if peek { "ready" } else { "claimed" };
        for pick in &picks.picked {
            println!(
                "{verb} {}: {}",
                render::paint_id(&show(&fold, &pick.flight), colored),
                pick.subject
            );
        }
        if picks.picked.is_empty() {
            println!("nothing ready");
        }
        for passed in &picks.passed {
            let reason = match &passed.reason {
                Skip::Waiting { on } => format!(
                    "waiting on {}",
                    on.iter()
                        .map(|dep| show(&fold, dep))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Skip::Collides { with, paths } => {
                    let on = match paths.as_slice() {
                        [path] => path.clone(),
                        paths => format!("{} paths", paths.len()),
                    };
                    format!("collides with {} on {on}", show(&fold, with))
                }
                Skip::NoVerdict { with } => format!("no verdict vs {}", show(&fold, with)),
            };
            println!(
                "{}",
                render::paint_dim(
                    &format!("passed {} · {reason}", show(&fold, &passed.flight)),
                    colored
                )
            );
        }
        println!("{}", super::tail(colored));
    }
    Ok(if picks.picked.is_empty() { 1 } else { 0 })
}

/// A wire id from the pick, in the board's display form. Infallible — the
/// ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

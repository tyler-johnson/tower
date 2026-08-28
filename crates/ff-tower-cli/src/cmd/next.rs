//! `ff tower next [-n <k>] [--peek]` — claim the next ready flight, or a
//! set of `k` that collide with neither each other nor anything already
//! flying, and hand each one a tree to fly in.
//!
//! The one verb whose success code varies: 0 when anything was picked; on
//! an empty pick, 3 when the crew gate is what emptied it — work exists
//! and it needs you — and 1 when the board is truly drained, fufu's "no."
//! DESIGN's loop contract: a loop runs until 1 or 3 and reports which. An
//! empty pick rides the success path with a full data envelope and only
//! the code says it, so `while ff tower next` terminates on the code
//! alone. The pipeline is
//! the board's — store, fold, gather, probe — with `pick` in place of
//! `enrich`, and unless `--peek` the picked set becomes one `claimed`
//! event per flight in a single atomic append.
//!
//! # The bay, and the branch
//!
//! A pick with nothing but an id and a subject makes the agent find its
//! own tree, so the claim is only half the hand-off. [`board::assign`]
//! joins each pick to a free bay out of the fold `pick` already ran over,
//! and this verb spends that: a part stamped `bay = "warm"` with nothing
//! free mints a slot, and then the flight is bound to a branch in the bay
//! with a session-tagged `ff start` or `ff switch` — the op row every
//! later flight-to-branch derivation reads. The target is the part's own
//! stamped branch, else the branch the flight is already derived onto,
//! else a minted `flight/<wire id>`, unique by construction because the
//! session tag is.
//!
//! Claims append *before* any fufu write, and a warm or bind refusal
//! lands on that pick's row rather than ending the walk — `probe` and
//! `gather`'s idiom. A bind that fails therefore leaves a claimed,
//! unbound flight, which is exactly the state `requeue` hands back.
//! `--peek` writes nothing: it reports the bay it would take and the
//! branch it would bind, and warms and binds neither.

use serde::Serialize;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Berth, Fold, Passed, Pick, Skip};
use ff_tower_core::ff::{self, Ff};
use ff_tower_core::log::{Kind, Store};

/// One shape either way: `claimed` is `false` under `--peek`, so the
/// envelope never lies about whether the write happened.
#[derive(Serialize)]
struct Data<'a> {
    picked: &'a [Row],
    claimed: bool,
    passed: &'a [Passed],
    /// Live and unclaimed, kept out of the pool by the crew stamp alone —
    /// the count behind exit 3.
    yours: usize,
}

/// One claimed flight and where it flies. The pick's own `branch` is the
/// stale one — what the log said before this run — so it does not ride
/// out under that name: `branch` here is effective, the branch the flight
/// was bound to when a bind happened and the derived one otherwise.
#[derive(Serialize)]
struct Row {
    flight: String,
    number: u64,
    subject: String,
    branch: Option<String>,
    /// The bay's path; `None` when the pool was full and no stamp asked
    /// for a slot.
    bay: Option<String>,
    bay_id: Option<String>,
    /// True when this run minted the bay.
    warmed: bool,
    /// The skill the flight's part is flown with, for the harness to
    /// resolve through `ff tower skills <name>`. Absent, not null, when
    /// the part names none.
    #[serde(skip_serializing_if = "Option::is_none")]
    skill: Option<String>,
    /// A warm or bind refusal, in fufu's words or tower's. The claim
    /// stands regardless.
    refused: Option<String>,
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
    let berths = board::assign(&fold, &reads, &picks.picked);

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

    let mut rows = Vec::new();
    for (pick, berth) in picks.picked.iter().zip(&berths) {
        rows.push(berth_row(&ff, &fold, pick, berth, peek)?);
    }

    if json {
        println!(
            "{}",
            machine::emit(
                "next",
                &Data {
                    picked: &rows,
                    claimed,
                    passed: &picks.passed,
                    yours: picks.yours,
                }
            )
        );
    } else {
        let colored = render::colored();
        let verb = if peek { "ready" } else { "claimed" };
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
            println!("  {}", render::paint_dim(&berth_line(row), colored));
        }
        if picks.picked.is_empty() {
            if picks.yours > 0 {
                let (count, noun, verb) = if picks.yours == 1 {
                    ("one".to_string(), "flight", "needs")
                } else {
                    (super::count(picks.yours), "flights", "need")
                };
                println!("nothing ready — {count} {noun} {verb} you");
                println!("{}", render::paint_dim("triage: ff tower triage", colored));
            } else {
                println!("nothing ready");
            }
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
                Skip::Collides { with, paths } => format!(
                    "collides with {} on {}",
                    show(&fold, with),
                    render::paths_phrase(paths)
                ),
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
    Ok(match (picks.picked.is_empty(), picks.yours) {
        (false, _) => 0,
        (true, 0) => 1,
        (true, _) => 3,
    })
}

/// Spend one berth: warm what the stamp asked for, bind the flight to a
/// branch in the bay, and say what happened. Under `--peek` nothing is
/// written and the row reports the bay that would be taken and the branch
/// that would be bound.
///
/// Tolerance is per row and deliberate. A refusal fufu shaped — a branch
/// checked out elsewhere, a pool root that is not there — is this pick's
/// news and not the walk's, so it lands in `refused` and the next pick
/// still gets its tree. Anything else is the seam failing to get an
/// answer at all, and that propagates.
fn berth_row(
    ff: &Ff,
    fold: &Fold,
    pick: &Pick,
    berth: &Berth,
    peek: bool,
) -> Result<Row, CliError> {
    let part = fold
        .flights
        .iter()
        .find(|flight| flight.id.to_string() == pick.flight)
        .and_then(|flight| flight.part.as_ref());
    let stamped = part.and_then(|part| part.branch.clone());
    let skill = part.and_then(|part| part.skill.clone());

    let mut bay = berth.bay.as_ref().map(|view| Slot {
        id: view.id.clone(),
        path: view.path.clone(),
        standing: view.branch.clone(),
        warmed: false,
    });
    let mut refused = None;

    if !peek && bay.is_none() && berth.wants_warm {
        match warm(ff) {
            Ok(added) => {
                bay = Some(Slot {
                    id: added.id,
                    path: added.path,
                    standing: Some(added.branch),
                    warmed: true,
                });
            }
            Err(err) => refused = Some(err),
        }
    }

    let mut branch = pick.branch.clone();
    if let Some(slot) = &bay {
        // The branch to fly on: what the procedure resolved at file time,
        // else wherever the flight is already derived, else one minted
        // off the session tag — unique because the tag is.
        let existing = stamped.or_else(|| pick.branch.clone());
        let target = existing
            .clone()
            .unwrap_or_else(|| format!("flight/{}", pick.flight));

        if peek || slot.standing.as_deref() == Some(target.as_str()) {
            // Already standing on it: nothing to move, and no op row to
            // write that is not there already.
            branch = Some(target);
        } else {
            // The one production use of the session tag: this call is
            // what puts the flight's name on the bay's operation chain,
            // and every later flight-to-branch derivation reads it back.
            let handle = ff.at_path(&slot.path).session(&pick.flight);
            let bound = match &existing {
                Some(existing) => handle.switch(existing).map(|switched| switched.to),
                None => handle.start(Some(&target)).map(|started| started.minted),
            };
            match bound {
                Ok(bound) => branch = Some(bound),
                Err(err @ ff::Error::Ff(_)) => refused = Some(err.to_string()),
                Err(err) => return Err(err.into()),
            }
        }
    }

    Ok(Row {
        flight: pick.flight.clone(),
        number: pick.number,
        subject: pick.subject.clone(),
        branch,
        bay: bay.as_ref().map(|slot| slot.path.clone()),
        bay_id: bay.as_ref().map(|slot| slot.id.clone()),
        warmed: bay.is_some_and(|slot| slot.warmed),
        skill,
        refused,
    })
}

/// The bay a pick ends up in, whether it was standing or minted here.
struct Slot {
    id: String,
    path: String,
    /// The branch the bay is on before any bind — a bay already there
    /// needs no move.
    standing: Option<String>,
    warmed: bool,
}

/// Mint the next slot under `tower.bays` and add the worktree — bare
/// `ff tower bay warm`, without the render. A warm is opportunistic, so
/// every way it can fail comes back as text for a row rather than as an
/// error: `usage/needs-path` when no pool root is set, `bay/pool-root`
/// when the one that is set will not open, and fufu's own refusal when
/// the tree cannot be made.
fn warm(ff: &Ff) -> Result<ff_tower_core::ff::WorktreeAdded, String> {
    let path = super::bay::mint_slot(ff).map_err(|err| err.to_string())?;
    ff.worktree_add(&path, None).map_err(|err| err.to_string())
}

/// The indented line under a claim: where the flight flies, and what went
/// wrong if something did. A full pool with no stamp asking for a slot is
/// not a failure — it is a claim with no tree, and the way out of it is
/// one command, so the line says the command.
fn berth_line(row: &Row) -> String {
    let mut phrases = Vec::new();
    match (&row.bay, &row.branch) {
        (Some(path), Some(branch)) => {
            phrases.push(path.clone());
            phrases.push(branch.clone());
        }
        (Some(path), None) => phrases.push(path.clone()),
        (None, _) if row.refused.is_none() => {
            phrases.push("no free bay · ff tower bay warm".to_string());
        }
        (None, _) => {}
    }
    if let Some(refused) = &row.refused {
        phrases.push(refused.clone());
    }
    phrases.join("  ")
}

/// A wire id from the pick, in the board's display form. Infallible — the
/// ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

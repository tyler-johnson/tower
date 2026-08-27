//! `ff tower explain <flight>` — why this is here, why this procedure,
//! and what it beat.
//!
//! `next`'s passed rows are the explained ranking for the whole walk;
//! this narrows it to one flight and adds the clauses `next` has no room
//! for — the standing that keeps a flight out of the pool, the routing
//! that put it under its procedure, and the passed rows attributable to
//! it. A read, exit 0 always: brief's pipeline plus the probes, because
//! the walk's outcome is a function of the verdicts. Not `ensure_active`
//! — a done flight explains with its mark, the log keeps the record.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Explanation, Fold, Skip, Standing};
use ff_tower_core::log::PartStamp;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let id = super::resolve(&fold, flight)?;

    let ff = super::ff()?;
    let reads = board::gather(&ff)?;
    let verdicts = board::probe(&ff, &fold, &reads)?;
    let explanation =
        board::explain(&fold, &reads, &verdicts, &id).expect("resolved to a filed flight");

    if json {
        println!("{}", machine::emit("explain", &explanation));
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        print!("{}", page(&fold, &explanation, now, render::colored()));
    }
    Ok(())
}

/// The page: head in the board's grammar, the standing line, the routing
/// explained, then one dim line per beat row.
fn page(fold: &Fold, explanation: &Explanation, now: i64, colored: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}  {}\n",
        render::paint_id(&show(fold, &explanation.id), colored),
        explanation.subject
    ));
    out.push_str(&format!(
        "    {}\n",
        standing(fold, &explanation.standing, now, colored)
    ));
    if let Some(part) = explanation.part.as_ref() {
        out.push_str(&format!("    {}\n", part_line(part, colored)));
    }
    if let (Some(by), Some(at)) = (explanation.routed_by.as_deref(), explanation.routed_at) {
        let mut phrase = format!(
            "routed {} · by {by} · {}",
            explanation.procedure,
            render::age(now, at)
        );
        if let Some(because) = explanation.because.as_deref() {
            phrase.push_str(&format!(" · {because}"));
        }
        out.push_str(&format!("    {}\n", render::paint_dim(&phrase, colored)));
    }
    for beaten in &explanation.beat {
        let reason = match &beaten.reason {
            Skip::Waiting { .. } => unreachable!("waiting rows never enter beat"),
            Skip::Collides { paths, .. } => format!("collides on {}", paths_phrase(paths)),
            Skip::NoVerdict { .. } => "no verdict".to_string(),
        };
        out.push_str(&format!(
            "    {}\n",
            render::paint_dim(
                &format!("beat {} · {reason}", show(fold, &beaten.flight)),
                colored
            )
        ));
    }
    out.push('\n');
    out.push_str(&super::tail(colored));
    out.push('\n');
    out
}

/// The standing, one line in `next`'s phrases where the walk speaks and
/// the board's where a mark does. Warn where the board warns — the open
/// question and the walk's refusals — dim everywhere else.
fn standing(fold: &Fold, standing: &Standing, now: i64, colored: bool) -> String {
    match standing {
        Standing::Done { by, at } => {
            render::paint_dim(&format!("done by {by} {}", render::age(now, *at)), colored)
        }
        Standing::Question { by, at, text } => format!(
            "{}{}",
            render::paint_warn(text, colored),
            render::paint_dim(
                &format!(" · asked by {by} {}", render::age(now, *at)),
                colored
            )
        ),
        Standing::Held { branch, resolving } => {
            let verb = if *resolving { "resolving" } else { "held" };
            render::paint_dim(&format!("{verb} on {branch}"), colored)
        }
        Standing::Claimed { by, .. } => render::paint_dim(&format!("claimed by {by}"), colored),
        Standing::Yours { crew } => render::paint_dim(
            &match crew.as_deref() {
                Some(crew) => format!("yours — crewed {crew}"),
                None => "yours — no part stamp".to_string(),
            },
            colored,
        ),
        Standing::Ready => render::paint_dim("ready", colored),
        Standing::Waiting { on } => render::paint_dim(
            &format!(
                "waiting on {}",
                on.iter()
                    .map(|dep| show(fold, dep))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            colored,
        ),
        Standing::Collides { with, paths } => render::paint_warn(
            &format!(
                "collides with {} on {}",
                show(fold, with),
                paths_phrase(paths)
            ),
            colored,
        ),
        Standing::NoVerdict { with } => {
            render::paint_warn(&format!("no verdict vs {}", show(fold, with)), colored)
        }
    }
}

/// `next`'s path phrase: the one path, or a count.
fn paths_phrase(paths: &[String]) -> String {
    match paths {
        [path] => path.clone(),
        paths => format!("{} paths", paths.len()),
    }
}

/// What part of its procedure this flight is, as the filing stamped it —
/// brief's line, same grammar.
fn part_line(part: &PartStamp, colored: bool) -> String {
    let mut phrases = vec![format!("part {}", part.id), part.crew.clone()];
    if let Some(skill) = part.skill.as_deref() {
        phrases.push(format!("skill {skill}"));
    }
    if let Some(bay) = part.bay.as_deref() {
        phrases.push(format!("bay {bay}"));
    }
    phrases.push(format!("done {}", part.done));
    render::paint_dim(&phrases.join(" · "), colored)
}

/// A wire id from the explanation, in the board's display form.
/// Infallible — the ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

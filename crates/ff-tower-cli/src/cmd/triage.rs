//! `ff tower triage [<flight> -p <procedure> [-m <why>]]` — the walk
//! through the unclassified pile.
//!
//! Bare is the pile: every live flight still filed under `open`,
//! regardless of part — plain filings, pre-procedure filings, and
//! hand-decomposed parts and their parents alike. Claimed and held
//! flights stay listed, because a claim does not classify. The pile is a
//! pure log read — no gather, no probe — and rendering it always exits 0.
//!
//! Named is the route: one `routed` event re-stamps the flight, with the
//! explanation stored beside it — deterministic and stored, never
//! recomputed, principle 11 applied to routing. The collapse rule is
//! `file`'s: a single-part procedure stamps the flight itself; a
//! multi-part one makes the flight a parent and the same atomic batch
//! files its parts on `decompose`'s edges, so no part is ever live and
//! unlinked. Routing a claimed flight is allowed and the claim stands;
//! routing back to `open` is the undo, no special case. Re-routing after
//! a multi-part route leaves the old part flights live, closed by hand.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Flight};
use ff_tower_core::log::Kind;
use ff_tower_core::procedure;

pub fn run(
    json: bool,
    flight: Option<&str>,
    procedure: Option<String>,
    message: Option<String>,
) -> Result<(), CliError> {
    match (flight, procedure) {
        (None, None) => pile(json),
        (Some(flight), Some(name)) => route(json, flight, &name, message),
        (Some(_), None) => Err(CliError::coded(
            "usage/no-procedure",
            "a flight but no procedure — `-p <name>` says where it routes",
            vec!["ff tower procedures".to_string()],
        )),
        (None, Some(_)) => Err(CliError::coded(
            "usage/no-flight",
            "a procedure but no flight to route to it",
            vec!["ff tower triage".to_string()],
        )),
    }
}

/// The pile: live flights whose stored stamp still says `open`.
fn pile(json: bool) -> Result<(), CliError> {
    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let pile: Vec<&Flight> = fold
        .flights
        .iter()
        .filter(|flight| flight.done.is_none() && flight.procedure == "open")
        .collect();

    if json {
        let rows: Vec<serde_json::Value> = pile
            .iter()
            .map(|flight| {
                serde_json::json!({
                    "flight": flight.id.to_string(),
                    "number": flight.number,
                    "subject": flight.subject,
                    "filed_by": flight.filed_by,
                    "filed_at": flight.filed_at,
                    "claimed_by": flight.claim.as_ref().map(|claim| claim.by.clone()),
                    "question": flight.question.as_ref().map(|q| q.text.clone()),
                })
            })
            .collect();
        println!(
            "{}",
            machine::emit("triage", &serde_json::json!({ "pile": rows }))
        );
        return Ok(());
    }

    let colored = render::colored();
    if pile.is_empty() {
        println!("nothing unclassified");
        return Ok(());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let refs: Vec<String> = pile
        .iter()
        .map(|flight| super::display(&fold, &flight.id))
        .collect();
    let id_width = width(&refs);
    for (reference, flight) in refs.iter().zip(&pile) {
        println!(
            "· {}  {}",
            render::paint_id(&format!("{reference:<id_width$}"), colored),
            flight.subject
        );
        let mut phrases = Vec::new();
        if let Some(question) = &flight.question {
            phrases.push(render::paint_warn(&question.text, colored));
        }
        if let Some(claim) = &flight.claim {
            phrases.push(render::paint_dim(
                &format!("claimed by {}", claim.by),
                colored,
            ));
        }
        phrases.push(render::paint_dim(
            &format!(
                "filed by {} {}",
                flight.filed_by,
                render::age(now, flight.filed_at)
            ),
            colored,
        ));
        println!("    {}", phrases.join(&render::paint_dim(" · ", colored)));
    }
    let noun = if pile.len() == 1 { "flight" } else { "flights" };
    println!(
        "{}",
        render::paint_dim(
            &format!(
                "{} {noun} unclassified · ff tower triage <flight> -p <procedure> to route one",
                pile.len()
            ),
            colored
        )
    );
    Ok(())
}

/// The route: one `routed` event, and — for a multi-part procedure — the
/// part filings and edges in the same atomic batch.
fn route(json: bool, flight: &str, name: &str, message: Option<String>) -> Result<(), CliError> {
    super::parse_ref(flight)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(CliError::coded(
            "usage/empty-procedure",
            "`-p` names an empty procedure",
            Vec::new(),
        ));
    }

    let store = super::store()?;
    let installed = procedure::registry(store.main_worktree().as_deref())?;
    let fold = board::fold(&store.read_all()?);
    let id = super::resolve(&fold, flight)?;
    let subject = super::ensure_active(&fold, &id)?.subject.clone();
    let definition = installed.get(name).ok_or_else(|| {
        CliError::coded(
            "procedure/not-found",
            format!(
                "no procedure `{name}` — installed: {}",
                installed.names().join(", ")
            ),
            vec!["ff tower procedures".to_string()],
        )
    })?;

    let because = message.unwrap_or_default();
    let ids = store.append_with(|mint| {
        super::classify(
            definition,
            &subject,
            super::Parent::Existing(id.clone()),
            |part| Kind::Routed {
                flight: id.clone(),
                procedure: definition.name.clone(),
                part,
                because: because.clone(),
            },
            mint,
        )
    })?;
    let (routed, rest) = ids.split_first().expect("the routed event is the first");
    let parts = if definition.parts.len() == 1 {
        0
    } else {
        definition.parts.len()
    };
    let (minted, linked) = rest.split_at(parts);

    if json {
        println!(
            "{}",
            machine::emit(
                "triage",
                &serde_json::json!({
                    "routed": super::appended(&store, routed)?,
                    "parts": super::appended_all(&store, minted)?,
                    "linked": super::appended_all(&store, linked)?,
                })
            )
        );
    } else {
        // Re-fold after the append: the echo's numbers live on the fold's
        // flights, so the minted parts must be in it.
        let fold = board::fold(&store.read_all()?);
        let colored = render::colored();
        println!(
            "routed {} to {name}: {subject}",
            render::paint_id(&super::display(&fold, &id), colored)
        );
        let refs: Vec<String> = minted.iter().map(|id| super::display(&fold, id)).collect();
        let subjects: Vec<String> = definition
            .parts
            .iter()
            .map(|part| format!("{subject} · {}", part.id))
            .collect();
        let id_width = width(&refs);
        let subject_width = width(&subjects);
        for ((reference, text), part) in refs.iter().zip(&subjects).zip(&definition.parts) {
            println!(
                "· {}  {text:<subject_width$}  {}",
                render::paint_id(&format!("{reference:<id_width$}"), colored),
                render::paint_dim(part.crew.name(), colored),
            );
        }
        if !because.is_empty() {
            println!("{}", render::paint_dim(&because, colored));
        }
        println!("{}", super::tail(colored));
    }
    Ok(())
}

fn width(items: &[String]) -> usize {
    items
        .iter()
        .map(|text| text.chars().count())
        .max()
        .unwrap_or(0)
}

//! `ff tower bay [list|warm|release]` — the pool: derived from `ff
//! worktree list`, never entered, never registered.
//!
//! `list` is the board's pipeline minus the probes — fold, gather, the
//! bay fold — because occupancy is the same flight-to-branch derivation
//! the board runs. `warm` and `release` are fufu's worktree verbs with
//! tower's one gate in front of the second: releasing a bay a live
//! flight sits in is refused as `bay/occupied`, and everything else —
//! `worktree/not-found`, `is-main`, `is-current`, a busy tree — stays
//! fufu's refusal, forwarded verbatim rather than re-implemented.

use crate::cli::BayAction;
use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, BayView, Fold};
use ff_tower_core::ff::Ff;
use ff_tower_core::log::Store;

pub fn run(json: bool, action: Option<&BayAction>) -> Result<(), CliError> {
    match action {
        None | Some(BayAction::List) => list(json),
        Some(BayAction::Warm { path, branch }) => warm(json, path.as_deref(), branch.as_deref()),
        Some(BayAction::Release { bay }) => release(json, bay),
    }
}

fn list(json: bool) -> Result<(), CliError> {
    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let fold = board::fold(&store.read_all()?);
    let reads = board::gather(&ff)?;
    let views = board::bays(&fold, &reads);

    if json {
        println!(
            "{}",
            machine::emit("bay list", &serde_json::json!({ "bays": views }))
        );
    } else {
        print!("{}", page(&fold, &views, render::colored()));
    }
    Ok(())
}

fn warm(json: bool, path: Option<&str>, branch: Option<&str>) -> Result<(), CliError> {
    // No fold and no gate: fufu's own refusals — `worktree/exists`,
    // `branch/checked-out-elsewhere`, `worktree/unborn` — forward
    // verbatim, and a named path passes through untouched so a relative
    // one resolves against the repository via `-C`. No path mints the
    // next slot under `tower.bays` instead.
    let ff = super::ff()?;
    let path = match path {
        Some(path) => path.to_string(),
        None => mint_slot(&ff)?,
    };
    let added = ff.worktree_add(&path, branch)?;

    if json {
        println!(
            "{}",
            machine::emit(
                "bay warm",
                &serde_json::json!({ "added": { "id": added.id, "path": added.path, "branch": added.branch } })
            )
        );
    } else {
        let colored = render::colored();
        println!(
            "warmed {}: {} on {}",
            render::paint_id(&added.id, colored),
            added.path,
            added.branch
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}

/// The next slot under `tower.bays`: `root/bay-<n>`, smallest free n.
///
/// The root may be relative, resolved against the main worktree's path —
/// never the shell's directory or the invoking bay, because the key is
/// common-dir-shared and the anchor must be too. A configured-but-missing
/// root is created: setting the key is the deliberate act, the directory
/// bootstraps itself. A number is taken while a worktree sits under the
/// root as `bay-<n>` or the directory exists on disk — the exists-guard
/// keeps a leftover or foreign directory from colliding — and since
/// `ff worktree remove` deletes the directory, released numbers come back
/// on their own.
fn mint_slot(ff: &Ff) -> Result<String, CliError> {
    let Some(root) = Store::open(ff.repo())?.pool_root() else {
        return Err(CliError::coded(
            "usage/needs-path",
            "bare `warm` needs a pool root — set `tower.bays` or name a path",
            vec![
                "git config tower.bays <dir>".to_string(),
                "ff tower bay warm <path>".to_string(),
            ],
        ));
    };

    let list = ff.worktree_list()?;
    let root = std::path::Path::new(&root);
    let root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        let main = list
            .worktrees
            .iter()
            .find(|row| row.id == "main")
            .and_then(|row| row.path.as_deref())
            .expect("the survey lists main with a path");
        std::path::Path::new(main).join(root)
    };
    let io = |err: std::io::Error| {
        CliError::coded(
            "bay/pool-root",
            format!("pool root `{}`: {err}", root.display()),
            vec!["git config tower.bays <dir>".to_string()],
        )
    };
    std::fs::create_dir_all(&root).map_err(&io)?;
    let root = std::fs::canonicalize(&root).map_err(&io)?;

    let taken: std::collections::HashSet<u64> = list
        .worktrees
        .iter()
        .filter_map(|row| std::fs::canonicalize(row.path.as_deref()?).ok())
        .filter(|path| path.parent() == Some(root.as_path()))
        .filter_map(|path| slot_number(path.file_name()?.to_str()?))
        .collect();

    let slot = (1..)
        .find(|n| !taken.contains(n) && !root.join(format!("bay-{n}")).exists())
        .expect("the naturals do not run out");
    Ok(root
        .join(format!("bay-{slot}"))
        .to_string_lossy()
        .into_owned())
}

/// `bay-<n>` with n ≥ 1, or nothing.
fn slot_number(name: &str) -> Option<u64> {
    name.strip_prefix("bay-")
        .and_then(|digits| digits.parse().ok())
        .filter(|n| *n >= 1)
}

fn release(json: bool, bay: &str) -> Result<(), CliError> {
    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let fold = board::fold(&store.read_all()?);
    let reads = board::gather(&ff)?;
    let views = board::bays(&fold, &reads);

    // Exact id first, then path — canonicalized on both sides, because
    // the survey's paths are absolute and the user's may be relative: an
    // uncanonicalized compare would let a spelling bypass the gate.
    let resolved = views.iter().find(|view| view.id == bay).or_else(|| {
        let target = std::fs::canonicalize(bay).ok()?;
        views
            .iter()
            .find(|view| std::fs::canonicalize(&view.path).is_ok_and(|path| path == target))
    });

    if let Some(view) = resolved
        && let Some(flight) = view.flight.as_deref()
    {
        let id = flight.parse().expect("the fold's ids parse");
        let flight_ref = super::display(&fold, &id);
        let subject = view.subject.as_deref().unwrap_or("");
        return Err(CliError::coded(
            "bay/occupied",
            format!("`{bay}` carries {flight_ref}: {subject} — a live flight keeps its bay"),
            vec![
                format!("ff tower done {flight_ref}"),
                "ff tower bay".to_string(),
            ],
        ));
    }

    // Free, or unresolved — fufu's `worktree/not-found` answers the
    // latter, and the capture-before-teardown makes the former safe.
    let target = resolved.map_or(bay, |view| view.id.as_str());
    let removed = ff.worktree_remove(target)?;

    if json {
        println!(
            "{}",
            machine::emit(
                "bay release",
                &serde_json::json!({ "removed": { "id": removed.id, "path": removed.path, "branch": removed.branch, "capture": removed.capture } })
            )
        );
    } else {
        let colored = render::colored();
        let mut line = format!(
            "released {}: {}",
            render::paint_id(&removed.id, colored),
            removed.path
        );
        if removed.capture.is_some() {
            line.push_str(&format!(
                "  {}",
                render::paint_dim("work captured", colored)
            ));
        }
        println!("{line}");
        println!("{}", super::tail(colored));
    }
    Ok(())
}

/// The pool, one row per bay in survey order: padded painted id, the
/// branch (or `(detached)`), the occupant or a dim `free`, and a dim
/// `here` on the invoking row.
fn page(fold: &Fold, views: &[BayView], colored: bool) -> String {
    let id_width = views
        .iter()
        .map(|view| view.id.chars().count())
        .max()
        .unwrap_or(0);
    let branch_width = views
        .iter()
        .map(|view| branch_text(view).chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for view in views {
        let id = format!("{:<id_width$}", view.id);
        let branch = format!("{:<branch_width$}", branch_text(view));
        let mut line = format!("{}  {branch}", render::paint_id(&id, colored));
        match (view.flight.as_deref(), view.subject.as_deref()) {
            (Some(flight), Some(subject)) => {
                let id = flight.parse().expect("the fold's ids parse");
                line.push_str(&format!(
                    "  {}  {subject}",
                    render::paint_id(&super::display(fold, &id), colored)
                ));
            }
            _ => line.push_str(&format!("  {}", render::paint_dim("free", colored))),
        }
        if view.current {
            line.push_str(&format!("  {}", render::paint_dim("here", colored)));
        }
        out.push_str(&line);
        out.push('\n');
    }

    let noun = if views.len() == 1 { "bay" } else { "bays" };
    out.push_str(&render::paint_dim(
        &format!("{} {noun} · ff tower bay warm to add one", views.len()),
        colored,
    ));
    out.push('\n');
    out
}

fn branch_text(view: &BayView) -> String {
    view.branch
        .clone()
        .unwrap_or_else(|| "(detached)".to_string())
}

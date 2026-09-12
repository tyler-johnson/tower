//! The two verbs: `atc hook` and `atc unhook`.
//!
//! Both are for humans: an unknown slug is a real error, every failure is
//! loud, and `--json` emits a report envelope. The machine half — what a
//! wired client actually runs — is `atc briefing <slug>`, in `cmd/briefing.rs`.

use super::{Change, InstallOptions, Integration, Status, Wiring};
use crate::error::CliError;
use crate::{machine, render};

// ---- atc hook / atc unhook -------------------------------------------------

pub fn hook(
    json: bool,
    slugs: Vec<String>,
    all: bool,
    list: bool,
    settings: bool,
    update: bool,
) -> Result<(), CliError> {
    if update {
        return refresh(json);
    }
    if list {
        return report(json, "hook", &super::statuses(), &[]);
    }
    let targets = targets(json, &slugs, all, Verb::Hook)?;
    act(json, &targets, &InstallOptions { settings }, Verb::Hook)
}

pub fn unhook(json: bool, slugs: Vec<String>, all: bool) -> Result<(), CliError> {
    let targets = targets(json, &slugs, all, Verb::Unhook)?;
    act(json, &targets, &InstallOptions::default(), Verb::Unhook)
}

#[derive(Clone, Copy)]
enum Verb {
    Hook,
    Unhook,
    /// `atc hook -u`: the install re-run through `repair`, so a machine
    /// stays on whatever mechanism it is on.
    Update,
}

impl Verb {
    fn word(self) -> &'static str {
        match self {
            Verb::Hook | Verb::Update => "hook",
            Verb::Unhook => "unhook",
        }
    }
}

// ---- atc hook -u -----------------------------------------------------------

/// `atc hook -u`: refresh what is wired, and wire nothing new.
///
/// The install re-run for every slug already wired, on whatever mechanism
/// it is on. An upgraded binary refreshes the machine this way, without a
/// person having to remember which verb does.
fn refresh(json: bool) -> Result<(), CliError> {
    // Wired is what tower wrote, whole or in part. A hand-written line is
    // never tower's to rewrite, and a slug that is not wired is exactly
    // what this verb must not add.
    let statuses = super::statuses();
    let targets: Vec<&'static dyn Integration> = statuses
        .iter()
        .filter(|status| matches!(status.wiring, Wiring::Wired { .. } | Wiring::Partial { .. }))
        .filter_map(|status| super::by_slug(status.slug))
        .collect();
    if targets.is_empty() {
        if json {
            return report(json, "hook", &statuses, &[]);
        }
        println!("nothing is wired on this machine — atc hook wires it");
        return Ok(());
    }
    act(json, &targets, &InstallOptions::default(), Verb::Update)
}

/// Which slugs this invocation acts on.
///
/// Named slugs are taken as given. `--all` is everything detected. Naming
/// nothing reports first and then asks, because a command that rewrites
/// config files on four different clients should say what it found before
/// it touches any of them — and when nothing may prompt, the report *is*
/// the answer and nothing is touched.
fn targets(
    json: bool,
    slugs: &[String],
    all: bool,
    verb: Verb,
) -> Result<Vec<&'static dyn Integration>, CliError> {
    if !slugs.is_empty() {
        if all {
            return Err(bad_flags(verb, slugs));
        }
        return slugs
            .iter()
            .map(|slug| super::by_slug(slug).ok_or_else(|| unknown_slug(slug)))
            .collect();
    }

    let detected: Vec<&'static dyn Integration> = super::all()
        .into_iter()
        .filter(|i| i.detect().is_present())
        .collect();

    if all {
        return Ok(detected);
    }

    // Bare `atc hook`: report, then ask.
    report(json, verb.word(), &super::statuses(), &[])?;
    if detected.is_empty() {
        return Ok(Vec::new());
    }
    if !machine::interactive() {
        // Nothing may prompt here, so nothing is acted on either. The
        // report already went out, which is the useful half.
        println!();
        println!("{}", nothing_hooked(&detected, verb));
        return Ok(Vec::new());
    }
    let names: Vec<&str> = detected.iter().map(|i| i.slug()).collect();
    if machine::confirm(&format!("{} {}?", verb.word(), names.join(", ")))? {
        Ok(detected)
    } else {
        println!("{}", nothing_hooked(&detected, verb));
        Ok(Vec::new())
    }
}

/// `--all` beside a name says less, not more. Spelled per verb rather
/// than through a placeholder, so the exits are lines a person can type.
fn bad_flags(verb: Verb, slugs: &[String]) -> CliError {
    let exits = match verb {
        Verb::Hook | Verb::Update => vec![
            "atc hook --all".to_string(),
            format!("atc hook {}", slugs.join(" ")),
        ],
        Verb::Unhook => vec![
            "atc unhook --all".to_string(),
            format!("atc unhook {}", slugs.join(" ")),
        ],
    };
    CliError::coded(
        "usage/bad-flags",
        "--all is every client detected, so naming one alongside it says less, not more",
        exits,
    )
}

/// The refusal a wrong name earns, shared with `atc briefing <client>`.
pub fn unknown_slug(slug: &str) -> CliError {
    CliError::coded(
        "usage/unknown-slug",
        format!("unknown client {slug:?} (known: {})", super::slugs()),
        vec!["atc hook -l".into()],
    )
}

/// Declining prints the explicit form, so the slugs are teachable rather
/// than something to go and look up.
fn nothing_hooked(detected: &[&'static dyn Integration], verb: Verb) -> String {
    let names: Vec<&str> = detected.iter().map(|i| i.slug()).collect();
    format!(
        "nothing {}ed. name what you want: atc {} {}",
        verb.word(),
        verb.word(),
        names.join(" ")
    )
}

fn act(
    json: bool,
    targets: &[&'static dyn Integration],
    opts: &InstallOptions,
    verb: Verb,
) -> Result<(), CliError> {
    if targets.is_empty() {
        // `--all` over a machine with nothing on it. Saying so beats
        // exiting silently, which reads as having done something.
        // `refresh` answers its own empty case before it gets here.
        if !json {
            println!(
                "nothing to {} — no agent client was detected",
                match verb {
                    Verb::Hook => "hook",
                    Verb::Unhook => "unhook",
                    Verb::Update => "refresh",
                }
            );
        }
        return Ok(());
    }
    let colored = render::colored();
    let mut acted: Vec<&'static str> = Vec::new();
    // Every failure is loud, and the first one stops the run: these
    // rewrite config files, and carrying on past one that would not write
    // is how half a machine ends up wired.
    for integration in targets {
        let change: Change = match verb {
            Verb::Hook => integration.install(opts)?,
            Verb::Unhook => integration.uninstall(opts)?,
            Verb::Update => integration.repair()?,
        };
        if change.changed {
            acted.push(integration.slug());
        }
        if !json {
            for (n, line) in change.lines.iter().enumerate() {
                if n == 0 {
                    let line = match verb {
                        Verb::Update if change.changed => format!("rewired — {line}"),
                        _ => line.clone(),
                    };
                    println!("{} {line}", render::paint_ok(integration.slug(), colored));
                } else {
                    println!("  {}", render::paint_dim(line, colored));
                }
            }
        }
    }
    if json {
        return report(json, verb.word(), &super::statuses(), &acted);
    }
    Ok(())
}

// ---- the report ------------------------------------------------------------

/// One rendering of `statuses()`, which is also what `atc doctor` reads —
/// the two cannot disagree, because there is only one derivation.
fn report(
    json: bool,
    cmd: &str,
    statuses: &[Status],
    acted: &[&'static str],
) -> Result<(), CliError> {
    if json {
        println!(
            "{}",
            machine::emit(
                cmd,
                &serde_json::json!({
                    "integrations": statuses,
                    "changed": acted,
                }),
            )
        );
        return Ok(());
    }
    let colored = render::colored();
    let rows: Vec<(&str, String, String)> = statuses
        .iter()
        .map(|status| (status.slug, client_of(status), describe(status)))
        .collect();
    let slug_width = rows.iter().map(|(slug, ..)| slug.len()).max().unwrap_or(6);
    let client_width = rows
        .iter()
        .map(|(_, client, _)| client.chars().count())
        .max()
        .unwrap_or(0);

    for (status, (slug, client, wiring)) in statuses.iter().zip(&rows) {
        let painted = match &status.wiring {
            Wiring::Wired { .. } => render::paint_ok(wiring, colored),
            Wiring::Partial { .. } => render::paint_warn(wiring, colored),
            _ => render::paint_dim(wiring, colored),
        };
        // Pad on the plain text and append after painting, so the escape
        // bytes never inflate a column.
        let pad = " ".repeat(client_width - client.chars().count());
        println!(
            "{slug:slug_width$}  {}{pad}  {painted}",
            render::paint_dim(client, colored)
        );
        if let Some(note) = &status.note {
            println!(
                "{:slug_width$}  {:client_width$}  {}",
                "",
                "",
                render::paint_dim(note, colored)
            );
        }
    }
    Ok(())
}

fn client_of(status: &Status) -> String {
    match &status.presence {
        super::Presence::Present { evidence } => evidence.display().to_string(),
        super::Presence::Absent => "not on this machine".into(),
    }
}

/// The wiring, in one phrase.
fn describe(status: &Status) -> String {
    let mut line = match status.wiring.at() {
        Some(at) => format!("{} — {}", status.wiring.word(), at.display()),
        None => status.wiring.word(),
    };
    // The skills ride along on the same line rather than earning a row:
    // they are a property of a client that is already listed, and a
    // client with no skills at all should not have to say so.
    if let Some(Wiring::Wired { .. }) = &status.skill {
        line.push_str(", skill");
    }
    line
}

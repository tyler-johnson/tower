//! Codex.
//!
//! Field-for-field compatible with Claude Code's payload, so it shares the
//! plain-text delivery and differs only in where its config lives.
//!
//! The one thing that needs saying out loud: Codex records trust against a
//! hook's hash and skips new or changed hooks until they are reviewed
//! through `/hooks`. Without that told, the notice silently never lands and
//! nothing explains why. The trust step gates the hook and nothing else:
//! the shipped skills are files Codex reads, not commands it runs.
//!
//! Two mechanisms, then, and they are independent. The hook is an entry
//! merged into a settings file that belongs to the user; the skills are
//! directories tower owns outright under `~/.codex/skills/`, written whole
//! and removed whole. Neither install can take the other down with it.

use std::path::PathBuf;

use super::{Change, InstallOptions, Integration, Presence, Status, Wiring, settings, skill};
use crate::error::CliError;
use settings::Need;

pub struct Codex;

const COMMAND: &str = "atc briefing codex";
const LEGACY: [&str; 0] = [];

/// Codex's `SessionStart` sources startup, resume, clear, and compact, so
/// the one event covers every context boundary the way Claude's does.
const EVENTS: [(&str, Option<&str>, Need); 1] = [("SessionStart", None, Need::Required)];

const TRUST: &str = "Codex trusts a hook by its hash: run /hooks in Codex to review this one, \
                     or it is skipped and the notice never lands";

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".codex"))
}

/// Where every skill lives: `~/.codex/skills/<name>/`, the directory
/// names the front matter carries, mentioned `$<name>` in a session.
fn skills_root() -> Result<PathBuf, CliError> {
    Ok(config_dir()?.join("skills"))
}

fn skill_wiring() -> Wiring {
    match skills_root() {
        Ok(root) => skill::wiring(&root),
        Err(_) => Wiring::NotWired,
    }
}

fn spec() -> Result<settings::Spec, CliError> {
    Ok(settings::Spec {
        path: config_dir()?.join("hooks.json"),
        shape: settings::Shape::Nested,
        events: &EVENTS,
        command: COMMAND.into(),
        legacy: &LEGACY,
        version: None,
    })
}

impl Integration for Codex {
    fn slug(&self) -> &'static str {
        "codex"
    }

    fn detect(&self) -> Presence {
        match config_dir() {
            Ok(dir) if dir.is_dir() => Presence::Present { evidence: dir },
            _ => Presence::Absent,
        }
    }

    fn status(&self) -> Status {
        let wiring = match spec() {
            Ok(spec) => settings::wiring(&spec),
            Err(err) => Wiring::Unavailable(err.to_string()),
        };
        let stale = spec().map(|spec| settings::stale(&spec)).unwrap_or(false);
        Status {
            slug: self.slug(),
            presence: self.detect(),
            // The trust step is news whenever the wiring is there: an
            // unreviewed hook and a missing one look identical from here.
            note: wiring.is_wired().then(|| TRUST.to_string()),
            wiring,
            skill: Some(skill_wiring()),
            stale,
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let mut change = settings::install(&spec()?)?;
        let root = skills_root()?;
        skill::write_all(&root)?;
        change.absorb(Change::changed(format!(
            "skills written to {}: {}",
            root.display(),
            skill::names()
        )));
        change.lines.push(TRUST.into());
        Ok(change)
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let mut change = settings::uninstall(&spec()?)?;
        let root = skills_root()?;
        if skill::remove_all(&root)? {
            change.absorb(Change::changed(format!(
                "removed {}: {}",
                root.display(),
                skill::names()
            )));
        }
        Ok(change)
    }

    /// Plain stdout, the same as Claude Code.
    fn envelope(&self, text: &str) -> String {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_briefing_goes_out_as_plain_text() {
        assert_eq!(Codex.envelope("hello"), "hello");
    }
}

//! Gemini CLI.
//!
//! Names the same payload fields as Claude Code and Codex. What it cannot
//! do is read plain text: injected context has to arrive as JSON, which is
//! why the envelope is asked of the adapter instead of assumed.

use std::path::PathBuf;

use super::{Change, InstallOptions, Integration, Presence, Status, Wiring, settings};
use crate::error::CliError;
use settings::{Class, Event, Need};

pub struct Gemini;

/// The hook command, and the spelling older installs carry — accepted
/// forever, since it sits in a file tower rewrites only when the
/// installer is run again.
const COMMAND: &str = "atc trigger gemini";
const LEGACY: [&str; 1] = ["atc briefing gemini"];

/// The boundary alone: Gemini CLI is not on this machine, so its
/// activity and end names are unverified, and the table stays at what
/// is known to fire.
const EVENTS: [Event; 1] = [Event {
    name: "SessionStart",
    matcher: None,
    class: Class::Boundary,
    need: Need::Required,
}];

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".gemini"))
}

fn spec() -> Result<settings::Spec, CliError> {
    Ok(settings::Spec {
        path: config_dir()?.join("settings.json"),
        shape: settings::Shape::Nested,
        events: &EVENTS,
        command: COMMAND.into(),
        legacy: &LEGACY,
        version: None,
    })
}

impl Integration for Gemini {
    fn slug(&self) -> &'static str {
        "gemini"
    }

    fn events(&self) -> &'static [Event] {
        &EVENTS
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
            wiring,
            note: None,
            skill: None,
            stale,
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        settings::install(&spec()?)
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        settings::uninstall(&spec()?)
    }

    /// Gemini reads injected context out of a JSON field, so plain stdout
    /// would be discarded — and discarded silently, which is the worst of
    /// the available failures.
    fn envelope(&self, text: &str) -> String {
        serde_json::json!({
            "hookSpecificOutput": { "additionalContext": text }
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_briefing_is_json_wrapped() {
        let out = Gemini.envelope("hello");
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["hookSpecificOutput"]["additionalContext"], "hello");
    }
}

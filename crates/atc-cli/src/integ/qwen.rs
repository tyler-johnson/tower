//! Qwen Code, the Gemini CLI fork.
//!
//! This adapter was Gemini CLI's. Gemini CLI is alive and unused — absent
//! from the client surveys tower reads — and its hook names have since
//! diverged from the family (`BeforeAgent`, `AfterAgent`), while Qwen
//! Code, the one fork with users, kept the inherited shape: hooks in
//! `~/.qwen/settings.json` under Claude's event names, and injected
//! context read out of `hookSpecificOutput.additionalContext`. So the
//! body of the adapter is the fork's inheritance, re-pointed at the
//! fork, and Gemini went. An existing `~/.gemini/settings.json` is left
//! as found; `atc briefing gemini` and `atc trigger gemini` are stored
//! spellings and keep answering from `retired.rs`.
//!
//! What Qwen cannot do is read plain text: injected context has to
//! arrive as JSON, which is why the envelope is asked of the adapter
//! instead of assumed. Its shell tool sets `QWEN_CODE=1`, the marker
//! its callsign resolves from, and `QWEN_CODE_SESSION_ID` on hooks and
//! shell tools alike, the session row. Whether it reads a skills
//! directory is unknown, so it gets the notice alone.

use std::path::PathBuf;

use super::{Change, InstallOptions, Integration, Presence, Status, Wiring, settings};
use crate::error::CliError;
use settings::{Class, Event, Need};

pub struct Qwen;

/// The hook command. No older spelling: nothing before this adapter
/// ever wrote `~/.qwen/settings.json`.
const COMMAND: &str = "atc trigger qwen";
const LEGACY: [&str; 0] = [];

/// Every name in Qwen's own hook enum, the family's five: the boundary,
/// whose `source` matcher would take `startup|resume|clear|compact` and
/// is left unset so every source fires; three activity events; and the
/// end.
const EVENTS: [Event; 5] = [
    Event {
        name: "SessionStart",
        matcher: None,
        class: Class::Boundary,
        need: Need::Required,
    },
    Event {
        name: "UserPromptSubmit",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
    Event {
        name: "PreToolUse",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
    Event {
        name: "Stop",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
    Event {
        name: "SessionEnd",
        matcher: None,
        class: Class::End,
        need: Need::Extra,
    },
];

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".qwen"))
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

impl Integration for Qwen {
    fn slug(&self) -> &'static str {
        "qwen"
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
            Err(err) => Wiring::Unavailable {
                complaint: err.to_string(),
            },
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

    /// Qwen reads injected context out of a JSON field, so plain stdout
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
        let out = Qwen.envelope("hello");
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["hookSpecificOutput"]["additionalContext"], "hello");
    }
}

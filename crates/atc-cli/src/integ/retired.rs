//! The trigger sources tower no longer writes and answers forever.
//!
//! A source name ends up inside a config file tower does not own and
//! rewrites only when somebody runs the installer again, which they may
//! never do. So a spelling once written is accepted for good, at the
//! cost of a line here: `atc briefing gemini` from a
//! `~/.gemini/settings.json` nobody cleaned keeps printing what it
//! printed, and `atc trigger cursor` from a `~/.cursor/hooks.json` the
//! same. Each carries the event table and the envelope it was written
//! with, and nothing else: no slug — `by_slug` never sees them, so
//! `atc hook cursor` is the ordinary unknown name — no detection, and no
//! install. The two files are left as found: neither adapter has a
//! successor to migrate to, Cursor's delivery being unverified and
//! Gemini CLI's hook names having diverged from the family.

use super::{Change, InstallOptions, Integration, Presence, Status, Wiring, settings};
use crate::error::CliError;
use settings::{Class, Event, Need};

pub struct Retired {
    source: &'static str,
    events: &'static [Event],
    envelope: fn(&str) -> String,
}

/// Cursor's boundary alone, in its own casing, from `~/.cursor/hooks.json`.
const CURSOR_EVENTS: [Event; 1] = [Event {
    name: "sessionStart",
    matcher: None,
    class: Class::Boundary,
    need: Need::Required,
}];

/// Gemini CLI's boundary alone, from `~/.gemini/settings.json`.
const GEMINI_EVENTS: [Event; 1] = [Event {
    name: "SessionStart",
    matcher: None,
    class: Class::Boundary,
    need: Need::Required,
}];

fn cursor_field(text: &str) -> String {
    serde_json::json!({ "additional_context": text }).to_string()
}

fn gemini_field(text: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": { "additionalContext": text }
    })
    .to_string()
}

static CURSOR: Retired = Retired {
    source: "cursor",
    events: &CURSOR_EVENTS,
    envelope: cursor_field,
};
static GEMINI: Retired = Retired {
    source: "gemini",
    events: &GEMINI_EVENTS,
    envelope: gemini_field,
};

/// Every retired source, for the tests that walk the class of each.
pub fn all() -> [&'static Retired; 2] {
    [&CURSOR, &GEMINI]
}

/// The retired source a stored spelling names, if it is one.
pub fn by_source(source: &str) -> Option<&'static dyn Integration> {
    all()
        .into_iter()
        .find(|retired| retired.source == source)
        .map(|retired| retired as &dyn Integration)
}

impl Integration for Retired {
    fn slug(&self) -> &'static str {
        self.source
    }

    fn events(&self) -> &'static [Event] {
        self.events
    }

    fn detect(&self) -> Presence {
        Presence::Absent
    }

    fn status(&self) -> Status {
        Status {
            slug: self.source,
            presence: Presence::Absent,
            wiring: Wiring::NotWired,
            note: None,
            skill: None,
            stale: false,
        }
    }

    /// Unreachable through `by_slug`, which never sees a retired source;
    /// the refusal is spelled once, in `verbs.rs`.
    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        Err(super::verbs::unknown_slug(self.source))
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        Err(super::verbs::unknown_slug(self.source))
    }

    fn envelope(&self, text: &str) -> String {
        (self.envelope)(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each retired source answers in the envelope it was written with,
    /// and no more events than it wired.
    #[test]
    fn each_retired_source_keeps_its_envelope_and_table() {
        let cursor = by_source("cursor").unwrap();
        let value: serde_json::Value = serde_json::from_str(&cursor.envelope("hello")).unwrap();
        assert_eq!(value["additional_context"], "hello");
        assert_eq!(cursor.class_of(Some("sessionStart")), Some(Class::Boundary));
        assert_eq!(cursor.class_of(Some("PreToolUse")), None);

        let gemini = by_source("gemini").unwrap();
        let value: serde_json::Value = serde_json::from_str(&gemini.envelope("hello")).unwrap();
        assert_eq!(value["hookSpecificOutput"]["additionalContext"], "hello");
        assert_eq!(gemini.class_of(Some("SessionStart")), Some(Class::Boundary));

        for live in ["claude", "codex", "qwen"] {
            assert!(by_source(live).is_none(), "{live} is a live source");
        }
    }

    /// Retired means unhookable: no presence, no wiring, and the
    /// install path is the unknown-slug refusal.
    #[test]
    fn a_retired_source_cannot_be_hooked() {
        for retired in all() {
            assert_eq!(retired.detect(), Presence::Absent, "{}", retired.source);
            assert_eq!(retired.status().wiring, Wiring::NotWired);
            let Err(err) = retired.install(&InstallOptions::default()) else {
                panic!("{} installs nothing", retired.source);
            };
            assert_eq!(err.id(), "usage/unknown-slug", "{}", retired.source);
            let Err(err) = retired.uninstall(&InstallOptions::default()) else {
                panic!("{} uninstalls nothing", retired.source);
            };
            assert_eq!(err.id(), "usage/unknown-slug", "{}", retired.source);
        }
    }
}

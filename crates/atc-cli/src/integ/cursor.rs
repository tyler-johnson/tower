//! Cursor's agent client.
//!
//! `cursor` is the agent, not the editor: a future editor integration gets
//! a slug of its own, because these slugs end up inside config files and
//! cannot be renamed afterward.
//!
//! Two things here differ from the other three. The config file is flatter
//! — an entry *is* a command, with no nested list — and injected context
//! goes back as a JSON field under Cursor's own name.
//!
//! `sessionStart` does not fire for cloud agents, so the notice is simply
//! absent there. That is reported rather than papered over.

use std::path::PathBuf;

use super::{Change, InstallOptions, Integration, Presence, Status, Wiring, settings};
use crate::error::CliError;
use settings::{Class, Event, Need};

pub struct Cursor;

/// The hook command, and the spelling older installs carry — accepted
/// forever, since it sits in a file tower rewrites only when the
/// installer is run again.
const COMMAND: &str = "atc trigger cursor";
const LEGACY: [&str; 1] = ["atc briefing cursor"];

/// The boundary alone: Cursor is not on this machine, so its activity
/// and end names are unverified, and the table stays at what is known
/// to fire.
const EVENTS: [Event; 1] = [Event {
    name: "sessionStart",
    matcher: None,
    class: Class::Boundary,
    need: Need::Required,
}];

const CLOUD: &str = "Cursor does not fire sessionStart for cloud agents, so the notice is \
                     absent there";

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".cursor"))
}

fn spec() -> Result<settings::Spec, CliError> {
    Ok(settings::Spec {
        path: config_dir()?.join("hooks.json"),
        shape: settings::Shape::Flat,
        events: &EVENTS,
        command: COMMAND.into(),
        legacy: &LEGACY,
        version: Some(1),
    })
}

impl Integration for Cursor {
    fn slug(&self) -> &'static str {
        "cursor"
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
            note: wiring.is_wired().then(|| CLOUD.to_string()),
            wiring,
            skill: None,
            stale,
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let mut change = settings::install(&spec()?)?;
        change.lines.push(CLOUD.into());
        Ok(change)
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        settings::uninstall(&spec()?)
    }

    /// Cursor takes injected context as a JSON field, the way Gemini does,
    /// under its own name.
    fn envelope(&self, text: &str) -> String {
        serde_json::json!({ "additional_context": text }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_briefing_is_json_wrapped() {
        let out = Cursor.envelope("hello");
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["additional_context"], "hello");
    }
}

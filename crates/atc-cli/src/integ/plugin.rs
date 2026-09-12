//! The plugin reading two adapters share: a `hooks/hooks.json` in the
//! nested shape, written whole from an event table, and read back the
//! way the client reads it to answer wired, partial, or stale.
//!
//! Claude Code's plugin and Codex's carry the same hooks file
//! — one entry per event, one command under each — and differ in the
//! manifest beside it and the directory it sits in. What is here is the
//! half that does not differ, so the two cannot drift in how they
//! decide a plugin is theirs. Each caller brings its own event table and
//! its own `is_ours`, because the command tail is the adapter's.

use std::path::Path;

use serde_json::Value;

use super::{Mechanism, Wiring, settings};
use crate::error::CliError;
use settings::{Event, Need};

/// The hooks file for `events`, every entry running `command`, with a
/// matcher where the table has one.
pub fn hooks_body(events: &[Event], command: &str) -> String {
    let mut table = serde_json::Map::new();
    for event in events {
        let mut entry = serde_json::Map::new();
        if let Some(matcher) = event.matcher {
            entry.insert("matcher".into(), matcher.into());
        }
        entry.insert(
            "hooks".into(),
            serde_json::json!([{ "type": "command", "command": command }]),
        );
        table.insert(
            event.name.to_string(),
            Value::Array(vec![Value::Object(entry)]),
        );
    }
    pretty(&serde_json::json!({ "hooks": Value::Object(table) }))
}

pub fn pretty(value: &Value) -> String {
    let mut body = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    body.push('\n');
    body
}

/// Every command a hooks.json runs under one event.
pub fn commands<'a>(value: &'a Value, event: &str) -> Vec<&'a str> {
    value["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["hooks"].as_array())
        .flatten()
        .filter_map(|c| c["command"].as_str())
        .collect()
}

/// Which of `events` a hooks.json does not carry, in table order.
pub fn missing<'a>(
    value: &Value,
    events: &'a [Event],
    is_ours: impl Fn(&str) -> bool,
) -> Vec<(&'a str, Need)> {
    events
        .iter()
        .filter(|event| !commands(value, event.name).iter().any(|c| is_ours(c)))
        .map(|event| (event.name, event.need))
        .collect()
}

/// Whether the plugin on disk is one an older tower wrote: a command in
/// a retired spelling, or an extra event missing while some event is
/// there. Either still delivers, so this is the repair `atc hook -u`
/// makes and never an outage.
pub fn stale(
    hooks_path: &Path,
    events: &[Event],
    is_ours: impl Fn(&str) -> bool,
    is_legacy: impl Fn(&str) -> bool,
) -> bool {
    let Ok(text) = std::fs::read_to_string(hooks_path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    let legacy = events
        .iter()
        .any(|event| commands(&value, event.name).iter().any(|c| is_legacy(c)));
    if legacy {
        return true;
    }
    let missing = missing(&value, events, is_ours);
    missing.len() < events.len() && missing.iter().any(|(_, need)| *need == Need::Extra)
}

/// Whether the plugin on disk is wired, read the way the client reads
/// it. `dir` is what the answer names: the plugin directory, not the
/// hooks file inside it.
pub fn wiring(
    hooks_path: &Path,
    dir: &Path,
    events: &[Event],
    is_ours: impl Fn(&str) -> bool,
) -> Wiring {
    let Ok(text) = std::fs::read_to_string(hooks_path) else {
        return Wiring::NotWired;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Wiring::Unavailable(format!("{}: not valid JSON", hooks_path.display()));
    };
    let missing = missing(&value, events, is_ours);
    if missing.len() == events.len() {
        return Wiring::NotWired;
    }
    // Only a required event's absence is partial delivery; an extra one
    // missing would be an older install, which `stale` reports instead.
    let required: Vec<&str> = missing
        .iter()
        .filter(|(_, need)| *need == Need::Required)
        .map(|(event, _)| *event)
        .collect();
    if required.is_empty() {
        return Wiring::Wired {
            mechanism: Mechanism::Plugin,
            at: dir.to_path_buf(),
        };
    }
    Wiring::Partial {
        missing: required.join(", "),
        at: dir.to_path_buf(),
    }
}

/// Write `body` at `path` when the bytes there differ, creating the
/// parent. Answers whether anything moved, so a second install can say
/// so instead of claiming a write.
pub fn write_if_changed(path: &Path, body: &str) -> Result<bool, CliError> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(body) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| super::failed(parent, err))?;
    }
    std::fs::write(path, body).map_err(|err| super::failed(path, err))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings::Class;

    const EVENTS: [Event; 2] = [
        Event {
            name: "SessionStart",
            matcher: Some("startup"),
            class: Class::Boundary,
            need: Need::Required,
        },
        Event {
            name: "SessionEnd",
            matcher: None,
            class: Class::End,
            need: Need::Extra,
        },
    ];

    fn ours(command: &str) -> bool {
        command.ends_with("trigger test")
    }

    /// The body carries every event with its matcher, and reads back
    /// as wired with nothing missing.
    #[test]
    fn the_body_round_trips_through_the_reading() {
        let body = hooks_body(&EVENTS, "/usr/bin/atc trigger test");
        let value: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["hooks"]["SessionStart"][0]["matcher"], "startup");
        assert!(value["hooks"]["SessionEnd"][0].get("matcher").is_none());
        assert_eq!(
            commands(&value, "SessionEnd"),
            vec!["/usr/bin/atc trigger test"]
        );
        assert!(missing(&value, &EVENTS, ours).is_empty());

        let tmp = tempfile::TempDir::new().unwrap();
        let hooks = tmp.path().join("plugin/hooks/hooks.json");
        assert_eq!(wiring(&hooks, tmp.path(), &EVENTS, ours), Wiring::NotWired);
        assert!(write_if_changed(&hooks, &body).unwrap());
        assert!(!write_if_changed(&hooks, &body).unwrap(), "bytes equal");
        assert!(matches!(
            wiring(&hooks, tmp.path(), &EVENTS, ours),
            Wiring::Wired {
                mechanism: Mechanism::Plugin,
                ..
            }
        ));
        assert!(!stale(&hooks, &EVENTS, ours, |_| false));
    }

    /// An extra event missing while the required one is there is stale
    /// and wired; the required one missing is partial; a retired
    /// spelling is stale on its own.
    #[test]
    fn missing_events_read_by_their_need() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks = tmp.path().join("hooks.json");
        std::fs::write(
            &hooks,
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"atc trigger test"}]}]}}"#,
        )
        .unwrap();
        assert!(matches!(
            wiring(&hooks, tmp.path(), &EVENTS, ours),
            Wiring::Wired { .. }
        ));
        assert!(stale(&hooks, &EVENTS, ours, |_| false));

        std::fs::write(
            &hooks,
            r#"{"hooks":{"SessionEnd":[{"hooks":[{"type":"command","command":"atc trigger test"}]}]}}"#,
        )
        .unwrap();
        assert!(matches!(
            wiring(&hooks, tmp.path(), &EVENTS, ours),
            Wiring::Partial { ref missing, .. } if missing == "SessionStart"
        ));

        std::fs::write(
            &hooks,
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"atc briefing test"}]}],"SessionEnd":[{"hooks":[{"type":"command","command":"atc briefing test"}]}]}}"#,
        )
        .unwrap();
        let legacy = |c: &str| c.ends_with("briefing test");
        let ours_or_legacy = |c: &str| ours(c) || legacy(c);
        assert!(matches!(
            wiring(&hooks, tmp.path(), &EVENTS, ours_or_legacy),
            Wiring::Wired { .. }
        ));
        assert!(stale(&hooks, &EVENTS, ours_or_legacy, legacy));

        std::fs::write(&hooks, "{ not json").unwrap();
        assert!(matches!(
            wiring(&hooks, tmp.path(), &EVENTS, ours),
            Wiring::Unavailable(_)
        ));
        assert!(!stale(&hooks, &EVENTS, ours, |_| false));
    }
}

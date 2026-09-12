//! Claude Code.
//!
//! The one client tower can wire through a directory it owns outright. A
//! plugin under `~/.claude/skills/tower/` auto-loads with no marketplace
//! and no install step, so install writes the directory whole and
//! uninstall removes it — none of the parse-preserve-foreign-entries
//! machinery a settings file needs, because there is no foreign content
//! in a directory that is entirely tower's. `--settings` is the escape
//! hatch back to settings entries if the plugin path ever misbehaves.
//!
//! The plugin carries the shipped skills too, under `skills/<name>/`,
//! which is the layout a plugin's own skills take and what makes them
//! `/tower:<name>` in a session. That is why the skills cost this adapter
//! nothing structural: four more files inside a directory that is written
//! whole and removed whole either way. `--settings` gets no skills,
//! because the skills ride the plugin.
//!
//! The migration from settings entries to the plugin is add-then-remove:
//! install the plugin, verify it, then strip the settings entries. The
//! other order leaves a window with no wiring at all; this one leaves a
//! window with the notice delivered twice, which is safe.

use std::path::PathBuf;

use super::{
    Change, InstallOptions, Integration, Mechanism, Presence, Status, Wiring, settings, skill,
};
use crate::error::CliError;
use settings::Need;

pub struct Claude;

/// The canonical hook command, and the spellings older installs carry.
/// A stored string is accepted forever: it sits in a file tower can only
/// rewrite when somebody runs the installer again, which they may never do.
const COMMAND: &str = "atc briefing claude";
const LEGACY: [&str; 0] = [];

/// The plugin bakes an absolute path, so recognizing our own wiring cannot
/// be an equality test: the binary moves, and a moved binary must still
/// read as wired rather than as gone.
const TAIL: &str = "briefing claude";

/// The one event tower wires: every context boundary Claude Code reports
/// — a fresh session, a resumed one, `/clear`, a compaction, a fork — is
/// where the notice has to be rebuilt, because the context it was in was
/// dropped or truncated. Delivery depends on it, so it is required.
const EVENTS: [(&str, Option<&str>, Need); 1] = [(
    "SessionStart",
    Some("startup|resume|clear|compact|fork"),
    Need::Required,
)];

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".claude"))
}

fn plugin_dir() -> Result<PathBuf, CliError> {
    Ok(config_dir()?.join("skills/tower"))
}

fn manifest_path() -> Result<PathBuf, CliError> {
    Ok(plugin_dir()?.join(".claude-plugin/plugin.json"))
}

fn hooks_path() -> Result<PathBuf, CliError> {
    Ok(plugin_dir()?.join("hooks/hooks.json"))
}

/// Where the plugin's skills live, which is where Claude Code looks for
/// them — the plugin directory is not itself a skill.
fn skills_root() -> Result<PathBuf, CliError> {
    Ok(plugin_dir()?.join("skills"))
}

fn skill_wiring() -> Wiring {
    match skills_root() {
        Ok(root) => skill::wiring(&root),
        Err(_) => Wiring::NotWired,
    }
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

fn is_ours(command: &str) -> bool {
    command.ends_with(TAIL) || LEGACY.contains(&command)
}

// ---- the plugin ------------------------------------------------------------

fn plugin_body() -> (String, String) {
    let command = super::exe_command("briefing claude");
    let manifest = serde_json::json!({
        "name": "tower",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "tower (atc) keeps this repository's board: flights for people and agents",
        "homepage": env!("CARGO_PKG_REPOSITORY"),
    });
    let mut events = serde_json::Map::new();
    for (event, matcher, _) in EVENTS {
        let mut entry = serde_json::Map::new();
        if let Some(matcher) = matcher {
            entry.insert("matcher".into(), matcher.into());
        }
        entry.insert(
            "hooks".into(),
            serde_json::json!([{ "type": "command", "command": command }]),
        );
        events.insert(
            event.to_string(),
            serde_json::Value::Array(vec![serde_json::Value::Object(entry)]),
        );
    }
    let hooks = serde_json::json!({ "hooks": serde_json::Value::Object(events) });
    (pretty(&manifest), pretty(&hooks))
}

fn pretty(value: &serde_json::Value) -> String {
    let mut body = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    body.push('\n');
    body
}

/// Which of `EVENTS` this hooks.json does not carry, in `EVENTS` order.
fn plugin_missing(value: &serde_json::Value) -> Vec<(&'static str, Need)> {
    EVENTS
        .iter()
        .filter(|(event, ..)| {
            !value["hooks"][*event].as_array().is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry["hooks"].as_array().is_some_and(|cmds| {
                        cmds.iter()
                            .any(|c| c["command"].as_str().is_some_and(is_ours))
                    })
                })
            })
        })
        .map(|(event, _, need)| (*event, *need))
        .collect()
}

/// Whether the plugin on disk is wired, read the way the client reads it.
fn plugin_wiring() -> Wiring {
    let Ok(path) = hooks_path() else {
        return Wiring::NotWired;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Wiring::NotWired;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Wiring::Unavailable(format!("{}: not valid JSON", path.display()));
    };
    let missing = plugin_missing(&value);
    let plugin = plugin_dir().unwrap_or_default();
    if missing.len() == EVENTS.len() {
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
            at: plugin,
        };
    }
    Wiring::Partial {
        missing: required.join(", "),
        at: plugin,
    }
}

/// Writes the plugin whole: manifest, hooks, and the four skills. Answers
/// whether any byte moved, so a second install and a refresh over a
/// current machine can say so instead of claiming a write.
fn write_plugin() -> Result<bool, CliError> {
    let (manifest, hooks) = plugin_body();
    let manifest_path = manifest_path()?;
    let hooks_path = hooks_path()?;
    let mut changed = false;
    for (path, body) in [(&manifest_path, &manifest), (&hooks_path, &hooks)] {
        if std::fs::read_to_string(path).ok().as_ref() == Some(body) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| super::failed(parent, err))?;
        }
        std::fs::write(path, body).map_err(|err| super::failed(path, err))?;
        changed = true;
    }
    let root = skills_root()?;
    if !matches!(skill::wiring(&root), Wiring::Wired { .. }) {
        skill::write_all(&root)?;
        changed = true;
    }
    Ok(changed)
}

fn remove_plugin() -> Result<bool, CliError> {
    let dir = plugin_dir()?;
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
    Ok(true)
}

// ---- the integration -------------------------------------------------------

impl Integration for Claude {
    fn slug(&self) -> &'static str {
        "claude"
    }

    fn detect(&self) -> Presence {
        match config_dir() {
            Ok(dir) if dir.is_dir() => Presence::Present { evidence: dir },
            _ => Presence::Absent,
        }
    }

    fn status(&self) -> Status {
        // The plugin is the mechanism, so it is read first; settings
        // entries are reported when they are what is actually there.
        let wiring = match plugin_wiring() {
            Wiring::NotWired => match spec() {
                Ok(spec) => settings::wiring(&spec),
                Err(err) => Wiring::Unavailable(err.to_string()),
            },
            other => other,
        };
        // Settings entries still deliver, so this is a repair to offer and
        // never an outage: the mechanism moved, and the wiring did not.
        let on_settings = matches!(
            &wiring,
            Wiring::Wired {
                mechanism: Mechanism::Settings,
                ..
            } | Wiring::Partial { .. }
        ) && !matches!(plugin_wiring(), Wiring::Wired { .. });
        let note = on_settings.then(|| {
            "`atc hook claude` moves it to the plugin, which loads on the client's next restart"
                .to_string()
        });
        // Being on the older mechanism is news, not a finding: it still
        // delivers, and moving costs a client restart. Only a retired
        // command spelling is stale; a skill that has drifted is its own
        // row, because it is its own repair.
        let stale = spec().map(|spec| settings::stale(&spec)).unwrap_or(false);
        Status {
            slug: self.slug(),
            presence: self.detect(),
            wiring,
            note,
            skill: Some(skill_wiring()),
            stale,
        }
    }

    fn install(&self, opts: &InstallOptions) -> Result<Change, CliError> {
        if opts.settings {
            let mut change = settings::install(&spec()?)?;
            if remove_plugin()? {
                change.absorb(Change::changed("removed the tower plugin"));
            }
            return Ok(change);
        }

        let written = write_plugin()?;
        // Verify before removing the other wiring: a plugin that did not
        // land must not take the settings entries down with it.
        let verified = plugin_wiring();
        if !matches!(verified, Wiring::Wired { .. }) {
            return Err(super::failed(
                &plugin_dir()?,
                format!(
                    "the plugin did not verify after the write ({}); settings entries left in place",
                    verified.word()
                ),
            ));
        }
        let mut change = if written {
            Change::changed(format!("plugin written to {}", plugin_dir()?.display()))
        } else {
            Change::unchanged(format!("already wired in {}", plugin_dir()?.display()))
        };
        // Now, and only now, the old wiring goes.
        let stripped = settings::uninstall(&spec()?)?;
        if stripped.changed {
            change.absorb(Change::changed(
                "moved off the settings entries it used to use",
            ));
        }
        if written {
            change.lines.push(format!(
                "skills written to {}: {}",
                skills_root()?.display(),
                skill::names()
            ));
            change.lines.push(
                "restart Claude Code to load it (`claude plugin list` shows it as tower@skills-dir)"
                    .into(),
            );
        }
        Ok(change)
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        // Remove both mechanisms: uninstall takes back exactly what install
        // ever added, and which of the two it added depends on when.
        let mut change = if remove_plugin()? {
            Change::changed(format!("removed {}", plugin_dir()?.display()))
        } else {
            Change::unchanged("no tower plugin installed")
        };
        let stripped = settings::uninstall(&spec()?)?;
        if stripped.changed {
            change.absorb(stripped);
        }
        Ok(change)
    }

    /// Repair in place: whichever mechanism this machine is already on
    /// stays. `atc hook -u` must never silently move somebody onto the
    /// plugin, because a running Claude Code will not load it until it
    /// restarts — the notice would go dark with nothing saying why.
    fn repair(&self) -> Result<Change, CliError> {
        let on_plugin = matches!(plugin_wiring(), Wiring::Wired { .. });
        self.install(&InstallOptions {
            settings: !on_plugin,
        })
    }

    /// Claude Code reads a `SessionStart` hook's stdout as context,
    /// verbatim.
    fn envelope(&self, text: &str) -> String {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_briefing_goes_out_as_plain_text() {
        assert_eq!(Claude.envelope("hello"), "hello");
    }

    /// Every event tower wires is in the plugin it writes, with its
    /// matcher — and the one event is the whole list.
    #[test]
    fn the_plugin_carries_every_event() {
        let (manifest, hooks) = plugin_body();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["name"], "tower");
        let value: serde_json::Value = serde_json::from_str(&hooks).unwrap();
        assert_eq!(
            value["hooks"].as_object().unwrap().len(),
            EVENTS.len(),
            "one event, no more: {value}"
        );
        for (event, matcher, _) in EVENTS {
            let entry = &value["hooks"][event][0];
            assert!(
                entry["hooks"][0]["command"].as_str().is_some_and(is_ours),
                "{event} runs tower: {value}"
            );
            assert_eq!(
                entry.get("matcher").and_then(serde_json::Value::as_str),
                matcher,
                "{event} matcher"
            );
        }
    }
}

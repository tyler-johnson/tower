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
//! The plugin carries the shipped skill too, under `skills/<name>/`,
//! which is the layout a plugin's own skills take and what makes it
//! `/tower:<name>` in a session. That is why the skill costs this adapter
//! nothing structural: the manual inside a directory that is written
//! whole and removed whole either way. `--settings` gets no skill,
//! because the skill rides the plugin.
//!
//! The migration from settings entries to the plugin is add-then-remove:
//! install the plugin, verify it, then strip the settings entries. The
//! other order leaves a window with no wiring at all; this one leaves a
//! window with the notice delivered twice, which is safe.

use std::path::PathBuf;

use super::{
    Change, InstallOptions, Integration, Mechanism, Presence, Status, Wiring, plugin, settings,
    skill,
};
use crate::error::CliError;
use settings::{Class, Event, Need};

pub struct Claude;

/// The canonical hook command, and the spellings older installs carry.
/// A stored string is accepted forever: it sits in a file tower can only
/// rewrite when somebody runs the installer again, which they may never do.
const COMMAND: &str = "atc trigger claude";
const LEGACY: [&str; 1] = ["atc briefing claude"];

/// The plugin bakes an absolute path, so recognizing our own wiring cannot
/// be an equality test: the binary moves, and a moved binary must still
/// read as wired rather than as gone. The legacy tail is the same rule
/// over the spelling an older plugin carries.
const TAIL: &str = "trigger claude";
const LEGACY_TAIL: &str = "briefing claude";

/// The events tower wires. `SessionStart` is every context boundary
/// Claude Code reports — a fresh session, a resumed one, `/clear`, a
/// compaction, a fork — where the notice has to be rebuilt, because the
/// context it was in was dropped or truncated; delivery depends on it,
/// so it is required. The rest keep the session's lease: a prompt, a
/// tool call, and the end of a turn are activity, and `SessionEnd` is
/// the release. They widen delivery rather than found it, so an install
/// without them is stale and never partial.
const EVENTS: [Event; 5] = [
    Event {
        name: "SessionStart",
        matcher: Some("startup|resume|clear|compact|fork"),
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
    Ok(super::home()?.join(".claude"))
}

fn plugin_dir() -> Result<PathBuf, CliError> {
    Ok(config_dir()?.join("skills").join("tower"))
}

fn manifest_path() -> Result<PathBuf, CliError> {
    Ok(plugin_dir()?.join(".claude-plugin").join("plugin.json"))
}

fn hooks_path() -> Result<PathBuf, CliError> {
    Ok(plugin_dir()?.join("hooks").join("hooks.json"))
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
    command.ends_with(TAIL) || is_legacy(command)
}

fn is_legacy(command: &str) -> bool {
    command.ends_with(LEGACY_TAIL)
}

// ---- the plugin ------------------------------------------------------------

/// The manifest and the hooks file. The reading of the hooks file is
/// `plugin.rs`, shared with Codex's plugin; the manifest and the
/// `.claude-plugin/` layout are this client's own.
fn plugin_body() -> (String, String) {
    let manifest = serde_json::json!({
        "name": "tower",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "tower (atc) keeps this repository's board: flights for people and agents",
        "homepage": env!("CARGO_PKG_REPOSITORY"),
    });
    let hooks = plugin::hooks_body(&EVENTS, &super::exe_command(TAIL));
    (plugin::pretty(&manifest), hooks)
}

/// Whether the plugin on disk is one an older tower wrote: a command in
/// the retired spelling, or an extra event missing while some event is
/// there. Either still delivers, so this is the repair `atc hook -u`
/// makes and never an outage.
fn plugin_stale() -> bool {
    let Ok(path) = hooks_path() else {
        return false;
    };
    plugin::stale(&path, &EVENTS, is_ours, is_legacy)
}

/// Whether the plugin on disk is wired, read the way the client reads it.
fn plugin_wiring() -> Wiring {
    let (Ok(path), Ok(dir)) = (hooks_path(), plugin_dir()) else {
        return Wiring::NotWired;
    };
    plugin::wiring(&path, &dir, &EVENTS, is_ours)
}

/// Writes the plugin whole: manifest, hooks, and the skill. Answers
/// whether any byte moved, so a second install and a refresh over a
/// current machine can say so instead of claiming a write.
fn write_plugin() -> Result<bool, CliError> {
    let (manifest, hooks) = plugin_body();
    let mut changed = plugin::write_if_changed(&manifest_path()?, &manifest)?;
    changed |= plugin::write_if_changed(&hooks_path()?, &hooks)?;
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
        // The plugin is the mechanism, so it is read first; settings
        // entries are reported when they are what is actually there.
        let wiring = match plugin_wiring() {
            Wiring::NotWired => match spec() {
                Ok(spec) => settings::wiring(&spec),
                Err(err) => Wiring::Unavailable {
                    complaint: err.to_string(),
                },
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
        // command spelling or a missing extra event is stale, on either
        // mechanism; a skill that has drifted is its own row, because it
        // is its own repair.
        let stale = plugin_stale() || spec().map(|spec| settings::stale(&spec)).unwrap_or(false);
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
    /// verbatim. The other events' hooks print nothing.
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
    /// matcher — the five, and no more.
    #[test]
    fn the_plugin_carries_every_event() {
        let (manifest, hooks) = plugin_body();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["name"], "tower");
        let value: serde_json::Value = serde_json::from_str(&hooks).unwrap();
        assert_eq!(EVENTS.len(), 5);
        assert_eq!(
            value["hooks"].as_object().unwrap().len(),
            EVENTS.len(),
            "five events, no more: {value}"
        );
        for event in EVENTS {
            let entry = &value["hooks"][event.name][0];
            assert!(
                entry["hooks"][0]["command"].as_str().is_some_and(is_ours),
                "{} runs tower: {value}",
                event.name
            );
            assert_eq!(
                entry.get("matcher").and_then(serde_json::Value::as_str),
                event.matcher,
                "{} matcher",
                event.name
            );
        }
        assert!(plugin::missing(&value, &EVENTS, is_ours).is_empty());
    }

    /// The retired spelling still reads as ours — an older plugin keeps
    /// delivering — and is what marks it stale.
    #[test]
    fn the_legacy_tail_is_ours_and_legacy() {
        assert!(is_ours("/usr/bin/atc briefing claude"));
        assert!(is_legacy("/usr/bin/atc briefing claude"));
        assert!(is_ours("/usr/bin/atc trigger claude"));
        assert!(!is_legacy("/usr/bin/atc trigger claude"));
        assert!(!is_ours("/usr/bin/other"));
    }
}

//! Cursor CLI 2026.09.10-fd3934a discovers user-local plugins without a marketplace registration. tower owns `~/.cursor/plugins/local/tower/` whole: the native manifest, flat hooks, and skills. The client reads them on a new session; the probe and captures are in `tests/fixtures/cursor/`.
//!
//! Plugin-only sessions fire sessionStart, preToolUse, and sessionEnd. This build gates beforeSubmitPrompt and stop on user/project settings even when plugin hooks exist, so those events are not wired. A resumed chat retains its context and skips sessionStart. Hooks run from the plugin directory and name the workspace in their payload; only the shell gets CURSOR_AGENT and CURSOR_CONVERSATION_ID.

use std::path::PathBuf;

use serde_json::{Map, Value};

use super::settings::{Class, Event, Need};
use super::{
    Change, InstallOptions, Integration, Mechanism, Presence, Status, Wiring, plugin, settings,
    skill,
};
use crate::error::CliError;

pub struct Cursor;

const TAIL: &str = "trigger cursor";
const NOTE: &str = "Cursor CLI loads local plugins in trusted workspaces; team policy must allow local plugin imports. User-local hooks are not available in cloud agents";
const EVENTS: [Event; 3] = [
    Event {
        name: "sessionStart",
        matcher: None,
        class: Class::Boundary,
        need: Need::Required,
    },
    Event {
        name: "preToolUse",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
    Event {
        name: "sessionEnd",
        matcher: None,
        class: Class::End,
        need: Need::Extra,
    },
];
const OLD_EVENTS: [Event; 1] = [EVENTS[0]];

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".cursor"))
}

fn plugin_dir() -> Result<PathBuf, CliError> {
    Ok(config_dir()?.join("plugins/local/tower"))
}

/// `cursor-agent` is the installer's unambiguous alias; `agent` is its primary name. The seam isolates tests from the developer's client.
fn binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ATC_CURSOR").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let paths = std::env::var_os("PATH")?;
    for name in [
        "cursor-agent",
        "cursor-agent.exe",
        "cursor-agent.cmd",
        "agent",
        "agent.exe",
        "agent.cmd",
    ] {
        if let Some(path) = std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|p| p.is_file())
        {
            return Some(path);
        }
    }
    None
}

fn old_spec() -> Result<settings::Spec, CliError> {
    Ok(settings::Spec {
        path: config_dir()?.join("hooks.json"),
        shape: settings::Shape::Flat,
        events: &OLD_EVENTS,
        command: "atc trigger cursor".into(),
        legacy: &["atc briefing cursor"],
        version: Some(1),
    })
}

fn bodies() -> (String, String) {
    let manifest = plugin::pretty(&serde_json::json!({
        "name": "tower", "version": env!("CARGO_PKG_VERSION"),
        "description": "tower (atc) keeps this repository's board: flights for people and agents",
        "homepage": env!("CARGO_PKG_REPOSITORY")
    }));
    let command = super::exe_command(TAIL);
    let mut hooks = Map::new();
    for event in EVENTS {
        hooks.insert(
            event.name.into(),
            serde_json::json!([{ "command": command }]),
        );
    }
    (
        manifest,
        plugin::pretty(&serde_json::json!({ "version": 1, "hooks": hooks })),
    )
}

fn has_event(hooks: &Value, event: &Event) -> bool {
    hooks["version"] == 1
        && hooks["hooks"][event.name]
            .as_array()
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry["command"]
                        .as_str()
                        .is_some_and(|cmd| cmd.ends_with(TAIL))
                })
            })
}

fn plugin_wiring() -> Result<Wiring, CliError> {
    let dir = plugin_dir()?;
    if !dir.exists() {
        return Ok(Wiring::NotWired);
    }
    let manifest = Value::Object(settings::load(&dir.join(".cursor-plugin/plugin.json"))?);
    let hooks = Value::Object(settings::load(&dir.join("hooks/hooks.json"))?);
    let mut missing = Vec::new();
    if manifest["name"] != "tower" || !manifest["version"].is_string() {
        missing.push("Cursor manifest");
    }
    for event in &EVENTS {
        if event.need == Need::Required && !has_event(&hooks, event) {
            missing.push(event.name);
        }
    }
    Ok(if missing.is_empty() {
        Wiring::Wired {
            mechanism: Mechanism::Plugin,
            at: dir,
        }
    } else {
        Wiring::Partial {
            missing: missing.join(", "),
            at: dir,
        }
    })
}

/// The verified plugin is already in place before migration runs. A malformed old file stays as found, with the failure reported.
fn strip_old(change: &mut Change) {
    match remove_old_settings() {
        Ok(stripped) if stripped.changed => change.absorb(stripped),
        Ok(_) => {}
        Err(err) => change
            .lines
            .push(format!("left ~/.cursor/hooks.json as found: {err}")),
    }
    if let Ok(root) = config_dir().map(|dir| dir.join("skills")) {
        match skill::remove_all(&root) {
            Ok(true) => change.absorb(Change::changed(format!(
                "removed old skills from {}: {}",
                root.display(),
                skill::names()
            ))),
            Ok(false) => {}
            Err(err) => change
                .lines
                .push(format!("left ~/.cursor/skills as found: {err}")),
        }
    }
}

fn remove_old_settings() -> Result<Change, CliError> {
    let spec = old_spec()?;
    let settings = settings::load(&spec.path)?;
    if let Some(hooks) = settings.get("hooks") {
        let hooks = hooks
            .as_object()
            .ok_or_else(|| super::malformed(&spec.path, "\"hooks\" is not an object"))?;
        if hooks
            .get("sessionStart")
            .is_some_and(|entries| !entries.is_array())
        {
            return Err(super::malformed(
                &spec.path,
                "\"sessionStart\" is not an array",
            ));
        }
    }
    if !settings::wiring(&spec).is_wired() {
        return Ok(Change::unchanged("no old Cursor settings wiring"));
    }
    settings::uninstall(&spec)
}

impl Integration for Cursor {
    fn slug(&self) -> &'static str {
        "cursor"
    }

    fn events(&self) -> &'static [Event] {
        &EVENTS
    }

    fn detect(&self) -> Presence {
        if let Ok(dir) = config_dir()
            && dir.is_dir()
        {
            return Presence::Present { evidence: dir };
        }
        binary().map_or(Presence::Absent, |evidence| Presence::Present { evidence })
    }

    fn status(&self) -> Status {
        let old = old_spec()
            .map(|spec| settings::wiring(&spec))
            .unwrap_or(Wiring::NotWired);
        let wiring = match plugin_wiring() {
            // Old settings must remain visible to `hook -u` so an upgrade migrates them.
            Ok(Wiring::NotWired) => old.clone(),
            Ok(wiring) => wiring,
            Err(err) => Wiring::Unavailable {
                complaint: err.to_string(),
            },
        };
        let skill = plugin_dir().map_or(Wiring::NotWired, |dir| skill::wiring(&dir.join("skills")));
        let stale = old.is_wired()
            || (wiring.is_wired() && matches!(skill, Wiring::NotWired))
            || plugin_dir().is_ok_and(|dir| {
                settings::load(&dir.join("hooks/hooks.json")).is_ok_and(|hooks| {
                    let hooks = Value::Object(hooks);
                    EVENTS.iter().any(|e| has_event(&hooks, e))
                        && EVENTS
                            .iter()
                            .any(|e| e.need == Need::Extra && !has_event(&hooks, e))
                })
            });
        Status {
            note: wiring.is_wired().then(|| NOTE.into()),
            slug: self.slug(),
            presence: self.detect(),
            wiring,
            stale,
            skill: Some(skill),
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let dir = plugin_dir()?;
        let (manifest, hooks) = bodies();
        let mut changed =
            plugin::write_if_changed(&dir.join(".cursor-plugin/plugin.json"), &manifest)?;
        changed |= plugin::write_if_changed(&dir.join("hooks/hooks.json"), &hooks)?;
        let skills = dir.join("skills");
        if !matches!(skill::wiring(&skills), Wiring::Wired { .. }) {
            skill::write_all(&skills)?;
            changed = true;
        }
        let verified = plugin_wiring()?;
        if !matches!(verified, Wiring::Wired { .. }) {
            return Err(super::failed(
                &dir,
                format!(
                    "the plugin did not verify after the write ({}); the old wiring is left in place",
                    verified.word()
                ),
            ));
        }
        let line = format!(
            "{} in {}; Cursor discovers it on the next session",
            if changed {
                "plugin and skills written"
            } else {
                "already wired"
            },
            dir.display()
        );
        let mut change = if changed {
            Change::changed(line)
        } else {
            Change::unchanged(line)
        };
        strip_old(&mut change);
        change.lines.push(NOTE.into());
        Ok(change)
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let dir = plugin_dir()?;
        let mut change = if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
            Change::changed(format!("removed {}", dir.display()))
        } else {
            Change::unchanged("no tower plugin installed")
        };
        strip_old(&mut change);
        Ok(change)
    }

    fn envelope(&self, text: &str) -> String {
        serde_json::json!({ "additional_context": text }).to_string()
    }
}

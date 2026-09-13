//! Copilot CLI's Agent Plugins 1.0 adapter. The dedicated `copilot/` namespace under `~/.agents/plugins/` keeps its manifest, hooks, and marketplace separate from Codex's incompatible formats. tower owns the plugin directory and marketplace file; registration is merged into the user's settings. Copilot loads local marketplace plugins live, including hand-written registrations (verified with 1.0.83).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::settings::{Class, Event, Need};
use super::{
    Change, InstallOptions, Integration, Mechanism, Presence, Status, Wiring, plugin, settings,
    skill,
};
use crate::error::CliError;

pub struct Copilot;

const MARKET: &str = "tower-atc";
const SELECTOR: &str = "tower@tower-atc";
const SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
const TAIL: &str = "trigger copilot";
const HOOKS: &str = "com.github.copilot/hooks/hooks.json";
const EVENTS: [Event; 5] = [
    Event {
        name: "sessionStart",
        matcher: None,
        class: Class::Boundary,
        need: Need::Required,
    },
    Event {
        name: "userPromptSubmitted",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
    Event {
        name: "preToolUse",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
    Event {
        name: "agentStop",
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

fn root() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".agents/plugins/copilot"))
}

fn config_dir() -> Result<PathBuf, CliError> {
    Ok(super::home()?.join(".copilot"))
}

/// A missing path under the seam means the client is absent, independent of the developer's PATH.
fn binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ATC_COPILOT").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        ["copilot", "copilot.exe", "copilot.cmd"]
            .into_iter()
            .map(|name| dir.join(name))
            .find(|p| p.is_file())
    })
}

fn bodies() -> (String, String) {
    let command = super::exe_command(TAIL);
    let mut hooks = Map::new();
    for event in EVENTS {
        hooks.insert(
            event.name.into(),
            serde_json::json!([{
                "type": "command", "bash": command,
                "env": { "ATC_HOOK_EVENT": event.name }, "timeoutSec": 30
            }]),
        );
    }
    let hooks = plugin::pretty(&serde_json::json!({"version": 1, "hooks": hooks}));
    let hash: String = Sha256::digest(hooks.as_bytes())
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect();
    let manifest = plugin::pretty(&serde_json::json!({
        "$schema": SCHEMA, "name": "tower",
        "version": format!("{}+atc.{hash}", env!("CARGO_PKG_VERSION")),
        "description": "tower (atc) keeps this repository's board: flights for people and agents",
        "homepage": env!("CARGO_PKG_REPOSITORY")
    }));
    (manifest, hooks)
}

fn marketplace() -> String {
    plugin::pretty(&serde_json::json!({
        "name": MARKET, "owner": {"name": "tower"},
        "plugins": [{"name": "tower", "source": "./tower"}]
    }))
}

fn registration(root: &Path) -> Value {
    serde_json::json!({"source": {"source": "directory", "path": root}})
}

/// Validate both maps before editing either. Malformed foreign settings are refused on install and uninstall alike.
fn load_settings(path: &Path) -> Result<Map<String, Value>, CliError> {
    let settings = settings::load(path)?;
    for key in ["enabledPlugins", "extraKnownMarketplaces"] {
        if settings.get(key).is_some_and(|value| !value.is_object()) {
            return Err(super::malformed(
                path,
                format!("\"{key}\" is not an object"),
            ));
        }
    }
    Ok(settings)
}

fn register(settings: &mut Map<String, Value>, root: &Path) {
    for (key, name, value) in [
        ("enabledPlugins", SELECTOR, Value::Bool(true)),
        ("extraKnownMarketplaces", MARKET, registration(root)),
    ] {
        settings
            .entry(key)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("validated settings map")
            .insert(name.into(), value);
    }
}

fn registered(settings: &Map<String, Value>, root: &Path) -> bool {
    settings.get("enabledPlugins").and_then(|v| v.get(SELECTOR)) == Some(&Value::Bool(true))
        && settings
            .get("extraKnownMarketplaces")
            .and_then(|v| v.get(MARKET))
            .is_some_and(|v| v["source"] == registration(root)["source"])
}

/// A command with the wrong event environment still runs, but cannot deliver the intended lifecycle event.
fn has_event(hooks: &Value, event: &Event) -> bool {
    hooks["version"] == 1
        && hooks["hooks"][event.name]
            .as_array()
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry["type"] == "command"
                        && entry["bash"].as_str().is_some_and(|c| c.ends_with(TAIL))
                        && entry["env"]["ATC_HOOK_EVENT"] == event.name
                })
            })
}

fn read_json(path: &Path) -> Result<Value, CliError> {
    settings::load(path).map(Value::Object)
}

fn wiring(root: &Path) -> Result<Wiring, CliError> {
    let dir = root.join("tower");
    let settings = load_settings(&config_dir()?.join("settings.json"))?;
    let has_registration = settings
        .get("enabledPlugins")
        .and_then(|v| v.get(SELECTOR))
        .is_some()
        || settings
            .get("extraKnownMarketplaces")
            .and_then(|v| v.get(MARKET))
            .is_some();
    if !dir.exists() && !root.join("marketplace.json").exists() && !has_registration {
        return Ok(Wiring::NotWired);
    }
    let manifest = read_json(&dir.join("plugin.json"))?;
    let hooks = read_json(&dir.join(HOOKS))?;
    let market = read_json(&root.join("marketplace.json"))?;
    let mut missing = Vec::new();
    if manifest["$schema"] != SCHEMA
        || manifest["name"] != "tower"
        || !manifest["version"].is_string()
    {
        missing.push("1.0 manifest");
    }
    if market["name"] != MARKET
        || !market["owner"]["name"].is_string()
        || !market["plugins"].as_array().is_some_and(|plugins| {
            plugins
                .iter()
                .any(|p| p["name"] == "tower" && p["source"] == "./tower")
        })
    {
        missing.push("marketplace entry");
    }
    if !registered(&settings, root) {
        missing.push("Copilot registration");
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

impl Integration for Copilot {
    fn slug(&self) -> &'static str {
        "copilot"
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
        let root = root();
        let wiring = root
            .as_ref()
            .map_err(|err| err.to_string())
            .and_then(|root| wiring(root).map_err(|err| err.to_string()))
            .unwrap_or_else(|complaint| Wiring::Unavailable { complaint });
        let skill = root.as_ref().map_or(Wiring::NotWired, |root| {
            skill::wiring(&root.join("tower/skills"))
        });
        let stale = root.as_ref().is_ok_and(|root| {
            read_json(&root.join("tower").join(HOOKS)).is_ok_and(|hooks| {
                EVENTS.iter().any(|e| has_event(&hooks, e))
                    && EVENTS
                        .iter()
                        .any(|e| e.need == Need::Extra && !has_event(&hooks, e))
            })
        });
        Status {
            slug: self.slug(),
            presence: self.detect(),
            wiring,
            note: None,
            skill: Some(skill),
            stale,
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let root = root()?;
        let dir = root.join("tower");
        let path = config_dir()?.join("settings.json");
        let before = load_settings(&path)?;
        let mut settings = before.clone();
        register(&mut settings, &root);
        let (manifest, hooks) = bodies();
        let mut changed = plugin::write_if_changed(&dir.join("plugin.json"), &manifest)?;
        changed |= plugin::write_if_changed(&dir.join(HOOKS), &hooks)?;
        let skills = dir.join("skills");
        if !matches!(skill::wiring(&skills), Wiring::Wired { .. }) {
            skill::write_all(&skills)?;
            changed = true;
        }
        changed |= plugin::write_if_changed(&root.join("marketplace.json"), &marketplace())?;
        if settings != before {
            settings::write(&path, &settings)?;
            changed = true;
        }
        let verified = wiring(&root)?;
        if !matches!(verified, Wiring::Wired { .. }) {
            return Err(super::failed(
                &dir,
                format!(
                    "the plugin did not verify after the write ({})",
                    verified.word()
                ),
            ));
        }
        let line = format!(
            "{} in {}; registered live as {SELECTOR}",
            if changed {
                "plugin and skills written"
            } else {
                "already wired"
            },
            dir.display()
        );
        Ok(if changed {
            Change::changed(line)
        } else {
            Change::unchanged(line)
        })
    }

    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let root = root()?;
        let path = config_dir()?.join("settings.json");
        let mut settings = load_settings(&path)?;
        let mut changed = false;
        for (key, name) in [
            ("enabledPlugins", SELECTOR),
            ("extraKnownMarketplaces", MARKET),
        ] {
            if let Some(map) = settings.get_mut(key).and_then(Value::as_object_mut) {
                changed |= map.remove(name).is_some();
            }
        }
        if changed {
            settings::write(&path, &settings)?;
        }
        let dir = root.join("tower");
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
            changed = true;
        }
        let market = root.join("marketplace.json");
        if market.exists() {
            std::fs::remove_file(&market).map_err(|err| super::failed(&market, err))?;
            changed = true;
        }
        Ok(if changed {
            Change::changed(format!(
                "removed {} and registration {SELECTOR}",
                dir.display()
            ))
        } else {
            Change::unchanged("no tower plugin installed")
        })
    }

    fn envelope(&self, text: &str) -> String {
        serde_json::json!({"additionalContext": text}).to_string()
    }
}

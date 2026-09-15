//! Copilot CLI's Agent Plugins 1.0 adapter. The dedicated `copilot/` namespace under `~/.agents/plugins/` keeps its manifest, hooks, and marketplace separate from Codex's incompatible formats. tower owns the plugin directory `copilot/tower/`; the marketplace file beside it is shared with fufu, which puts its own plugin under the same root. tower merges one entry, `{"name":"tower","source":"./tower"}`, into `plugins[]` and takes the file's `name` and `owner` when another tool created it, so the selector follows the file: `tower@tower-atc` in a marketplace tower wrote, `tower@fufu-ff` in one fufu did. Uninstall drops tower's entry and removes the file and the marketplace registration only when nothing else is listed. Registration is merged into the user's settings. Copilot loads local marketplace plugins live, including hand-written registrations (verified with 1.0.83).

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

/// The marketplace tower writes when there is none; a file another tool created keeps its own name, and the selector follows it.
const MARKET: &str = "tower-atc";
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

fn marketplace_path(root: &Path) -> PathBuf {
    root.join("marketplace.json")
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

fn marketplace_entry() -> Value {
    serde_json::json!({"name": "tower", "source": "./tower"})
}

/// The marketplace as a map: the file as found, or the one tower writes when there is none.
fn load_marketplace(root: &Path) -> Result<Map<String, Value>, CliError> {
    let mut market = settings::load(&marketplace_path(root))?;
    if !market.contains_key("name") {
        market.insert("name".into(), MARKET.into());
    }
    if !market.contains_key("owner") {
        market.insert("owner".into(), serde_json::json!({"name": "tower"}));
    }
    Ok(market)
}

/// The marketplace's own name, the half of `tower@<name>` the registration needs, read from the file as found.
fn market_name(root: &Path) -> String {
    settings::load(&marketplace_path(root))
        .ok()
        .and_then(|market| market.get("name")?.as_str().map(str::to_string))
        .unwrap_or_else(|| MARKET.to_string())
}

fn selector(name: &str) -> String {
    format!("tower@{name}")
}

/// tower's entry, replaced in place or appended; everything else as found.
fn merge_entry(path: &Path, market: &mut Map<String, Value>) -> Result<(), CliError> {
    let plugins = market
        .entry("plugins".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let plugins = plugins
        .as_array_mut()
        .ok_or_else(|| super::malformed(path, "\"plugins\" is not an array"))?;
    let entry = marketplace_entry();
    match plugins.iter_mut().find(|p| p["name"] == "tower") {
        Some(slot) => *slot = entry,
        None => plugins.push(entry),
    }
    Ok(())
}

/// Whether the marketplace lists tower's plugin the way tower writes it.
fn has_entry(market: &Value) -> bool {
    market["plugins"].as_array().is_some_and(|plugins| {
        plugins
            .iter()
            .any(|p| p["name"] == "tower" && p["source"] == "./tower")
    })
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

fn register(settings: &mut Map<String, Value>, root: &Path, name: &str) {
    for (key, entry, value) in [
        ("enabledPlugins", selector(name), Value::Bool(true)),
        (
            "extraKnownMarketplaces",
            name.to_string(),
            registration(root),
        ),
    ] {
        settings
            .entry(key)
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("validated settings map")
            .insert(entry, value);
    }
}

fn registered(settings: &Map<String, Value>, root: &Path, name: &str) -> bool {
    settings
        .get("enabledPlugins")
        .and_then(|v| v.get(selector(name)))
        == Some(&Value::Bool(true))
        && settings
            .get("extraKnownMarketplaces")
            .and_then(|v| v.get(name))
            .is_some_and(|v| v["source"] == registration(root)["source"])
}

/// Whether the settings name tower's plugin at all. The marketplace registration under `name` is not asked, since fufu's plugin shares it.
fn mentions(settings: &Map<String, Value>, name: &str) -> bool {
    settings
        .get("enabledPlugins")
        .and_then(|v| v.get(selector(name)))
        .is_some()
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
    let market_path = marketplace_path(root);
    let name = market_name(root);
    // The file alone is not tower's presence: fufu's plugin shares it.
    let listed = settings::load(&market_path).is_ok_and(|m| has_entry(&Value::Object(m)));
    if !dir.exists() && !listed && !mentions(&settings, &name) {
        return Ok(Wiring::NotWired);
    }
    let manifest = read_json(&dir.join("plugin.json"))?;
    let hooks = read_json(&dir.join(HOOKS))?;
    let market = read_json(&market_path)?;
    let mut missing = Vec::new();
    if manifest["$schema"] != SCHEMA
        || manifest["name"] != "tower"
        || !manifest["version"].is_string()
    {
        missing.push("1.0 manifest");
    }
    if !market["name"].is_string() || !has_entry(&market) {
        missing.push("marketplace entry");
    }
    if !registered(&settings, root, &name) {
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
        let market_path = marketplace_path(&root);
        let market_before = settings::load(&market_path)?;
        let mut market = load_marketplace(&root)?;
        merge_entry(&market_path, &mut market)?;
        let name = market["name"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| MARKET.to_string());
        let mut settings = before.clone();
        register(&mut settings, &root, &name);
        let (manifest, hooks) = bodies();
        let mut changed = plugin::write_if_changed(&dir.join("plugin.json"), &manifest)?;
        changed |= plugin::write_if_changed(&dir.join(HOOKS), &hooks)?;
        let skills = dir.join("skills");
        if !matches!(skill::wiring(&skills), Wiring::Wired { .. }) {
            skill::write_all(&skills)?;
            changed = true;
        }
        if market != market_before || !market_path.exists() {
            settings::write(&market_path, &market)?;
            changed = true;
        }
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
            "{} in {}; registered live as {}",
            if changed {
                "plugin and skills written"
            } else {
                "already wired"
            },
            dir.display(),
            selector(&name)
        );
        Ok(if changed {
            Change::changed(line)
        } else {
            Change::unchanged(line)
        })
    }

    /// Exactly what install wrote: the plugin directory, tower's entry in the marketplace, and the registration. The marketplace file and its `extraKnownMarketplaces` entry go only when nothing else is listed there, since fufu's plugin shares the root.
    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let root = root()?;
        let path = config_dir()?.join("settings.json");
        let mut settings = load_settings(&path)?;
        let market_path = marketplace_path(&root);
        let name = market_name(&root);
        let mut changed = false;
        let mut market_stays = false;
        if market_path.exists() {
            let mut market = settings::load(&market_path)?;
            if let Some(plugins) = market.get_mut("plugins").and_then(Value::as_array_mut) {
                let before = plugins.len();
                plugins.retain(|p| p["name"] != "tower");
                changed |= plugins.len() != before;
                market_stays = !plugins.is_empty();
            }
            if market_stays {
                settings::write(&market_path, &market)?;
            } else {
                std::fs::remove_file(&market_path)
                    .map_err(|err| super::failed(&market_path, err))?;
                changed = true;
            }
        }
        let mut settings_changed = false;
        if let Some(map) = settings
            .get_mut("enabledPlugins")
            .and_then(Value::as_object_mut)
        {
            settings_changed |= map.remove(&selector(&name)).is_some();
        }
        if !market_stays
            && let Some(map) = settings
                .get_mut("extraKnownMarketplaces")
                .and_then(Value::as_object_mut)
        {
            settings_changed |= map.remove(&name).is_some();
        }
        if settings_changed {
            settings::write(&path, &settings)?;
            changed = true;
        }
        let dir = root.join("tower");
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|err| super::failed(&dir, err))?;
            changed = true;
        }
        Ok(if changed {
            Change::changed(format!(
                "removed {} and registration {}",
                dir.display(),
                selector(&name)
            ))
        } else {
            Change::unchanged("no tower plugin installed")
        })
    }

    fn envelope(&self, text: &str) -> String {
        serde_json::json!({"additionalContext": text}).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The marketplace tower creates carries its own name and owner; one fufu created keeps both, and tower's entry joins its list.
    #[test]
    fn the_marketplace_entry_joins_a_foreign_file() {
        let path = Path::new("marketplace.json");
        let mut market = serde_json::json!({
            "name": "fufu-ff", "owner": {"name": "fufu"},
            "plugins": [{"name": "fufu", "source": "./fufu"}]
        })
        .as_object()
        .unwrap()
        .clone();
        merge_entry(path, &mut market).unwrap();
        assert_eq!(market["name"], "fufu-ff");
        assert_eq!(market["owner"]["name"], "fufu");
        assert_eq!(market["plugins"].as_array().unwrap().len(), 2);
        assert_eq!(market["plugins"][0]["source"], "./fufu");
        assert_eq!(market["plugins"][1]["source"], "./tower");
        assert!(has_entry(&Value::Object(market.clone())));
        merge_entry(path, &mut market).unwrap();
        assert_eq!(market["plugins"].as_array().unwrap().len(), 2, "in place");
        assert_eq!(selector("fufu-ff"), "tower@fufu-ff");

        let mut bad = serde_json::json!({"plugins": 1})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            merge_entry(path, &mut bad).unwrap_err().id(),
            "hook/malformed"
        );
    }
}

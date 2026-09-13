//! Per-machine declarations, carried from fufu at a44093cc. Recorded paths are evidence; execution resolves PATH afresh.

use crate::error::CliError;
use crate::manifest::{Handshake, Manifest};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub fn path() -> Option<PathBuf> {
    Some(
        config_root_from(std::env::consts::OS, |name| std::env::var_os(name))?
            .join("tower/extensions.json"),
    )
}

fn config_root_from(os: &str, env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let home =
        |suffix: &str| Some(PathBuf::from(env("HOME").filter(|v| !v.is_empty())?).join(suffix));
    match os {
        "macos" => home("Library/Application Support"),
        "windows" => env("APPDATA").filter(|v| !v.is_empty()).map(PathBuf::from),
        _ => env("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home(".config")),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Declared {
    pub manifest: Manifest,
    pub path: PathBuf,
    pub declared_at: i64,
}
impl Declared {
    pub fn name(&self) -> &str {
        &self.manifest.name
    }
    pub fn resolve(&self) -> Option<PathBuf> {
        crate::ext::resolve(self.name())
    }
}
#[derive(Debug, Serialize)]
pub struct Stale {
    pub name: String,
    pub contract: u32,
    pub version: Option<String>,
}
#[derive(Debug, Default, Serialize)]
pub struct Registry {
    #[serde(rename = "file")]
    pub path: Option<PathBuf>,
    #[serde(rename = "declared")]
    entries: Vec<Declared>,
    pub stale: Vec<Stale>,
    pub unreadable: Option<String>,
}
impl Registry {
    pub fn declared(&self) -> &[Declared] {
        &self.entries
    }
    pub fn get(&self, name: &str) -> Option<&Declared> {
        self.entries.iter().find(|entry| entry.name() == name)
    }
}

static CACHE: RwLock<Option<&'static Registry>> = RwLock::new(None);
pub fn read() -> &'static Registry {
    if let Some(registry) = *CACHE.read().unwrap_or_else(|e| e.into_inner()) {
        return registry;
    }
    let mut cache = CACHE.write().unwrap_or_else(|e| e.into_inner());
    if let Some(registry) = *cache {
        return registry;
    }
    let registry = Box::leak(Box::new(load(path().as_deref())));
    *cache = Some(registry);
    registry
}
fn invalidate() {
    *CACHE.write().unwrap_or_else(|e| e.into_inner()) = None;
}

pub fn load(file: Option<&Path>) -> Registry {
    let Some(file) = file else {
        return Registry::default();
    };
    let mut registry = Registry {
        path: Some(file.into()),
        ..Registry::default()
    };
    let records = match raw(file) {
        Ok(records) => records,
        Err(why) => {
            registry.unreadable = Some(why);
            return registry;
        }
    };
    for record in records {
        let name = record.manifest.get("name").and_then(|v| v.as_str());
        let contract = record.manifest.get("contract").and_then(|v| v.as_u64());
        let (Some(name), Some(contract)) = (name, contract) else {
            registry.entries.clear();
            registry.stale.clear();
            registry.unreadable = Some(
                "a record is missing its name or its contract, so it names no extension".into(),
            );
            return registry;
        };
        if contract != u64::from(atc_core::machine::CONTRACT) {
            registry.stale.push(Stale {
                name: name.into(),
                contract: contract.try_into().unwrap_or(u32::MAX),
                version: record.manifest["version"].as_str().map(str::to_string),
            });
            continue;
        }
        match crate::manifest::parse(record.manifest) {
            Ok(manifest) => registry.entries.push(Declared {
                manifest,
                path: record.path,
                declared_at: record.declared_at,
            }),
            Err(err) => {
                registry.entries.clear();
                registry.stale.clear();
                registry.unreadable = Some(err.to_string());
                return registry;
            }
        }
    }
    registry
}

pub fn declare(shook: &Handshake) -> Result<(), CliError> {
    declare_into(&writable()?, shook)?;
    invalidate();
    Ok(())
}
fn declare_into(file: &Path, shook: &Handshake) -> Result<(), CliError> {
    let mut records = for_writing(file)?;
    let fresh = Record {
        path: shook.path.clone(),
        declared_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        manifest: serde_json::to_value(&shook.manifest).expect("manifest serializes"),
    };
    if let Some(at) = records
        .iter()
        .position(|r| r.manifest["name"] == shook.manifest.name)
    {
        records[at] = fresh;
    } else {
        records.push(fresh);
    }
    write(file, &records)
}
pub fn forget(name: &str) -> Result<bool, CliError> {
    let forgotten = forget_from(&writable()?, name)?;
    if forgotten {
        invalidate();
    }
    Ok(forgotten)
}
fn forget_from(file: &Path, name: &str) -> Result<bool, CliError> {
    let mut records = for_writing(file)?;
    let before = records.len();
    records.retain(|record| record.manifest["name"] != name);
    if records.len() == before {
        return Ok(false);
    }
    write(file, &records)?;
    Ok(true)
}
fn writable() -> Result<PathBuf, CliError> {
    path().ok_or_else(|| CliError::coded("extension/registry-unwritable", "there is nowhere to record a declaration: nothing in the environment names a config directory", vec!["atc doctor".into()]))
}
fn for_writing(file: &Path) -> Result<Vec<Record>, CliError> {
    raw(file).map_err(|why| {
        CliError::coded(
            "extension/registry-unreadable",
            format!("{} is not a registry tower can read: {why}", file.display()),
            vec!["atc doctor".into()],
        )
    })
}
fn raw(file: &Path) -> Result<Vec<Record>, String> {
    let body = match std::fs::read(file) {
        Ok(body) => body,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.to_string()),
    };
    serde_json::from_slice::<File>(&body)
        .map(|file| file.extensions)
        .map_err(|err| err.to_string())
}
fn write(file: &Path, records: &[Record]) -> Result<(), CliError> {
    let failed = |err: std::io::Error| {
        CliError::coded(
            "extension/registry-unwritable",
            format!("{} could not be written: {err}", file.display()),
            vec!["atc doctor".into()],
        )
    };
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    let mut body = serde_json::to_string_pretty(&File {
        atc: atc_core::machine::CONTRACT,
        extensions: records.to_vec(),
    })
    .expect("registry serializes");
    body.push('\n');
    let tmp = file.with_extension("json.atc-tmp");
    std::fs::write(&tmp, body).map_err(failed)?;
    std::fs::rename(tmp, file).map_err(failed)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct File {
    atc: u32,
    extensions: Vec<Record>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record {
    path: PathBuf,
    declared_at: i64,
    manifest: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shook(name: &str) -> Handshake {
        Handshake {
            path: PathBuf::from(format!("/bin/atc-{name}")),
            manifest: crate::manifest::parse(serde_json::json!({
                "name": name, "version":"1.0.0", "contract":1,
                "verbs":[{"name":"go", "read_only":true}], "undoable":false,
                "future":{"kept":true}
            }))
            .unwrap(),
        }
    }
    fn file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tower/extensions.json");
        (dir, file)
    }
    #[test]
    fn absent_file_and_absent_config_are_empty_and_quiet() {
        let (_dir, path) = file();
        for registry in [load(None), load(Some(&path))] {
            assert!(registry.declared().is_empty());
            assert!(registry.stale.is_empty());
            assert!(registry.unreadable.is_none());
        }
    }
    #[test]
    fn declarations_keep_order_and_unknown_fields_through_replacement_and_removal() {
        let (_dir, file) = file();
        declare_into(&file, &shook("probe")).unwrap();
        declare_into(&file, &shook("other")).unwrap();
        let mut fresh = shook("probe");
        fresh.manifest.version = "2.0.0".into();
        declare_into(&file, &fresh).unwrap();
        let registry = load(Some(&file));
        assert_eq!(
            registry
                .declared()
                .iter()
                .map(Declared::name)
                .collect::<Vec<_>>(),
            ["probe", "other"]
        );
        assert_eq!(registry.get("probe").unwrap().manifest.version, "2.0.0");
        assert_eq!(
            registry.get("probe").unwrap().manifest.extra["future"]["kept"],
            true
        );
        assert!(registry.get("probe").unwrap().declared_at > 0);
        assert!(forget_from(&file, "probe").unwrap());
        assert!(!forget_from(&file, "probe").unwrap());
        assert_eq!(load(Some(&file)).declared()[0].name(), "other");
        let body = std::fs::read_to_string(&file).unwrap();
        assert!(body.starts_with("{\n  \"atc\": 1,"));
        assert!(body.ends_with("}\n"));
        assert_eq!(
            std::fs::read_dir(file.parent().unwrap()).unwrap().count(),
            1
        );
    }
    #[test]
    fn foreign_contracts_survive_writes_without_being_parsed_or_described() {
        let (_dir, file) = file();
        declare_into(&file, &shook("probe")).unwrap();
        let mut data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        data["atc"] = 99.into();
        data["extensions"][0]["manifest"]["contract"] = 99.into();
        data["extensions"][0]["manifest"]["verbs"] = "a future shape".into();
        std::fs::write(&file, data.to_string()).unwrap();
        declare_into(&file, &shook("other")).unwrap();
        let registry = load(Some(&file));
        assert_eq!(registry.declared().len(), 1);
        assert_eq!(registry.stale.len(), 1);
        assert_eq!(registry.stale[0].name, "probe");
        assert!(registry.unreadable.is_none());
        assert!(forget_from(&file, "probe").unwrap());
    }
    #[test]
    fn corrupt_files_fail_closed_and_are_not_overwritten() {
        for body in [
            "",
            "{",
            "[]",
            "{\"atc\":1}",
            "{\"atc\":1,\"extensions\":{}}",
        ] {
            let (_dir, file) = file();
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, body).unwrap();
            let registry = load(Some(&file));
            assert!(registry.declared().is_empty());
            assert!(registry.unreadable.is_some());
            assert_eq!(
                declare_into(&file, &shook("probe")).unwrap_err().id(),
                "extension/registry-unreadable"
            );
            assert_eq!(std::fs::read_to_string(&file).unwrap(), body);
        }
    }
    #[test]
    fn a_bad_record_costs_the_whole_read() {
        let (_dir, file) = file();
        declare_into(&file, &shook("probe")).unwrap();
        declare_into(&file, &shook("other")).unwrap();
        let original: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        for field in ["name", "contract", "verbs"] {
            let mut data = original.clone();
            data["extensions"][1]["manifest"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            std::fs::write(&file, data.to_string()).unwrap();
            let registry = load(Some(&file));
            assert!(registry.declared().is_empty());
            assert!(registry.unreadable.is_some());
        }
    }
    #[test]
    fn a_record_does_not_require_a_binary_on_path() {
        let (_dir, file) = file();
        declare_into(&file, &shook("nothing-on-path-answers-to-this")).unwrap();
        let registry = load(Some(&file));
        assert_eq!(registry.declared().len(), 1);
        assert!(registry.declared()[0].resolve().is_none());
    }
    #[test]
    fn config_roots_follow_the_platform_and_empty_values_fall_back() {
        let env = |key: &str| match key {
            "HOME" => Some("/home/u".into()),
            "XDG_CONFIG_HOME" => Some("/xdg".into()),
            "APPDATA" => Some("/roaming".into()),
            _ => None,
        };
        assert_eq!(config_root_from("linux", env), Some("/xdg".into()));
        assert_eq!(
            config_root_from("macos", env),
            Some("/home/u/Library/Application Support".into())
        );
        assert_eq!(config_root_from("windows", env), Some("/roaming".into()));
        assert_eq!(
            config_root_from("linux", |key| if key == "XDG_CONFIG_HOME" {
                Some("".into())
            } else {
                env(key)
            }),
            Some("/home/u/.config".into())
        );
        assert_eq!(config_root_from("linux", |_| None), None);
    }
}

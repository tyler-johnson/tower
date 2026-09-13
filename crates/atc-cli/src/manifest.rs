//! The declaring and skill handshakes, carried from fufu at a44093cc. Unknown fields survive recording.

use crate::error::CliError;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

pub const FLAG: &str = "--atc-manifest";
pub const SKILL_FLAG: &str = "--atc-skill";
pub const SKILL_FILE: &str = "SKILL.md";
const MAX_SKILL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub contract: u32,
    pub verbs: Vec<Verb>,
    pub undoable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub briefing: Option<Briefing>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tools: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<Update>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verb {
    pub name: String,
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Briefing {
    Line(String),
    Ask(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Build {
    Official,
    Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brew: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub releases: Option<String>,
}

impl Manifest {
    pub fn build(&self) -> Build {
        self.build.unwrap_or(Build::Official)
    }
}

#[derive(Debug)]
pub struct Handshake {
    pub path: PathBuf,
    pub manifest: Manifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillFile {
    pub path: PathBuf,
    pub content: String,
}

pub fn handshake(name: &str) -> Result<Handshake, CliError> {
    let path = crate::ext::resolve(name).ok_or_else(|| {
        CliError::coded(
            "extension/not-found",
            format!("no atc-{name} on PATH, so there is nothing to ask for a manifest"),
            vec!["atc doctor".into()],
        )
    })?;
    let manifest = ask(&path, name)?;
    Ok(Handshake { path, manifest })
}

/// Human-invoked handshakes have closed stdin and no time box, as in fufu.
fn answer(path: &Path, args: &[&str]) -> Result<serde_json::Value, String> {
    let output = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("ATC_NONINTERACTIVE", "1")
        .output()
        .map_err(|err| format!("it would not run: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!("it exited with {} and said nothing", output.status)
        } else {
            format!("it exited with {}: {}", output.status, stderr.trim())
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();
    let envelope = if !line.is_empty() && !line.contains('\n') {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .filter(|v| v.get("atc").is_some())
    } else {
        None
    };
    let envelope = envelope.ok_or_else(|| "its stdout is not one envelope on one line — a banner, a progress line, or a pretty-printed envelope costs it the handshake".to_string())?;
    if let Some(error) = envelope.get("error") {
        return Err(format!(
            "it answered with an error, {}",
            error["id"].as_str().unwrap_or("")
        ));
    }
    envelope
        .get("data")
        .cloned()
        .ok_or_else(|| "its envelope carries neither data nor error".into())
}

pub fn ask(path: &Path, name: &str) -> Result<Manifest, CliError> {
    let data = answer(path, &[FLAG]).map_err(|why| {
        CliError::coded(
            "extension/handshake-failed",
            format!("atc-{name} {FLAG} did not answer with a manifest: {why}"),
            vec![],
        )
    })?;
    let manifest = parse(data)?;
    accept(&manifest, name)?;
    Ok(manifest)
}

pub fn accept(manifest: &Manifest, name: &str) -> Result<(), CliError> {
    if manifest.contract != atc_core::machine::CONTRACT {
        return Err(CliError::coded(
            "extension/unsupported-contract",
            format!(
                "atc-{name} speaks contract {}, and this tower speaks {}",
                manifest.contract,
                atc_core::machine::CONTRACT
            ),
            vec![],
        ));
    }
    if manifest.name != name {
        return Err(CliError::coded(
            "extension/name-mismatch",
            format!(
                "atc-{name} calls itself `{}`, and a manifest names the binary tower resolved",
                manifest.name
            ),
            vec![],
        ));
    }
    Ok(())
}

pub fn parse(value: serde_json::Value) -> Result<Manifest, CliError> {
    let manifest: Manifest = serde_json::from_value(value).map_err(|err| bad(err.to_string()))?;
    if !crate::ext::valid_name(&manifest.name) {
        return Err(bad(format!(
            "`{}` is not a name an extension can have: ASCII letters and digits, `-` and `_`, starting with a letter or a digit",
            manifest.name
        )));
    }
    if manifest.version.is_empty() {
        return Err(bad("version is empty, so there is nothing to record"));
    }
    if manifest.verbs.is_empty() {
        return Err(bad(
            "verbs is empty: an extension tower will describe has to answer to at least one verb",
        ));
    }
    for verb in &manifest.verbs {
        if verb.name.is_empty() || verb.name.split_whitespace().count() != 1 {
            return Err(bad(format!(
                "`{}` is not one word, and a verb's name is one word",
                verb.name
            )));
        }
    }
    for skill in &manifest.skills {
        if !skill_name(&manifest.name, skill) {
            return Err(bad(format!(
                "`{skill}` is not a name a skill of atc-{name} can have: ASCII letters and digits, `-` and `_`, and either `{name}` itself or starting with `{name}-`",
                name = manifest.name
            )));
        }
    }
    if let Some(update) = &manifest.update {
        for (field, value) in [
            ("update.brew", &update.brew),
            ("update.install", &update.install),
            ("update.bin", &update.bin),
            ("update.releases", &update.releases),
        ] {
            if value.as_deref().is_some_and(|v| v.trim().is_empty()) {
                return Err(bad(format!(
                    "{field} is empty, and a recipe names something"
                )));
            }
        }
        if update.brew.is_none() && update.install.is_none() && update.releases.is_none() {
            return Err(bad(
                "update names no recipe: a block carries at least one of brew, install, and releases",
            ));
        }
        if update.bin.is_some() && update.install.is_none() {
            return Err(bad(
                "update.bin says where the install script places the binary, and only install carries one",
            ));
        }
    }
    Ok(manifest)
}

pub fn skill_name(name: &str, skill: &str) -> bool {
    crate::ext::valid_name(skill)
        && (skill == name
            || skill
                .strip_prefix(name)
                .and_then(|s| s.strip_prefix('-'))
                .is_some_and(|s| !s.is_empty()))
}

fn bad(why: impl std::fmt::Display) -> CliError {
    CliError::coded(
        "extension/bad-manifest",
        format!("that is not a manifest tower can read: {why}"),
        vec![],
    )
}

pub fn ask_skill(path: &Path, name: &str, skill: &str) -> Result<Vec<SkillFile>, CliError> {
    let data = answer(path, &[SKILL_FLAG, skill]).map_err(|why| {
        CliError::coded(
            "extension/skill-failed",
            format!("atc-{name} {SKILL_FLAG} {skill} did not answer with a skill: {why}"),
            vec!["atc doctor".into()],
        )
    })?;
    parse_skill(data)
}

pub fn parse_skill(value: serde_json::Value) -> Result<Vec<SkillFile>, CliError> {
    #[derive(Deserialize)]
    struct Files {
        files: Vec<SkillFile>,
    }
    let files = serde_json::from_value::<Files>(value)
        .map_err(|err| bad_skill(err.to_string()))?
        .files;
    if files.is_empty() {
        return Err(bad_skill(
            "files is empty: a skill is at least its SKILL.md",
        ));
    }
    let mut seen = Vec::new();
    let mut bytes = 0usize;
    for file in &files {
        let path = file.path.as_path();
        if path.as_os_str().is_empty()
            || !path.components().all(|p| matches!(p, Component::Normal(_)))
        {
            return Err(bad_skill(format!(
                "`{}` is not a path a skill's file can have: relative, with no `..` and no leading `.`, so it lands inside the skill's own directory",
                path.display()
            )));
        }
        if seen.contains(&path) {
            return Err(bad_skill(format!(
                "two files are at `{}`, and one path holds one file",
                path.display()
            )));
        }
        seen.push(path);
        bytes += file.content.len();
    }
    if !seen.contains(&Path::new(SKILL_FILE)) {
        return Err(bad_skill(
            "no SKILL.md at the root, and SKILL.md is the file a client reads a skill by",
        ));
    }
    if bytes > MAX_SKILL_BYTES {
        return Err(bad_skill(format!(
            "its files weigh {bytes} bytes together, and a skill is a manual: the cap is {MAX_SKILL_BYTES}"
        )));
    }
    Ok(files)
}

fn bad_skill(why: impl std::fmt::Display) -> CliError {
    CliError::coded(
        "extension/bad-skill",
        format!("that is not a skill tower can read: {why}"),
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn smallest() -> serde_json::Value {
        json!({"name":"probe","version":"1","contract":1,"verbs":[{"name":"go","read_only":true}],"undoable":false})
    }
    #[test]
    fn required_fields_and_defaults_match_fufus_contract() {
        let manifest = parse(smallest()).unwrap();
        assert!(manifest.briefing.is_none());
        assert!(manifest.skills.is_empty());
        assert!(!manifest.tools);
        assert_eq!(manifest.build(), Build::Official);
        assert!(manifest.update.is_none());
        let written = serde_json::to_value(manifest).unwrap();
        for field in ["briefing", "skills", "tools", "build", "update"] {
            assert!(written.get(field).is_none());
        }
        for field in ["name", "version", "contract", "verbs", "undoable"] {
            let mut value = smallest();
            value.as_object_mut().unwrap().remove(field);
            assert_eq!(parse(value).unwrap_err().id(), "extension/bad-manifest");
        }
    }
    #[test]
    fn worked_manifest_round_trips_every_supported_field_and_future_fields() {
        let mut value = smallest();
        value["briefing"] = json!(true);
        value["skills"] = json!(["probe", "probe-plan"]);
        value["tools"] = json!(true);
        value["build"] = json!("source");
        value["update"] = json!({"brew":"owner/tap/probe","install":"https://example.com/install.sh","bin":"~/.local/bin","releases":"https://github.com/owner/probe/releases/latest"});
        value["future"] = json!({"preserved": true});
        let manifest = parse(value.clone()).unwrap();
        assert_eq!(manifest.build(), Build::Source);
        assert!(matches!(manifest.briefing, Some(Briefing::Ask(true))));
        assert_eq!(serde_json::to_value(manifest).unwrap(), value);
    }
    #[test]
    fn malformed_names_verbs_skills_and_update_blocks_are_refused() {
        for (field, values) in [
            ("name", vec![json!("../escape"), json!(""), json!("-probe")]),
            ("version", vec![json!("")]),
            (
                "verbs",
                vec![
                    json!([]),
                    json!([{"name":"two words","read_only":true}]),
                    json!([{"name":"","read_only":true}]),
                ],
            ),
            (
                "skills",
                vec![
                    json!(["plan"]),
                    json!(["other-plan"]),
                    json!(["probeplan"]),
                    json!(["probe-"]),
                    json!(["../probe"]),
                ],
            ),
            (
                "update",
                vec![
                    json!({}),
                    json!({"bin":"~/.local/bin"}),
                    json!({"brew":""}),
                    json!({"install":"  "}),
                    json!({"brew":"probe","bin":"x"}),
                    json!({"install":"x","bin":""}),
                ],
            ),
            ("build", vec![json!("cargo"), json!(true)]),
            ("tools", vec![json!("yes")]),
        ] {
            for bad in values {
                let mut value = smallest();
                value[field] = bad;
                assert_eq!(
                    parse(value.clone()).unwrap_err().id(),
                    "extension/bad-manifest",
                    "{value}"
                );
            }
        }
        for skill in ["probe", "probe-plan", "probe-a_b-2"] {
            assert!(skill_name("probe", skill));
        }
    }
    #[test]
    fn contract_is_checked_before_resolved_name() {
        let mut manifest = parse(smallest()).unwrap();
        manifest.contract = 99;
        assert_eq!(
            accept(&manifest, "other").unwrap_err().id(),
            "extension/unsupported-contract"
        );
        manifest.contract = 1;
        assert_eq!(
            accept(&manifest, "other").unwrap_err().id(),
            "extension/name-mismatch"
        );
        accept(&manifest, "probe").unwrap();
    }
    #[test]
    fn skill_bundles_are_refused_whole_on_paths_shape_and_size() {
        for files in [
            json!([]),
            json!([{"path":"README.md","content":"x"}]),
            json!([{"path":"docs/SKILL.md","content":"x"}]),
            json!([{"path":"SKILL.md","content":42}]),
        ] {
            assert_eq!(
                parse_skill(json!({"files":files})).unwrap_err().id(),
                "extension/bad-skill"
            );
        }
        for path in [
            "../escape",
            "/absolute",
            "./SKILL.md",
            "a/../../b",
            "",
            "SKILL.md",
        ] {
            assert_eq!(
                parse_skill(
                    json!({"files":[{"path":"SKILL.md","content":"x"},{"path":path,"content":"y"}]})
                )
                .unwrap_err()
                .id(),
                "extension/bad-skill",
                "{path}"
            );
        }
        parse_skill(json!({"files":[{"path":"SKILL.md","content":"x".repeat(MAX_SKILL_BYTES)}]}))
            .unwrap();
        assert!(parse_skill(json!({"files":[{"path":"SKILL.md","content":"x".repeat(MAX_SKILL_BYTES / 2 + 1)},{"path":"more.md","content":"x".repeat(MAX_SKILL_BYTES / 2 + 1)}]})).is_err());
    }
}

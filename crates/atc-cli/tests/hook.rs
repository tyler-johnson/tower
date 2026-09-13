//! `atc hook` and `atc unhook` against a scratch home: the report, the
//! Claude plugin directory, the Codex plugin and its marketplace entry
//! with the migration off the settings file an older tower wrote, Qwen's
//! settings merge, OpenCode's plugin module, the shells' marked rc
//! lines, the refresh, and doctor's row per client.
//!
//! Every path here is env-redirected — HOME, USERPROFILE, the XDG roots,
//! ZDOTDIR, LOCALAPPDATA — so the suite never touches a real config
//! file. Client presence is faked by creating the client's config
//! directory, a shell's by creating its rc file; `SHELL` is scrubbed, so
//! the developer's login shell is not present in a fixture.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use atc_testsupport::{Repo, scrub};

/// The spawn: `home` is the scratch HOME, `cwd` where the verb runs,
/// `stdin` what it is fed (piped and closed either way, so nothing here
/// is a terminal and nothing may prompt).
fn atc(home: &Path, cwd: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    atc_env(home, cwd, args, stdin, &[])
}

/// The same spawn with more variables set — `ZDOTDIR`, a shell's own.
fn atc_env(
    home: &Path,
    cwd: &Path,
    args: &[&str],
    stdin: Option<&str>,
    env: &[(&str, &str)],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    command
        .args(args)
        .current_dir(cwd)
        .env("HOME", home)
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        // The update cache root forks to `LOCALAPPDATA` on Windows.
        .env("LOCALAPPDATA", home.join("cache"))
        // Nothing here spawns fufu, and nothing spawns Codex; the
        // developer's OpenCode, if any, is not on this machine either.
        .env("ATC_FF", "/nonexistent")
        .env("ATC_CODEX", "/nonexistent")
        .env("ATC_COPILOT", "/nonexistent")
        .env("ATC_CURSOR", "/nonexistent")
        .env("ATC_OPENCODE", "/nonexistent");
    // A test's own variables last, so one can open a seam the defaults
    // close.
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn atc");
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(text.as_bytes())
            .expect("write stdin");
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().expect("wait for atc")
}

fn text(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is utf-8")
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    text(out)
}

fn envelope(out: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("an envelope")
}

fn json_at(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn home() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

/// Every slug, in the order the listing walks them: the clients, then
/// the shells.
const SLUGS: [&str; 10] = [
    "claude",
    "codex",
    "qwen",
    "opencode",
    "copilot",
    "cursor",
    "bash",
    "zsh",
    "fish",
    "powershell",
];

/// The marker every rc line tower writes carries, and fufu's, which sits
/// in the same file and is not tower's to touch.
const MARKER: &str = "# tower — added by `atc hook`";
const FUFU: &str = "# fufu — added by `ff hook`";

/// Claude's table, in the order it is written; Codex and Qwen name the
/// same five.
const CLAUDE_EVENTS: [&str; 5] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "Stop",
    "SessionEnd",
];

/// The compiled skills, read back from the binary rather than the
/// source tree: `atc skills` is the store's shelf and never the
/// binary's, so the comparison source is the file the plugin writes for
/// a fresh install, against what a second install would write.
fn compiled(name: &str) -> String {
    let scratch = home();
    ok(&atc(
        scratch.path(),
        scratch.path(),
        &["hook", "claude"],
        None,
    ));
    std::fs::read_to_string(
        scratch
            .path()
            .join(".claude/skills/tower/skills")
            .join(name)
            .join("SKILL.md"),
    )
    .unwrap()
}

// ---- the report ------------------------------------------------------------

#[test]
fn the_list_reports_detected_clients_and_nothing_wired() {
    let home = home();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    std::fs::create_dir_all(home.path().join(".codex")).unwrap();

    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    for slug in SLUGS {
        assert!(listing.contains(slug), "every slug has a row: {listing:?}");
    }
    // Every row says not wired; the client column is what separates a
    // client that is here from one that is not. A shell is here when its
    // rc file is — none is — except PowerShell on Windows, which ships
    // with the OS.
    let lines: Vec<&str> = listing.lines().collect();
    assert_eq!(lines.len(), SLUGS.len(), "{listing:?}");
    let present = [
        true,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        cfg!(windows),
    ];
    for (line, present) in lines.iter().zip(present) {
        assert!(line.ends_with("not wired"), "{line:?}");
        assert_eq!(line.contains("not on this machine"), !present, "{line:?}");
    }

    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    assert!(out.status.success());
    let value = envelope(&out);
    assert_eq!(value["cmd"], "hook");
    let rows = value["data"]["integrations"].as_array().unwrap();
    assert_eq!(rows.len(), SLUGS.len(), "one row per slug: {rows:?}");
    assert_eq!(rows[0]["slug"], "claude");
    assert_eq!(rows[0]["wiring"]["state"], "not-wired");
    assert_eq!(rows[0]["presence"]["state"], "present");
    assert_eq!(rows[1]["slug"], "codex");
    assert_eq!(rows[1]["presence"]["state"], "present");
    assert_eq!(rows[2]["slug"], "qwen");
    assert_eq!(rows[2]["presence"]["state"], "absent");
    assert_eq!(rows[3]["slug"], "opencode");
    assert_eq!(rows[3]["presence"]["state"], "absent");
    assert_eq!(value["data"]["changed"], serde_json::json!([]));
}

/// Naming nothing where nothing may prompt reports and touches nothing.
/// The report is the useful half; acting without being asked is not.
#[test]
fn bare_hook_acts_on_nothing_when_it_cannot_ask() {
    let home = home();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_atc"))
        .args(["hook"])
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("XDG_CACHE_HOME", home.path().join("cache"))
        .env("LOCALAPPDATA", home.path().join("cache"))
        .env("ATC_FF", "/nonexistent")
        .env("ATC_CODEX", "/nonexistent")
        .env("ATC_COPILOT", "/nonexistent")
        .env("ATC_OPENCODE", "/nonexistent")
        .env("ATC_CURSOR", "/nonexistent")
        .env("ATC_NONINTERACTIVE", "1")
        .stdin(Stdio::null())
        .output()
        .expect("spawn atc");
    let listing = ok(&out);
    assert!(listing.contains("claude"), "reports: {listing:?}");
    assert!(
        listing.contains("name what you want: atc hook claude"),
        "teaches the explicit form: {listing:?}"
    );
    assert!(
        !home.path().join(".claude/skills/tower").exists(),
        "nothing was wired"
    );
}

/// A shell nobody supports is a usage error, and so is `shell` itself:
/// it is the trigger's source, not a slug, and nothing ever wrote it.
#[test]
fn unknown_slugs_are_hard_errors() {
    let home = home();
    for verb in ["hook", "unhook"] {
        for bad in ["tcsh", "shell"] {
            let out = atc(home.path(), home.path(), &["--json", verb, bad], None);
            assert_eq!(out.status.code(), Some(2), "{verb} {bad}: a usage error");
            let value = envelope(&out);
            assert_eq!(value["cmd"], verb);
            assert_eq!(value["error"]["id"], "usage/unknown-slug");
            let message = value["error"]["message"].as_str().unwrap();
            assert!(message.contains(bad), "{message}");
            assert!(
                message.contains("claude") && message.contains("bash"),
                "names the known ones: {message}"
            );
        }
    }
}

// ---- the cursor plugin -----------------------------------------------------

fn cursor_status(home: &Path) -> serde_json::Value {
    let out = atc(home, home, &["hook", "-l", "--json"], None);
    ok(&out);
    envelope(&out)["data"]["integrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slug"] == "cursor")
        .unwrap()
        .clone()
}

#[test]
fn cursor_round_trips_without_changing_foreign_hooks_plugins_or_settings() {
    let home = home();
    let home = home.path();
    let dir = home.join(".cursor/plugins/local/tower");
    let foreign = [
        (
            ".cursor/hooks.json",
            r#"{"version":1,"hooks":{"sessionStart":[{"command":"foreign"}]},"extra":true}"#,
        ),
        (".cursor/cli-config.json", r#"{"theme":"dark"}"#),
        (
            ".cursor/plugins/local/other/plugin.json",
            r#"{"name":"other"}"#,
        ),
        (
            ".agents/plugins/marketplace.json",
            r#"{"name":"mine","plugins":[]}"#,
        ),
    ];
    for (path, body) in foreign {
        let path = home.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
    let out = atc(home, home, &["hook", "cursor", "--json"], None);
    ok(&out);
    assert_eq!(
        envelope(&out)["data"]["changed"],
        serde_json::json!(["cursor"])
    );
    let row = cursor_status(home);
    assert_eq!(row["wiring"]["state"], "wired", "{row}");
    assert_eq!(row["wiring"]["mechanism"], "plugin");
    assert_eq!(row["skill"]["state"], "wired");
    assert!(row["note"].as_str().unwrap().contains("cloud agents"));
    assert!(row.get("stale").is_none());
    assert_eq!(
        json_at(&dir.join(".cursor-plugin/plugin.json"))["name"],
        "tower"
    );
    assert!(!dir.join("plugin.json").exists());
    let hooks = json_at(&dir.join("hooks/hooks.json"));
    assert_eq!(hooks["version"], 1);
    assert_eq!(hooks["hooks"].as_object().unwrap().len(), 3);
    for event in ["sessionStart", "preToolUse", "sessionEnd"] {
        let entries = hooks["hooks"][event].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        let command = entries[0]["command"].as_str().unwrap();
        assert!(command.ends_with("trigger cursor"), "{command}");
        assert!(
            command.len() > "atc trigger cursor".len(),
            "absolute binary path: {command}"
        );
        assert!(entries[0].get("hooks").is_none());
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("skills/tower/SKILL.md")).unwrap(),
        compiled("tower")
    );
    let hook_path = dir.join("hooks/hooks.json");
    let before = std::fs::metadata(&hook_path).unwrap().modified().unwrap();
    let out = atc(home, home, &["hook", "cursor", "--json"], None);
    ok(&out);
    assert_eq!(envelope(&out)["data"]["changed"], serde_json::json!([]));
    assert_eq!(
        std::fs::metadata(&hook_path).unwrap().modified().unwrap(),
        before
    );
    ok(&atc(home, home, &["unhook", "cursor"], None));
    assert!(!dir.exists());
    assert_eq!(cursor_status(home)["wiring"]["state"], "not-wired");
    let out = atc(home, home, &["unhook", "cursor", "--json"], None);
    ok(&out);
    assert_eq!(envelope(&out)["data"]["changed"], serde_json::json!([]));
    for (path, body) in foreign {
        assert_eq!(std::fs::read_to_string(home.join(path)).unwrap(), body);
    }
}

#[test]
fn cursor_refresh_migrates_both_stored_spellings_and_the_old_manual() {
    for spelling in ["atc trigger cursor", "atc briefing cursor"] {
        let home = home();
        let home = home.path();
        let path = home.join(".cursor/hooks.json");
        let old_skill = home.join(".cursor/skills/tower/SKILL.md");
        std::fs::create_dir_all(old_skill.parent().unwrap()).unwrap();
        std::fs::write(&old_skill, "old tower manual").unwrap();
        let foreign_skill = home.join(".cursor/skills/plan/SKILL.md");
        std::fs::create_dir_all(foreign_skill.parent().unwrap()).unwrap();
        std::fs::write(&foreign_skill, "my plan skill").unwrap();
        let seed = serde_json::json!({"version":1,"hooks":{
            "sessionStart":[{"command":spelling},{"command":"foreign"}],
            "afterFileEdit":[{"command":"formatter"}]
        },"extra":true});
        std::fs::write(&path, seed.to_string()).unwrap();
        let row = cursor_status(home);
        assert_eq!(row["wiring"]["mechanism"], "settings");
        assert_eq!(row["stale"], true);
        ok(&atc(home, home, &["hook", "-u"], None));
        assert_eq!(cursor_status(home)["wiring"]["mechanism"], "plugin");
        assert!(!old_skill.exists());
        assert_eq!(
            std::fs::read_to_string(&foreign_skill).unwrap(),
            "my plan skill"
        );
        let remaining = json_at(&path);
        assert_eq!(
            remaining["hooks"]["sessionStart"],
            serde_json::json!([{"command":"foreign"}])
        );
        assert_eq!(
            remaining["hooks"]["afterFileEdit"],
            seed["hooks"]["afterFileEdit"]
        );
        assert_eq!(remaining["extra"], true);
    }
}

#[test]
fn cursor_keeps_old_wiring_on_write_failure_and_reports_malformed_migration() {
    let home = home();
    let home = home.path();
    let dir = home.join(".cursor/plugins/local/tower");
    std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
    std::fs::write(&dir, "blocks plugin creation").unwrap();
    let old = home.join(".cursor/hooks.json");
    let seed = r#"{"version":1,"hooks":{"sessionStart":[{"command":"atc trigger cursor"}]}}"#;
    std::fs::write(&old, seed).unwrap();
    let out = atc(home, home, &["hook", "cursor", "--json"], None);
    assert!(!out.status.success());
    assert_eq!(std::fs::read_to_string(&old).unwrap(), seed);
    std::fs::remove_file(&dir).unwrap();
    for bad in ["{ broken", "[]", r#"{"hooks":false}"#] {
        std::fs::write(&old, bad).unwrap();
        let text = ok(&atc(home, home, &["hook", "cursor"], None));
        assert!(
            text.contains("left ~/.cursor/hooks.json as found"),
            "{text}"
        );
        assert_eq!(std::fs::read_to_string(&old).unwrap(), bad);
        assert_eq!(cursor_status(home)["wiring"]["state"], "wired");
        let text = ok(&atc(home, home, &["unhook", "cursor"], None));
        assert!(
            text.contains("left ~/.cursor/hooks.json as found"),
            "{text}"
        );
        assert!(!dir.exists());
        assert_eq!(std::fs::read_to_string(&old).unwrap(), bad);
    }
    std::fs::write(&old, seed).unwrap();
    ok(&atc(home, home, &["unhook", "cursor"], None));
    assert_eq!(
        cursor_status(home)["wiring"]["state"],
        "not-wired",
        "unhook also removes an unmigrated settings install"
    );
}

#[test]
fn cursor_repairs_missing_events_manifest_and_manual_and_doctor_agrees() {
    let repo = Repo::new();
    let home = repo.path().parent().unwrap();
    let dir = home.join(".cursor/plugins/local/tower");
    ok(&atc(home, home, &["hook", "cursor"], None));
    let path = dir.join("hooks/hooks.json");
    let hooks = json_at(&path);
    let doctor = || {
        envelope(&atc(home, repo.path(), &["doctor", "--json"], None))["data"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["check"] == "hook/cursor")
            .unwrap()
            .clone()
    };
    assert_eq!(doctor()["level"], "ok");
    for event in ["preToolUse", "sessionEnd", "sessionStart"] {
        let mut damaged = hooks.clone();
        damaged["hooks"].as_object_mut().unwrap().remove(event);
        std::fs::write(&path, damaged.to_string()).unwrap();
        let row = cursor_status(home);
        assert_eq!(
            row["wiring"]["state"],
            if event == "sessionStart" {
                "partial"
            } else {
                "wired"
            }
        );
        assert_eq!(doctor()["level"], "warn");
        ok(&atc(home, home, &["hook", "-u"], None));
        assert_eq!(json_at(&path), hooks);
        assert_eq!(doctor()["level"], "ok");
    }
    std::fs::remove_file(dir.join(".cursor-plugin/plugin.json")).unwrap();
    assert_eq!(cursor_status(home)["wiring"]["state"], "partial");
    ok(&atc(home, home, &["hook", "-u"], None));
    std::fs::write(dir.join("skills/tower/SKILL.md"), "old manual").unwrap();
    assert_eq!(doctor()["level"], "warn");
    ok(&atc(home, home, &["hook", "-u"], None));
    assert_eq!(doctor()["level"], "ok");
    std::fs::write(&path, "{ broken").unwrap();
    assert_eq!(cursor_status(home)["wiring"]["state"], "unavailable");
    ok(&atc(home, home, &["hook", "cursor"], None));
    assert_eq!(doctor()["level"], "ok");
    std::fs::remove_file(dir.join("skills/tower/SKILL.md")).unwrap();
    assert_eq!(doctor()["level"], "warn");
    ok(&atc(home, home, &["hook", "-u"], None));
    assert_eq!(cursor_status(home)["skill"]["state"], "wired");
    assert_eq!(doctor()["level"], "ok");
}

#[test]
fn cursor_is_detected_by_directory_or_binary() {
    let home = home();
    let home = home.path();
    assert_eq!(cursor_status(home)["presence"]["state"], "absent");
    let binary = home.join("cursor-agent");
    std::fs::write(&binary, "fake").unwrap();
    let out = atc_env(
        home,
        home,
        &["hook", "-l", "--json"],
        None,
        &[("ATC_CURSOR", binary.to_str().unwrap())],
    );
    ok(&out);
    let value = envelope(&out);
    let row = value["data"]["integrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["slug"] == "cursor")
        .unwrap();
    assert_eq!(row["presence"]["state"], "present");
    assert_eq!(row["presence"]["evidence"], binary.to_str().unwrap());
    std::fs::create_dir(home.join(".cursor")).unwrap();
    assert_eq!(cursor_status(home)["presence"]["state"], "present");
}

// ---- the copilot plugin ----------------------------------------------------

fn copilot_status(home: &Path) -> serde_json::Value {
    let out = atc(home, home, &["hook", "-l", "--json"], None);
    ok(&out);
    envelope(&out)["data"]["integrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slug"] == "copilot")
        .unwrap()
        .clone()
}

#[test]
fn the_copilot_plugin_round_trips_beside_codex_and_foreign_settings() {
    let home = home();
    let home = home.path();
    let dir = home.join(".agents/plugins/copilot/tower");
    let settings = home.join(".copilot/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let foreign = serde_json::json!({
        "theme": "dark", "enabledPlugins": {"other@mine": true},
        "extraKnownMarketplaces": {"mine": {"source": {"source": "directory", "path": "/mine"}}}
    });
    std::fs::write(&settings, foreign.to_string()).unwrap();
    ok(&atc(home, home, &["hook", "codex"], None));
    let codex_market = home.join(".agents/plugins/marketplace.json");
    let codex_manifest = home.join(".agents/plugins/tower/.codex-plugin/plugin.json");
    let codex_before = (
        std::fs::read(&codex_market).unwrap(),
        std::fs::read(&codex_manifest).unwrap(),
    );

    let out = atc(home, home, &["hook", "copilot", "--json"], None);
    ok(&out);
    assert_eq!(
        envelope(&out)["data"]["changed"],
        serde_json::json!(["copilot"])
    );
    let status = copilot_status(home);
    assert_eq!(status["wiring"]["state"], "wired", "{status}");
    assert_eq!(status["wiring"]["mechanism"], "plugin");
    assert_eq!(status["skill"]["state"], "wired");
    assert!(status.get("note").is_none(), "no trust step: {status}");
    let manifest = json_at(&dir.join("plugin.json"));
    assert_eq!(
        manifest["$schema"],
        "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json"
    );
    assert_eq!(manifest["name"], "tower");
    let version = manifest["version"].as_str().unwrap();
    let (pkg, hash) = version.split_once("+atc.").unwrap();
    assert_eq!(pkg, env!("CARGO_PKG_VERSION"));
    assert_eq!(hash.len(), 8);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!dir.join(".codex-plugin").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("skills/tower/SKILL.md")).unwrap(),
        compiled("tower")
    );
    let hooks = json_at(&dir.join("com.github.copilot/hooks/hooks.json"));
    assert_eq!(hooks["version"], 1);
    assert_eq!(hooks["hooks"].as_object().unwrap().len(), 5);
    for event in [
        "sessionStart",
        "userPromptSubmitted",
        "preToolUse",
        "agentStop",
        "sessionEnd",
    ] {
        let entries = hooks["hooks"][event].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["type"], "command");
        assert!(
            entries[0]["bash"]
                .as_str()
                .unwrap()
                .ends_with("trigger copilot")
        );
        assert_eq!(entries[0]["env"]["ATC_HOOK_EVENT"], event);
        assert_eq!(entries[0]["timeoutSec"], 30);
        assert!(entries[0].get("hooks").is_none());
    }
    let market = json_at(&home.join(".agents/plugins/copilot/marketplace.json"));
    assert_eq!(market["name"], "tower-atc");
    assert_eq!(market["owner"]["name"], "tower");
    assert_eq!(market["plugins"][0]["source"], "./tower");
    let registration = json_at(&settings);
    assert_eq!(registration["enabledPlugins"]["tower@tower-atc"], true);
    assert_eq!(
        registration["extraKnownMarketplaces"]["tower-atc"]["source"]["source"],
        "directory"
    );
    assert_eq!(
        registration["extraKnownMarketplaces"]["tower-atc"]["source"]["path"],
        home.join(".agents/plugins/copilot").display().to_string()
    );

    let before = std::fs::metadata(&settings).unwrap().modified().unwrap();
    let out = atc(home, home, &["hook", "copilot", "--json"], None);
    ok(&out);
    assert_eq!(envelope(&out)["data"]["changed"], serde_json::json!([]));
    assert_eq!(
        std::fs::metadata(&settings).unwrap().modified().unwrap(),
        before
    );
    ok(&atc(home, home, &["unhook", "copilot"], None));
    assert!(!dir.exists());
    assert!(
        !home
            .join(".agents/plugins/copilot/marketplace.json")
            .exists()
    );
    assert_eq!(json_at(&settings), foreign);
    assert_eq!(std::fs::read(&codex_market).unwrap(), codex_before.0);
    assert_eq!(std::fs::read(&codex_manifest).unwrap(), codex_before.1);
    assert_eq!(copilot_status(home)["wiring"]["state"], "not-wired");
    let out = atc(home, home, &["unhook", "copilot", "--json"], None);
    ok(&out);
    assert_eq!(envelope(&out)["data"]["changed"], serde_json::json!([]));
}

#[test]
fn copilot_refuses_malformed_settings_before_writing_or_removing_files() {
    for bad in [
        "{ broken",
        "[]",
        r#"{"enabledPlugins":false}"#,
        r#"{"extraKnownMarketplaces":[]}"#,
    ] {
        let home = home();
        let home = home.path();
        let settings = home.join(".copilot/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, bad).unwrap();
        let out = atc(home, home, &["--json", "hook", "copilot"], None);
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(envelope(&out)["error"]["id"], "hook/malformed");
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), bad);
        assert!(!home.join(".agents/plugins/copilot").exists());

        std::fs::write(&settings, "{}").unwrap();
        ok(&atc(home, home, &["hook", "copilot"], None));
        std::fs::write(&settings, bad).unwrap();
        assert_eq!(copilot_status(home)["wiring"]["state"], "unavailable");
        let out = atc(home, home, &["--json", "unhook", "copilot"], None);
        assert_eq!(envelope(&out)["error"]["id"], "hook/malformed");
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), bad);
        assert!(
            home.join(".agents/plugins/copilot/tower/plugin.json")
                .exists()
        );
    }
}

#[test]
fn copilot_repairs_missing_events_manifest_registration_and_skill() {
    let repo = Repo::new();
    let home = repo.path().parent().unwrap();
    let dir = home.join(".agents/plugins/copilot/tower");
    ok(&atc(home, home, &["hook", "copilot"], None));
    let path = dir.join("com.github.copilot/hooks/hooks.json");
    let hooks = json_at(&path);
    let doctor = || {
        envelope(&atc(home, repo.path(), &["doctor", "--json"], None))["data"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["check"] == "hook/copilot")
            .unwrap()
            .clone()
    };
    assert_eq!(doctor()["level"], "ok");
    let mut old = hooks.clone();
    old["hooks"].as_object_mut().unwrap().remove("preToolUse");
    std::fs::write(&path, old.to_string()).unwrap();
    let status = copilot_status(home);
    assert_eq!(status["wiring"]["state"], "wired");
    assert_eq!(status["stale"], true);
    assert_eq!(doctor()["level"], "warn");
    ok(&atc(home, home, &["hook", "-u"], None));
    assert_eq!(json_at(&path), hooks);

    for damage in ["event", "manifest", "marketplace", "registration", "skill"] {
        match damage {
            "event" => {
                let mut wrong = hooks.clone();
                wrong["hooks"]["sessionStart"][0]["env"]["ATC_HOOK_EVENT"] = "agentStop".into();
                std::fs::write(&path, wrong.to_string()).unwrap();
            }
            "manifest" => std::fs::remove_file(dir.join("plugin.json")).unwrap(),
            "marketplace" => {
                std::fs::remove_file(home.join(".agents/plugins/copilot/marketplace.json")).unwrap()
            }
            "registration" => std::fs::write(
                home.join(".copilot/settings.json"),
                r#"{"enabledPlugins":{"tower@tower-atc":false}}"#,
            )
            .unwrap(),
            "skill" => std::fs::write(dir.join("skills/tower/SKILL.md"), "old").unwrap(),
            _ => unreachable!(),
        }
        let status = copilot_status(home);
        if damage != "skill" {
            assert_eq!(status["wiring"]["state"], "partial", "{damage}: {status}");
        }
        assert_eq!(doctor()["level"], "warn", "{damage}");
        ok(&atc(home, home, &["hook", "-u"], None));
        assert_eq!(doctor()["level"], "ok", "{damage}");
    }
}

#[test]
fn copilot_is_detected_by_directory_or_binary() {
    let home = home();
    let home = home.path();
    assert_eq!(copilot_status(home)["presence"]["state"], "absent");
    let binary = home.join("copilot");
    std::fs::write(&binary, "fake").unwrap();
    let out = atc_env(
        home,
        home,
        &["hook", "-l", "--json"],
        None,
        &[("ATC_COPILOT", binary.to_str().unwrap())],
    );
    let value = envelope(&out);
    let row = value["data"]["integrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["slug"] == "copilot")
        .unwrap();
    assert_eq!(row["presence"]["state"], "present");
    std::fs::create_dir(home.join(".copilot")).unwrap();
    assert_eq!(copilot_status(home)["presence"]["state"], "present");
}

// ---- the claude plugin -----------------------------------------------------

#[test]
fn the_claude_plugin_round_trips() {
    let home = home();
    let plugin = home.path().join(".claude/skills/tower");

    let said = ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    assert!(said.contains("plugin written to"), "{said:?}");
    let manifest = json_at(&plugin.join(".claude-plugin/plugin.json"));
    assert_eq!(manifest["name"], "tower");
    let hooks = json_at(&plugin.join("hooks/hooks.json"));
    let events = hooks["hooks"].as_object().unwrap();
    assert_eq!(
        events.keys().collect::<Vec<_>>(),
        CLAUDE_EVENTS.to_vec(),
        "the five events, in order: {hooks}"
    );
    assert_eq!(
        hooks["hooks"]["SessionStart"][0]["matcher"],
        "startup|resume|clear|compact|fork"
    );
    for event in &CLAUDE_EVENTS[1..] {
        let entry = &hooks["hooks"][*event][0];
        assert!(
            entry.get("matcher").is_none(),
            "{event}: no matcher: {entry}"
        );
        // The binary's absolute path is baked in, so the plugin does not
        // depend on `atc` being on whatever PATH the client happens to
        // have.
        let command = entry["hooks"][0]["command"].as_str().unwrap();
        assert!(command.ends_with("trigger claude"), "{event}: {command:?}");
        assert!(
            command.len() > "atc trigger claude".len(),
            "absolute path baked in: {command:?}"
        );
    }
    assert!(!plugin.join(".mcp.json").exists(), "no server rides along");

    // The skill rides inside the plugin, under the layout a plugin's
    // own skills take, front matter first and byte for byte.
    let on_disk = std::fs::read_to_string(plugin.join("skills/tower/SKILL.md"))
        .expect("the tower skill lands with the plugin");
    assert!(
        on_disk.starts_with("---\nname: tower\n"),
        "front matter first"
    );
    assert_eq!(on_disk, compiled("tower"), "the compiled text");

    // Idempotent, and reported as already wired.
    let again = ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    assert!(again.contains("already wired in"), "{again:?}");
    let again = ok(&atc(
        home.path(),
        home.path(),
        &["--json", "hook", "claude"],
        None,
    ));
    let value: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(
        value["data"]["changed"],
        serde_json::json!([]),
        "nothing moved, so nothing is claimed"
    );
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(listing.contains("wired (plugin)"), "{listing:?}");
    assert!(
        listing.contains(", skill"),
        "the report says the skills are there: {listing:?}"
    );

    ok(&atc(home.path(), home.path(), &["unhook", "claude"], None));
    assert!(!plugin.exists(), "the directory tower owns goes whole");
}

/// The escape hatch back to settings entries buys the hook and nothing
/// else: the skills ride the plugin, so a machine on `--settings` has
/// none.
#[test]
fn the_settings_hatch_wires_the_event_and_no_skill() {
    let home = home();
    let skill = home
        .path()
        .join(".claude/skills/tower/skills/tower/SKILL.md");

    ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    assert!(skill.exists());

    ok(&atc(
        home.path(),
        home.path(),
        &["hook", "claude", "--settings"],
        None,
    ));
    assert!(!skill.exists(), "the plugin went, and the skills with it");
    let v = json_at(&home.path().join(".claude/settings.json"));
    assert_eq!(
        v["hooks"]["SessionStart"][0]["matcher"],
        "startup|resume|clear|compact|fork"
    );
    assert_eq!(
        v["hooks"].as_object().unwrap().keys().collect::<Vec<_>>(),
        CLAUDE_EVENTS.to_vec(),
        "the five events: {v}"
    );
    for event in CLAUDE_EVENTS {
        assert_eq!(
            v["hooks"][event][0]["hooks"][0]["command"], "atc trigger claude",
            "{event}"
        );
    }
}

/// The escape hatch: `--settings` wires the entries and removes the plugin,
/// which is the migration run backwards — and `hook claude` runs it
/// forwards again, stripping the entries only after the plugin verified.
#[test]
fn the_settings_escape_hatch_goes_back() {
    let home = home();
    let plugin = home.path().join(".claude/skills/tower");
    let settings = home.path().join(".claude/settings.json");

    ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    assert!(plugin.exists());

    ok(&atc(
        home.path(),
        home.path(),
        &["hook", "claude", "--settings"],
        None,
    ));
    assert!(!plugin.exists(), "the plugin went");
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(listing.contains("wired (settings)"), "{listing:?}");

    ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    assert!(plugin.join("hooks/hooks.json").exists());
    let v = json_at(&settings);
    assert!(v.get("hooks").is_none(), "settings entries stripped: {v}");
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(listing.contains("wired (plugin)"), "{listing:?}");
}

/// `atc unhook claude` takes back whatever install put there, whichever
/// mechanism it used — including a settings file left behind by a tower
/// old enough to predate the plugin.
#[test]
fn unhook_claude_removes_both_mechanisms() {
    let home = home();
    let settings = home.path().join(".claude/settings.json");

    ok(&atc(
        home.path(),
        home.path(),
        &["hook", "claude", "--settings"],
        None,
    ));
    // Plant a plugin beside it, as the add-then-remove window would.
    ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    std::fs::write(
        &settings,
        r#"{"hooks":{"SessionStart":[{"matcher":"startup","hooks":[{"type":"command","command":"atc trigger claude"}]}]}}"#,
    )
    .unwrap();

    ok(&atc(home.path(), home.path(), &["unhook", "claude"], None));
    assert!(!home.path().join(".claude/skills/tower").exists());
    let v = json_at(&settings);
    assert!(v.get("hooks").is_none(), "both mechanisms cleared: {v}");
}

// ---- the codex plugin ------------------------------------------------------

/// The plugin Codex reads through the personal marketplace: the legacy
/// manifest with a hashed version, the five events under one absolute
/// command, the manual, and the marketplace entry beside it; idempotent,
/// refreshed by `-u`, stale when an event is missing, and removed whole.
#[test]
fn the_codex_plugin_round_trips() {
    let home = home();
    let plugin = home.path().join(".agents/plugins/tower");
    let marketplace = home.path().join(".agents/plugins/marketplace.json");

    let said = ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    assert!(said.contains("plugin written to"), "{said:?}");
    assert!(said.contains("marketplace entry written to"), "{said:?}");
    assert!(
        said.contains("Codex is not on PATH — run: codex plugin add tower@tower"),
        "{said:?}"
    );
    assert!(said.contains("Codex trusts a hook by its hash"), "{said:?}");

    // The legacy manifest, and no root one: Codex loads a plugin's hooks
    // from the legacy manifest alone, and a root `$schema` manifest
    // beside it would win and drop them.
    let manifest = json_at(&plugin.join(".codex-plugin/plugin.json"));
    assert!(!plugin.join("plugin.json").exists(), "no root manifest");
    assert!(manifest.get("$schema").is_none(), "{manifest}");
    assert_eq!(manifest["name"], "tower");
    let version = manifest["version"].as_str().unwrap();
    let (_, suffix) = version.split_once("+atc.").expect("a hash suffix");
    assert_eq!(suffix.len(), 8, "{version}");
    assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()), "{version}");

    let hooks_path = plugin.join("hooks/hooks.json");
    let hooks = json_at(&hooks_path);
    assert_eq!(
        hooks["hooks"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        CLAUDE_EVENTS.to_vec(),
        "the five events, in order: {hooks}"
    );
    for event in CLAUDE_EVENTS {
        let entry = &hooks["hooks"][event][0];
        assert!(
            entry.get("matcher").is_none(),
            "{event}: no matcher: {entry}"
        );
        let command = entry["hooks"][0]["command"].as_str().unwrap();
        assert!(command.ends_with("trigger codex"), "{event}: {command:?}");
        assert!(
            command.len() > "atc trigger codex".len(),
            "absolute path baked in: {command:?}"
        );
    }
    assert!(!plugin.join("mcp.json").exists(), "no server rides along");
    let on_disk = std::fs::read_to_string(plugin.join("skills/tower/SKILL.md"))
        .expect("the tower skill lands with the plugin");
    assert_eq!(on_disk, compiled("tower"), "the compiled text");

    let market = json_at(&marketplace);
    assert_eq!(market["name"], "tower");
    let plugins = market["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["name"], "tower");
    assert_eq!(plugins[0]["source"]["source"], "local");
    assert_eq!(plugins[0]["source"]["path"], "./.agents/plugins/tower");
    assert_eq!(plugins[0]["policy"]["installation"], "INSTALLED_BY_DEFAULT");
    assert_eq!(plugins[0]["policy"]["authentication"], "ON_INSTALL");

    // Idempotent, and reported as already wired.
    let again = ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    assert!(again.contains("already wired in"), "{again:?}");
    assert!(!again.contains("marketplace entry written"), "{again:?}");
    let again = ok(&atc(
        home.path(),
        home.path(),
        &["--json", "hook", "codex"],
        None,
    ));
    let value: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(value["data"]["changed"], serde_json::json!([]));
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("codex")).unwrap();
    assert!(row.contains("wired (plugin)"), "{row:?}");
    assert!(row.contains(", skill"), "{row:?}");
    assert!(!row.contains("stale"), "{row:?}");
    assert!(
        listing.contains("Codex trusts a hook by its hash"),
        "the trust step is on the row: {listing:?}"
    );

    // -u over a current plugin moves nothing.
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("already wired in"), "{said:?}");
    assert!(!said.contains("rewired"), "{said:?}");

    // An extra event missing reads as stale, and -u restores the bytes.
    let current = std::fs::read_to_string(&hooks_path).unwrap();
    let mut fewer: serde_json::Value = serde_json::from_str(&current).unwrap();
    fewer["hooks"].as_object_mut().unwrap().remove("SessionEnd");
    std::fs::write(&hooks_path, serde_json::to_string_pretty(&fewer).unwrap()).unwrap();
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("codex")).unwrap();
    assert!(row.contains("wired (plugin)"), "{row:?}");
    assert!(row.contains("stale — atc hook -u rewrites it"), "{row:?}");
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&hooks_path).unwrap(), current);

    let said = ok(&atc(home.path(), home.path(), &["unhook", "codex"], None));
    assert!(said.contains("removed the tower entry from"), "{said:?}");
    assert!(said.contains("codex plugin remove tower@tower"), "{said:?}");
    assert!(!plugin.exists(), "the directory tower owns goes whole");
    let market = json_at(&marketplace);
    assert_eq!(market["name"], "tower", "the file keeps its name");
    assert_eq!(market["plugins"], serde_json::json!([]));
    let said = ok(&atc(home.path(), home.path(), &["unhook", "codex"], None));
    assert!(said.contains("no tower plugin installed"), "{said:?}");
}

/// The marketplace is a file tower does not own: a foreign plugin, the
/// file's own name, and its key order all survive hook and unhook, with
/// tower's entry appended then removed.
#[test]
fn a_foreign_marketplace_survives() {
    let home = home();
    let marketplace = home.path().join(".agents/plugins/marketplace.json");
    std::fs::create_dir_all(marketplace.parent().unwrap()).unwrap();
    let seed = serde_json::json!({
        "plugins": [{
            "name": "theirs",
            "source": { "source": "local", "path": "./.agents/plugins/theirs" },
            "policy": { "installation": "AVAILABLE" }
        }],
        "name": "mine",
        "metadata": { "description": "my plugins" }
    });
    std::fs::write(&marketplace, serde_json::to_string_pretty(&seed).unwrap()).unwrap();

    let said = ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    assert!(said.contains("codex plugin add tower@mine"), "{said:?}");
    let v = json_at(&marketplace);
    assert_eq!(v["name"], "mine");
    assert_eq!(v["metadata"]["description"], "my plugins");
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["plugins", "name", "metadata"], "{v}");
    let plugins = v["plugins"].as_array().unwrap();
    assert_eq!(plugins.len(), 2);
    assert_eq!(
        plugins[0], seed["plugins"][0],
        "the foreign entry, value for value"
    );
    assert_eq!(plugins[1]["name"], "tower");

    ok(&atc(home.path(), home.path(), &["unhook", "codex"], None));
    let v = json_at(&marketplace);
    assert_eq!(v["name"], "mine");
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["plugins", "name", "metadata"], "{v}");
    assert_eq!(v["plugins"], seed["plugins"]);
}

/// A marketplace that will not parse is refused, the plugin already
/// written — since the plugin delivers and the file is not tower's to
/// guess at.
#[test]
fn a_malformed_marketplace_is_refused_untouched() {
    let home = home();
    let marketplace = home.path().join(".agents/plugins/marketplace.json");
    std::fs::create_dir_all(marketplace.parent().unwrap()).unwrap();
    std::fs::write(&marketplace, "{ not json").unwrap();
    let out = atc(home.path(), home.path(), &["--json", "hook", "codex"], None);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(envelope(&out)["error"]["id"], "hook/malformed");
    assert_eq!(std::fs::read_to_string(&marketplace).unwrap(), "{ not json");
}

/// Add-then-remove: once the plugin has verified, what an older tower
/// wrote into Codex's settings file goes — tower's entries and nothing
/// beside them, in either spelling — and the skill directory beside
/// it; a file of the user's own under `~/.codex/skills` stays.
#[test]
fn the_migration_strips_the_old_codex_wiring() {
    let home = home();
    let codex = home.path().join(".codex/hooks.json");
    std::fs::create_dir_all(codex.parent().unwrap()).unwrap();
    let mut events = serde_json::Map::new();
    for (n, event) in CLAUDE_EVENTS.iter().enumerate() {
        let ours = if n == 0 {
            "atc briefing codex"
        } else {
            "atc trigger codex"
        };
        events.insert(
            (*event).into(),
            serde_json::json!([
                { "hooks": [{ "type": "command", "command": "ff trigger codex" }] },
                { "hooks": [{ "type": "command", "command": ours }] }
            ]),
        );
    }
    std::fs::write(
        &codex,
        serde_json::to_string_pretty(&serde_json::json!({ "hooks": events })).unwrap(),
    )
    .unwrap();
    let old_skill = home.path().join(".codex/skills/tower/SKILL.md");
    std::fs::create_dir_all(old_skill.parent().unwrap()).unwrap();
    std::fs::write(&old_skill, "---\nname: tower\ndescription: old\n---\n").unwrap();
    let theirs = home.path().join(".codex/skills/theirs/SKILL.md");
    std::fs::create_dir_all(theirs.parent().unwrap()).unwrap();
    std::fs::write(&theirs, "---\nname: theirs\n---\n").unwrap();

    // Before the plugin, the settings entries are not wiring the plugin
    // adapter reports — they deliver, and `atc hook codex` is the move.
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("codex")).unwrap();
    assert!(row.ends_with("not wired"), "{row:?}");

    let said = ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    assert!(said.contains("moved off ~/.codex/hooks.json"), "{said:?}");
    assert!(said.contains("removed ~/.codex/skills/tower"), "{said:?}");

    let v = json_at(&codex);
    for event in CLAUDE_EVENTS {
        let entries = v["hooks"][event].as_array().unwrap();
        assert_eq!(entries.len(), 1, "{event}: only fufu's stays: {v}");
        assert_eq!(entries[0]["hooks"][0]["command"], "ff trigger codex");
    }
    assert!(!old_skill.exists(), "the old skill directory goes");
    assert!(theirs.exists(), "a skill of the user's own stays");

    // The second run has nothing left to strip and says so by silence.
    let again = ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    assert!(!again.contains("moved off"), "{again:?}");
    assert!(!again.contains("removed ~/.codex"), "{again:?}");
}

// ---- the opencode plugin ---------------------------------------------------

/// The plugin module OpenCode loads from its config directory: one
/// file tower owns whole, the binary's path baked in as a string
/// literal, the three hooks, and the manual beside it; idempotent,
/// stale when a byte differs under tower's header, refreshed by `-u`,
/// refused when the file is someone else's, and removed whole with
/// nothing else in either directory touched.
#[test]
fn the_opencode_plugin_round_trips() {
    let home = home();
    let config = home.path().join("xdg").join("opencode");
    let plugin = config.join("plugins").join("tower.js");
    let skill = config.join("skills").join("tower").join("SKILL.md");

    let said = ok(&atc(home.path(), home.path(), &["hook", "opencode"], None));
    assert!(said.contains("plugin written to"), "{said:?}");
    assert!(said.contains("skills written to"), "{said:?}");
    assert!(said.contains("the notice is standing"), "{said:?}");
    assert!(said.contains("restart OpenCode to load it"), "{said:?}");

    let body = std::fs::read_to_string(&plugin).expect("the plugin file");
    assert!(
        body.starts_with("// Written by `atc hook opencode`."),
        "{body}"
    );
    let exe = serde_json::Value::String(env!("CARGO_BIN_EXE_atc").to_string()).to_string();
    assert!(
        body.contains(&format!("const ATC = {exe};")),
        "the binary's path as a JS string literal: {body}"
    );
    for needle in [
        "trigger opencode",
        "OPENCODE_SESSION_ID",
        "\"shell.env\"",
        "\"experimental.chat.system.transform\"",
        "\"tool.execute.before\"",
    ] {
        assert!(body.contains(needle), "{needle} in {body}");
    }
    assert!(!body.contains("__ATC__"), "{body}");
    assert_eq!(
        std::fs::read_to_string(&skill).expect("the skill lands beside it"),
        compiled("tower")
    );

    // Idempotent, and reported as already wired.
    let again = ok(&atc(home.path(), home.path(), &["hook", "opencode"], None));
    assert!(again.contains("already wired in"), "{again:?}");
    assert!(!again.contains("restart OpenCode"), "{again:?}");
    let again = ok(&atc(
        home.path(),
        home.path(),
        &["--json", "hook", "opencode"],
        None,
    ));
    let value: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(value["data"]["changed"], serde_json::json!([]));
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("opencode")).unwrap();
    assert!(row.contains("wired (plugin)"), "{row:?}");
    assert!(row.contains(", skill"), "{row:?}");
    assert!(!row.contains("stale"), "{row:?}");
    assert!(
        listing.contains("the notice is standing"),
        "the standing notice is on the row: {listing:?}"
    );

    // -u over a current plugin moves nothing.
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("already wired in"), "{said:?}");
    assert!(!said.contains("rewired"), "{said:?}");

    // A byte changed under tower's header is stale, and -u restores it.
    std::fs::write(&plugin, format!("{body}\n// one more byte\n")).unwrap();
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("opencode")).unwrap();
    assert!(row.contains("wired (plugin)"), "{row:?}");
    assert!(row.contains("stale — atc hook -u rewrites it"), "{row:?}");
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    assert_eq!(rows[3]["slug"], "opencode");
    assert_eq!(rows[3]["stale"], true, "{rows}");
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&plugin).unwrap(), body);

    // A tower.js without the header is someone else's: reported, the
    // install refused, and unhook leaves it.
    let foreign = "export const Mine = async () => ({});\n";
    std::fs::write(&plugin, foreign).unwrap();
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("opencode")).unwrap();
    assert!(row.contains("written by hand — left alone"), "{row:?}");
    let out = atc(
        home.path(),
        home.path(),
        &["--json", "hook", "opencode"],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    let value = envelope(&out);
    assert_eq!(value["error"]["id"], "hook/failed");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not tower's file"),
        "{value}"
    );
    assert_eq!(std::fs::read_to_string(&plugin).unwrap(), foreign);
    let said = ok(&atc(
        home.path(),
        home.path(),
        &["unhook", "opencode"],
        None,
    ));
    assert!(said.contains("is not tower's — left alone"), "{said:?}");
    assert_eq!(
        std::fs::read_to_string(&plugin).unwrap(),
        foreign,
        "unhook leaves what tower did not write"
    );
    // The -u pass skips it: not wired, nothing to rewrite.
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(!said.contains("opencode"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&plugin).unwrap(), foreign);
    assert!(
        !skill.exists(),
        "the skill is tower's whatever sits under the plugin's name, and unhook took it"
    );

    // Back to tower's — the plugin file restored, the skill rewritten by
    // the install — with a neighbor in each directory: unhook takes
    // exactly the two paths.
    std::fs::write(&plugin, &body).unwrap();
    let said = ok(&atc(home.path(), home.path(), &["hook", "opencode"], None));
    assert!(said.contains("plugin written to"), "{said:?}");
    assert!(skill.is_file());
    let other = config.join("plugins/other.js");
    std::fs::write(&other, "export const Other = async () => ({});\n").unwrap();
    let mine = config.join("skills/mine/SKILL.md");
    std::fs::create_dir_all(mine.parent().unwrap()).unwrap();
    std::fs::write(&mine, "---\nname: mine\n---\n").unwrap();
    let said = ok(&atc(
        home.path(),
        home.path(),
        &["unhook", "opencode"],
        None,
    ));
    assert!(
        said.contains(&format!("removed {}", plugin.display())),
        "{said:?}"
    );
    assert!(
        said.contains(&format!(
            "removed {}",
            config.join("skills").join("tower").display()
        )),
        "{said:?}"
    );
    assert!(!plugin.exists());
    assert!(!config.join("skills/tower").exists());
    assert!(other.is_file(), "a neighbor plugin stays");
    assert!(mine.is_file(), "a neighbor skill stays");
    let said = ok(&atc(
        home.path(),
        home.path(),
        &["unhook", "opencode"],
        None,
    ));
    assert!(said.contains("no tower plugin installed"), "{said:?}");
}

/// OpenCode is present when its config directory is, or when the
/// binary is — `ATC_OPENCODE` names it, the seam the harness closes
/// with a path that is not a file.
#[test]
fn opencode_is_detected_by_its_directory_or_its_binary() {
    let home = home();
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("opencode")).unwrap();
    assert!(row.contains("not on this machine"), "{row:?}");

    let binary = home.path().join("bin/opencode");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(&binary, "#!/bin/sh\n").unwrap();
    let out = atc_env(
        home.path(),
        home.path(),
        &["--json", "hook", "-l"],
        None,
        &[("ATC_OPENCODE", binary.to_str().unwrap())],
    );
    let rows = envelope(&out)["data"]["integrations"].clone();
    assert_eq!(rows[3]["slug"], "opencode");
    assert_eq!(rows[3]["presence"]["state"], "present", "{rows}");
    assert_eq!(
        rows[3]["presence"]["evidence"],
        binary.to_str().unwrap(),
        "{rows}"
    );

    std::fs::create_dir_all(home.path().join("xdg/opencode")).unwrap();
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    assert_eq!(rows[3]["presence"]["state"], "present", "{rows}");
    assert_eq!(
        rows[3]["presence"]["evidence"],
        home.path().join("xdg").join("opencode").to_str().unwrap(),
        "the directory is the evidence when it is there: {rows}"
    );
}

/// Gemini is an ordinary unknown name to the hook and its file stays as found.
#[test]
fn gemini_is_an_unknown_slug_and_its_file_is_left() {
    let home = home();
    let gemini = home.path().join(".gemini/settings.json");
    std::fs::create_dir_all(gemini.parent().unwrap()).unwrap();
    let gemini_seed = r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"atc briefing gemini"}]}]}}"#;
    std::fs::write(&gemini, gemini_seed).unwrap();

    for verb in ["hook", "unhook"] {
        let out = atc(home.path(), home.path(), &["--json", verb, "gemini"], None);
        assert_eq!(out.status.code(), Some(2), "{verb} gemini");
        let value = envelope(&out);
        assert_eq!(value["error"]["id"], "usage/unknown-slug");
        let message = value["error"]["message"].as_str().unwrap();
        assert!(message.contains("unknown client or shell"), "{message}");
        assert!(message.contains("codex"), "names the known: {message}");
    }
    std::fs::create_dir_all(home.path().join(".codex")).unwrap();
    ok(&atc(home.path(), home.path(), &["hook", "--all"], None));
    assert_eq!(std::fs::read_to_string(&gemini).unwrap(), gemini_seed);
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(!listing.contains("gemini"), "{listing:?}");
}

// ---- qwen ------------------------------------------------------------------

/// Qwen Code takes the family's five events in its own settings file,
/// no matcher, no skills directory.
#[test]
fn qwen_is_wired_in_its_settings_file() {
    let home = home();
    ok(&atc(home.path(), home.path(), &["hook", "qwen"], None));
    let qwen = json_at(&home.path().join(".qwen/settings.json"));
    assert_eq!(
        qwen["hooks"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        CLAUDE_EVENTS.to_vec(),
        "Qwen names the family's five: {qwen}"
    );
    for event in CLAUDE_EVENTS {
        let entry = &qwen["hooks"][event][0];
        assert_eq!(entry["hooks"][0]["command"], "atc trigger qwen", "{event}");
        assert_eq!(entry["hooks"][0]["type"], "command");
        assert!(
            entry.get("matcher").is_none(),
            "{event}: no matcher: {qwen}"
        );
    }
    assert!(!home.path().join(".qwen/skills").exists());
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("qwen")).unwrap();
    assert!(row.contains("wired (settings)"), "{row:?}");
    assert!(!row.contains(", skill"), "{row:?}");

    ok(&atc(home.path(), home.path(), &["unhook", "qwen"], None));
    let v = json_at(&home.path().join(".qwen/settings.json"));
    assert!(v.get("hooks").is_none(), "the entries went: {v}");
}

#[test]
fn install_preserves_foreign_content() {
    let home = home();
    let settings = home.path().join(".qwen/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let foreign = serde_json::json!({
        "model": "qwen3",
        "hooks": {
            "SessionStart": [
                { "hooks": [{ "type": "command", "command": "my-banner" }] }
            ],
            "Stop": [
                { "hooks": [{ "type": "command", "command": "notify-send done" }] }
            ]
        },
        "env": { "FOO": "bar" }
    });
    std::fs::write(&settings, serde_json::to_string_pretty(&foreign).unwrap()).unwrap();

    ok(&atc(home.path(), home.path(), &["hook", "qwen"], None));
    let v = json_at(&settings);
    assert_eq!(v["model"], "qwen3", "foreign top-level fields preserved");
    assert_eq!(v["env"]["FOO"], "bar");
    assert_eq!(
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"], "my-banner",
        "foreign hook entries preserved value-identical"
    );
    assert_eq!(
        v["hooks"]["Stop"][0]["hooks"][0]["command"], "notify-send done",
        "the foreign Stop entry stays first"
    );
    assert_eq!(
        v["hooks"]["Stop"][1]["hooks"][0]["command"], "atc trigger qwen",
        "ours appended under the event the foreign one already held"
    );
    assert_eq!(
        v["hooks"]["SessionStart"][1]["hooks"][0]["command"], "atc trigger qwen",
        "our entry appended after foreign ones"
    );
    // The user's key order survives the round trip.
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["model", "hooks", "env"], "{v}");

    // Uninstall removes only ours.
    ok(&atc(home.path(), home.path(), &["unhook", "qwen"], None));
    let v = json_at(&settings);
    assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        "my-banner"
    );
    assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(
        v["hooks"]["Stop"][0]["hooks"][0]["command"],
        "notify-send done"
    );
    assert!(v["hooks"].get("SessionEnd").is_none(), "{v}");
    assert_eq!(v["model"], "qwen3");
}

#[test]
fn install_refuses_malformed_files_untouched() {
    let home = home();
    let settings = home.path().join(".qwen/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();

    for bad in [
        "{ not json",
        "[1, 2, 3]",
        r#"{ "hooks": "not an object" }"#,
        r#"{ "hooks": { "SessionStart": "not an array" } }"#,
    ] {
        std::fs::write(&settings, bad).unwrap();
        let out = atc(home.path(), home.path(), &["--json", "hook", "qwen"], None);
        assert_eq!(out.status.code(), Some(1), "must refuse: {bad}");
        assert_eq!(envelope(&out)["error"]["id"], "hook/malformed");
        assert_eq!(
            std::fs::read_to_string(&settings).unwrap(),
            bad,
            "file untouched on refusal"
        );
    }
}

// ---- atc hook -u -----------------------------------------------------------

#[test]
fn update_rewrites_only_what_is_wired() {
    let home = home();

    // Nothing wired: says so, creates nothing.
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("nothing is wired"), "{said:?}");
    assert!(std::fs::read_dir(home.path()).unwrap().next().is_none());

    // A client present is still not a client wired.
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(!home.path().join(".claude/skills").exists());

    // Wired and current: -u says nothing changed.
    ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("already wired in"), "{said:?}");
    assert!(!said.contains("rewired"), "{said:?}");

    // Wired, then drifted by hand: -u restores the bytes.
    let skill = home
        .path()
        .join(".claude/skills/tower/skills/tower/SKILL.md");
    let shipped = std::fs::read_to_string(&skill).unwrap();
    std::fs::write(&skill, "an older tower wrote this").unwrap();
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), shipped);

    // A skill an older tower shipped and this one does not, recognized
    // by the front matter it shipped with: doctor names the repair, and
    // -u makes it by removing the directory.
    let work = home.path().join(".claude/skills/tower/skills/work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(
        work.join("SKILL.md"),
        "---\nname: work\ndescription: claim, do, hold or commit, repeat — the loop that pairs with `atc next`\n---\n# work\n",
    )
    .unwrap();
    let repo = Repo::new();
    repo.pin_writer("pi");
    let out = atc(home.path(), repo.path(), &["doctor", "--json"], None);
    let rows = envelope(&out)["data"]["rows"].clone();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["check"] == "hook/claude")
        .cloned()
        .unwrap_or_else(|| panic!("a hook/claude row: {rows}"));
    assert_eq!(row["level"], "warn", "{row}");
    assert!(
        row["message"].as_str().unwrap().contains("atc hook -u"),
        "{row}"
    );
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert!(!work.exists(), "the retired skill goes");
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("already wired in"), "{said:?}");
    assert!(!said.contains("rewired"), "{said:?}");

    // -u names nothing: the flag is the whole instruction.
    let out = atc(home.path(), home.path(), &["hook", "-u", "claude"], None);
    assert_eq!(out.status.code(), Some(2));
}

// ---- the retired spelling --------------------------------------------------

/// A plugin an older tower wrote — `atc briefing claude`, one event —
/// still delivers, so it reads as wired; it reads as stale too, on the
/// listing and in the envelope, and `-u` moves it to the trigger and the
/// five events.
#[test]
fn an_old_plugin_reads_as_stale_and_update_rewrites_it() {
    let home = home();
    let plugin = home.path().join(".claude/skills/tower");
    ok(&atc(home.path(), home.path(), &["hook", "claude"], None));
    let hooks_path = plugin.join("hooks/hooks.json");
    let current = std::fs::read_to_string(&hooks_path).unwrap();
    let exe = json_at(&hooks_path)["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .trim_end_matches("trigger claude")
        .to_string();
    let old = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "matcher": "startup|resume|clear|compact|fork",
                "hooks": [{ "type": "command", "command": format!("{exe}briefing claude") }]
            }]
        }
    });
    std::fs::write(&hooks_path, serde_json::to_string_pretty(&old).unwrap()).unwrap();

    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(listing.contains("wired (plugin)"), "{listing:?}");
    assert!(
        listing.contains("stale — atc hook -u rewrites it"),
        "{listing:?}"
    );
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    assert_eq!(rows[0]["slug"], "claude");
    assert_eq!(rows[0]["stale"], true, "{rows}");
    assert_eq!(rows[0]["wiring"]["state"], "wired", "{rows}");

    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&hooks_path).unwrap(), current);
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(!listing.contains("stale"), "{listing:?}");
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    assert!(rows[0].get("stale").is_none(), "cleared: {rows}");

    // A current plugin missing the extra events is the same repair.
    let mut fewer: serde_json::Value = serde_json::from_str(&current).unwrap();
    fewer["hooks"].as_object_mut().unwrap().remove("SessionEnd");
    std::fs::write(&hooks_path, serde_json::to_string_pretty(&fewer).unwrap()).unwrap();
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(listing.contains("wired (plugin)"), "{listing:?}");
    assert!(listing.contains("stale"), "{listing:?}");
    ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert_eq!(std::fs::read_to_string(&hooks_path).unwrap(), current);
}

/// The same for a settings file: an entry that still says
/// `atc briefing claude` is wired and stale, and `-u` rewrites it in
/// place — no second entry — and adds the events it lacked.
#[test]
fn an_old_settings_entry_reads_as_stale_and_update_rewrites_it() {
    let home = home();
    let settings = home.path().join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(
        &settings,
        r#"{"model":"opus","hooks":{"SessionStart":[{"matcher":"startup|resume|clear|compact|fork","hooks":[{"type":"command","command":"atc briefing claude"}]}]}}"#,
    )
    .unwrap();

    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(listing.contains("wired (settings)"), "{listing:?}");
    assert!(
        listing.contains("stale — atc hook -u rewrites it"),
        "{listing:?}"
    );
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    assert_eq!(rows[0]["stale"], true, "{rows}");

    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    let v = json_at(&settings);
    assert_eq!(v["model"], "opus");
    assert_eq!(
        v["hooks"].as_object().unwrap().keys().collect::<Vec<_>>(),
        CLAUDE_EVENTS.to_vec(),
        "{v}"
    );
    let starts = v["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(starts.len(), 1, "rewritten in place: {starts:?}");
    assert_eq!(starts[0]["hooks"][0]["command"], "atc trigger claude");
    assert!(
        !home.path().join(".claude/skills/tower").exists(),
        "-u stays on the mechanism it found"
    );
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    assert!(!listing.contains("stale"), "{listing:?}");
}

// ---- the shells ------------------------------------------------------------

/// The marked lines tower writes for a shell: every line carries the
/// marker, and the text is what the suite compares byte for byte.
fn marked(home: &Path, slug: &str, rc: &Path) -> String {
    let scratch = home.join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    let scratch_rc = match rc.strip_prefix(home) {
        Ok(rel) => scratch.join(rel),
        Err(_) => panic!("{} is under the fixture", rc.display()),
    };
    ok(&atc(&scratch, &scratch, &["hook", slug], None));
    let text = std::fs::read_to_string(&scratch_rc).unwrap();
    std::fs::remove_dir_all(&scratch).unwrap();
    for line in text.lines() {
        assert!(
            line.ends_with(MARKER),
            "{slug}: every line is marked: {line}"
        );
    }
    text
}

/// `atc hook bash` appends the marked lines after what is there, byte
/// for byte; a second run is byte-identical; `-u` leaves a current file
/// unchanged; `unhook` restores the seed exactly.
#[test]
fn the_bash_lines_round_trip_byte_for_byte() {
    let home = home();
    let rc = home.path().join(".bashrc");
    let seed = "# mine\nexport EDITOR=vim\n";
    std::fs::write(&rc, seed).unwrap();
    let lines = marked(home.path(), "bash", &rc);
    assert!(lines.contains("atc session --mint"), "{lines}");
    assert!(lines.contains("atc trigger shell --end"), "{lines}");
    assert!(lines.contains("PROMPT_COMMAND"), "{lines}");

    let said = ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    assert!(said.contains("wired into"), "{said:?}");
    assert!(said.contains("restart the shell"), "{said:?}");
    let wired = format!("{seed}{lines}");
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), wired);

    let said = ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    assert!(said.contains("already wired in"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), wired);
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("bash")).unwrap();
    assert!(row.contains("wired (rc)"), "{row:?}");
    assert!(row.contains(&rc.display().to_string()), "{row:?}");
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    let bash = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slug"] == "bash")
        .unwrap();
    assert_eq!(bash["wiring"]["state"], "wired");
    assert_eq!(bash["wiring"]["mechanism"], "rc");
    assert_eq!(bash["presence"]["state"], "present");

    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("already wired in"), "{said:?}");
    assert!(!said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), wired);

    // An older tower's marked line is rewritten in place by -u.
    std::fs::write(
        &rc,
        format!("{seed}PROMPT_COMMAND=\"atc trigger shell\"  {MARKER}\n"),
    )
    .unwrap();
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), wired);

    let said = ok(&atc(home.path(), home.path(), &["unhook", "bash"], None));
    assert!(said.contains("removed the session lines"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), seed);
    let said = ok(&atc(home.path(), home.path(), &["unhook", "bash"], None));
    assert!(said.contains("nothing wired in"), "{said:?}");

    // A file with no trailing newline gets one before the lines; a
    // missing file is created.
    std::fs::write(&rc, "# no newline").unwrap();
    ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    assert_eq!(
        std::fs::read_to_string(&rc).unwrap(),
        format!("# no newline\n{lines}")
    );
    std::fs::remove_file(&rc).unwrap();
    let said = ok(&atc(home.path(), home.path(), &["unhook", "bash"], None));
    assert!(said.contains("not found"), "{said:?}");
    ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), lines);
}

/// A line a person wrote that calls the trigger is reported and left:
/// `hook` adds nothing beside it, `-u` does not count it, and `unhook`
/// restores the seed exactly.
#[test]
fn a_hand_written_trigger_line_is_reported_and_left_alone() {
    let home = home();
    let rc = home.path().join(".bashrc");
    let seed = "# mine\nPROMPT_COMMAND=\"atc trigger shell;$PROMPT_COMMAND\"\n";
    std::fs::write(&rc, seed).unwrap();

    let said = ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    assert!(
        said.contains("already calls atc trigger shell by hand — leaving it alone"),
        "{said:?}"
    );
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), seed);
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("bash")).unwrap();
    assert!(row.contains("written by hand — left alone"), "{row:?}");
    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    let rows = envelope(&out)["data"]["integrations"].clone();
    let bash = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["slug"] == "bash")
        .unwrap();
    assert_eq!(bash["wiring"]["state"], "hand-written");
    assert_eq!(bash["wiring"]["at"], rc.display().to_string());

    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("nothing is wired"), "{said:?}");
    let said = ok(&atc(home.path(), home.path(), &["unhook", "bash"], None));
    assert!(said.contains("nothing wired in"), "{said:?}");
    assert!(said.contains("written by hand"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), seed);
}

/// fufu's lines in the same rc file are fufu's: `hook` and `unhook`
/// leave them byte for byte, and neither reads them as tower's.
#[test]
fn fufus_lines_in_the_same_file_are_untouched() {
    let home = home();
    let rc = home.path().join(".bashrc");
    let seed = format!(
        "# mine\nalias git='ff git'  {FUFU}\n[[ $PROMPT_COMMAND == *\"ff trigger shell\"* ]] || PROMPT_COMMAND=\"ff trigger shell;$PROMPT_COMMAND\"  {FUFU}\n"
    );
    std::fs::write(&rc, &seed).unwrap();
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    let row = listing.lines().find(|l| l.starts_with("bash")).unwrap();
    assert!(row.ends_with("not wired"), "{row:?}");

    let lines = marked(home.path(), "bash", &rc);
    ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    assert_eq!(
        std::fs::read_to_string(&rc).unwrap(),
        format!("{seed}{lines}")
    );
    ok(&atc(home.path(), home.path(), &["unhook", "bash"], None));
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), seed);
}

/// Each shell's rc file is where that shell reads it: zsh under
/// `ZDOTDIR`, fish and PowerShell under `XDG_CONFIG_HOME`.
#[test]
fn each_shell_writes_where_the_shell_reads() {
    let home = home();
    let zdot = home.path().join("zdot");
    std::fs::create_dir_all(&zdot).unwrap();
    let zshrc = zdot.join(".zshrc");
    let zdotdir = zdot.display().to_string();
    ok(&atc_env(
        home.path(),
        home.path(),
        &["hook", "zsh"],
        None,
        &[("ZDOTDIR", zdotdir.as_str())],
    ));
    let text = std::fs::read_to_string(&zshrc).unwrap();
    assert!(
        text.contains("precmd_functions+=(_tower_ambient)"),
        "{text}"
    );
    assert!(text.contains("zshexit_functions+=(_tower_exit)"), "{text}");
    assert!(!home.path().join(".zshrc").exists(), "ZDOTDIR wins");
    ok(&atc(home.path(), home.path(), &["hook", "zsh"], None));
    assert!(home.path().join(".zshrc").is_file(), "HOME without ZDOTDIR");

    ok(&atc(home.path(), home.path(), &["hook", "fish"], None));
    let fish = home.path().join("xdg/fish/config.fish");
    let text = std::fs::read_to_string(&fish).unwrap();
    assert!(text.contains("--on-event fish_prompt"), "{text}");
    assert!(text.contains("--on-event fish_exit"), "{text}");
    assert!(text.contains("$fish_pid"), "{text}");

    if !cfg!(windows) {
        ok(&atc(
            home.path(),
            home.path(),
            &["hook", "powershell"],
            None,
        ));
        let profile = home
            .path()
            .join("xdg/powershell/Microsoft.PowerShell_profile.ps1");
        let text = std::fs::read_to_string(&profile).unwrap();
        assert!(text.contains("function global:prompt"), "{text}");
        assert!(text.contains("PowerShell.Exiting"), "{text}");
        assert!(text.contains("$PID"), "{text}");
    }

    // Every wired shell is present, and unhook takes each back to empty.
    let listing = ok(&atc(home.path(), home.path(), &["hook", "-l"], None));
    for slug in ["zsh", "fish"] {
        let row = listing.lines().find(|l| l.starts_with(slug)).unwrap();
        assert!(row.contains("wired (rc)"), "{row:?}");
    }
    ok(&atc(home.path(), home.path(), &["unhook", "fish"], None));
    assert_eq!(std::fs::read_to_string(&fish).unwrap(), "");
}

/// A CRLF profile keeps its endings through the append and the removal.
#[test]
fn a_crlf_rc_file_keeps_its_line_endings() {
    let home = home();
    let rc = home.path().join(".bashrc");
    let seed = "# mine\r\nexport EDITOR=vim\r\n";
    std::fs::write(&rc, seed).unwrap();
    ok(&atc(home.path(), home.path(), &["hook", "bash"], None));
    let text = std::fs::read_to_string(&rc).unwrap();
    assert!(text.starts_with(seed), "{text:?}");
    assert!(
        text.lines().all(|_| true) && !text.replace("\r\n", "").contains('\n'),
        "every line ends in CRLF: {text:?}"
    );
    assert!(text.ends_with(&format!("{MARKER}\r\n")), "{text:?}");
    ok(&atc(home.path(), home.path(), &["unhook", "bash"], None));
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), seed);
}

/// Doctor: a wired shell is ok under its mechanism's word, and a
/// hand-written line is information naming the file.
#[test]
fn doctor_reads_the_shells() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let home = repo.path().parent().unwrap();
    let rc = home.join(".bashrc");
    let row = |home: &Path| -> serde_json::Value {
        let out = atc(home, repo.path(), &["doctor", "--json"], None);
        envelope(&out)["data"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["check"] == "hook/bash")
            .cloned()
            .unwrap_or_else(|| panic!("a hook/bash row"))
    };
    std::fs::write(&rc, "# mine\n").unwrap();
    ok(&atc(home, home, &["hook", "bash"], None));
    let found = row(home);
    assert_eq!(found["level"], "ok", "{found}");
    assert_eq!(
        found["message"],
        format!("bash: rc wired in {}", rc.display()),
        "{found}"
    );
    let human = ok(&atc(home, repo.path(), &["doctor"], None));
    assert!(human.contains("ok    bash: rc wired in"), "{human:?}");

    std::fs::write(&rc, "atc trigger shell\n").unwrap();
    let found = row(home);
    assert_eq!(found["level"], "info", "{found}");
    assert_eq!(
        found["message"],
        format!(
            "bash: atc trigger shell is wired by hand in {}",
            rc.display()
        ),
        "{found}"
    );
}

// ---- doctor ----------------------------------------------------------------

/// One row per client, in every state, from the same derivation the
/// report reads.
#[test]
fn doctor_has_a_row_per_client() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let home = repo.path().parent().unwrap();
    let rows = |home: &Path| -> (Vec<serde_json::Value>, u64) {
        let out = atc(home, repo.path(), &["doctor", "--json"], None);
        let value = envelope(&out);
        (
            value["data"]["rows"].as_array().unwrap().clone(),
            value["data"]["findings"].as_u64().unwrap(),
        )
    };
    // PowerShell's row is set aside on Windows, where the shell is
    // present unconditionally: 5.1 ships with the OS, and there is no
    // `$SHELL` to scrub.
    let hook_rows = |rows: &[serde_json::Value]| -> Vec<serde_json::Value> {
        rows.iter()
            .filter(|row| {
                row["check"].as_str().is_some_and(|check| {
                    check.starts_with("hook/") && !(cfg!(windows) && check == "hook/powershell")
                })
            })
            .cloned()
            .collect()
    };

    // A fresh machine: no client, no row.
    let (all, findings) = rows(home);
    assert!(hook_rows(&all).is_empty(), "{all:?}");
    assert_eq!(findings, 0);

    // Present and not wired: information, naming the verb.
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    let (all, findings) = rows(home);
    let hook = hook_rows(&all);
    assert_eq!(hook.len(), 1, "{hook:?}");
    assert_eq!(hook[0]["check"], "hook/claude");
    assert_eq!(hook[0]["level"], "info");
    assert!(
        hook[0]["message"]
            .as_str()
            .unwrap()
            .contains("atc hook claude"),
        "{:?}",
        hook[0]
    );
    assert_eq!(findings, 0);

    // Wired: ok.
    ok(&atc(home, home, &["hook", "claude"], None));
    let (all, findings) = rows(home);
    let hook = hook_rows(&all);
    assert_eq!(hook[0]["level"], "ok");
    assert!(
        hook[0]["message"]
            .as_str()
            .unwrap()
            .contains("claude: plugin wired in"),
        "{:?}",
        hook[0]
    );
    assert_eq!(findings, 0);
    let human = ok(&atc(home, repo.path(), &["doctor"], None));
    assert!(human.contains("ok    claude: plugin wired in"), "{human:?}");

    // A skill an older tower wrote: the one finding, and the repair named.
    std::fs::write(
        home.join(".claude/skills/tower/skills/tower/SKILL.md"),
        "an older tower wrote this",
    )
    .unwrap();
    let (all, findings) = rows(home);
    let hook = hook_rows(&all);
    assert_eq!(hook[0]["level"], "warn");
    assert!(
        hook[0]["message"].as_str().unwrap().contains("atc hook -u"),
        "{:?}",
        hook[0]
    );
    assert_eq!(findings, 1);
    let out = atc(home, repo.path(), &["doctor"], None);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out).contains("WARN  claude:"), "{}", text(&out));
}

/// Doctor reads the Codex plugin the way it reads Claude's: an ok row
/// under the plugin's word, naming the directory.
#[test]
fn doctor_reads_the_codex_plugin() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let home = repo.path().parent().unwrap();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    let row = |home: &Path| -> serde_json::Value {
        let out = atc(home, repo.path(), &["doctor", "--json"], None);
        envelope(&out)["data"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["check"] == "hook/codex")
            .cloned()
            .unwrap_or_else(|| panic!("a hook/codex row"))
    };
    let found = row(home);
    assert_eq!(found["level"], "info", "{found}");
    assert!(
        found["message"]
            .as_str()
            .unwrap()
            .contains("atc hook codex"),
        "{found}"
    );
    ok(&atc(home, home, &["hook", "codex"], None));
    let found = row(home);
    assert_eq!(found["level"], "ok", "{found}");
    assert_eq!(
        found["message"],
        format!(
            "codex: plugin wired in {}",
            home.join(".agents").join("plugins").join("tower").display()
        ),
        "{found}"
    );
    let human = ok(&atc(home, repo.path(), &["doctor"], None));
    assert!(human.contains("ok    codex: plugin wired in"), "{human:?}");
}

/// Doctor reads the OpenCode plugin the way it reads the others: an ok
/// row naming the file, a warn row when a byte drifted under tower's
/// header, and an info row for a file that is not tower's.
#[test]
fn doctor_reads_the_opencode_plugin() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let home = repo.path().parent().unwrap();
    let plugin = home
        .join("xdg")
        .join("opencode")
        .join("plugins")
        .join("tower.js");
    std::fs::create_dir_all(home.join("xdg/opencode")).unwrap();
    let row = |home: &Path| -> serde_json::Value {
        let out = atc(home, repo.path(), &["doctor", "--json"], None);
        envelope(&out)["data"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["check"] == "hook/opencode")
            .cloned()
            .unwrap_or_else(|| panic!("a hook/opencode row"))
    };
    let found = row(home);
    assert_eq!(found["level"], "info", "{found}");
    assert!(
        found["message"]
            .as_str()
            .unwrap()
            .contains("atc hook opencode"),
        "{found}"
    );
    ok(&atc(home, home, &["hook", "opencode"], None));
    let found = row(home);
    assert_eq!(found["level"], "ok", "{found}");
    assert_eq!(
        found["message"],
        format!("opencode: plugin wired in {}", plugin.display()),
        "{found}"
    );

    let body = std::fs::read_to_string(&plugin).unwrap();
    std::fs::write(&plugin, format!("{body}// drift\n")).unwrap();
    let found = row(home);
    assert_eq!(found["level"], "warn", "{found}");
    assert!(
        found["message"]
            .as_str()
            .unwrap()
            .contains("written by an older tower — `atc hook -u` rewrites it"),
        "{found}"
    );

    std::fs::write(&plugin, "export const Mine = async () => ({});\n").unwrap();
    let found = row(home);
    assert_eq!(found["level"], "info", "{found}");
    assert_eq!(
        found["message"],
        format!("opencode: {} is not tower's — left alone", plugin.display()),
        "{found}"
    );
}

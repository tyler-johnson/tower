//! `atc hook` and `atc unhook` against a scratch home: the report, the
//! Claude plugin directory, the settings merge for the three clients
//! that take one, the refresh, and doctor's row per client.
//!
//! Every path here is env-redirected — HOME, USERPROFILE, the XDG roots,
//! LOCALAPPDATA — so the suite never touches a real config file. Client
//! presence is faked by creating the client's config directory.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use atc_testsupport::{Repo, scrub};

/// The spawn: `home` is the scratch HOME, `cwd` where the verb runs,
/// `stdin` what it is fed (piped and closed either way, so nothing here
/// is a terminal and nothing may prompt).
fn atc(home: &Path, cwd: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    let mut child = command
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
        // Nothing here spawns fufu.
        .env("ATC_FF", "/nonexistent")
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
    for slug in ["claude", "codex", "cursor", "gemini"] {
        assert!(listing.contains(slug), "every slug has a row: {listing:?}");
    }
    // Every row says not wired; the client column is what separates a
    // client that is here from one that is not.
    let lines: Vec<&str> = listing.lines().collect();
    assert_eq!(lines.len(), 4, "{listing:?}");
    for (line, present) in lines.iter().zip([true, true, false, false]) {
        assert!(line.ends_with("not wired"), "{line:?}");
        assert_eq!(line.contains("not on this machine"), !present, "{line:?}");
    }

    let out = atc(home.path(), home.path(), &["--json", "hook", "-l"], None);
    assert!(out.status.success());
    let value = envelope(&out);
    assert_eq!(value["cmd"], "hook");
    let rows = value["data"]["integrations"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "one row per slug: {rows:?}");
    assert_eq!(rows[0]["slug"], "claude");
    assert_eq!(rows[0]["wiring"]["state"], "not-wired");
    assert_eq!(rows[0]["presence"]["state"], "present");
    assert_eq!(rows[2]["presence"]["state"], "absent");
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

#[test]
fn unknown_slugs_are_hard_errors() {
    let home = home();
    for verb in ["hook", "unhook"] {
        let out = atc(home.path(), home.path(), &["--json", verb, "tcsh"], None);
        assert_eq!(out.status.code(), Some(2), "{verb}: a usage error");
        let value = envelope(&out);
        assert_eq!(value["cmd"], verb);
        assert_eq!(value["error"]["id"], "usage/unknown-slug");
        let message = value["error"]["message"].as_str().unwrap();
        assert!(message.contains("tcsh"), "{message}");
        assert!(
            message.contains("claude"),
            "names the known ones: {message}"
        );
    }
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
        vec!["SessionStart"],
        "exactly one event: {hooks}"
    );
    assert_eq!(
        hooks["hooks"]["SessionStart"][0]["matcher"],
        "startup|resume|clear|compact|fork"
    );
    // The binary's absolute path is baked in, so the plugin does not
    // depend on `atc` being on whatever PATH the client happens to have.
    let command = hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(command.ends_with("briefing claude"), "{command:?}");
    assert!(
        command.len() > "atc briefing claude".len(),
        "absolute path baked in: {command:?}"
    );
    assert!(!plugin.join(".mcp.json").exists(), "no server rides along");

    // The four skills ride inside the plugin, under the layout a plugin's
    // own skills take, front matter first and byte for byte.
    for name in ["tower", "plan", "work", "review"] {
        let on_disk = std::fs::read_to_string(plugin.join("skills").join(name).join("SKILL.md"))
            .unwrap_or_else(|_| panic!("the {name} skill lands with the plugin"));
        assert!(
            on_disk.starts_with(&format!("---\nname: {name}\n")),
            "{name}: front matter first"
        );
        assert_eq!(on_disk, compiled(name), "{name}: the compiled text");
    }

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
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        "atc briefing claude"
    );
    assert_eq!(
        v["hooks"]["SessionStart"][0]["matcher"],
        "startup|resume|clear|compact|fork"
    );
    assert_eq!(v["hooks"].as_object().unwrap().len(), 1, "one event: {v}");
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
        r#"{"hooks":{"SessionStart":[{"matcher":"startup","hooks":[{"type":"command","command":"atc briefing claude"}]}]}}"#,
    )
    .unwrap();

    ok(&atc(home.path(), home.path(), &["unhook", "claude"], None));
    assert!(!home.path().join(".claude/skills/tower").exists());
    let v = json_at(&settings);
    assert!(v.get("hooks").is_none(), "both mechanisms cleared: {v}");
}

// ---- the settings clients --------------------------------------------------

/// One config file per vendor, each in the shape that vendor documents.
#[test]
fn each_client_is_wired_in_its_own_schema() {
    let home = home();

    ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    let codex = json_at(&home.path().join(".codex/hooks.json"));
    let entry = &codex["hooks"]["SessionStart"][0];
    assert_eq!(entry["hooks"][0]["command"], "atc briefing codex");
    assert_eq!(entry["hooks"][0]["type"], "command");
    assert!(entry.get("matcher").is_none(), "no matcher: {codex}");
    assert_eq!(codex["hooks"].as_object().unwrap().len(), 1, "{codex}");
    for name in ["tower", "plan", "work", "review"] {
        let skill = home
            .path()
            .join(".codex/skills")
            .join(name)
            .join("SKILL.md");
        assert!(skill.exists(), "the {name} skill lands beside the wiring");
        assert!(
            std::fs::read_to_string(&skill)
                .unwrap()
                .starts_with(&format!("---\nname: {name}\n"))
        );
    }

    ok(&atc(home.path(), home.path(), &["hook", "gemini"], None));
    let gemini = json_at(&home.path().join(".gemini/settings.json"));
    let entry = &gemini["hooks"]["SessionStart"][0];
    assert_eq!(entry["hooks"][0]["command"], "atc briefing gemini");
    assert!(entry.get("matcher").is_none(), "no matcher: {gemini}");
    assert!(!home.path().join(".gemini/skills").exists());

    ok(&atc(home.path(), home.path(), &["hook", "cursor"], None));
    let cursor = json_at(&home.path().join(".cursor/hooks.json"));
    assert_eq!(cursor["version"], 1);
    let entry = &cursor["hooks"]["sessionStart"][0];
    assert_eq!(entry["command"], "atc briefing cursor");
    assert!(entry.get("hooks").is_none(), "the flat shape: {cursor}");
    assert!(entry.get("matcher").is_none(), "no matcher: {cursor}");

    // Removing the wiring removes the skills, because unhook takes back
    // exactly what hook added — both halves of it.
    ok(&atc(home.path(), home.path(), &["unhook", "codex"], None));
    for name in ["tower", "plan", "work", "review"] {
        assert!(!home.path().join(".codex/skills").join(name).exists());
    }
    let v = json_at(&home.path().join(".codex/hooks.json"));
    assert!(v.get("hooks").is_none(), "the entries went too: {v}");
}

#[test]
fn install_preserves_foreign_content() {
    let home = home();
    let settings = home.path().join(".codex/hooks.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let foreign = serde_json::json!({
        "model": "opus",
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

    ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    let v = json_at(&settings);
    assert_eq!(v["model"], "opus", "foreign top-level fields preserved");
    assert_eq!(v["env"]["FOO"], "bar");
    assert_eq!(
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"], "my-banner",
        "foreign hook entries preserved value-identical"
    );
    assert_eq!(
        v["hooks"]["Stop"][0]["hooks"][0]["command"],
        "notify-send done"
    );
    assert_eq!(
        v["hooks"]["SessionStart"][1]["hooks"][0]["command"], "atc briefing codex",
        "our entry appended after foreign ones"
    );
    // The user's key order survives the round trip.
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["model", "hooks", "env"], "{v}");

    // Uninstall removes only ours.
    ok(&atc(home.path(), home.path(), &["unhook", "codex"], None));
    let v = json_at(&settings);
    assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        "my-banner"
    );
    assert_eq!(
        v["hooks"]["Stop"][0]["hooks"][0]["command"],
        "notify-send done"
    );
    assert_eq!(v["model"], "opus");
}

#[test]
fn install_refuses_malformed_files_untouched() {
    let home = home();
    let settings = home.path().join(".codex/hooks.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();

    for bad in [
        "{ not json",
        "[1, 2, 3]",
        r#"{ "hooks": "not an object" }"#,
        r#"{ "hooks": { "SessionStart": "not an array" } }"#,
    ] {
        std::fs::write(&settings, bad).unwrap();
        let out = atc(home.path(), home.path(), &["--json", "hook", "codex"], None);
        assert_eq!(out.status.code(), Some(1), "must refuse: {bad}");
        assert_eq!(envelope(&out)["error"]["id"], "hook/malformed");
        assert_eq!(
            std::fs::read_to_string(&settings).unwrap(),
            bad,
            "file untouched on refusal"
        );
        assert!(
            !home.path().join(".codex/skills").exists(),
            "the refusal stops the install before the skills: {bad}"
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
        .join(".claude/skills/tower/skills/plan/SKILL.md");
    let shipped = std::fs::read_to_string(&skill).unwrap();
    std::fs::write(&skill, "an older tower wrote this").unwrap();
    let said = ok(&atc(home.path(), home.path(), &["hook", "-u"], None));
    assert!(said.contains("rewired"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), shipped);

    // -u names nothing: the flag is the whole instruction.
    let out = atc(home.path(), home.path(), &["hook", "-u", "claude"], None);
    assert_eq!(out.status.code(), Some(2));
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
    let hook_rows = |rows: &[serde_json::Value]| -> Vec<serde_json::Value> {
        rows.iter()
            .filter(|row| {
                row["check"]
                    .as_str()
                    .is_some_and(|check| check.starts_with("hook/"))
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
        home.join(".claude/skills/tower/skills/work/SKILL.md"),
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

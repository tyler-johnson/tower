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

/// Claude's table, in the order it is written; Codex names the same
/// five.
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

// ---- the settings clients --------------------------------------------------

/// One config file per vendor, each in the shape that vendor documents.
#[test]
fn each_client_is_wired_in_its_own_schema() {
    let home = home();

    ok(&atc(home.path(), home.path(), &["hook", "codex"], None));
    let codex = json_at(&home.path().join(".codex/hooks.json"));
    assert_eq!(
        codex["hooks"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        CLAUDE_EVENTS.to_vec(),
        "Codex names Claude's five: {codex}"
    );
    for event in CLAUDE_EVENTS {
        let entry = &codex["hooks"][event][0];
        assert_eq!(entry["hooks"][0]["command"], "atc trigger codex", "{event}");
        assert_eq!(entry["hooks"][0]["type"], "command");
        assert!(
            entry.get("matcher").is_none(),
            "{event}: no matcher: {codex}"
        );
    }
    let skill = home.path().join(".codex/skills/tower/SKILL.md");
    assert!(skill.exists(), "the tower skill lands beside the wiring");
    assert!(
        std::fs::read_to_string(&skill)
            .unwrap()
            .starts_with("---\nname: tower\n")
    );

    ok(&atc(home.path(), home.path(), &["hook", "gemini"], None));
    let gemini = json_at(&home.path().join(".gemini/settings.json"));
    let entry = &gemini["hooks"]["SessionStart"][0];
    assert_eq!(entry["hooks"][0]["command"], "atc trigger gemini");
    assert!(entry.get("matcher").is_none(), "no matcher: {gemini}");
    assert_eq!(
        gemini["hooks"].as_object().unwrap().len(),
        1,
        "the boundary alone: {gemini}"
    );
    assert!(!home.path().join(".gemini/skills").exists());

    ok(&atc(home.path(), home.path(), &["hook", "cursor"], None));
    let cursor = json_at(&home.path().join(".cursor/hooks.json"));
    assert_eq!(cursor["version"], 1);
    let entry = &cursor["hooks"]["sessionStart"][0];
    assert_eq!(entry["command"], "atc trigger cursor");
    assert!(entry.get("hooks").is_none(), "the flat shape: {cursor}");
    assert!(entry.get("matcher").is_none(), "no matcher: {cursor}");
    assert_eq!(
        cursor["hooks"].as_object().unwrap().len(),
        1,
        "the boundary alone: {cursor}"
    );

    // Removing the wiring removes the skill, because unhook takes back
    // exactly what hook added — both halves of it.
    ok(&atc(home.path(), home.path(), &["unhook", "codex"], None));
    assert!(!home.path().join(".codex/skills/tower").exists());
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
        v["hooks"]["Stop"][0]["hooks"][0]["command"], "notify-send done",
        "the foreign Stop entry stays first"
    );
    assert_eq!(
        v["hooks"]["Stop"][1]["hooks"][0]["command"], "atc trigger codex",
        "ours appended under the event the foreign one already held"
    );
    assert_eq!(
        v["hooks"]["SessionStart"][1]["hooks"][0]["command"], "atc trigger codex",
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
    assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(
        v["hooks"]["Stop"][0]["hooks"][0]["command"],
        "notify-send done"
    );
    assert!(v["hooks"].get("SessionEnd").is_none(), "{v}");
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

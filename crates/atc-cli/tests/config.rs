//! `atc config` against real repositories: the registry, the arity
//! dispatch, the scope walk, and the unset ladder.
//!
//! Every spawn points `HOME` into the fixture tempdir: gix reads the
//! global scope from `HOME`, so without the override the developer's
//! `~/.gitconfig` would leak into list output and a `--global` test
//! would write to it.

use std::path::Path;
use std::process::{Command, Output};

use atc_testsupport::{Repo, scrub};

fn atc(repo: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    command
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", root(repo).join("xdg"))
        .env("HOME", root(repo))
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", root(repo))
        .env_remove("GIT_CONFIG_GLOBAL")
        .output()
        .expect("spawn atc")
}

/// The fixture tempdir holding `repo/` — the suite's `HOME`, and where
/// a `--global` set lands as `.gitconfig`.
fn root(repo: &Path) -> std::path::PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .to_path_buf()
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn envelope(output: &Output) -> serde_json::Value {
    serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("an envelope")
}

/// Assert a refusal: the exit code, and the envelope's error id.
fn refusal(output: &Output, code: i32, id: &str) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let envelope = envelope(output);
    assert_eq!(envelope["error"]["id"], serde_json::json!(id));
    envelope
}

#[test]
fn list_shows_every_setting_with_defaults_and_the_trailer() {
    let repo = Repo::new();
    let text = stdout(&atc(repo.path(), &["config"]));

    assert!(text.contains("defaultFileStatus  ready"), "{text}");
    assert!(text.contains("serveHost  127.0.0.1"), "{text}");
    assert!(text.contains("servePort  7420"), "{text}");
    assert!(text.contains("leaseWindow  2m"), "{text}");
    assert!(text.contains("updateCheck  1d"), "{text}");
    assert!(text.contains("autoUpdate  true"), "{text}");
    assert_eq!(text.matches("(default)").count(), 6, "{text}");
    assert!(
        text.contains("Set with:     atc config <key> <value>   (--global: every repo)"),
        "{text}"
    );
    assert!(
        text.contains("Remove with:  atc config --unset <key>"),
        "{text}"
    );
    assert!(
        text.contains("Stored as plain git config under tower.<key>"),
        "{text}"
    );
}

#[test]
fn list_json_pins_the_registry() {
    let repo = Repo::new();
    let out = atc(repo.path(), &["config", "--json"]);
    let envelope = envelope(&out);
    assert!(out.status.success(), "{envelope}");

    let settings = envelope["data"]["settings"]
        .as_array()
        .expect("a settings array");
    assert_eq!(settings.len(), 6, "{envelope}");
    let keys: Vec<&str> = settings
        .iter()
        .map(|entry| entry["key"].as_str().expect("a key"))
        .collect();
    assert_eq!(
        keys,
        [
            "defaultFileStatus",
            "serveHost",
            "servePort",
            "leaseWindow",
            "updateCheck",
            "autoUpdate"
        ]
    );
    let kinds: Vec<&str> = settings
        .iter()
        .map(|entry| entry["kind"].as_str().expect("a kind"))
        .collect();
    assert_eq!(
        kinds,
        ["choice", "host", "port", "duration", "cadence", "bool"]
    );
    let file_status = &settings[0];
    assert_eq!(file_status["value"], serde_json::json!("ready"));
    assert_eq!(
        file_status["git_key"],
        serde_json::json!("tower.defaultFileStatus")
    );
    for entry in settings {
        assert_eq!(entry["source"], serde_json::Value::Null, "{entry}");
        assert_eq!(entry["default"], serde_json::json!(true), "{entry}");
        assert_eq!(
            entry["git_key"],
            serde_json::json!(format!("tower.{}", entry["key"].as_str().unwrap())),
            "{entry}"
        );
    }
}

#[test]
fn get_answers_bare_for_every_spelling() {
    let repo = Repo::new();
    for spelling in ["updateCheck", "UPDATECHECK", "tower.updateCheck"] {
        let text = stdout(&atc(repo.path(), &["config", spelling]));
        assert_eq!(text, "1d\n", "{spelling}");
    }
}

#[test]
fn an_unknown_key_is_the_usage_envelope_exit_2() {
    let repo = Repo::new();
    let out = atc(repo.path(), &["config", "--json", "nope"]);
    let envelope = refusal(&out, 2, "usage/unknown-key");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("unknown setting \"nope\" — `atc config` lists them all")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["atc config"])
    );
}

#[test]
fn set_round_trips_through_real_git() {
    let repo = Repo::new();
    let text = stdout(&atc(repo.path(), &["config", "servePort", "7777"]));
    assert_eq!(text, "servePort = 7777 (this repo)\n");

    // Interop proof: what the verb wrote, git reads.
    assert_eq!(repo.git(&["config", "tower.servePort"]).trim(), "7777");

    let text = stdout(&atc(repo.path(), &["config", "servePort"]));
    assert_eq!(text, "7777\n");

    let list = stdout(&atc(repo.path(), &["config"]));
    let port_line = list
        .lines()
        .find(|line| line.starts_with("servePort  "))
        .expect("a servePort line");
    assert!(!port_line.contains("(default)"), "{port_line}");
}

/// `leaseWindow`: set, read back, and unset to the default, through
/// the duration kind.
#[test]
fn the_lease_window_sets_and_unsets() {
    let repo = Repo::new();
    assert_eq!(
        stdout(&atc(repo.path(), &["config", "leaseWindow"])),
        "2m\n"
    );
    let text = stdout(&atc(repo.path(), &["config", "leaseWindow", "30s"]));
    assert_eq!(text, "leaseWindow = 30s (this repo)\n");
    assert_eq!(repo.git(&["config", "tower.leaseWindow"]).trim(), "30s");
    assert_eq!(
        stdout(&atc(repo.path(), &["config", "leaseWindow"])),
        "30s\n"
    );
    let text = stdout(&atc(repo.path(), &["config", "--unset", "leaseWindow"]));
    assert_eq!(text, "leaseWindow unset — back to the default (2m)\n");
    assert_eq!(
        stdout(&atc(repo.path(), &["config", "leaseWindow"])),
        "2m\n"
    );
}

#[test]
fn a_standing_comment_survives_a_set() {
    let repo = Repo::new();
    let config_path = repo.path().join(".git/config");
    let original = std::fs::read_to_string(&config_path).expect("read config");
    std::fs::write(&config_path, format!("{original}\n# hands off\n")).expect("write config");

    stdout(&atc(repo.path(), &["config", "updateCheck", "12h"]));

    let after = std::fs::read_to_string(&config_path).expect("read back");
    assert!(after.contains("# hands off"), "comment lost: {after}");
    assert!(after.contains("updateCheck = 12h"), "{after}");
}

#[test]
fn invalid_values_exit_2_and_write_nothing() {
    let repo = Repo::new();

    let out = atc(repo.path(), &["config", "--json", "updateCheck", "5x"]);
    let envelope = refusal(&out, 2, "usage/bad-value");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!(
            "invalid value for updateCheck: want true, false, or a duration like 12h or 7d"
        )
    );

    let out = atc(repo.path(), &["config", "--json", "autoUpdate", "maybe"]);
    let envelope = refusal(&out, 2, "usage/bad-value");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("invalid value for autoUpdate: want true or false")
    );

    // The choice kind: the refusal names the list itself.
    let out = atc(
        repo.path(),
        &["config", "--json", "defaultFileStatus", "nope"],
    );
    let envelope = refusal(&out, 2, "usage/bad-value");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!(
            "invalid value for defaultFileStatus: want one of backlog, ready, in_progress"
        )
    );
    // A real status that cannot be filed is off the list too.
    let out = atc(
        repo.path(),
        &["config", "--json", "defaultFileStatus", "held"],
    );
    refusal(&out, 2, "usage/bad-value");

    // The duration kind: a span of at least a second, no bool spelling.
    for bad in ["xyz", "0s", "true"] {
        let out = atc(repo.path(), &["config", "--json", "leaseWindow", bad]);
        let envelope = refusal(&out, 2, "usage/bad-value");
        assert_eq!(
            envelope["error"]["message"],
            serde_json::json!("invalid value for leaseWindow: want a duration like 30s, 2m, or 1h"),
            "{bad}"
        );
    }

    // Nothing touched disk on any refusal.
    let listed = repo.git(&["config", "--local", "-l"]);
    assert!(!listed.contains("tower.updatecheck"), "{listed}");
    assert!(!listed.contains("tower.autoupdate"), "{listed}");
    assert!(!listed.contains("tower.defaultfilestatus"), "{listed}");
    assert!(!listed.contains("tower.leasewindow"), "{listed}");

    // The listed words set, and read back.
    for word in ["backlog", "ready", "in_progress"] {
        let text = stdout(&atc(repo.path(), &["config", "defaultFileStatus", word]));
        assert_eq!(text, format!("defaultFileStatus = {word} (this repo)\n"));
        assert_eq!(
            stdout(&atc(repo.path(), &["config", "defaultFileStatus"])),
            format!("{word}\n")
        );
    }
}

#[test]
fn global_set_creates_home_gitconfig_and_the_list_reports_global() {
    let repo = Repo::new();
    let global = root(repo.path()).join(".gitconfig");
    assert!(!global.exists());

    let text = stdout(&atc(
        repo.path(),
        &["config", "updateCheck", "12h", "--global"],
    ));
    assert_eq!(text, "updateCheck = 12h (every repo)\n");

    let content = std::fs::read_to_string(&global).expect("read global config");
    assert!(content.contains("[tower]"), "{content}");
    assert!(content.contains("updateCheck = 12h"), "{content}");

    let out = atc(repo.path(), &["config", "--json"]);
    let envelope = envelope(&out);
    let entry = &envelope["data"]["settings"][4];
    assert_eq!(entry["key"], serde_json::json!("updateCheck"));
    assert_eq!(entry["value"], serde_json::json!("12h"));
    assert_eq!(entry["source"], serde_json::json!("global"));
    assert_eq!(entry["default"], serde_json::json!(false));
}

#[test]
fn the_unset_ladder() {
    let repo = Repo::new();
    stdout(&atc(
        repo.path(),
        &["config", "updateCheck", "12h", "--global"],
    ));
    stdout(&atc(repo.path(), &["config", "updateCheck", "2w"]));

    // Local unset under a standing global.
    let text = stdout(&atc(repo.path(), &["config", "--unset", "updateCheck"]));
    assert_eq!(
        text,
        "updateCheck unset here — 12h still applies from global config\n"
    );
    assert_eq!(
        stdout(&atc(repo.path(), &["config", "updateCheck"])),
        "12h\n"
    );

    // Not set here, but the global still answers — with the hint.
    let text = stdout(&atc(repo.path(), &["config", "--unset", "updateCheck"]));
    assert_eq!(
        text,
        "updateCheck is not set here, but 12h applies from global config — try --global\n"
    );

    // Global unset — back to the default.
    let text = stdout(&atc(
        repo.path(),
        &["config", "--unset", "updateCheck", "--global"],
    ));
    assert_eq!(text, "updateCheck unset — back to the default (1d)\n");

    // Not set anywhere.
    let text = stdout(&atc(repo.path(), &["config", "--unset", "updateCheck"]));
    assert_eq!(text, "updateCheck is not set — the default (1d) applies\n");
}

#[test]
fn set_and_unset_json_payloads() {
    let repo = Repo::new();

    let out = atc(repo.path(), &["config", "--json", "servePort", "7777"]);
    assert_eq!(
        envelope(&out)["data"],
        serde_json::json!({ "key": "servePort", "value": "7777", "global": false })
    );

    let out = atc(repo.path(), &["config", "--json", "--unset", "servePort"]);
    assert_eq!(
        envelope(&out)["data"],
        serde_json::json!({ "key": "servePort", "global": false, "removed": true, "still_applies": null })
    );

    let out = atc(repo.path(), &["config", "--json", "--unset", "servePort"]);
    assert_eq!(
        envelope(&out)["data"],
        serde_json::json!({ "key": "servePort", "global": false, "removed": false, "still_applies": null })
    );
}

#[test]
fn config_works_before_identity_is_set() {
    // The reason the verb does not ride `Store`: config is what you run
    // on a half-configured machine, and `Store::open` fails without
    // `user.email`.
    let repo = Repo::new();
    repo.git(&["config", "--unset", "user.email"]);

    let text = stdout(&atc(repo.path(), &["config"]));
    assert!(text.contains("updateCheck"), "{text}");
}

#[test]
#[cfg(target_os = "linux")]
fn update_check_syncs_cache() {
    // The write-through: a config write keeps the passive lane's cached
    // interval honest, so the hot path's one file read never trusts a
    // stale encoding. Linux-gated like fufu's — the cache root rides
    // XDG_CACHE_HOME only there.
    let repo = Repo::new();
    let cache = tempfile::tempdir().expect("cache tempdir");
    let state_file = cache.path().join("tower").join("update.json");

    let run = |args: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
        command
            .args(args)
            .current_dir(repo.path())
            .env("XDG_CONFIG_HOME", root(repo.path()).join("xdg"))
            .env("XDG_CACHE_HOME", cache.path())
            .env("HOME", root(repo.path()))
            .env_remove("GIT_CONFIG_GLOBAL");
        scrub(&mut command);
        command.output().expect("spawn atc")
    };

    stdout(&run(&["config", "updateCheck", "12h"]));
    let content = std::fs::read_to_string(&state_file).expect("state file");
    assert!(
        content.contains("\"interval_secs\":43200"),
        "expected 43200: {content}"
    );

    stdout(&run(&["config", "updateCheck", "false"]));
    let content = std::fs::read_to_string(&state_file).expect("state file");
    assert!(
        content.contains("\"interval_secs\":-1"),
        "expected -1: {content}"
    );

    stdout(&run(&["config", "--unset", "updateCheck"]));
    let content = std::fs::read_to_string(&state_file).expect("state file");
    assert!(
        content.contains("\"interval_secs\":0"),
        "expected 0 after unset: {content}"
    );
}

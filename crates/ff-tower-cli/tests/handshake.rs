//! fufu's three handshakes against the real binary, spawned the way fufu
//! spawns them: a plain tempdir outside any repository, no `FF_REPO`,
//! `FF_NONINTERACTIVE=1`, and `TOWER_FF=/nonexistent` — an answer that
//! reached for fufu or a repository would be the failure these notice.

use std::path::Path;
use std::process::{Command, Output};

fn ff_tower(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .current_dir(dir)
        .env("TOWER_FF", "/nonexistent")
        .env("FF_NONINTERACTIVE", "1")
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .env("LOCALAPPDATA", dir.join("cache"))
        .env("XDG_CONFIG_HOME", dir.join("xdg"))
        .env("HOME", dir)
        .env("USERPROFILE", dir)
        .env_remove("FF_REPO")
        .output()
        .expect("spawn ff-tower")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// One line of envelope on stdout, nothing on stderr, exit 0.
fn reply(out: &Output) -> serde_json::Value {
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(out);
    assert_eq!(text.lines().count(), 1, "one line: {text}");
    let v: serde_json::Value = serde_json::from_str(&text).expect("an envelope");
    assert_eq!(v["ff"], 1);
    assert!(v.get("error").is_none(), "{text}");
    v
}

fn refusal(out: &Output, code: i32, id: &str) -> serde_json::Value {
    assert_eq!(out.status.code(), Some(code), "{}", stdout(out));
    let text = stdout(out);
    assert_eq!(text.lines().count(), 1, "one line: {text}");
    let v: serde_json::Value = serde_json::from_str(&text).expect("an envelope");
    assert_eq!(v["ff"], 1);
    assert_eq!(v["error"]["id"], id, "{text}");
    assert!(v.get("data").is_none(), "{text}");
    v
}

#[test]
fn the_manifest_declares_tower() {
    let dir = tempfile::TempDir::new().unwrap();
    let v = reply(&ff_tower(dir.path(), &["--ff-manifest"]));
    assert_eq!(v["cmd"], "tower --ff-manifest");
    let data = &v["data"];
    assert_eq!(data["name"], "tower");
    assert_eq!(data["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(data["contract"], 1);
    assert_eq!(data["undoable"], false);
    assert_eq!(data["briefing"], true);
    assert_eq!(data["tools"], true);
    assert_eq!(data["skills"], serde_json::json!(["tower"]));
    for absent in ["mcp", "events"] {
        assert!(
            data.get(absent).is_none(),
            "{absent} is not declared: {data}"
        );
    }

    let verbs = data["verbs"].as_array().expect("verbs");
    assert!(!verbs.is_empty());
    for verb in verbs {
        let name = verb["name"].as_str().expect("a name");
        assert!(
            !name.is_empty() && !name.contains(char::is_whitespace),
            "{name:?}"
        );
        assert!(verb["read_only"].is_boolean(), "{verb}");
        assert!(
            !verb["summary"].as_str().unwrap_or_default().is_empty(),
            "{verb}"
        );
    }
    let read_only = |name: &str| {
        verbs
            .iter()
            .find(|verb| verb["name"] == name)
            .unwrap_or_else(|| panic!("no `{name}` verb: {verbs:?}"))["read_only"]
            .as_bool()
            .unwrap()
    };
    assert!(read_only("board"));
    assert!(!read_only("done"));
    assert!(read_only("briefing"));
    assert!(
        !verbs.iter().any(|verb| verb["name"] == "list"),
        "bay's actions are not verbs"
    );
}

#[test]
fn the_wrong_arity_is_a_usage_refusal() {
    let dir = tempfile::TempDir::new().unwrap();
    for (args, cmd) in [
        (&["--ff-manifest", "x"][..], "tower --ff-manifest"),
        (&["--ff-skill"][..], "tower --ff-skill"),
        (&["--ff-skill", "a", "b"][..], "tower --ff-skill"),
        (&["--ff-tools", "x"][..], "tower --ff-tools"),
    ] {
        let out = ff_tower(dir.path(), args);
        let v = refusal(&out, 2, "tower/usage/bad-flags");
        assert_eq!(v["cmd"], cmd, "{args:?}");
        assert!(out.stderr.is_empty(), "{args:?}: the envelope is stdout's");
    }
}

#[test]
fn a_skill_the_binary_does_not_ship_is_refused() {
    let dir = tempfile::TempDir::new().unwrap();
    for name in ["tower-plan", "tower-nothing"] {
        let out = ff_tower(dir.path(), &["--ff-skill", name]);
        let v = refusal(&out, 1, "tower/skill/unknown");
        assert_eq!(v["cmd"], "tower --ff-skill");
        let message = v["error"]["message"].as_str().unwrap();
        assert!(message.contains(name), "{message}");
        // Nothing to type: the registry's own fallback, the lookup, is
        // the one exit — fufu reads only that it is an error.
        assert_eq!(
            v["error"]["exits"],
            serde_json::json!(["ff tower explain tower/skill/unknown"])
        );
    }
}

#[test]
fn the_tools_are_the_loops_four_gestures() {
    let dir = tempfile::TempDir::new().unwrap();
    let v = reply(&ff_tower(dir.path(), &["--ff-tools"]));
    assert_eq!(v["cmd"], "tower --ff-tools");
    let tools = v["data"].as_array().expect("an array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    let mut want = vec!["next", "brief", "hold", "done"];
    want.sort_unstable();
    assert_eq!(sorted, want);

    // fufu's `check_tools`, mirrored.
    let mut seen = std::collections::HashSet::new();
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "{name}"
        );
        assert!(seen.insert(name), "{name} twice");
        assert!(
            !tool["description"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .is_empty(),
            "{name}"
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{name}");
        let read_only = tool["annotations"]["readOnlyHint"]
            .as_bool()
            .unwrap_or_else(|| panic!("{name}: readOnlyHint"));
        let destructive = tool["annotations"]["destructiveHint"]
            .as_bool()
            .unwrap_or_else(|| panic!("{name}: destructiveHint"));
        assert!(!(read_only && destructive), "{name}");
        assert!(
            tool["inputSchema"].get("additionalProperties").is_none(),
            "{name}: fufu adds cwd"
        );
        let properties = tool["inputSchema"]["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name}: properties"));
        for key in properties.keys() {
            assert!(
                !["json", "help", "cwd"].contains(&key.as_str()),
                "{name}: {key}"
            );
        }
    }

    let tool = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap();
    assert_eq!(tool("brief")["annotations"]["readOnlyHint"], true);
    assert_eq!(tool("done")["annotations"]["readOnlyHint"], false);

    let brief = &tool("brief")["inputSchema"];
    assert_eq!(brief["positional"], serde_json::json!(["flight"]));
    assert_eq!(brief["required"], serde_json::json!(["flight"]));

    let next = &tool("next")["inputSchema"]["properties"];
    assert_eq!(next["count"]["type"], "integer");
    assert_eq!(next["peek"]["type"], "boolean");

    let hold = &tool("hold")["inputSchema"];
    assert_eq!(hold["positional"], serde_json::json!(["flight"]));
    assert_eq!(hold["required"], serde_json::json!(["flight"]));
    assert_eq!(hold["properties"]["message"]["type"], "string");

    let done = &tool("done")["inputSchema"];
    assert_eq!(done["positional"], serde_json::json!(["flight"]));
    assert!(done.get("required").is_none(), "bare done: {done}");
}

/// The handshakes are compiled in: a repository that does not exist
/// changes nothing, because nothing reads it.
#[test]
fn the_handshakes_ignore_the_repository() {
    let dir = tempfile::TempDir::new().unwrap();
    for args in [
        &["--ff-manifest"][..],
        &["--ff-skill", "tower"][..],
        &["--ff-tools"][..],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_ff-tower"))
            .args(args)
            .current_dir(dir.path())
            .env("TOWER_FF", "/nonexistent")
            .env("FF_NONINTERACTIVE", "1")
            .env("FF_REPO", "/nonexistent/dir")
            .env("HOME", dir.path())
            .output()
            .expect("spawn");
        let text = stdout(&out);
        assert_eq!(text.lines().count(), 1, "{args:?}: {text}");
        let v: serde_json::Value = serde_json::from_str(&text).expect("an envelope");
        assert_eq!(v["ff"], 1, "{args:?}");
        assert!(v.get("data").is_some(), "{args:?}: {text}");
        if args[0] == "--ff-skill" {
            let files = v["data"]["files"].as_array().expect("files");
            assert_eq!(files.len(), 1, "{text}");
            assert_eq!(files[0]["path"], "SKILL.md");
            let content = files[0]["content"].as_str().expect("content");
            assert!(content.starts_with("---\nname: tower\n"), "{content:.60}");
        }
    }
}

/// The skill reply is the embedded manual, byte for byte: what `ff hook`
/// writes to disk is what the binary compares against for staleness.
#[test]
fn the_tower_skill_is_the_embedded_manual() {
    let dir = tempfile::TempDir::new().unwrap();
    let v = reply(&ff_tower(dir.path(), &["--ff-skill", "tower"]));
    assert_eq!(v["cmd"], "tower --ff-skill");
    assert_eq!(
        v["data"]["files"][0]["content"],
        include_str!("../src/integ/skill.md")
    );
}

/// A handshake flag is argv[1] or it is nothing: riding a verb, it is
/// clap's unknown argument, and no manifest reaches stdout.
#[test]
fn a_handshake_flag_behind_a_verb_is_claps_refusal() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["board", "--ff-manifest"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "{}", stdout(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--ff-manifest"), "{err}");
}

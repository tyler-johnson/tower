//! OpenCode against the real binary: what it sends once a session runs
//! after `atc hook opencode`.
//!
//! The layer above `tests/hook.rs`, which proves what tower writes and
//! not what the client loads. OpenCode loads every plugin module under
//! its config directory at startup, so the proof is a headless
//! `opencode run` against `atc_testsupport::mock_model` through a custom
//! provider in `opencode.json`: the standing notice in the first
//! request's system prompt, the `whoami` output carried back on the
//! next turn with `opencode` on every axis, and the lease under the
//! session OpenCode named. No model judges anything.
//!
//! Needs `opencode` on `PATH`. Skips with a line when it is absent, and
//! fails instead under `ATC_LIVE=1`, which is how CI runs it. The first
//! run installs `@ai-sdk/openai-compatible` under the scratch cache —
//! network, once — so the deadline is generous.

mod support;

use std::process::Command;
use std::time::Duration;

use atc_testsupport::Repo;
use atc_testsupport::mock_model::MockModel;
use serde_json::Value;
use support::{
    NOTICE_HEAD, atc_bin, client_env, hook, live_client, run_within, scratch_home, whoami_from,
};

#[test]
fn an_opencode_session_carries_the_notice_and_the_shell_knows_its_pilot() {
    let Some(opencode) = live_client("opencode") else {
        return;
    };
    let repo = Repo::new();
    let home = scratch_home(&repo, ".config/opencode");
    // The adapter honors `XDG_CONFIG_HOME`, which `client_env` and
    // `hook` both point at `home/xdg`, so the plugin lands where the
    // client under the same environment reads it.
    hook(&home, repo.path(), "opencode");
    let config = home.join("xdg/opencode");
    assert!(config.join("plugins/tower.js").is_file());

    // The mock as a custom provider: the OpenAI-compatible package posts
    // to `/v1/chat/completions`, the dialect the mock speaks for Qwen.
    let whoami = format!("{} whoami --json", atc_bin());
    let model = MockModel::start(&whoami);
    let provider = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {
            "mock": {
                "npm": "@ai-sdk/openai-compatible",
                "name": "mock",
                "options": { "baseURL": format!("{}/v1", model.base_url()), "apiKey": "sk-mock" },
                "models": { "mock": { "name": "mock" } }
            }
        }
    });
    std::fs::write(
        config.join("opencode.json"),
        serde_json::to_string_pretty(&provider).unwrap(),
    )
    .unwrap();

    let mut command = Command::new(&opencode);
    client_env(&mut command, &home, repo.path());
    command.args([
        "run",
        "--model",
        "mock/mock",
        "--format",
        "json",
        "--dangerously-skip-permissions",
        "say hi",
    ]);
    let out = run_within(&mut command, Duration::from_secs(180));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "opencode run exited {:?}\nstdout: {stdout}",
        out.status.code()
    );

    // One JSON object per line; anything else the client prints is
    // skipped.
    let events: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line.trim()).ok())
        .collect();
    assert!(!events.is_empty(), "no JSON events in {stdout}");
    let errors: Vec<&Value> = events
        .iter()
        .filter(|event| event["type"] == "error")
        .collect();
    assert!(errors.is_empty(), "the client reported errors: {errors:?}");
    let session_id = events
        .iter()
        .find_map(|event| event["sessionID"].as_str())
        .unwrap_or_else(|| panic!("no sessionID in {stdout}"))
        .to_string();
    let output = events
        .iter()
        .filter(|event| event["type"] == "tool_use")
        .find_map(|event| {
            let text = match &event["part"]["state"]["output"] {
                Value::String(text) => text.clone(),
                _ => event.to_string(),
            };
            text.contains(r#""cmd":"whoami""#).then_some(text)
        })
        .unwrap_or_else(|| panic!("no tool_use carrying the whoami envelope in {stdout}"));
    let seen = whoami_from(&output);
    assert_eq!(seen["data"]["client"], "opencode", "{seen}");
    assert_eq!(seen["data"]["session_source"], "opencode", "{seen}");
    assert_eq!(seen["data"]["callsign"], "opencode", "{seen}");
    assert_eq!(seen["data"]["session"], session_id, "{seen}");

    // The standing notice is in the first request's system prompt, and
    // a later request carries the tool's output.
    let requests = model.requests();
    let chats: Vec<_> = requests
        .iter()
        .filter(|request| request.path.ends_with("/chat/completions"))
        .collect();
    assert!(
        chats.len() >= 2,
        "two turns reach the model: {:?}",
        requests.iter().map(|r| &r.path).collect::<Vec<_>>()
    );
    assert!(
        chats[0].body.contains(NOTICE_HEAD),
        "the notice is in the first request: {}",
        chats[0].body
    );
    let carried = chats.iter().skip(1).any(|request| {
        let body: Value = serde_json::from_str(&request.body).expect("a body parses");
        body["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|message| message["role"] == "tool" && message.to_string().contains("whoami"))
    });
    assert!(carried, "a later request carries the tool's output");

    assert!(
        home.join(".local/state/atc/leases")
            .join(&session_id)
            .is_file(),
        "the lease is keyed by the session OpenCode named"
    );
}

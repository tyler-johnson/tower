//! Claude Code against the real binary: what it loads after `atc hook
//! claude`, and what it sends once a session runs.
//!
//! The layer above `tests/hook.rs`, which proves what tower writes and
//! not what the client reads — the gap #149 found in Codex. Here the
//! client lists its plugins with no session started, and a headless
//! `claude -p` runs against `atc_testsupport::mock_model`, where the
//! proof is the request body Claude Code sent: the notice in the prompt,
//! the `whoami` output carried back. No model judges anything.
//!
//! Needs `claude` on `PATH`. Skips with a line when it is absent, and
//! fails instead under `ATC_LIVE=1`, which is how CI runs it.

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
fn claude_lists_the_plugin() {
    let Some(claude) = live_client("claude") else {
        return;
    };
    let repo = Repo::new();
    let home = scratch_home(&repo, ".claude");
    hook(&home, repo.path(), "claude");

    let mut command = Command::new(&claude);
    client_env(&mut command, &home, repo.path());
    command.args(["plugin", "list"]);
    let out = run_within(&mut command, Duration::from_secs(60));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "claude plugin list exited {:?}\nstdout: {stdout}",
        out.status.code()
    );
    assert!(stdout.contains("tower@skills-dir"), "{stdout}");
    assert!(stdout.contains("loaded"), "{stdout}");
}

#[test]
fn a_claude_session_carries_the_notice_and_the_shell_knows_its_pilot() {
    let Some(claude) = live_client("claude") else {
        return;
    };
    let repo = Repo::new();
    let home = scratch_home(&repo, ".claude");
    hook(&home, repo.path(), "claude");

    let whoami = format!("{} whoami --json", atc_bin());
    let model = MockModel::start(&whoami);
    let mut command = Command::new(&claude);
    client_env(&mut command, &home, repo.path());
    command
        .env("ANTHROPIC_BASE_URL", model.base_url())
        .env("ANTHROPIC_API_KEY", "sk-mock")
        .env("DISABLE_TELEMETRY", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .args([
            "-p",
            "say hi",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            "claude-sonnet-5",
            // The exact command is allowed and nothing else: no bypass.
            "--allowedTools",
            &format!("Bash({whoami})"),
        ]);
    let out = run_within(&mut command, Duration::from_secs(120));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "claude -p exited {:?}\nstdout: {stdout}",
        out.status.code()
    );

    let events: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let session_id = events
        .iter()
        .find(|event| event["type"] == "result")
        .and_then(|event| event["session_id"].as_str())
        .unwrap_or_else(|| panic!("no result event in {stdout}"))
        .to_string();
    let result = events
        .iter()
        .filter(|event| event["type"] == "user")
        .flat_map(|event| {
            event["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .find(|block| block["type"] == "tool_result")
        .unwrap_or_else(|| panic!("no tool_result in {stdout}"));
    let output = match &result["content"] {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("tool_result content is {other}"),
    };
    let seen = whoami_from(&output);
    assert_eq!(seen["data"]["client"], "claude", "{seen}");
    assert_eq!(seen["data"]["session_source"], "claude", "{seen}");
    assert_eq!(seen["data"]["callsign"], "claude", "{seen}");
    assert_eq!(seen["data"]["session"], session_id, "{seen}");
    assert_eq!(seen["data"]["lease"]["fresh"], true, "{seen}");

    let requests = model.requests();
    let messages: Vec<_> = requests
        .iter()
        .filter(|request| request.path.contains("/v1/messages"))
        .collect();
    assert!(
        messages.len() >= 2,
        "two turns reach the model: {:?}",
        requests.iter().map(|r| &r.path).collect::<Vec<_>>()
    );
    let first: Value = serde_json::from_str(&messages[0].body).expect("the first body parses");
    let notice = first["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("no messages[] in {first}"))
        .iter()
        .filter(|message| message["role"] == "system")
        .flat_map(|message| match &message["content"] {
            Value::String(text) => vec![text.clone()],
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|block| block["text"].as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        })
        .find(|text| text.contains(NOTICE_HEAD));
    assert!(
        notice.is_some(),
        "the notice is a system message in the first request: {first}"
    );
    let last = &messages[messages.len() - 1].body;
    assert!(last.contains("tool_result"), "{last}");
    assert!(last.contains("whoami"), "{last}");
}

//! Qwen Code against the real binary: what it sends once a session runs
//! after `atc hook qwen`.
//!
//! The layer above `tests/hook.rs`, which proves what tower writes and
//! not what the client reads — the gap #149 found in Codex. Qwen Code
//! has no verb that lists its hooks, so the settings file stays the
//! contract (`hook.rs::qwen_is_wired_in_its_settings_file`) and the
//! model-free layer is nothing beyond the binary answering. The session
//! layer is a headless `qwen -p` against `atc_testsupport::mock_model`,
//! where the proof is the request body Qwen Code sent: the notice in
//! the prompt, the `whoami` output carried back. No model judges
//! anything.
//!
//! Needs `qwen` on `PATH`. Skips with a line when it is absent, and
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
fn a_qwen_session_carries_the_notice_and_the_shell_knows_its_pilot() {
    let Some(qwen) = live_client("qwen") else {
        return;
    };
    let repo = Repo::new();
    let home = scratch_home(&repo, ".qwen");
    hook(&home, repo.path(), "qwen");

    // The settings name the bare `atc trigger qwen`; `client_env` puts
    // this build's binary first on PATH for it.
    let whoami = format!("{} whoami --json", atc_bin());
    let model = MockModel::start(&whoami);
    let mut command = Command::new(&qwen);
    client_env(&mut command, &home, repo.path());
    command.args([
        "--auth-type",
        "openai",
        "--openai-base-url",
        &format!("{}/v1", model.base_url()),
        "--openai-api-key",
        "sk-mock",
        "-m",
        "mock",
        "--output-format",
        "json",
        "--allowed-tools",
        &format!("run_shell_command({whoami})"),
        // `-p` rather than a positional prompt: with stdin closed the
        // positional form fails on "No input provided via stdin".
        "-p",
        "say hi",
    ]);
    let out = run_within(&mut command, Duration::from_secs(120));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "qwen -p exited {:?}\nstdout: {stdout}",
        out.status.code()
    );

    // One JSON array of events; anything the client prints ahead of it
    // is skipped to the first bracket.
    let start = stdout
        .find('[')
        .unwrap_or_else(|| panic!("no event array in {stdout}"));
    let events: Vec<Value> = serde_json::Deserializer::from_str(&stdout[start..])
        .into_iter()
        .next()
        .expect("an array")
        .unwrap_or_else(|err| panic!("the events do not parse: {err}\n{stdout}"));
    let session_id = events
        .iter()
        .find(|event| event["type"] == "system" && event["subtype"] == "init")
        .and_then(|event| event["session_id"].as_str())
        .unwrap_or_else(|| panic!("no init event in {stdout}"))
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
    assert_eq!(seen["data"]["client"], "qwen", "{seen}");
    assert_eq!(seen["data"]["session_source"], "qwen", "{seen}");
    assert_eq!(seen["data"]["callsign"], "qwen", "{seen}");
    assert_eq!(seen["data"]["session"], session_id, "{seen}");

    // First and any, never a count: Qwen Code may extract memories with
    // one more request on its way out.
    let requests = model.requests();
    let chats: Vec<_> = requests
        .iter()
        .filter(|request| request.path.ends_with("/v1/chat/completions"))
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
}

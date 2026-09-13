//! Copilot's real plugin loader against tower's installed 1.0 plugin and hand-written registration. This layer needs no model or credentials; CI pins the client version so a loader change fails here.

mod support;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use atc_testsupport::Repo;
use support::{client_env, hook, live_client, run_within, scratch_home};

fn list(client: &Path, home: &Path, cwd: &Path) -> String {
    let mut command = Command::new(client);
    client_env(&mut command, home, cwd);
    command.args(["plugin", "list"]);
    let out = run_within(&mut command, Duration::from_secs(30));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn copilot_loads_the_registered_plugin_live_and_forgets_it_on_unhook() {
    let Some(copilot) = live_client("copilot") else {
        return;
    };
    let repo = Repo::new();
    let home = scratch_home(&repo, ".copilot");
    assert!(!list(&copilot, &home, repo.path()).contains("tower@tower-atc"));
    hook(&home, repo.path(), "copilot");
    let listing = list(&copilot, &home, repo.path());
    assert!(listing.contains("Live Plugins"), "{listing}");
    let row = listing
        .lines()
        .find(|line| line.contains("tower@tower-atc"))
        .unwrap_or_else(|| panic!("plugin not loaded: {listing}"));
    assert!(row.contains("(enabled)"), "{row}");
    assert!(
        listing.contains(&home.join(".agents/plugins/copilot").display().to_string()),
        "{listing}"
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    client_env(&mut command, &home, repo.path());
    let out = command.args(["unhook", "copilot"]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let listing = list(&copilot, &home, repo.path());
    assert!(!listing.contains("tower@tower-atc"), "{listing}");
}

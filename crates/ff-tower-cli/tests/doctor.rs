//! `ff tower doctor` against real repositories: the healthy ok row, the
//! drifted seam, and the registry and config rows.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_testsupport::{FakeFf, Repo};

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    ff_tower_via(repo, args, None)
}

/// The spawn, with the `TOWER_FF` seam: a named program reaches the
/// binary's `ff()` through the environment, the way `cmd/mod.rs` reads
/// it back.
fn ff_tower_via(repo: &Path, args: &[&str], program: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ff-tower"));
    command
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo));
    if let Some(program) = program {
        command.env("TOWER_FF", program);
    }
    command.output().expect("spawn ff-tower")
}

/// The fixture's own config root, beside the repository inside the
/// tempdir and never created — an empty user layer. A suite that read the
/// developer's real `~/.config/tower/procedures` would pass or fail by
/// whose machine it is running on.
fn xdg(repo: &Path) -> std::path::PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .join("xdg")
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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

/// The doctor rows out of a `--json` run, with the findings count.
fn rows(output: &Output) -> (Vec<serde_json::Value>, u64) {
    let envelope = envelope(output);
    assert_eq!(envelope["cmd"], serde_json::json!("tower doctor"));
    let rows = envelope["data"]["rows"].as_array().expect("rows").clone();
    let findings = envelope["data"]["findings"].as_u64().expect("findings");
    (rows, findings)
}

fn row<'a>(rows: &'a [serde_json::Value], check: &str) -> &'a serde_json::Value {
    rows.iter()
        .find(|row| row["check"] == serde_json::json!(check))
        .unwrap_or_else(|| panic!("no `{check}` row in {rows:?}"))
}

#[test]
fn a_fresh_repository_is_healthy() {
    let repo = repo();

    let out = ff_tower(repo.path(), &["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("contract 1"), "{text}");
    assert!(text.contains("healthy"), "{text}");

    // The envelope shape, pinned: `rows` with level/check/message, and
    // the findings count beside them.
    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let (rows, findings) = rows(&out);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0]["level"], serde_json::json!("ok"));
    assert_eq!(rows[0]["check"], serde_json::json!("ff/version"));
    let message = rows[0]["message"].as_str().expect("message");
    assert!(message.contains("contract 1"), "{message}");
    // The update row: test binaries are never official, so the passive
    // lane reports the source build — info, never a finding.
    assert_eq!(rows[1]["level"], serde_json::json!("info"));
    assert_eq!(rows[1]["check"], serde_json::json!("tower/update"));
    assert_eq!(
        rows[1]["message"],
        serde_json::json!("source build — updates via cargo install")
    );
    assert_eq!(findings, 0);
}

#[test]
fn a_drifted_contract_is_a_finding() {
    let repo = repo();
    let fake = FakeFf::script(
        r#"for a in "$@"; do printf '%s\n' "$a" >> "$0.argv"; done
printf '%s\n' '{"ff":99,"cmd":"version","data":{"version":"9.9.9"}}'
"#,
    );

    let out = ff_tower_via(repo.path(), &["doctor", "--json"], Some(fake.path()));
    assert_eq!(out.status.code(), Some(1));
    let (all, findings) = rows(&out);
    assert_eq!(
        all.len(),
        2,
        "the seam row and the seamless update row: {all:?}"
    );
    assert_eq!(all[0]["level"], serde_json::json!("warn"));
    assert_eq!(all[0]["check"], serde_json::json!("ff/contract"));
    row(&all, "tower/update");
    let message = all[0]["message"].as_str().expect("message");
    assert!(message.contains("99") && message.contains('1'), "{message}");
    assert_eq!(findings, 1);

    // On a broken seam the version call is the only spawn. Line-exact,
    // because the recorded `-C` path itself contains "op".
    let recorded = std::fs::read_to_string(fake.dir().join("ff.argv")).expect("argv recorded");
    let words: Vec<&str> = recorded.lines().collect();
    assert!(words.contains(&"version"), "{recorded}");
    assert!(
        !words.contains(&"op") && !words.contains(&"status"),
        "a drifted contract fails every spawn — doctor must not try: {recorded}"
    );
}

#[test]
fn a_procedure_naming_a_ghost_skill_is_a_finding() {
    let repo = repo();
    repo.write(
        ".tower/procedures/spooky.toml",
        concat!(
            "name = \"spooky\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nskill    = \"ghost\"\n\n",
            "[[flight]]\nid       = \"end\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );

    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let (all, findings) = rows(&out);
    let unresolved = row(&all, "skill/unresolved");
    assert_eq!(unresolved["level"], serde_json::json!("warn"));
    let message = unresolved["message"].as_str().expect("message");
    assert!(message.contains("spooky"), "{message}");
    assert!(message.contains("pass"), "{message}");
    assert!(message.contains("`ghost`"), "{message}");
    // The engine ships empty, so no skills at all is the ordinary case
    // and the row says that rather than trailing an empty list.
    assert!(message.contains("the skill shelf is empty"), "{message}");
    assert_eq!(findings, 1);

    // Install the missing prose: the row goes, and an empty registry
    // alone never warned — `a_fresh_repository_is_healthy` holds that
    // half.
    repo.write(".tower/skills/ghost.md", "# ghost\n");
    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let (all, findings) = rows(&out);
    assert!(
        !all.iter()
            .any(|row| row["check"] == serde_json::json!("skill/unresolved")),
        "{all:?}"
    );
    assert_eq!(findings, 0);

    // With a second skill on the shelf the row names the set again.
    repo.write(
        ".tower/procedures/spooky.toml",
        concat!(
            "name = \"spooky\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nskill    = \"phantom\"\n\n",
            "[[flight]]\nid       = \"end\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );
    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    let (all, _) = rows(&out);
    let message = row(&all, "skill/unresolved")["message"]
        .as_str()
        .expect("message")
        .to_string();
    assert!(message.contains("installed: ghost"), "{message}");
}

#[test]
fn a_procedure_that_ends_on_an_agent_is_a_finding() {
    // DESIGN.md:338 warns rather than refuses, so the definition loads
    // and doctor is where it surfaces — by name and by flight.
    let repo = repo();
    repo.write(
        ".tower/procedures/script.toml",
        concat!(
            "name = \"script\"\n\n",
            "[[flight]]\nid       = \"plan\"\nassignee = \"me\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nafter    = [\"plan\"]\n",
        ),
    );

    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let (all, findings) = rows(&out);
    let warned = row(&all, "procedure/no-human-end");
    assert_eq!(warned["level"], serde_json::json!("warn"));
    assert_eq!(
        warned["message"],
        serde_json::json!(
            "procedure script ends on agent flight pass — a procedure should end with you"
        )
    );
    assert_eq!(findings, 1);

    // One human terminal flight is enough to quiet it: `verdict` closes
    // the shape, and the row goes.
    repo.write(
        ".tower/procedures/script.toml",
        concat!(
            "name = \"script\"\n\n",
            "[[flight]]\nid       = \"plan\"\nassignee = \"me\"\n\n",
            "[[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nafter    = [\"plan\"]\n\n",
            "[[flight]]\nid       = \"verdict\"\nassignee = \"me\"\nafter    = [\"pass\"]\n",
        ),
    );
    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let (all, findings) = rows(&out);
    assert!(
        !all.iter()
            .any(|row| row["check"] == serde_json::json!("procedure/no-human-end")),
        "{all:?}"
    );
    assert_eq!(findings, 0);
}

#[test]
fn a_default_file_status_no_flight_can_be_filed_with_is_a_finding() {
    // The word the status was called before the rename, left in git
    // config: filings would silently land Ready, so doctor names it.
    let repo = repo();
    repo.git(&["config", "tower.defaultFileStatus", "triage"]);

    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let (all, findings) = rows(&out);
    let stale = row(&all, "config/default-file-status");
    assert_eq!(stale["level"], serde_json::json!("warn"));
    let message = stale["message"].as_str().expect("message");
    assert!(message.contains("`triage`"), "{message}");
    assert!(message.contains("filings land ready"), "{message}");
    assert!(
        message.contains("config defaultFileStatus backlog"),
        "{message}"
    );
    assert_eq!(findings, 1);

    // The new word, or no setting at all, is no row.
    repo.git(&["config", "tower.defaultFileStatus", "backlog"]);
    let out = ff_tower(repo.path(), &["doctor", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let (all, findings) = rows(&out);
    assert!(
        !all.iter()
            .any(|row| row["check"] == serde_json::json!("config/default-file-status")),
        "{all:?}"
    );
    assert_eq!(findings, 0);
}

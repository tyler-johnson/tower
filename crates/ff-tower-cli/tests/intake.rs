//! Intake matching riding the binary: a bare filing a rule covers files
//! under the rule's procedure on the same invocation, attributed to the
//! invoker, and no verb walks the board afterwards.

use std::path::Path;
use std::process::{Command, Output};

use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::Repo;

fn ff_tower(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .env("FF_REPO", repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .output()
        .expect("spawn ff-tower")
}

/// The fixture's own config root, beside the repository inside the
/// tempdir and never created — an empty user layer.
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

fn filed(subject: &str, status: &str, labels: &[&str]) -> Kind {
    Kind::Filed {
        procedure: None,
        subject: subject.to_string(),
        body: String::new(),
        status: status.to_string(),
        assignee: None,
        priority: "none".to_string(),
        labels: labels.iter().map(|label| label.to_string()).collect(),
        skill: None,
        bay: None,
        done: "asserted".to_string(),
        branch: None,
    }
}

const CHORES: &str = "name = \"chores\"\n\
     [[match]]\n\
     name  = \"chore-label\"\n\
     label = \"chore\"\n\
     [[flight]]\n\
     id       = \"work\"\n\
     assignee = \"me\"\n\
     skill    = \"tidy\"\n";

#[test]
fn board_reads_a_satisfied_waiter_ready_with_the_closers_byline() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    Store::open(repo.path())
        .expect("open")
        .append(vec![
            filed("the dep", "ready", &[]),
            filed("the waiter", "ready", &[]),
            Kind::Linked {
                from: "pi.2".parse().expect("id"),
                to: "pi.1".parse().expect("id"),
            },
            Kind::Status {
                flight: "pi.1".parse().expect("id"),
                status: "done".to_string(),
                reason: None,
            },
        ])
        .expect("append");

    // Nothing to conclude: the fold derives the release, so this one
    // invocation renders the waiter Ready with no event appended.
    let board = envelope(&ff_tower(repo.path(), &["board", "--json"]));
    let ready = board["data"]["ready"].as_array().expect("ready");
    let waiter = ready
        .iter()
        .find(|flight| flight["id"] == serde_json::json!("pi.2"))
        .expect("the waiter is on the board");
    assert_eq!(waiter["status"], serde_json::json!("ready"));
    assert_eq!(
        waiter["status_reason"],
        serde_json::json!("dependency pi.1 done")
    );

    // Attributed and explained: the mark is the closer's, and the
    // waiter's own history carries no move.
    let brief = envelope(&ff_tower(repo.path(), &["brief", "pi.2", "--json"]));
    assert_eq!(brief["data"]["status"], serde_json::json!("ready"));
    assert_eq!(
        brief["data"]["status_by"],
        serde_json::json!("tests@tower.invalid")
    );
    assert_eq!(
        brief["data"]["status_reason"],
        serde_json::json!("dependency pi.1 done")
    );
    let history = brief["data"]["history"].as_array().expect("history");
    assert!(
        history
            .iter()
            .all(|moment| moment["what"] != serde_json::json!("status")),
        "no advance on the record: {history:?}"
    );
}

#[test]
fn a_repo_layer_rule_routes_a_labeled_filing_on_the_same_invocation() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/chores.toml", CHORES);

    // No `defaultFileStatus` change: the rule decides the shape, and
    // the default setting clears it. The envelope carries the routing
    // beside the filing.
    let file = envelope(&ff_tower(
        repo.path(),
        &["file", "sweep the logs", "--label", "chore", "--json"],
    ));
    assert_eq!(file["data"]["routed"]["kind"], serde_json::json!("routed"));

    let brief = envelope(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["status"], serde_json::json!("ready"));
    assert_eq!(brief["data"]["procedure"], serde_json::json!("chores"));
    assert_eq!(brief["data"]["skill"], serde_json::json!("tidy"));
    let history = brief["data"]["history"].as_array().expect("history");
    let routed = history
        .iter()
        .find(|moment| moment["what"] == serde_json::json!("routed"))
        .unwrap_or_else(|| panic!("the routing is on the record: {history:?}"));
    assert_eq!(routed["procedure"], serde_json::json!("chores"));
    assert!(
        routed["because"]
            .as_str()
            .is_some_and(|because| !because.is_empty()),
        "the routing says why: {routed}"
    );
}

#[test]
fn the_human_echo_says_which_rule_chose_the_procedure() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/chores.toml", CHORES);

    let out = stdout(&ff_tower(
        repo.path(),
        &["file", "sweep the logs", "--label", "chore"],
    ));
    assert!(
        out.contains("under chores · matched label chore: sweep the logs"),
        "stdout: {out}"
    );

    // A filing no rule covers echoes as it always did.
    let out = stdout(&ff_tower(repo.path(), &["file", "plain"]));
    assert!(out.contains("in ready: plain"), "stdout: {out}");
}

#[test]
fn a_broken_rule_file_leaves_board_running_and_file_and_procedures_refusing() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo.write(".tower/procedures/broken.toml", "name = \"broken\"\n");

    // The board never touches the registry: it renders, with no stderr
    // line about anything.
    let out = ff_tower(repo.path(), &["board", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let board = envelope(&out);
    assert_eq!(board["cmd"], serde_json::json!("tower board"));

    // The surfaces that read a definition refuse loudly — a bare
    // filing now among them.
    for args in [
        ["file", "x", "--json"].as_slice(),
        ["procedures", "--json"].as_slice(),
    ] {
        let out = ff_tower(repo.path(), args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
        let refusal = envelope(&out);
        assert_eq!(
            refusal["error"]["id"],
            serde_json::json!("tower/procedure/no-parts"),
            "{args:?}"
        );
    }
    assert!(
        Store::open(repo.path())
            .expect("open")
            .read_all()
            .expect("read")
            .is_empty(),
        "the refused filing wrote nothing"
    );
}

#[test]
fn explain_carries_the_rule_refusals() {
    let repo = Repo::new();
    for id in ["procedure/empty-rule", "procedure/duplicate-rule"] {
        let out = envelope(&ff_tower(repo.path(), &["explain", id, "--json"]));
        assert_eq!(out["data"]["id"], serde_json::json!(format!("tower/{id}")));
    }
}

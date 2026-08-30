//! `ff tower procedures [<name>]` — the read-only half of the registry.
//!
//! The verb spawns no fufu, so no `TOWER_FF` appears here. It does read
//! the registry, so every spawn points `XDG_CONFIG_HOME` at the fixture's
//! own tempdir: a suite that read the developer's real
//! `~/.config/tower/procedures` would pass or fail by whose machine it is.

use std::path::Path;
use std::process::{Command, Output};

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

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

#[test]
fn the_listing_is_the_shipped_set_with_flights_and_lanes() {
    let repo = repo();
    let out = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert_eq!(
        out,
        "open    built-in\n\
         · work  me\n\
         \n\
         review  built-in\n\
         · pass     agent\n\
         · smoke    me\n\
         · verdict  me\n\
         \n\
         2 procedures · ff tower procedures <name> for one\n"
    );
}

#[test]
fn the_detail_carries_the_flights_the_inert_rule_and_where_to_fork() {
    let repo = repo();
    let out = stdout(&ff_tower(repo.path(), &["procedures", "review"]));
    assert!(out.starts_with("review  built-in\n"), "{out}");
    assert!(out.contains("    subject branch\n"), "{out}");
    // The rule prints by name with its predicates, and an adapter-keyed
    // one says outright that it cannot fire — a rule that looks live and
    // never runs is an hour of debugging.
    assert!(out.contains("match\n"), "{out}");
    assert!(
        out.contains(
            "· github-reviews  source github · event review_requested · \
             inert until an adapter can fire it\n"
        ),
        "{out}"
    );
    assert!(
        out.contains("· pass     agent · skill review · done asserted\n"),
        "{out}"
    );
    assert!(
        out.contains("· smoke    me · bay warm · done asserted\n"),
        "{out}"
    );
    assert!(
        out.contains("· verdict  me · after pass, smoke · done asserted\n"),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "fork: {}\n",
            repo.path()
                .join(".tower")
                .join("procedures")
                .join("review.toml")
                .display()
        )),
        "{out}"
    );
}

#[test]
fn the_json_form_is_the_registry_as_data() {
    let repo = repo();
    let all = envelope(&ff_tower(repo.path(), &["procedures", "--json"]));
    assert_eq!(all["cmd"], serde_json::json!("procedures"));
    let procedures = all["data"]["procedures"].as_array().expect("procedures");
    assert_eq!(procedures.len(), 2);
    assert_eq!(procedures[0]["name"], serde_json::json!("open"));
    assert_eq!(
        procedures[0]["source"],
        serde_json::json!({"layer": "built-in", "path": null})
    );

    let one = envelope(&ff_tower(repo.path(), &["procedures", "review", "--json"]));
    let review = &one["data"]["procedure"];
    assert_eq!(review["name"], serde_json::json!("review"));
    assert_eq!(review["subject"], serde_json::json!("branch"));
    assert_eq!(
        review["matches"],
        serde_json::json!([{
            "name": "github-reviews",
            "source": "github",
            "event": "review_requested",
            "label": null,
            "priority": null,
            "skill": null,
            "assignee": null,
        }])
    );
    let flights = review["flights"].as_array().expect("flights");
    assert_eq!(flights.len(), 3);
    assert_eq!(
        flights[0],
        serde_json::json!({
            "id": "pass",
            "assignee": "agent",
            "skill": "review",
            "after": [],
            "done": "asserted",
            "bay": null,
            "priority": null,
            "labels": [],
        })
    );
    assert_eq!(flights[2]["after"], serde_json::json!(["pass", "smoke"]));
}

#[test]
fn a_repo_definition_overrides_the_built_in_and_says_so() {
    let repo = repo();
    repo.write(
        ".tower/procedures/review.toml",
        "name = \"review\"\n\n[[flight]]\nid       = \"read it\"\nassignee = \"me\"\n",
    );

    let out = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert!(out.contains("review  repo\n"), "the layer is named: {out}");
    assert!(out.contains("· read it  me\n"), "{out}");
    assert!(
        !out.contains("verdict"),
        "the built-in is gone whole: {out}"
    );

    // The detail names the file rather than offering a fork of it.
    let detail = stdout(&ff_tower(repo.path(), &["procedures", "review"]));
    assert!(
        detail.contains(&format!(
            "file: {}\n",
            repo.path()
                .join(".tower")
                .join("procedures")
                .join("review.toml")
                .display()
        )),
        "{detail}"
    );

    let one = envelope(&ff_tower(repo.path(), &["procedures", "review", "--json"]));
    assert_eq!(
        one["data"]["procedure"]["source"]["layer"],
        serde_json::json!("repo")
    );
}

#[test]
fn a_definition_that_does_not_load_is_refused_by_path() {
    let repo = repo();
    repo.write(".tower/procedures/broken.toml", "name = \"broken\"\n");
    let out = ff_tower(repo.path(), &["procedures", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("procedure/no-parts")
    );
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower procedures"])
    );
}

#[test]
fn a_name_that_is_not_installed_is_refused_naming_the_set() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["procedures", "ghost", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("procedure/not-found")
    );
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("no procedure `ghost` — installed: open, review")
    );
}

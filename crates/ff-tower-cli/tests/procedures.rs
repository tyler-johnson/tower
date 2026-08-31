//! `ff tower procedures [<name>]` — the read-only half of the registry.
//!
//! The verb spawns no fufu, so no `TOWER_FF` appears here. It does read
//! the registry, so every spawn points `XDG_CONFIG_HOME` at the fixture's
//! own tempdir: a suite that read the developer's real
//! `~/.config/tower/procedures` would pass or fail by whose machine it is.
//!
//! The engine ships empty, so every listing here is over files the test
//! wrote itself — `docs/procedures/`'s two shapes, not anything in the
//! binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ff_tower_testsupport::Repo;

/// `docs/procedures/review.toml`, verbatim enough to assert against.
const REVIEW: &str = "\
name    = \"review\"
subject = \"branch\"

[[match]]
name   = \"github-reviews\"
source = \"github\"
event  = \"review_requested\"

[[flight]]
id       = \"pass\"
assignee = \"agent\"
skill    = \"review\"
done     = \"asserted\"

[[flight]]
id       = \"smoke\"
assignee = \"me\"
bay      = \"warm\"
done     = \"asserted\"

[[flight]]
id       = \"verdict\"
assignee = \"me\"
after    = [\"pass\", \"smoke\"]
done     = \"asserted\"
";

/// `docs/procedures/ticket.toml`: one flight, yours.
const TICKET: &str = "\
name = \"ticket\"

[[flight]]
id       = \"work\"
assignee = \"me\"
done     = \"asserted\"
";

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
fn xdg(repo: &Path) -> PathBuf {
    repo.parent()
        .expect("the fixture nests the repository")
        .join("xdg")
}

/// A definition in the fixture's user layer, under the config root every
/// spawn is pointed at.
fn install_user(repo: &Path, name: &str, text: &str) {
    let dir = xdg(repo).join("tower").join("procedures");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join(format!("{name}.toml")), text).expect("write");
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

/// A repository with both example shapes in its own layer.
fn stocked() -> Repo {
    let repo = repo();
    repo.write(".tower/procedures/review.toml", REVIEW);
    repo.write(".tower/procedures/ticket.toml", TICKET);
    repo
}

#[test]
fn an_empty_registry_says_so_and_where_a_definition_goes() {
    // The engine ships empty, so this is the fresh box's answer — not a
    // bare zero, and not a fault.
    let repo = repo();
    let out = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert_eq!(
        out,
        format!(
            "no procedures installed\n\
             author: {} · {}\n\
             examples: docs/procedures/ in the tower repository\n",
            repo.path()
                .join(".tower")
                .join("procedures")
                .join("<name>.toml")
                .display(),
            xdg(repo.path())
                .join("tower")
                .join("procedures")
                .join("<name>.toml")
                .display()
        )
    );

    // And the JSON form is an empty set rather than an absence.
    let all = envelope(&ff_tower(repo.path(), &["procedures", "--json"]));
    assert_eq!(all["data"]["procedures"], serde_json::json!([]));
}

#[test]
fn the_listing_is_what_is_installed_with_flights_and_lanes() {
    let repo = stocked();
    let out = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert_eq!(
        out,
        "review  repo\n\
         · pass     agent\n\
         · smoke    me\n\
         · verdict  me\n\
         \n\
         ticket  repo\n\
         · work  me\n\
         \n\
         2 procedures · ff tower procedures <name> for one\n"
    );
}

#[test]
fn the_detail_carries_the_flights_the_inert_rule_and_the_file() {
    let repo = stocked();
    let out = stdout(&ff_tower(repo.path(), &["procedures", "review"]));
    assert!(out.starts_with("review  repo\n"), "{out}");
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
            "file: {}\n",
            repo.path()
                .join(".tower")
                .join("procedures")
                .join("review.toml")
                .display()
        )),
        "{out}"
    );
    // Every terminal flight of this shape is yours, so nothing warns.
    assert!(!out.contains("end with you"), "{out}");
}

#[test]
fn the_json_form_is_the_registry_as_data() {
    let repo = stocked();
    let all = envelope(&ff_tower(repo.path(), &["procedures", "--json"]));
    assert_eq!(all["cmd"], serde_json::json!("procedures"));
    let procedures = all["data"]["procedures"].as_array().expect("procedures");
    assert_eq!(procedures.len(), 2);
    assert_eq!(procedures[0]["name"], serde_json::json!("review"));
    assert_eq!(
        procedures[0]["source"],
        serde_json::json!({
            "layer": "repo",
            "path": repo
                .path()
                .join(".tower")
                .join("procedures")
                .join("review.toml")
                .display()
                .to_string(),
        })
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
fn a_repo_definition_overrides_the_user_layers_and_says_so() {
    let repo = repo();
    install_user(repo.path(), "review", REVIEW);
    install_user(repo.path(), "ticket", TICKET);
    repo.write(
        ".tower/procedures/review.toml",
        "name = \"review\"\n\n[[flight]]\nid       = \"read it\"\nassignee = \"me\"\n",
    );

    let out = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert!(out.contains("review  repo\n"), "the layer is named: {out}");
    assert!(out.contains("· read it  me\n"), "{out}");
    assert!(!out.contains("verdict"), "the user's is gone whole: {out}");
    // A name the repository layer does not carry is the user's still.
    assert!(out.contains("ticket  user\n"), "{out}");

    // The detail names the file it was read from.
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
fn a_shape_that_ends_on_an_agent_loads_and_warns_on_both_renders() {
    // DESIGN.md:338, as a warning: the definition still loads, still
    // lists, still files — and both renders say, by name and by flight,
    // that it comes back to no one.
    let repo = repo();
    repo.write(
        ".tower/procedures/script.toml",
        "name = \"script\"\n\n\
         [[flight]]\nid       = \"plan\"\nassignee = \"me\"\n\n\
         [[flight]]\nid       = \"pass\"\nassignee = \"agent\"\nafter    = [\"plan\"]\n",
    );

    let list = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert!(
        list.contains("! ends on agent flight pass — a procedure should end with you\n"),
        "{list}"
    );
    let detail = stdout(&ff_tower(repo.path(), &["procedures", "script"]));
    assert!(
        detail.contains("! ends on agent flight pass — a procedure should end with you\n"),
        "{detail}"
    );

    // One human terminal flight is enough to quiet it: `verdict` closes
    // the review shape, so nothing warns there.
    let repo = stocked();
    let list = stdout(&ff_tower(repo.path(), &["procedures"]));
    assert!(!list.contains("end with you"), "{list}");
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
    let repo = stocked();
    let out = ff_tower(repo.path(), &["procedures", "ghost", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let envelope = envelope(&out);
    assert_eq!(
        envelope["error"]["id"],
        serde_json::json!("procedure/not-found")
    );
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!("no procedure `ghost` — installed: review, ticket")
    );

    // On an empty registry the same refusal says as much rather than
    // trailing an empty list.
    let bare = self::repo();
    let out = ff_tower(bare.path(), &["procedures", "ghost", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        self::envelope(&out)["error"]["message"],
        serde_json::json!("no procedure `ghost` — nothing installed")
    );
}

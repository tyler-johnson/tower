//! Definitions, loaded: DESIGN's `review` block round-tripping to
//! flights and edges, the six refusals, the shipped set, and the three
//! layers.
//!
//! The layering tests call `procedure::layered` with tempdirs rather than
//! `procedure::registry` with `XDG_CONFIG_HOME` set: environment is
//! process-global and these tests run in parallel in one process, so a
//! suite that set a variable would answer for whichever test read it
//! first. The CLI suite sets the variable on the child it spawns, where
//! it is a per-process fact again.

use std::path::Path;

use ff_tower_core::procedure::{self, Assignee, Bay, Done, Source};

/// DESIGN.md's *Procedures* block, verbatim.
const REVIEW: &str = r#"
name    = "review"
subject = "branch"            # may resolve to a PR later

[[match]]                     # only ever runs on adapter signals
source = "github"
event  = "review_requested"

[[flight]]
id       = "pass"
assignee = "agent"
skill    = "review"

[[flight]]
id       = "smoke"
assignee = "me"
bay      = "warm"             # build the tree ahead of me

[[flight]]
id    = "verdict"
assignee = "me"
after = ["pass", "smoke"]
"#;

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::write(dir.join(name), text).expect("write");
}

#[test]
fn designs_review_block_loads_to_its_flights_and_edges() {
    let definition = procedure::load(REVIEW, Source::BuiltIn).expect("loads");
    assert_eq!(definition.name, "review");
    assert_eq!(definition.subject.as_deref(), Some("branch"));

    assert_eq!(definition.matches.len(), 1);
    assert_eq!(definition.matches[0].source, "github");
    assert_eq!(definition.matches[0].event, "review_requested");

    let ids: Vec<&str> = definition
        .flights
        .iter()
        .map(|flight| flight.id.as_str())
        .collect();
    assert_eq!(ids, ["pass", "smoke", "verdict"]);

    let pass = &definition.flights[0];
    assert_eq!(pass.assignee, Assignee::Agent);
    assert_eq!(pass.skill.as_deref(), Some("review"));
    assert!(pass.after.is_empty());
    assert_eq!(pass.done, Done::Asserted);
    assert!(pass.bay.is_none());
    assert!(pass.priority.is_none());
    assert!(pass.labels.is_empty());

    let smoke = &definition.flights[1];
    assert_eq!(smoke.assignee, Assignee::Me);
    assert_eq!(smoke.bay, Some(Bay::Warm));
    // Concurrency is the absence of a declaration: neither names the
    // other, so both fly at once.
    assert!(smoke.after.is_empty());

    let verdict = &definition.flights[2];
    assert_eq!(verdict.assignee, Assignee::Me);
    assert_eq!(verdict.after, ["pass", "smoke"]);
}

#[test]
fn a_procedure_with_no_flights_is_refused() {
    let err = procedure::load(r#"name = "empty""#, Source::BuiltIn).expect_err("refused");
    assert_eq!(err.id(), "procedure/no-parts");
    assert!(err.to_string().contains("empty"), "{err}");
}

#[test]
fn a_duplicate_flight_id_is_refused() {
    let err = procedure::load(
        r#"
name = "twice"
[[flight]]
id       = "same"
assignee = "me"
[[flight]]
id       = "same"
assignee = "me"
"#,
        Source::BuiltIn,
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/duplicate-part");
    assert!(err.to_string().contains("`same`"), "{err}");
}

#[test]
fn an_after_naming_nothing_is_refused() {
    let err = procedure::load(
        r#"
name = "typo"
[[flight]]
id       = "first"
assignee = "agent"
[[flight]]
id       = "last"
assignee = "me"
after    = ["frist"]
"#,
        Source::BuiltIn,
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/unknown-after");
    assert!(err.to_string().contains("`frist`"), "{err}");
}

#[test]
fn a_cycle_through_after_is_refused() {
    let err = procedure::load(
        r#"
name = "circular"
[[flight]]
id       = "a"
assignee = "me"
after    = ["b"]
[[flight]]
id       = "b"
assignee = "me"
after    = ["a"]
"#,
        Source::BuiltIn,
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/cyclic");

    // A part that waits on itself is the one-node case of the same thing.
    let err = procedure::load(
        r#"
name = "selfish"
[[flight]]
id       = "only"
assignee = "me"
after    = ["only"]
"#,
        Source::BuiltIn,
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/cyclic");
}

#[test]
fn a_procedure_that_does_not_end_with_you_is_refused() {
    // Principle 12, at load: nothing waits on `pass`, so the procedure
    // ends on an agent and is a script.
    let err = procedure::load(
        r#"
name = "script"
[[flight]]
id       = "plan"
assignee = "me"
[[flight]]
id       = "pass"
assignee = "agent"
after    = ["plan"]
"#,
        Source::BuiltIn,
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/no-human-end");
    assert!(err.to_string().contains("`pass`"), "{err}");
    assert!(err.to_string().contains("ends with you"), "{err}");

    // Terminal is about the edges, not the order: a part declared last
    // that something waits on is not an end, and one declared first that
    // nothing waits on is.
    let err = procedure::load(
        r#"
name = "parallel"
[[flight]]
id       = "loose"
assignee = "agent"
[[flight]]
id       = "mine"
assignee = "me"
"#,
        Source::BuiltIn,
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/no-human-end");
    assert!(err.to_string().contains("`loose`"), "{err}");
}

#[test]
fn toml_that_does_not_parse_is_refused_by_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user = dir.path().join("user");
    write(&user, "broken.toml", "name = \"broken\"\n[[flight\n");

    let err = procedure::layered(Some(&user), None).expect_err("refused");
    assert_eq!(err.id(), "procedure/invalid");
    let message = err.to_string();
    assert!(message.contains("broken.toml"), "names the path: {message}");
    assert!(!message.contains('\n'), "one line: {message}");
}

#[test]
fn the_shipped_set_is_open_and_review_both_built_in() {
    let installed = procedure::layered(None, None).expect("the built-ins load");
    assert_eq!(installed.names(), ["open", "review"]);
    for definition in installed.definitions() {
        assert_eq!(definition.source, Source::BuiltIn);
    }

    // `open` is the single-flight case the collapse rule turns on.
    let open = installed.get("open").expect("open");
    assert_eq!(open.flights.len(), 1);
    assert_eq!(open.flights[0].assignee, Assignee::Me);
}

#[test]
fn a_user_file_overrides_a_built_in_and_a_repo_file_overrides_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user = dir.path().join("user");
    let repo = dir.path().join("repo");

    write(
        &user,
        "review.toml",
        r#"
name = "review"
[[flight]]
id       = "mine"
assignee = "me"
"#,
    );

    let installed = procedure::layered(Some(&user), None).expect("loads");
    let review = installed.get("review").expect("review");
    assert_eq!(review.source, Source::User(user.join("review.toml")));
    // Wholesale, never field by field: the built-in's three flights are
    // gone, not merged with.
    let ids: Vec<&str> = review
        .flights
        .iter()
        .map(|flight| flight.id.as_str())
        .collect();
    assert_eq!(ids, ["mine"]);
    assert!(review.subject.is_none());
    assert_eq!(installed.get("open").expect("open").source, Source::BuiltIn);

    write(
        &repo,
        "review.toml",
        r#"
name = "review"
[[flight]]
id       = "ours"
assignee = "me"
"#,
    );

    let installed = procedure::layered(Some(&user), Some(&repo)).expect("loads");
    let review = installed.get("review").expect("review");
    assert_eq!(review.source, Source::Repo(repo.join("review.toml")));
    assert_eq!(review.flights[0].id, "ours");
    assert_eq!(installed.names(), ["open", "review"]);
}

#[test]
fn a_layer_is_keyed_by_the_name_inside_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user = dir.path().join("user");
    write(
        &user,
        "anything.toml",
        r#"
name = "chore"
[[flight]]
id       = "only"
assignee = "me"
"#,
    );

    let installed = procedure::layered(Some(&user), None).expect("loads");
    assert_eq!(installed.names(), ["chore", "open", "review"]);
    assert!(installed.get("anything").is_none());
}

#[test]
fn a_missing_directory_is_an_empty_layer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = procedure::layered(Some(&dir.path().join("nope")), None).expect("not an error");
    assert_eq!(installed.names(), ["open", "review"]);
}

#[test]
fn a_file_that_is_not_toml_is_left_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user = dir.path().join("user");
    write(&user, "README.md", "how to fork a procedure\n");
    let installed = procedure::layered(Some(&user), None).expect("loads");
    assert_eq!(installed.names(), ["open", "review"]);
}

//! Definitions, loaded: DESIGN's `review` block round-tripping to
//! flights and edges, the six refusals, the human-end warning, and the
//! two layers over an engine that ships empty.
//!
//! The layering tests call `procedure::layered` with tempdirs rather than
//! `procedure::registry` with `XDG_CONFIG_HOME` set: environment is
//! process-global and these tests run in parallel in one process, so a
//! suite that set a variable would answer for whichever test read it
//! first. The CLI suite sets the variable on the child it spawns, where
//! it is a per-process fact again.

use std::path::{Path, PathBuf};

use ff_tower_core::procedure::{self, Assignee, Bay, Done, Source};

/// DESIGN.md's *Procedures* block, verbatim.
const REVIEW: &str = r#"
name    = "review"
subject = "branch"            # may resolve to a PR later

[[match]]                     # adapter-keyed, so inert until an adapter can fire it
name   = "github-reviews"
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

/// The ticket shape, `docs/procedures/ticket.toml`'s: one flight, yours.
const TICKET: &str = r#"
name = "ticket"

[[flight]]
id       = "work"
assignee = "me"
done     = "asserted"
"#;

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::write(dir.join(name), text).expect("write");
}

/// The source a hand-written definition is loaded under: a refusal names
/// the definition by its path, so the tests read as a real file's would.
fn at(name: &str) -> Source {
    Source::Repo(PathBuf::from(name))
}

#[test]
fn designs_review_block_loads_to_its_flights_and_edges() {
    let definition = procedure::load(REVIEW, at("review.toml")).expect("loads");
    assert_eq!(definition.name, "review");
    assert_eq!(definition.subject.as_deref(), Some("branch"));

    assert_eq!(definition.matches.len(), 1);
    assert_eq!(definition.matches[0].name, "github-reviews");
    assert_eq!(definition.matches[0].source.as_deref(), Some("github"));
    assert_eq!(
        definition.matches[0].event.as_deref(),
        Some("review_requested")
    );

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
    let err = procedure::load(r#"name = "empty""#, at("empty.toml")).expect_err("refused");
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
        at("twice.toml"),
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
        at("typo.toml"),
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
        at("circular.toml"),
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
        at("selfish.toml"),
    )
    .expect_err("refused");
    assert_eq!(err.id(), "procedure/cyclic");
}

#[test]
fn a_procedure_that_does_not_end_with_you_loads_and_warns() {
    // DESIGN.md:338, and it warns rather than refuses: the file is
    // personal, and the boundary that actually holds is
    // `never auto-outward`.
    let definition = procedure::load(
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
        at("script.toml"),
    )
    .expect("a shape that ends on an agent still loads");
    assert_eq!(definition.no_human_end(), Some(vec!["pass"]));

    // Terminal is about the edges, not the order: a flight declared last
    // that something waits on is not an end, and one declared first that
    // nothing waits on is. Both terminals here are agents.
    let definition = procedure::load(
        r#"
name = "parallel"
[[flight]]
id       = "loose"
assignee = "agent"
[[flight]]
id       = "also"
assignee = "agent"
"#,
        at("parallel.toml"),
    )
    .expect("loads");
    assert_eq!(definition.no_human_end(), Some(vec!["loose", "also"]));
}

#[test]
fn one_human_terminal_flight_is_enough_to_stay_silent() {
    // *All* the terminal flights, not any: a shape whose last human
    // flight sits beside an agent one is fine, because one human close
    // is the boundary the rule is about.
    let definition = procedure::load(
        r#"
name = "beside"
[[flight]]
id       = "loose"
assignee = "agent"
[[flight]]
id       = "mine"
assignee = "me"
"#,
        at("beside.toml"),
    )
    .expect("loads");
    assert!(definition.no_human_end().is_none());

    // And the ordinary shape: everything funnels into one flight of
    // yours.
    let definition = procedure::load(REVIEW, at("review.toml")).expect("loads");
    assert!(definition.no_human_end().is_none());
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
fn the_engine_ships_empty() {
    // Principle 12: no layer sits under the two, so a box with neither
    // directory has nothing installed.
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = procedure::layered(None, None).expect("not an error");
    assert!(installed.is_empty());
    assert!(
        procedure::layered(
            Some(&dir.path().join("nope")),
            Some(&dir.path().join("nor"))
        )
        .expect("a missing directory is an empty layer")
        .is_empty()
    );
}

#[test]
fn a_repo_file_replaces_a_user_file_wholesale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user = dir.path().join("user");
    let repo = dir.path().join("repo");

    write(&user, "review.toml", REVIEW);
    write(&user, "ticket.toml", TICKET);

    let installed = procedure::layered(Some(&user), None).expect("loads");
    assert_eq!(installed.names(), ["review", "ticket"]);
    let review = installed.get("review").expect("review");
    assert_eq!(review.source, Source::User(user.join("review.toml")));
    assert_eq!(review.flights.len(), 3);

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
    // Wholesale, never field by field: the user file's three flights are
    // gone, not merged with.
    let ids: Vec<&str> = review
        .flights
        .iter()
        .map(|flight| flight.id.as_str())
        .collect();
    assert_eq!(ids, ["ours"]);
    assert!(review.subject.is_none());
    assert_eq!(installed.names(), ["review", "ticket"]);
    // A name the repository layer does not carry is the user's still.
    assert_eq!(
        installed.get("ticket").expect("ticket").source,
        Source::User(user.join("ticket.toml"))
    );
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
    assert_eq!(installed.names(), ["chore"]);
    assert!(installed.get("anything").is_none());
}

#[test]
fn a_file_that_is_not_toml_is_left_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let user = dir.path().join("user");
    write(&user, "README.md", "how to fork a procedure\n");
    write(&user, "ticket.toml", TICKET);
    let installed = procedure::layered(Some(&user), None).expect("loads");
    assert_eq!(installed.names(), ["ticket"]);
}

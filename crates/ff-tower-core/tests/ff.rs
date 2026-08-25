//! The seam, against a real fufu and against fakes.
//!
//! The split is deliberate. Anything a working `ff` will produce is tested
//! against a working `ff`, because the point of these is to fail when the
//! contract moves — a hand-written envelope would only prove tower parses
//! tower. The fakes cover what a healthy fufu will not emit on request: a
//! contract from the future, a held exit, output that is not an envelope.

use ff_tower_core::ff::{Error, Exit, Ff, Head, Pairing};
use ff_tower_testsupport::{FakeFf, Repo};

// ---------------------------------------------------------------- real fufu

#[test]
fn status_reads_a_real_repository() {
    let repo = Repo::new();
    let status = Ff::at(repo.path()).status().expect("status");

    assert_eq!(status.head.branch(), Some("main"));
    assert!(status.open.clean, "a fresh commit leaves a clean tree");
    assert!(status.changes.is_empty());
    assert!(status.conflicts.is_empty());
    assert!(status.held.is_none());
    assert!(status.editing.is_none());
}

#[test]
fn status_sees_work_that_has_not_been_committed() {
    // The thesis, at the smallest scale that can prove it: an edit with no
    // commit behind it is visible, and it is visible in `open`.
    let repo = Repo::new();
    repo.write("src/main.rs", "fn main() {}\n");

    let status = Ff::at(repo.path()).status().expect("status");

    assert!(!status.open.clean);
    assert!(
        status.open.time.is_some(),
        "an uncommitted edit still has a snapshot time"
    );
    let paths: Vec<&str> = status.changes.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["src/main.rs"]);
    assert!(status.insertions > 0);
}

#[test]
fn collide_reads_the_sideways_axis() {
    let repo = Repo::new();
    repo.ff(&["start", "-b", "left"]);
    repo.write("shared.txt", "left side\n");
    repo.ff(&["commit", "-m", "left: touch shared"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("shared.txt", "right side\n");
    repo.ff(&["commit", "-m", "right: touch shared"]);

    let collisions = Ff::at(repo.path()).collide(&[], None).expect("collide");

    let pair = collisions
        .pairs
        .iter()
        .find(|p| {
            let names = [p.a.as_str(), p.b.as_str()];
            names.contains(&"left") && names.contains(&"right")
        })
        .expect("left and right were both ranked in");

    match &pair.pairing {
        Pairing::Collide { paths } => assert_eq!(paths, &["shared.txt"]),
        other => panic!("two edits to one file should collide, got {other:?}"),
    }
    assert!(!pair.pairing.is_clear());
    assert!(
        !(collisions.clear.contains(&"left".to_string())
            && collisions.clear.contains(&"right".to_string())),
        "the clear set must not hold both sides of a collision: {:?}",
        collisions.clear
    );
}

#[test]
fn collide_is_clear_when_branches_touch_different_files() {
    let repo = Repo::new();
    repo.ff(&["start", "-b", "left"]);
    repo.write("left.txt", "left\n");
    repo.ff(&["commit", "-m", "left: its own file"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("right.txt", "right\n");
    repo.ff(&["commit", "-m", "right: its own file"]);

    let collisions = Ff::at(repo.path()).collide(&[], None).expect("collide");
    let pair = collisions
        .pairs
        .iter()
        .find(|p| {
            let names = [p.a.as_str(), p.b.as_str()];
            names.contains(&"left") && names.contains(&"right")
        })
        .expect("both ranked in");
    assert!(pair.pairing.is_clear(), "got {:?}", pair.pairing);
}

#[test]
fn a_refusal_comes_back_with_its_id() {
    // fufu's own error envelope, raised by pointing at somewhere that is not
    // a repository. The id is the stable half and the part tower may match
    // on; the message is fufu's wording and is not asserted.
    let empty = tempfile::tempdir().expect("tempdir");
    let err = Ff::at(empty.path()).status().expect_err("not a repository");

    assert_eq!(err.ff_id(), Some("repo/not-found"));
    match err {
        Error::Ff(refusal) => {
            assert!(!refusal.message.is_empty());
            assert!(
                refusal.exits.iter().any(|exit| exit.starts_with("ff ")),
                "a refusal should carry the way out: {:?}",
                refusal.exits
            );
        }
        other => panic!("expected a fufu refusal, got {other:?}"),
    }
}

#[test]
fn the_session_tag_reaches_the_capture_chain() {
    // The design's rule: every fufu call tower makes carries `--session
    // <flight>`. This proves the flag arrives where it is supposed to —
    // fufu records it on the operation the call captured.
    let repo = Repo::new();
    repo.write("tagged.txt", "work an agent did\n");

    Ff::at(repo.path())
        .session("flight-7")
        .status()
        .expect("status");

    let evolog = repo.ff(&["evolog", "--json", "--session", "flight-7"]);
    assert!(
        evolog.contains("flight-7"),
        "the session tag never reached the log: {evolog}"
    );
}

// --------------------------------------------------------------- fake fufu

/// The argv tower builds, read back from a fake that echoes it.
fn argv(ff: &Ff, verb: &str) -> Vec<String> {
    let fake = FakeFf::script(
        r#"for a in "$@"; do printf '%s\n' "$a" >> "$0.argv"; done
printf '{"ff":1,"cmd":"status","data":null}\n'
"#,
    );
    let _ = ff
        .clone()
        .program(fake.path())
        .run::<Option<()>>(verb, &[] as &[&str]);
    let recorded = std::fs::read_to_string(fake.dir().join("ff.argv")).expect("argv recorded");
    recorded.lines().map(str::to_string).collect()
}

#[test]
fn every_call_addresses_its_repository_and_puts_json_after_the_verb() {
    let args = argv(&Ff::at("/some/bay"), "status");

    assert_eq!(args[0], "-C", "a bay is addressed, not chdir'd into");
    assert_eq!(args[1], "/some/bay");
    let verb = args.iter().position(|a| a == "status").expect("the verb");
    assert_eq!(
        args[verb + 1],
        "--json",
        "--json belongs to the verb, so nothing positional can swallow it"
    );
}

#[test]
fn a_session_handle_tags_every_call() {
    let args = argv(&Ff::at("/some/bay").session("flight-7"), "status");
    let at = args
        .iter()
        .position(|a| a == "--session")
        .expect("the session flag");
    assert_eq!(args[at + 1], "flight-7");
    assert!(
        at < args.iter().position(|a| a == "status").unwrap(),
        "--session is fufu's flag and sits before the verb"
    );
}

#[test]
fn a_contract_tower_does_not_read_is_refused_before_the_payload() {
    // The payload here is nonsense for a status. The point is that the
    // version is checked first, so the error names the contract rather than
    // a field that moved inside it.
    let fake = FakeFf::saying(r#"{"ff":99,"cmd":"status","data":{"nothing":true}}"#, 0);
    let err = Ff::at("/repo")
        .program(fake.path())
        .status()
        .expect_err("contract 99");

    match err {
        Error::Contract {
            expected, found, ..
        } => {
            assert_eq!(expected, ff_tower_core::ff::CONTRACT);
            assert_eq!(found, 99);
        }
        other => panic!("expected a contract error, got {other:?}"),
    }
}

#[test]
fn an_envelope_answering_for_another_verb_is_refused() {
    let fake = FakeFf::saying(r#"{"ff":1,"cmd":"collide","data":{}}"#, 0);
    let err = Ff::at("/repo")
        .program(fake.path())
        .status()
        .expect_err("wrong verb");

    match err {
        Error::Mismatched { asked, answered } => {
            assert_eq!(asked, "status");
            assert_eq!(answered, "collide");
        }
        other => panic!("expected a mismatch, got {other:?}"),
    }
}

#[test]
fn held_is_an_outcome_and_not_an_error() {
    // Exit 3 with a good envelope: something happened, it has a report, and
    // nothing moved. The payload comes back and the code rides beside it.
    let fake = FakeFf::saying(r#"{"ff":1,"cmd":"sync","data":{"landed":false}}"#, 3);
    let run = Ff::at("/repo")
        .program(fake.path())
        .run::<serde_json::Value>("sync", &[] as &[&str])
        .expect("a held verb still answered");

    assert_eq!(run.exit, Exit::Held);
    assert_eq!(run.data["landed"], serde_json::json!(false));
}

#[test]
fn a_usage_error_with_no_envelope_says_what_the_process_said() {
    // What `ff nosuchverb --json` actually does: clap writes usage to stderr
    // and no envelope is emitted at all. The error has to carry that, or it
    // is undiagnosable.
    let fake = FakeFf::script("echo 'error: unrecognized subcommand' >&2\nexit 2\n");
    let err = Ff::at("/repo")
        .program(fake.path())
        .status()
        .expect_err("no envelope");

    match err {
        Error::Unparsable { ref stderr, .. } => {
            assert!(stderr.contains("unrecognized subcommand"), "got {stderr:?}");
        }
        other => panic!("expected unparsable, got {other:?}"),
    }
    assert!(err.ff_id().is_none(), "this is not fufu saying no");
}

#[test]
fn a_missing_ff_says_so_in_its_own_words() {
    let err = Ff::at("/repo")
        .program("/nonexistent/ff")
        .status()
        .expect_err("no such program");

    match err {
        Error::NotInstalled { ref program } => assert_eq!(program, "/nonexistent/ff"),
        other => panic!("expected NotInstalled, got {other:?}"),
    }
    assert!(
        err.to_string().contains("fufu"),
        "the message should name the dependency: {err}"
    );
}

#[test]
fn an_unknown_field_does_not_break_a_parse() {
    // fufu grows payload fields between releases without moving the
    // contract. A tower that failed to render because of one would be broken
    // by an upgrade that broke nothing.
    let fake = FakeFf::saying(
        r#"{"ff":1,"cmd":"status","data":{"head":{"state":"branch","name":"main","ref":"refs/heads/main","commit":"abc"},"open":{"id":null,"id_letters":null,"pending":null,"subject":null,"clean":true,"base":null,"time":null},"changes":[],"insertions":0,"deletions":0,"conflicts":[],"held":null,"session":null,"a_field_from_next_year":42}}"#,
        0,
    );
    let status = Ff::at("/repo")
        .program(fake.path())
        .status()
        .expect("unknown fields are ignored");
    assert_eq!(status.head.branch(), Some("main"));
    assert!(matches!(status.head, Head::Branch { .. }));
}

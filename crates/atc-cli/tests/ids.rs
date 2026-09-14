//! The two names of a flight: the dense human number against the sparse
//! wire id, and every spelling a verb accepts.

use std::path::Path;
mod support;
use std::process::{Command, Output};

use atc_testsupport::{Repo, scrub};

fn atc(repo: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_atc"));
    scrub(&mut command);
    command
        .args(args)
        .current_dir(repo)
        .env("XDG_CONFIG_HOME", xdg(repo))
        .output()
        .expect("spawn atc")
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

/// Assert a refusal: the exit code, and the envelope's error id.
fn refusal(output: &Output, code: i32, id: &str) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let envelope = envelope(output);
    assert_eq!(envelope["error"]["id"], serde_json::json!(id));
    envelope
}

fn repo() -> Repo {
    let repo = Repo::new();
    repo.pin_writer("pi");
    repo
}

/// File, comment, file: the comment consumes seq 2, so the second filing
/// is event `pi.3` — and flight #2.
fn repo_with_a_seq_gap() -> Repo {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the first flight"]));
    stdout(&atc(repo.path(), &["comment", "1", "-m", "a note"]));
    stdout(&atc(repo.path(), &["file", "the second flight"]));
    repo
}

#[test]
fn the_flight_number_is_dense_while_the_event_seq_is_not() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "the first flight"]));
    stdout(&atc(repo.path(), &["comment", "1", "-m", "a note"]));
    let out = stdout(&atc(repo.path(), &["file", "the second flight"]));
    assert_eq!(out, "filed #2 in ready: the second flight\nboard: atc\n");

    let rendered = stdout(&atc(repo.path(), &[]));
    assert!(rendered.contains("#2"), "got {rendered}");

    // The split, visible on one view: flight #2 rides event pi.3.
    let board = envelope(&atc(repo.path(), &["--json"]));
    let second = &board["data"]["ready"][1];
    assert_eq!(second["id"], support::flight(repo.path(), 2));
    assert_eq!(second["display"], serde_json::json!("#2"));
    assert_eq!(second["writer"], serde_json::json!("pi"));
    assert!(second.get("number").is_none());
}

#[test]
fn both_names_brief_the_same_flight() {
    let repo = repo_with_a_seq_gap();
    let wire = support::flight(repo.path(), 2);
    for reference in ["2", "pi#2", wire.as_str()] {
        let brief = envelope(&atc(repo.path(), &["brief", reference, "--json"]));
        assert_eq!(brief["data"]["id"], wire, "`{reference}`");
        assert_eq!(
            brief["data"]["display"],
            serde_json::json!("#2"),
            "`{reference}`"
        );
    }
}

#[test]
fn done_by_number_finishes_the_second_filed_flight_not_event_two() {
    let repo = repo_with_a_seq_gap();
    let out = stdout(&atc(repo.path(), &["done", "2"]));
    assert_eq!(out, "done #2: the second flight\nboard: atc\n");

    let board = envelope(&atc(repo.path(), &["--json"]));
    let open = board["data"]["ready"].as_array().expect("open");
    let ids: Vec<&str> = open.iter().filter_map(|view| view["id"].as_str()).collect();
    assert_eq!(ids, ["pi.1"], "the first flight stays");
}

#[test]
fn two_writers_have_distinct_global_numbers_and_keep_ordinal_aliases() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "from the pi"]));
    repo.pin_writer("qi");
    stdout(&atc(repo.path(), &["file", "from the qi"]));

    let out = stdout(&atc(repo.path(), &[]));
    assert!(out.contains("#1"), "global name: {out}");
    assert!(out.contains("#2"), "global name: {out}");
    assert!(!out.contains("pi.1"), "the wire form never renders: {out}");

    let out = atc(repo.path(), &["comment", "1", "-m", "x", "--json"]);
    assert_eq!(
        envelope(&out)["data"]["commented"]["body"]["flight"],
        "pi.1"
    );
    stdout(&out);

    // `writer#n` resolves exactly; the pasted `#`-prefixed form too.
    stdout(&atc(repo.path(), &["comment", "qi#1", "-m", "one"]));
    stdout(&atc(repo.path(), &["comment", "#qi#1", "-m", "two"]));
    let board = envelope(&atc(repo.path(), &["--json"]));
    let open = board["data"]["ready"].as_array().expect("open");
    let qi = open
        .iter()
        .find(|view| view["id"] == serde_json::json!("qi.1"))
        .expect("qi.1 in open");
    assert_eq!(qi["comments"], serde_json::json!(2));

    let out = atc(repo.path(), &["comment", "qi#9", "-m", "x", "--json"]);
    refusal(&out, 1, "flight/not-found");
}

#[test]
fn display_names_include_writers_hidden_by_the_closed_window() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "still open"]));
    repo.pin_writer("qi");
    stdout(&atc(repo.path(), &["file", "now hidden"]));
    stdout(&atc(repo.path(), &["done", "qi#1"]));
    let board = envelope(&atc(repo.path(), &["--closed", "none", "--json"]));
    let rows = board["data"]["ready"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "pi.1");
    assert_eq!(rows[0]["writer"], "pi");
    assert_eq!(rows[0]["display"], "#1");
    assert!(rows[0].get("number").is_none());
    let rendered = stdout(&atc(repo.path(), &["--closed", "none"]));
    assert!(rendered.contains("#1"), "{rendered}");
    assert!(
        !rendered.contains("qi#1"),
        "the other writer is hidden: {rendered}"
    );
    for (reference, display) in [("pi#1", "#1"), ("qi#1", "#2")] {
        let brief = envelope(&atc(repo.path(), &["brief", reference, "--json"]));
        assert_eq!(brief["data"]["display"], display);
        assert!(brief["data"].get("number").is_none());
        let text = stdout(&atc(repo.path(), &["brief", reference]));
        let wire = brief["data"]["id"].as_str().unwrap();
        assert!(text.contains(&format!("{wire} · {reference}")), "{text}");
    }
    let picked = envelope(&atc(repo.path(), &["next", "none", "--peek", "--json"]));
    let pick = &picked["data"]["picked"][0];
    for key in ["id", "writer", "display"] {
        assert_eq!(pick[key], rows[0][key], "{key}");
    }
    assert!(pick.get("number").is_none());
    let out = atc(repo.path(), &["file", "another", "--json"]);
    let filed = envelope(&out);
    assert_eq!(filed["data"]["flights"][0]["display"], "#3");
}

#[test]
fn global_numbers_in_prose_resolve_across_writers() {
    let repo = repo();
    stdout(&atc(repo.path(), &["file", "from the pi"]));
    repo.pin_writer("qi");
    stdout(&atc(repo.path(), &["file", "from the qi"]));

    let out = atc(repo.path(), &["comment", "pi#1", "-m", "see #1", "--json"]);
    stdout(&out);
    assert_eq!(
        envelope(&out)["data"]["commented"]["body"]["text"],
        "see #pi.1"
    );

    // The exact form is stored by wire id and printed long, like a row.
    stdout(&atc(repo.path(), &["comment", "pi#1", "-m", "see qi#1"]));
    let brief = envelope(&atc(repo.path(), &["brief", "pi#1", "--json"]));
    assert_eq!(
        brief["data"]["comments"][1]["text"],
        serde_json::json!("see #qi.1")
    );
    let text = stdout(&atc(repo.path(), &["brief", "pi#1"]));
    assert!(text.contains("see #2"), "{text}");
    let text = stdout(&atc(repo.path(), &["brief", "qi#1"]));
    assert!(
        text.contains("referenced by\n· #1  from the pi\n"),
        "{text}"
    );
}

#[test]
fn a_bad_reference_names_the_reference_spellings() {
    let repo = repo();
    let out = atc(repo.path(), &["comment", "not-an-id", "-m", "x", "--json"]);
    let envelope = refusal(&out, 2, "usage/bad-flight");
    assert_eq!(
        envelope["error"]["message"],
        serde_json::json!(
            "`not-an-id` is not a flight — `<n>`, `<writer>#<n>`, `~<n>`, `<writer>~<n>`, or `<writer>.<seq>`"
        )
    );
}

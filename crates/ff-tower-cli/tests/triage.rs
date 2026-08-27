//! `ff tower triage` against real repositories: the pile, the route, the
//! collapse and the parent shape, the override, and the refusal matrix.
//!
//! Bare triage and the route spawn no fufu, but the route reads the
//! procedure registry, so every spawn points `XDG_CONFIG_HOME` at the
//! fixture's own tempdir; repo-layer definitions land in
//! `.tower/procedures` inside the working tree.

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

/// A single-part procedure in the repository layer — the collapse shape.
fn install_chore(repo: &Repo) {
    repo.write(
        ".tower/procedures/chore.toml",
        "name = \"chore\"\n\n[[part]]\nid   = \"do\"\ncrew = \"you\"\n",
    );
}

/// A two-part procedure — an agent-crewed part with a you-crewed end, the
/// minimal shape principle 12 admits.
fn install_pipeline(repo: &Repo) {
    repo.write(
        ".tower/procedures/pipeline.toml",
        concat!(
            "name = \"pipeline\"\n\n",
            "[[part]]\nid   = \"pass\"\ncrew = \"agent\"\n\n",
            "[[part]]\nid    = \"verdict\"\ncrew  = \"you\"\nafter = [\"pass\"]\n",
        ),
    );
}

#[test]
fn the_pile_lists_open_flights_and_omits_classified_and_done_ones() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(repo.path(), &["file", "still unclassified"]));
    stdout(&ff_tower(
        repo.path(),
        &["file", "already routed", "-p", "chore"],
    ));
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "3"]));
    // A claim does not classify — the flight stays in the pile.
    stdout(&ff_tower(repo.path(), &["claim", "1"]));

    let out = ff_tower(repo.path(), &["triage"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("still unclassified"), "{text}");
    assert!(text.contains("claimed by"), "{text}");
    assert!(!text.contains("already routed"), "{text}");
    assert!(!text.contains("finished"), "{text}");
    assert!(text.contains("1 flight unclassified"), "{text}");
}

#[test]
fn an_empty_pile_says_so_and_exits_0() {
    let repo = repo();
    let out = ff_tower(repo.path(), &["triage"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout(&out), "nothing unclassified\n");
}

#[test]
fn the_pile_under_json_pins_its_row_shape() {
    let repo = repo();
    stdout(&ff_tower(repo.path(), &["file", "needs a look"]));
    // `hold` exits 3 by design — an outcome, not an error.
    let held = ff_tower(repo.path(), &["hold", "1", "-m", "which repo?"]);
    assert_eq!(held.status.code(), Some(3));

    let out = ff_tower(repo.path(), &["triage", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    assert_eq!(envelope["tower"], serde_json::json!(1));
    assert_eq!(envelope["cmd"], serde_json::json!("triage"));
    let pile = envelope["data"]["pile"].as_array().expect("pile");
    assert_eq!(pile.len(), 1);
    assert_eq!(pile[0]["flight"], serde_json::json!("pi.1"));
    assert_eq!(pile[0]["number"], serde_json::json!(1));
    assert_eq!(pile[0]["subject"], serde_json::json!("needs a look"));
    assert!(pile[0]["filed_by"].is_string(), "{pile:?}");
    assert!(pile[0]["filed_at"].is_i64(), "{pile:?}");
    assert_eq!(pile[0]["claimed_by"], serde_json::Value::Null);
    assert_eq!(pile[0]["question"], serde_json::json!("which repo?"));
}

#[test]
fn a_route_to_a_single_part_procedure_collapses_onto_the_flight() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(repo.path(), &["file", "sort the pile"]));

    let out = ff_tower(
        repo.path(),
        &[
            "triage",
            "1",
            "-p",
            "chore",
            "-m",
            "it is a chore",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    let routed = &envelope["data"]["routed"];
    assert_eq!(routed["id"], serde_json::json!("pi.2"));
    assert_eq!(routed["kind"], serde_json::json!("routed"));
    assert_eq!(routed["body"]["flight"], serde_json::json!("pi.1"));
    assert_eq!(routed["body"]["procedure"], serde_json::json!("chore"));
    assert_eq!(
        routed["body"]["because"],
        serde_json::json!("it is a chore")
    );
    assert_eq!(
        routed["body"]["part"],
        serde_json::json!({"id": "do", "crew": "you", "done": "asserted"})
    );
    assert_eq!(envelope["data"]["parts"], serde_json::json!([]));
    assert_eq!(envelope["data"]["linked"], serde_json::json!([]));

    // The board carries the new stamp, and the brief explains it.
    let brief = envelope_of(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["procedure"], serde_json::json!("chore"));
    assert_eq!(brief["data"]["part"]["id"], serde_json::json!("do"));
    assert!(brief["data"]["routed_by"].is_string(), "{brief}");
    assert_eq!(brief["data"]["because"], serde_json::json!("it is a chore"));

    let brief = stdout(&ff_tower(repo.path(), &["brief", "1"]));
    assert!(brief.contains("routed chore · by"), "{brief}");
    assert!(brief.contains("it is a chore"), "{brief}");
}

fn envelope_of(output: &Output) -> serde_json::Value {
    assert!(output.status.success(), "exit {:?}", output.status.code());
    envelope(output)
}

#[test]
fn the_route_echo_names_the_procedure_and_the_because() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(repo.path(), &["file", "sort the pile"]));

    let out = stdout(&ff_tower(
        repo.path(),
        &["triage", "1", "-p", "chore", "-m", "it is a chore"],
    ));
    assert_eq!(
        out,
        "routed #1 to chore: sort the pile\nit is a chore\nboard: ff tower\n"
    );
}

#[test]
fn a_route_to_a_multi_part_procedure_makes_the_flight_a_parent() {
    let repo = repo();
    install_pipeline(&repo);
    stdout(&ff_tower(repo.path(), &["file", "the branch"]));

    let out = ff_tower(repo.path(), &["triage", "1", "-p", "pipeline", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let envelope = envelope(&out);
    let routed = &envelope["data"]["routed"];
    assert_eq!(routed["body"]["procedure"], serde_json::json!("pipeline"));
    assert!(
        routed["body"].get("part").is_none() || routed["body"]["part"].is_null(),
        "a parent carries no stamp: {routed}"
    );
    let parts = envelope["data"]["parts"].as_array().expect("parts");
    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts[0]["body"]["subject"],
        serde_json::json!("the branch · pass")
    );
    assert_eq!(parts[0]["body"]["part"]["crew"], serde_json::json!("agent"));
    assert_eq!(
        parts[1]["body"]["subject"],
        serde_json::json!("the branch · verdict")
    );
    let linked = envelope["data"]["linked"].as_array().expect("linked");
    let edges: Vec<(String, String)> = linked
        .iter()
        .map(|event| {
            (
                event["body"]["from"].as_str().expect("from").to_string(),
                event["body"]["to"].as_str().expect("to").to_string(),
            )
        })
        .collect();
    // The parent waits on both parts, and `verdict` waits on `pass`.
    assert!(edges.contains(&(
        "pi.1".to_string(),
        parts[0]["id"].as_str().unwrap().to_string()
    )));
    assert!(edges.contains(&(
        "pi.1".to_string(),
        parts[1]["id"].as_str().unwrap().to_string()
    )));
    assert!(edges.contains(&(
        parts[1]["id"].as_str().unwrap().to_string(),
        parts[0]["id"].as_str().unwrap().to_string()
    )));

    // `next` now claims the agent part, and only it.
    let out = ff_tower(repo.path(), &["next"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("claimed #2: the branch · pass"), "{text}");

    // Routed off `open`, the parent leaves the pile.
    let pile = stdout(&ff_tower(repo.path(), &["triage"]));
    assert_eq!(pile, "nothing unclassified\n");
}

#[test]
fn a_second_route_wins_and_a_route_back_to_open_is_the_undo() {
    let repo = repo();
    install_chore(&repo);
    install_pipeline(&repo);
    stdout(&ff_tower(repo.path(), &["file", "the branch"]));
    stdout(&ff_tower(repo.path(), &["triage", "1", "-p", "pipeline"]));
    stdout(&ff_tower(repo.path(), &["triage", "1", "-p", "chore"]));

    let brief = envelope_of(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["procedure"], serde_json::json!("chore"));
    assert_eq!(brief["data"]["part"]["id"], serde_json::json!("do"));

    // The first route's part flights stay live, closed by hand — the
    // pile does not reclaim them, they are `pipeline`-stamped.
    let pile = stdout(&ff_tower(repo.path(), &["triage"]));
    assert!(!pile.contains("· pass"), "{pile}");

    stdout(&ff_tower(repo.path(), &["triage", "1", "-p", "open"]));
    let brief = envelope_of(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["procedure"], serde_json::json!("open"));
    let pile = stdout(&ff_tower(repo.path(), &["triage"]));
    assert!(pile.contains("the branch"), "{pile}");
}

#[test]
fn a_claimed_flight_routes_and_the_claim_stands() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(repo.path(), &["file", "sort the pile"]));
    stdout(&ff_tower(repo.path(), &["claim", "1"]));

    stdout(&ff_tower(repo.path(), &["triage", "1", "-p", "chore"]));
    let brief = envelope_of(&ff_tower(repo.path(), &["brief", "1", "--json"]));
    assert_eq!(brief["data"]["procedure"], serde_json::json!("chore"));
    assert!(brief["data"]["claimed_by"].is_string(), "{brief}");
}

#[test]
fn the_refusal_matrix() {
    let repo = repo();
    install_chore(&repo);
    stdout(&ff_tower(repo.path(), &["file", "a flight"]));
    stdout(&ff_tower(repo.path(), &["file", "finished"]));
    stdout(&ff_tower(repo.path(), &["done", "2"]));

    let out = ff_tower(repo.path(), &["triage", "1", "--json"]);
    refusal(&out, 2, "usage/no-procedure");

    let out = ff_tower(repo.path(), &["triage", "-p", "chore", "--json"]);
    refusal(&out, 2, "usage/no-flight");

    let out = ff_tower(repo.path(), &["triage", "1", "-p", "  ", "--json"]);
    refusal(&out, 2, "usage/empty-procedure");

    let out = ff_tower(repo.path(), &["triage", "1", "-p", "nope", "--json"]);
    let envelope = refusal(&out, 1, "procedure/not-found");
    let message = envelope["error"]["message"].as_str().expect("message");
    assert!(message.contains("chore"), "{message}");
    assert_eq!(
        envelope["error"]["exits"],
        serde_json::json!(["ff tower procedures"])
    );

    let out = ff_tower(repo.path(), &["triage", "bogus", "-p", "chore", "--json"]);
    refusal(&out, 2, "usage/bad-flight");

    let out = ff_tower(repo.path(), &["triage", "9", "-p", "chore", "--json"]);
    refusal(&out, 1, "flight/not-found");

    let out = ff_tower(repo.path(), &["triage", "2", "-p", "chore", "--json"]);
    refusal(&out, 1, "flight/done");
}

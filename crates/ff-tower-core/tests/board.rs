//! The board's repository-facing half, against a real fufu.
//!
//! Section classification lives in `board/model.rs`'s unit tests over
//! hand-built rows — the authoritative held/resolving coverage, cheaper
//! and more precise than coercing a real repository into a held state.
//! (A real-held-branch fixture is deliberately skipped for that reason.)
//! What runs here is what only a real repository can prove: the wrappers
//! parse what fufu actually emits, and the whole pipeline connects.

use ff_tower_core::board;
use ff_tower_core::ff::Ff;
use ff_tower_core::log::{Kind, Store};
use ff_tower_testsupport::Repo;

fn filed(subject: &str) -> Kind {
    Kind::Filed {
        procedure: None,
        subject: subject.to_string(),
        body: String::new(),
        status: "triage".to_string(),
        assignee: None,
        priority: "none".to_string(),
        labels: Vec::new(),
        skill: None,
        bay: None,
        done: "asserted".to_string(),
        branch: None,
    }
}

#[test]
fn op_log_reads_a_tagged_capture_from_a_real_repository() {
    // The argv gate: `op log --json session(glob:*) -n 0` puts a bare
    // global flag before a positional, which nothing else exercises. This
    // lands before anything in board/ depends on the wrapper.
    let repo = Repo::new();
    repo.write("work.txt", "an agent was here\n");
    Ff::at(repo.path())
        .session("flight-1")
        .status()
        .expect("status");

    let ops = Ff::at(repo.path())
        .op_log("session(glob:*)")
        .expect("op log");

    assert_eq!(ops.len(), 1, "one tagged capture, got {ops:?}");
    assert_eq!(ops[0].session.as_deref(), Some("flight-1"));
    assert_eq!(ops[0].branch.as_deref(), Some("main"));
    assert!(ops[0].time > 0);
}

#[test]
fn branch_list_reads_a_real_repository() {
    let repo = Repo::new();
    let branches = Ff::at(repo.path()).branch_list().expect("branch list");

    let main = branches
        .named
        .iter()
        .find(|branch| branch.name == "main")
        .expect("main is listed");
    assert!(main.tip.is_some());
    assert!(!main.held && !main.resolving);
    assert!(branches.anonymous.is_empty());
}

#[test]
fn a_tagged_flight_assembles_into_the_air_and_an_untouched_one_stays_open() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    let ids = store
        .append(vec![filed("flown"), filed("untouched")])
        .expect("append");
    assert_eq!(ids[0].to_string(), "pi.1");

    // Standing in for an agent: dirty the tree and take a capture tagged
    // with the flight's id, which is exactly what a claimed flight does.
    repo.write("work.txt", "an agent was here\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");

    let events = store.read_all().expect("read_all");
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.in_the_air.len(), 1);
    let flown = &board.in_the_air[0];
    assert_eq!(flown.id, "pi.1");
    assert_eq!(flown.branch.as_deref(), Some("main"));
    assert!(flown.tip.is_some());
    assert!(flown.current, "the fixture's worktree sits on main");
    assert!(flown.last_motion.is_some());

    assert_eq!(board.open.len(), 1);
    assert_eq!(board.open[0].id, "pi.2");
    assert!(board.holding.is_empty());
    assert!(board.waiting_on_you.is_empty());
    assert!(board.unrouted.is_empty());
}

#[test]
fn a_held_flight_assembles_into_waiting_on_you() {
    // Serde, fold, and enrich end to end: the lifecycle kinds go through a
    // real store and come back out as the waiting section.
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    let ids = store
        .append(vec![filed("stuck on a question")])
        .expect("append");
    let flight = ids[0].clone();
    store
        .append(vec![
            Kind::Status {
                flight: flight.clone(),
                status: "in_progress".to_string(),
                reason: None,
            },
            Kind::Held {
                flight,
                question: "which retry path?".to_string(),
            },
        ])
        .expect("append");

    let events = store.read_all().expect("read_all");
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.waiting_on_you.len(), 1);
    let view = &board.waiting_on_you[0];
    assert_eq!(view.id, "pi.1");
    assert_eq!(view.question.as_deref(), Some("which retry path?"));
    assert!(view.asked_at.is_some());
    assert_eq!(view.status, "held", "the hold is a status move too");
    assert!(board.in_the_air.is_empty() && board.holding.is_empty() && board.open.is_empty());
}

#[test]
fn colliding_branches_carry_their_verdicts_through_the_board() {
    // The probe step end to end: two flights on branches that both edit
    // `shared.txt` carry mirrored `collides` entries, and an untouched
    // flight carries none.
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store
        .append(vec![
            filed("left work"),
            filed("right work"),
            filed("untouched"),
        ])
        .expect("append");

    repo.ff(&["start", "-b", "left"]);
    repo.write("shared.txt", "left side\n");
    Ff::at(repo.path())
        .session("pi.1")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "left: touch shared"]);

    repo.ff(&["switch", "main"]);
    repo.ff(&["start", "-b", "right"]);
    repo.write("shared.txt", "right side\n");
    Ff::at(repo.path())
        .session("pi.2")
        .status()
        .expect("status");
    repo.ff(&["commit", "-m", "right: touch shared"]);

    let events = store.read_all().expect("read_all");
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.in_the_air.len(), 2);
    let one = board
        .in_the_air
        .iter()
        .find(|v| v.id == "pi.1")
        .expect("pi.1");
    let two = board
        .in_the_air
        .iter()
        .find(|v| v.id == "pi.2")
        .expect("pi.2");
    assert_eq!(one.collides.len(), 1, "{one:?}");
    assert_eq!(one.collides[0].with, "pi.2");
    assert_eq!(one.collides[0].paths, ["shared.txt"]);
    assert_eq!(two.collides.len(), 1, "{two:?}");
    assert_eq!(two.collides[0].with, "pi.1");
    assert_eq!(two.collides[0].paths, ["shared.txt"]);
    assert!(one.unanswered.is_empty() && two.unanswered.is_empty());

    assert_eq!(board.open.len(), 1);
    let untouched = &board.open[0];
    assert_eq!(untouched.id, "pi.3");
    assert!(untouched.collides.is_empty() && untouched.unanswered.is_empty());
}

#[test]
fn a_bay_tagged_capture_reaches_the_board_from_main() {
    // The widened reads end to end: a session-tagged capture made inside
    // a bay lives on that bay's chain, and the board rendered from main
    // still sees the flight in the air on the bay's branch.
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store.append(vec![filed("bay work")]).expect("append");

    let bay = repo.bay_path("bay1");
    Ff::at(repo.path())
        .worktree_add(bay.to_str().expect("utf8"), Some("feather"))
        .expect("add");
    std::fs::write(bay.join("work.txt"), "an agent in a bay\n").expect("write");
    Ff::at(&bay).session("pi.1").status().expect("status");

    let events = store.read_all().expect("read_all");
    let board = board::assemble(&Ff::at(repo.path()), &events).expect("assemble");

    assert_eq!(board.in_the_air.len(), 1, "{board:?}");
    let view = &board.in_the_air[0];
    assert_eq!(view.id, "pi.1");
    assert_eq!(view.branch.as_deref(), Some("feather"));
    assert!(!view.current, "the render's own worktree sits on main");
    assert!(board.open.is_empty());
}

#[test]
fn gather_merges_bay_chains_and_skips_a_refusing_bay() {
    // A bay can vanish between the survey and its poll; the fan-out skips
    // it the way `probe` tolerates a vanished branch, and the healthy
    // bay's rows still merge in.
    let fake = ff_tower_testsupport::FakeFf::script(
        r#"case "$3 $4" in
  "op log")
    case "$2" in
      "/warm/bay") printf '%s\n' '{"ff":1,"cmd":"op log","data":{"ops":[{"branch":"feather","session":"pi.2","time":60}]}}' ;;
      "/gone/bay") printf '%s\n' '{"ff":1,"cmd":"op log","error":{"id":"repo/not-found","message":"gone","exits":[]}}'; exit 1 ;;
      *) printf '%s\n' '{"ff":1,"cmd":"op log","data":{"ops":[{"branch":"main","session":"pi.1","time":50}]}}' ;;
    esac ;;
  "branch list") printf '%s\n' '{"ff":1,"cmd":"branch list","data":{"named":[],"anonymous":[]}}' ;;
  "status --json") printf '%s\n' '{"ff":1,"cmd":"status","data":{"head":{"state":"branch","name":"main","ref":"refs/heads/main","commit":"abc"},"open":{"id":null,"id_letters":null,"pending":null,"subject":null,"clean":true,"base":null,"time":null},"changes":[],"insertions":0,"deletions":0,"conflicts":[],"held":null,"session":null}}' ;;
  "worktree list") printf '%s\n' '{"ff":1,"cmd":"worktree list","data":{"worktrees":[{"id":"main","path":"/main","branch":"main","current":true},{"id":"warm","path":"/warm/bay","branch":"feather","current":false},{"id":"gone","path":"/gone/bay","branch":"lost","current":false}]}}' ;;
esac
"#,
    );
    let ff = Ff::at("/main").program(fake.path());
    let reads = board::gather(&ff).expect("gather");

    assert_eq!(reads.worktrees.len(), 3, "the survey is carried whole");
    let sessions: Vec<&str> = reads
        .ops
        .iter()
        .filter_map(|op| op.session.as_deref())
        .collect();
    assert_eq!(
        sessions,
        ["pi.1", "pi.2"],
        "the warm bay's chain merged; the gone bay skipped, not fatal"
    );
    assert_eq!(reads.current_branch.as_deref(), Some("main"));
}

#[test]
fn gather_survives_a_bay_whose_directory_is_gone_and_carries_orphans() {
    // A hand-deleted bay directory fails `-C` before verb dispatch, and
    // fufu's error envelope answers for `map` while tower asked `op log`
    // — `Mismatched`, not a refusal. The fan-out skips it the way it
    // skips a refusing bay, so the board survives the very state doctor
    // diagnoses. The survey's orphans ride through into the reads.
    let fake = ff_tower_testsupport::FakeFf::script(
        r#"case "$3 $4" in
  "op log")
    case "$2" in
      "/gone/bay") printf '%s
' '{"ff":1,"cmd":"map","error":{"id":"usage/no-such-directory","message":"-C /gone/bay: No such file or directory (os error 2)","exits":["ff status","ff worktree list"]}}'; exit 2 ;;
      *) printf '%s
' '{"ff":1,"cmd":"op log","data":{"ops":[{"branch":"main","session":"pi.1","time":50}]}}' ;;
    esac ;;
  "branch list") printf '%s
' '{"ff":1,"cmd":"branch list","data":{"named":[],"anonymous":[]}}' ;;
  "status --json") printf '%s
' '{"ff":1,"cmd":"status","data":{"head":{"state":"branch","name":"main","ref":"refs/heads/main","commit":"abc"},"open":{"id":null,"id_letters":null,"pending":null,"subject":null,"clean":true,"base":null,"time":null},"changes":[],"insertions":0,"deletions":0,"conflicts":[],"held":null,"session":null}}' ;;
  "worktree list") printf '%s
' '{"ff":1,"cmd":"worktree list","data":{"worktrees":[{"id":"main","path":"/main","branch":"main","current":true},{"id":"gone","path":"/gone/bay","branch":"lost","current":false}],"orphans":[{"id":"old","chain":"refs/fufu/wt/old/ops","tip":"t","branch":"feather","time":5}]}}' ;;
esac
"#,
    );
    let ff = Ff::at("/main").program(fake.path());
    let reads = board::gather(&ff).expect("gather survives the mismatch");

    let sessions: Vec<&str> = reads
        .ops
        .iter()
        .filter_map(|op| op.session.as_deref())
        .collect();
    assert_eq!(sessions, ["pi.1"], "the gone bay skipped, not fatal");
    assert_eq!(reads.orphans.len(), 1, "the survey's orphans are carried");
    assert_eq!(reads.orphans[0].branch.as_deref(), Some("feather"));
}

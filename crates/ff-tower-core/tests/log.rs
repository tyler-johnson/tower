//! The log store, driven end to end against real repositories.
//!
//! The store is ordinary git objects on an orphan ref, and that is the
//! point — so wherever the shape itself is the claim (parents, signature,
//! tree contents), the assertions read the chain back through git rather
//! than through the code under test.

use std::path::Path;

use ff_tower_core::board;
use ff_tower_core::log::{Error, EventId, Kind, Store};
use ff_tower_testsupport::Repo;

/// The fixture's author (set by `Repo::new`), and the chain the pinned
/// writer `pi` writes for it.
const AUTHOR: &str = "tests@tower.invalid";
const REF: &str = "refs/tower/log/tests@tower.invalid/pi";

fn filed(subject: &str) -> Kind {
    Kind::Filed {
        procedure: "open".to_string(),
        subject: subject.to_string(),
        body: String::new(),
    }
}

#[test]
fn a_round_trip_reads_back_in_order() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    assert_eq!(store.author(), AUTHOR);

    store.append(vec![filed("first")]).expect("append");
    store
        .append(vec![Kind::Commented {
            flight: "pi.1".parse().expect("id"),
            text: "a note".to_string(),
        }])
        .expect("append");
    store
        .append(vec![Kind::Linked {
            from: "pi.3".parse().expect("id"),
            to: "pi.1".parse().expect("id"),
        }])
        .expect("append");

    let events = store.read().expect("read");
    let ids: Vec<String> = events.iter().map(|event| event.id.to_string()).collect();
    assert_eq!(ids, ["pi.1", "pi.2", "pi.3"]);
    assert_eq!(events[0].author, AUTHOR);
    assert!(matches!(&events[0].kind, Kind::Filed { subject, .. } if subject == "first"));
    assert!(
        matches!(&events[1].kind, Kind::Commented { flight, .. } if flight.to_string() == "pi.1")
    );
    assert!(matches!(&events[2].kind, Kind::Linked { from, to } if from.seq == 3 && to.seq == 1));
}

#[test]
fn a_batch_is_one_commit_and_several_events() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");

    let ids = store
        .append(vec![filed("a"), filed("b"), filed("c")])
        .expect("append");
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[2].to_string(), "pi.3");

    assert_eq!(repo.git(&["rev-list", "--count", REF]).trim(), "1");
    assert_eq!(store.read().expect("read").len(), 3);
}

#[test]
fn a_batch_can_name_its_own_ids() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store.append(vec![filed("the broad task")]).expect("append");

    // The parts and the parent's dependency on them, in one append: the
    // `linked` events name ids this same batch mints.
    let parent: EventId = "pi.1".parse().expect("id");
    let ids = store
        .append_with(|mint| {
            vec![
                filed("part one"),
                filed("part two"),
                Kind::Linked {
                    from: parent.clone(),
                    to: mint(0),
                },
                Kind::Linked {
                    from: parent.clone(),
                    to: mint(1),
                },
            ]
        })
        .expect("append");
    let named: Vec<String> = ids.iter().map(ToString::to_string).collect();
    assert_eq!(named, ["pi.2", "pi.3", "pi.4", "pi.5"]);

    // One commit for the four events, so there is no window where the
    // parent is live, unlinked, and claimable.
    assert_eq!(repo.git(&["rev-list", "--count", REF]).trim(), "2");

    let fold = board::fold(&store.read_all().expect("read"));
    let flight = |seq: &str| {
        let id: EventId = seq.parse().expect("id");
        fold.flights
            .iter()
            .find(|flight| flight.id == id)
            .expect("a filed flight")
    };
    assert_eq!(
        flight("pi.1").depends_on,
        [
            "pi.2".parse::<EventId>().expect("id"),
            "pi.3".parse().expect("id")
        ]
    );
    assert_eq!(flight("pi.2").blocks, std::slice::from_ref(&parent));
    assert_eq!(flight("pi.3").blocks, std::slice::from_ref(&parent));
}

#[test]
fn the_chain_is_an_orphan_with_one_parent_per_commit() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    for subject in ["a", "b", "c"] {
        store.append(vec![filed(subject)]).expect("append");
    }

    // Three commits; each names exactly one parent, except the root, which
    // names none — so a parent walk is the chain and nothing else.
    let rows = repo.git(&["rev-list", "--parents", REF]);
    let rows: Vec<&str> = rows.lines().collect();
    assert_eq!(rows.len(), 3);
    assert!(
        rows[..2]
            .iter()
            .all(|row| row.split_whitespace().count() == 2)
    );
    assert_eq!(rows[2].split_whitespace().count(), 1);

    // Signed by tower, not by the user: a write cannot depend on config,
    // and the signature is the decode gate's first check.
    let identities = repo.git(&["log", "--format=%an <%ae> %cn <%ce>", REF]);
    for line in identities.lines() {
        assert_eq!(line, "tower <tower@local> tower <tower@local>");
    }

    // The tree holds events.json and nothing else.
    let tree = repo.git(&["cat-file", "-p", &format!("{REF}^{{tree}}")]);
    let entries: Vec<&str> = tree.lines().collect();
    assert_eq!(entries.len(), 1, "{tree}");
    assert!(entries[0].ends_with("events.json"), "{tree}");
}

/// The load-bearing one: N processes appending at once all survive. This
/// is the test that catches the bug fufu found — without it the lock is
/// untested decoration. Processes rather than threads, because the lock is
/// a file. The vehicle is re-exec: each child is this same test binary,
/// filtered to [`concurrent_append_child`], which appends only when the
/// env var points it at a repository.
#[test]
fn concurrent_appends_all_survive() {
    let repo = Repo::new();
    repo.pin_writer("pi");

    let exe = std::env::current_exe().expect("current_exe");
    let children: Vec<std::process::Child> = (0..6)
        .map(|_| {
            std::process::Command::new(&exe)
                .args(["--exact", "concurrent_append_child"])
                .env("TOWER_LOG_STRESS_REPO", repo.path())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("spawn")
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().expect("wait");
        assert!(
            output.status.success(),
            "a child lost its append\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    let store = Store::open(repo.path()).expect("open");
    let events = store.read().expect("read");
    let mut seqs: Vec<u64> = events.iter().map(|event| event.id.seq).collect();
    seqs.sort_unstable();
    // Contiguous, no duplicates, none lost: the exact shape a silent
    // overwrite would break.
    assert_eq!(seqs, (1..=30).collect::<Vec<u64>>());
}

#[test]
fn concurrent_append_child() {
    let Ok(path) = std::env::var("TOWER_LOG_STRESS_REPO") else {
        return; // Only a body when spawned by the test above.
    };
    let store = Store::open(Path::new(&path)).expect("open");
    for n in 0..5 {
        store
            .append(vec![filed(&format!("stress {n}"))])
            .expect("append");
    }
}

#[test]
fn two_writers_under_one_author_fold_into_one_stream() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    store.append(vec![filed("mine")]).expect("append");
    store.append(vec![filed("mine again")]).expect("append");

    // The same author writing from another machine: a second writer, a
    // second ref, and nothing to merge.
    repo.pin_writer("laptop");
    let roaming = Store::open(repo.path()).expect("open");
    roaming.append(vec![filed("theirs")]).expect("append");

    let events = roaming.read_all().expect("read_all");
    assert_eq!(events.len(), 3);
    let mut writers: Vec<&str> = events.iter().map(|event| event.writer.as_str()).collect();
    writers.sort_unstable();
    writers.dedup();
    assert_eq!(writers, ["laptop", "pi"]);
    // Ordered by (time, writer, seq), the union's sort key.
    let keys: Vec<(i64, &str, u64)> = events
        .iter()
        .map(|event| (event.time, event.writer.as_str(), event.id.seq))
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]), "{keys:?}");
    // read() stays one writer's view: only the chain this store writes.
    assert_eq!(roaming.read().expect("read").len(), 1);
}

#[test]
fn an_unknown_kind_survives_byte_identical() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");

    // An event from a newer tower: a kind this binary has never heard of.
    // Dropping it would silently lose authored intent, so it must ride the
    // chain untouched.
    let body = r#"{"upstream":"LIN-123","kept":[1,2,3]}"#;
    let unknown = Kind::Unknown {
        kind: "promoted".to_string(),
        body: serde_json::value::RawValue::from_string(body.to_string()).expect("raw"),
    };
    store.append(vec![unknown]).expect("append");

    let events = store.read().expect("read");
    let Kind::Unknown {
        kind,
        body: read_back,
    } = &events[0].kind
    else {
        panic!("parsed into a known kind");
    };
    assert_eq!(kind, "promoted");
    assert_eq!(read_back.get(), body);
}

#[test]
fn a_future_chain_version_is_refused_before_its_payload() {
    let repo = Repo::new();
    repo.pin_writer("pi");

    // A tower-signed commit from a future tower: right identity, a version
    // this binary does not read, and a payload it must never look at —
    // the tree here does not even hold an events.json.
    let commit = repo.git(&[
        "-c",
        "user.name=tower",
        "-c",
        "user.email=tower@local",
        "commit-tree",
        "HEAD^{tree}",
        "-m",
        "0 event(s)",
        "-m",
        "tower-v: 99\ntower-seq: 1",
    ]);
    repo.git(&["update-ref", REF, commit.trim()]);

    let store = Store::open(repo.path()).expect("open");
    match store.read() {
        Err(Error::Version {
            found: 99, reads, ..
        }) => assert_eq!(reads, 1),
        other => panic!("expected a version refusal, got {other:?}"),
    }
}

#[test]
fn a_foreign_commit_on_the_ref_is_not_a_tower_log() {
    let repo = Repo::new();
    repo.pin_writer("pi");

    // Right trailers, wrong identity: written by the fixture's user, not
    // by tower. The signature gate must refuse it before anything else —
    // this is what keeps a hand-edited commit out of the board.
    let commit = repo.git(&[
        "commit-tree",
        "HEAD^{tree}",
        "-m",
        "1 event(s)",
        "-m",
        "tower-v: 1\ntower-seq: 2",
    ]);
    repo.git(&["update-ref", REF, commit.trim()]);

    let store = Store::open(repo.path()).expect("open");
    assert!(matches!(store.read(), Err(Error::NotTowerLog { .. })));
}

#[test]
fn no_ref_reads_as_an_empty_log() {
    let repo = Repo::new();
    let store = Store::open(repo.path()).expect("open");
    // No writer minted, no ref written: an empty log twice over, never an
    // error.
    assert!(store.read().expect("read").is_empty());
    assert!(store.read_all().expect("read_all").is_empty());

    // Even with a writer pinned, an absent ref is empty rather than an
    // error.
    repo.pin_writer("pi");
    let store = Store::open(repo.path()).expect("open");
    assert!(store.read().expect("read").is_empty());
}

#[test]
fn the_working_tree_is_never_touched() {
    let repo = Repo::new();
    repo.pin_writer("pi");
    assert_eq!(repo.git(&["status", "--porcelain"]), "");

    let store = Store::open(repo.path()).expect("open");
    store.append(vec![filed("a"), filed("b")]).expect("append");
    store.append(vec![filed("c")]).expect("append");

    // This is what "never touching the working tree" means operationally:
    // git sees nothing, and fufu sees nothing.
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
    repo.ff(&["status", "--json"]);
}

#[test]
fn a_first_append_mints_the_writer_and_keeps_it() {
    let repo = Repo::new();
    let store = Store::open(repo.path()).expect("open");
    assert_eq!(store.writer(), None);

    let ids = store.append(vec![filed("first")]).expect("append");
    let writer = ids[0].writer.clone();

    // `<sanitized-hostname>-<4 chars>`: a legal ref component with a hash
    // suffix, so two machines sharing a hostname cannot share a ref.
    let (host, suffix) = writer.rsplit_once('-').expect("a suffixed writer");
    assert!(!host.is_empty());
    assert_eq!(suffix.len(), 4);
    assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(
        writer
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "{writer}"
    );

    // Persisted to local config, and reused rather than re-minted.
    assert_eq!(repo.git(&["config", "tower.writer"]).trim(), writer);
    let again = store.append(vec![filed("second")]).expect("append");
    assert_eq!(again[0].writer, writer);
    assert_eq!(again[0].seq, 2);

    // A fresh open resolves the same writer from config.
    let reopened = Store::open(repo.path()).expect("open");
    assert_eq!(reopened.writer(), Some(writer.as_str()));
}

#[test]
fn pool_root_reads_the_key_and_is_none_without_it() {
    let repo = Repo::new();
    let store = Store::open(repo.path()).expect("open");
    assert_eq!(store.pool_root(), None);

    // The snapshot is taken at open, so the key lands before a fresh one.
    repo.git(&["config", "tower.bays", "../bays"]);
    let store = Store::open(repo.path()).expect("open");
    assert_eq!(store.pool_root().as_deref(), Some("../bays"));
}

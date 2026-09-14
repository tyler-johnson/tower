use atc_core::{
    board,
    log::{Store, sync::Touch},
    verb::{self, Fields},
};
use atc_testsupport::Repo;

// The allocation budget stays at the product default. A one-second budget is not enough on the Windows runner, where a fetch and push against the bare fixture spawn enough git processes to cross the deadline and file provisionally. Tests that measure the deadline set a one-second budget themselves.
fn remote(repo: &Repo, path: &std::path::Path, writer: &str) {
    repo.pin_writer(writer);
    repo.git(&["remote", "add", "shared", path.to_str().expect("path")]);
    repo.git(&["config", "tower.remote", "shared"]);
    repo.git(&["config", "tower.syncInterval", "1s"]);
}

fn file(store: &Store, subject: &str) -> (String, String) {
    let result = verb::file(store, subject, Fields::default(), None).expect("file");
    let row = &result.payload.flights[0];
    (row.id.clone(), row.display.clone())
}

#[test]
fn two_writers_allocate_and_converge() {
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("shared.git");
    a.git(&["init", "--bare", bare.to_str().expect("path")]);
    remote(&a, &bare, "alpha");
    remote(&b, &bare, "beta");
    let a = Store::open(a.path()).expect("a");
    let b = Store::open(b.path()).expect("b");
    let first = file(&a, "first");
    let second = file(&b, "second");
    assert_eq!(first.1, "#1");
    assert_eq!(second.1, "#2");
    a.touch(Touch::Number).expect("sync a");
    b.touch(Touch::Number).expect("sync b");
    for store in [&a, &b] {
        let fold = store.snapshot().expect("snapshot");
        assert_eq!(fold.flights.len(), 2);
        for (id, display) in [&first, &second] {
            assert_eq!(board::display(&fold, &id.parse().expect("id")), *display);
        }
    }
}

#[test]
fn offline_filing_keeps_its_guess_then_claims_the_same_digits() {
    let a = Repo::new();
    let bare = a.path().join("shared.git");
    a.git(&["init", "--bare", bare.to_str().expect("path")]);
    remote(&a, &bare, "alpha");
    let store = Store::open(a.path()).expect("open");
    assert_eq!(file(&store, "online").1, "#1");
    a.git(&["remote", "set-url", "shared", "/missing-tower-test-remote"]);
    let offline = Store::open(a.path()).expect("offline");
    let pending = file(&offline, "offline");
    assert_eq!(pending.1, "~2");
    a.git(&["remote", "set-url", "shared", bare.to_str().expect("path")]);
    let online = Store::open(a.path()).expect("online");
    online.touch(Touch::Number).expect("recover");
    let fold = online.snapshot().expect("snapshot");
    assert_eq!(board::display(&fold, &pending.0.parse().expect("id")), "#2");
    assert!(board::resolve(&fold, "~2").is_err());
}

#[test]
fn enrollment_refuses_then_renumbers_once() {
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("shared.git");
    a.git(&["init", "--bare", bare.to_str().expect("path")]);
    remote(&a, &bare, "alpha");
    let first = Store::open(a.path()).expect("first");
    file(&first, "remote");
    b.pin_writer("beta");
    let local = Store::open(b.path()).expect("local");
    let own = file(&local, "local");
    assert_eq!(own.1, "#1");
    remote(&b, &bare, "beta");
    let joining = Store::open(b.path()).expect("joining");
    let error = joining
        .touch(Touch::Number)
        .expect_err("confirmation required");
    assert_eq!(error.id(), "sync/renumber-required");
    joining.touch(Touch::Enroll).expect("confirm");
    let count = joining.read_all().expect("events").len();
    joining.touch(Touch::Number).expect("retry");
    assert_eq!(joining.read_all().expect("events").len(), count);
    let fold = joining.snapshot().expect("snapshot");
    assert_eq!(board::display(&fold, &own.0.parse().expect("id")), "#2");
    assert_eq!(
        board::resolve(&fold, "beta#1").expect("alias").to_string(),
        own.0
    );
}

#[test]
fn concurrent_single_and_range_claims_do_not_overlap() {
    let hub = Repo::new();
    let bare = hub.path().join("shared.git");
    hub.git(&["init", "--bare", bare.to_str().expect("path")]);
    let fixtures: Vec<_> = (0..4)
        .map(|n| {
            let fixture = Repo::new();
            remote(&fixture, &bare, &format!("writer{n}"));
            fixture
        })
        .collect();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(fixtures.len()));
    let threads: Vec<_> = fixtures
        .iter()
        .enumerate()
        .map(|(n, fixture)| {
            let path = fixture.path().to_path_buf();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let store = Store::open(&path).unwrap();
                barrier.wait();
                let parent = file(&store, &format!("work{n}"));
                if n % 2 == 0 {
                    verb::decompose(&store, &parent.0, &["one".to_string(), "two".to_string()])
                        .expect("range");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("writer");
    }
    let store = Store::open(fixtures[0].path()).unwrap();
    store.touch(Touch::Number).expect("converge");
    let fold = store.snapshot().unwrap();
    assert_eq!(fold.flights.len(), 8);
    let mut numbers: Vec<_> = fold
        .flights
        .iter()
        .map(|f| f.global_number.expect("claimed"))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), 8);
}

#[test]
fn a_provisional_input_is_pinned_before_sync_advances_its_guess() {
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("shared.git");
    a.git(&["init", "--bare", bare.to_str().unwrap()]);
    remote(&a, &bare, "alpha");
    remote(&b, &bare, "beta");
    let online = Store::open(a.path()).unwrap();
    file(&online, "seed");
    let bstore = Store::open(b.path()).unwrap();
    bstore.touch(Touch::Number).unwrap();
    b.git(&["remote", "set-url", "shared", "/missing-tower-test-remote"]);
    let offline = Store::open(b.path()).unwrap();
    let pending = file(&offline, "offline");
    assert_eq!(pending.1, "~2");
    file(&online, "takes number two");
    b.git(&["remote", "set-url", "shared", bare.to_str().unwrap()]);
    let recovered = Store::open(b.path()).unwrap();
    verb::comment(
        &recovered,
        "~2",
        Some("reference beta~2".to_string()),
        false,
    )
    .expect("comment pinned");
    let fold = recovered.snapshot().unwrap();
    let own = board::flight(&fold, &pending.0.parse().unwrap());
    assert_eq!(own.global_number, Some(3));
    assert_eq!(own.comments[0].text, format!("reference #{}", pending.0));
}

#[test]
fn divergent_writer_chains_are_preserved_and_keep_refusing() {
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("shared.git");
    a.git(&["init", "--bare", bare.to_str().unwrap()]);
    remote(&a, &bare, "same");
    remote(&b, &bare, "same");
    let a_store = Store::open(a.path()).unwrap();
    let original = file(&a_store, "original");
    let b_store = Store::open(b.path()).unwrap();
    b_store.touch(Touch::Number).unwrap();
    a.git(&["remote", "set-url", "shared", "/missing-tower-test-remote"]);
    let offline = Store::open(a.path()).unwrap();
    verb::comment(&offline, &original.0, Some("local fork".to_string()), false).unwrap();
    verb::comment(
        &b_store,
        &original.0,
        Some("remote fork".to_string()),
        false,
    )
    .unwrap();
    b_store.touch(Touch::Number).unwrap();
    a.git(&["remote", "set-url", "shared", bare.to_str().unwrap()]);
    a.git(&["config", "tower.syncInterval", "30s"]);
    let reconnected = Store::open(a.path()).unwrap();
    let name = "refs/tower/log/tests@tower.invalid/same";
    let before = a.git(&["rev-parse", name]);
    assert_eq!(
        reconnected.touch(Touch::Number).unwrap_err().id(),
        "sync/refused"
    );
    assert_eq!(
        reconnected.touch(Touch::Ordinary).unwrap_err().id(),
        "sync/refused"
    );
    assert_eq!(a.git(&["rev-parse", name]), before);
}

#[test]
fn offline_attempts_observe_cadence_but_filing_uses_its_own_deadline() {
    let fixture = Repo::new();
    remote(
        &fixture,
        std::path::Path::new("/missing-tower-test-remote"),
        "offline",
    );
    fixture.git(&["config", "tower.syncInterval", "30s"]);
    fixture.git(&["config", "tower.numberTimeout", "1s"]);
    let store = Store::open(fixture.path()).unwrap();
    store.touch(Touch::Ordinary).unwrap();
    let attempted = store.sync_state().unwrap().attempted;
    let start = std::time::Instant::now();
    store.touch(Touch::Ordinary).unwrap();
    assert!(start.elapsed() < std::time::Duration::from_millis(200));
    assert_eq!(store.sync_state().unwrap().attempted, attempted);
    let start = std::time::Instant::now();
    assert_eq!(file(&store, "provisional").1, "~1");
    assert!(start.elapsed() >= std::time::Duration::from_millis(800));
    assert!(start.elapsed() < std::time::Duration::from_millis(1500));
}

#[test]
fn allocation_lock_timeout_does_not_start_a_second_render_budget() {
    use std::time::{Duration, Instant};
    let fixture = Repo::new();
    remote(
        &fixture,
        std::path::Path::new("/missing-tower-test-remote"),
        "local",
    );
    fixture.git(&["config", "tower.numberTimeout", "1s"]);
    let store = Store::open(fixture.path()).unwrap();
    let held = store
        .coordinate(Instant::now() + Duration::from_secs(1))
        .unwrap();
    held.initialize().unwrap();
    let start = Instant::now();
    let minted = file(&store, "filed despite unavailable coordinator");
    assert_eq!(minted.1, "~1");
    assert!(start.elapsed() < Duration::from_millis(1500));
    assert_eq!(store.current().unwrap().flights.len(), 1);
    drop(held);
}

#[test]
fn malformed_remote_logs_never_replace_canonical_refs() {
    let fixture = Repo::new();
    let bare = fixture.path().join("shared.git");
    fixture.git(&["init", "--bare", bare.to_str().unwrap()]);
    remote(&fixture, &bare, "local");
    fixture.git(&["push", "shared", "HEAD:refs/tower/log/foreign/invalid"]);
    let store = Store::open(fixture.path()).unwrap();
    assert_eq!(
        store.touch(Touch::Number).unwrap_err().id(),
        "log/not-tower"
    );
    assert!(
        fixture
            .git(&["for-each-ref", "--format=%(refname)", "refs/tower/log/"])
            .trim()
            .is_empty()
    );
}

#[test]
fn local_legacy_migration_preserves_single_writer_numbers() {
    let fixture = Repo::new();
    fixture.pin_writer("legacy");
    let store = Store::open(fixture.path()).unwrap();
    let filed = |subject: &str| atc_core::log::Kind::Filed {
        procedure: None,
        subject: subject.into(),
        body: String::new(),
        status: "ready".into(),
        assignee: None,
        priority: "none".into(),
        labels: vec![],
        skill: None,
        bay: None,
        done: "asserted".into(),
        branch: None,
    };
    let ids = store.append(vec![filed("first"), filed("second")]).unwrap();
    let snapshot = store.snapshot().unwrap();
    for (offset, id) in ids.iter().enumerate() {
        assert_eq!(board::display(&snapshot, id), format!("#{}", offset + 1));
    }
    let events = store.read_all().unwrap().len();
    store.snapshot().unwrap();
    assert_eq!(store.read_all().unwrap().len(), events);
}

#[cfg(unix)]
#[test]
fn a_confirmed_claim_survives_failed_log_publication() {
    use std::os::unix::fs::PermissionsExt;
    let a = Repo::new();
    let b = Repo::new();
    let bare = a.path().join("shared.git");
    a.git(&["init", "--bare", bare.to_str().unwrap()]);
    remote(&a, &bare, "alpha");
    remote(&b, &bare, "beta");
    let store = Store::open(a.path()).unwrap();
    file(&store, "seed");
    let hook = bare.join("hooks/update");
    std::fs::write(
        &hook,
        "#!/bin/sh\ncase \"$1\" in refs/tower/log/*) exit 1;; esac\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let own = file(&store, "publication fails");
    assert_eq!(own.1, "#2");
    assert_ne!(store.sync_state().unwrap().result.as_deref(), Some("ok"));
    std::fs::remove_file(hook).unwrap();
    store.touch(Touch::Number).unwrap();
    let other = Store::open(b.path()).unwrap();
    other.touch(Touch::Number).unwrap();
    assert_eq!(
        board::display(&other.snapshot().unwrap(), &own.0.parse().unwrap()),
        "#2"
    );
}

#[test]
fn a_remote_counter_rollback_is_refused() {
    let fixture = Repo::new();
    let bare = fixture.path().join("shared.git");
    fixture.git(&["init", "--bare", bare.to_str().unwrap()]);
    remote(&fixture, &bare, "alpha");
    let store = Store::open(fixture.path()).unwrap();
    file(&store, "one");
    let before = store.counter().unwrap().unwrap();
    file(&store, "two");
    fixture.git(&[
        "--git-dir",
        bare.to_str().unwrap(),
        "update-ref",
        "refs/tower/seq",
        &before.tip.to_string(),
    ]);
    assert_eq!(store.touch(Touch::Number).unwrap_err().id(), "sync/refused");
    assert_eq!(store.counter().unwrap().unwrap().value, 2);
}

#[test]
fn losing_a_local_counter_never_reuses_existing_claims() {
    let fixture = Repo::new();
    fixture.pin_writer("local");
    let store = Store::open(fixture.path()).unwrap();
    file(&store, "one");
    fixture.git(&["update-ref", "-d", "refs/tower/seq"]);
    assert_eq!(
        store.touch(Touch::Ordinary).unwrap_err().id(),
        "sync/counter-invalid"
    );
    assert!(verb::file(&store, "must not collide", Fields::default(), None).is_err());
    assert_eq!(board::fold(&store.read_all().unwrap()).flights.len(), 1);
}

#[test]
fn an_accepted_but_unrecorded_remote_reservation_recovers_once() {
    use std::time::{Duration, Instant};
    let fixture = Repo::new();
    let bare = fixture.path().join("shared.git");
    fixture.git(&["init", "--bare", bare.to_str().unwrap()]);
    remote(&fixture, &bare, "alpha");
    let store = Store::open(fixture.path()).unwrap();
    let own = file(&store, "one");
    let held = store
        .coordinate(Instant::now() + Duration::from_secs(2))
        .unwrap();
    let counter = store.counter().unwrap().unwrap();
    let reservation = held.prepare(&counter, &[own.0.parse().unwrap()]).unwrap();
    let state_path = fixture.path().join(".git/tower/sync.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    state["pending_remote"] = serde_json::json!(format!("shared\n{}", bare.display()));
    std::fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
    fixture.git(&[
        "push",
        &format!("--force-with-lease=refs/tower/seq:{}", counter.tip),
        "shared",
        &format!("{}:refs/tower/seq", reservation.tip),
    ]);
    drop(held);
    store.touch(Touch::Number).unwrap();
    assert_eq!(
        board::display(&store.snapshot().unwrap(), &own.0.parse().unwrap()),
        "#2"
    );
    let events = store.read_all().unwrap().len();
    store.touch(Touch::Number).unwrap();
    assert_eq!(store.read_all().unwrap().len(), events);
}

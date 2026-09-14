//! The shared counter's local transaction and recovery primitives.
//!
//! Lock order: common-directory coordination, then writer append. The coordination lock is crash-released and must never be unlinked. Transport callers use the same guard and deadline; writer append locks never span transport.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::time::{Duration, Instant};

use gix::refs::transaction::PreviousValue;

use super::{Error, EventId, Kind, Result, Store, chain, ref_target, wall_clock};

pub const SEQ_REF: &str = "refs/tower/seq";
const RESERVATION_FILE: &str = "reservation.json";
const VERSION: &str = "tower-counter: 1";

/// A locally observed counter, validated through its root.
#[derive(Debug, Clone)]
pub struct Counter {
    pub tip: gix::ObjectId,
    pub lineage: gix::ObjectId,
    pub value: u64,
}

/// A range bound to exact wire IDs, recoverable from its commit after a crash.
#[derive(Debug, Clone)]
pub struct Reservation {
    pub tip: gix::ObjectId,
    pub parent: gix::ObjectId,
    pub first: u64,
    pub flights: Vec<EventId>,
}

impl Reservation {
    pub fn assignments(&self) -> impl Iterator<Item = (EventId, u64)> + '_ {
        self.flights
            .iter()
            .enumerate()
            .map(|(offset, id)| (id.clone(), self.first + offset as u64))
    }
}

/// Exclusive counter access on this repository, including all linked worktrees.
pub struct Coordination<'a> {
    pub(super) store: &'a Store,
    _lock: File,
}

impl Store {
    /// Waiting uses the caller's deadline. The kernel releases ownership on crash.
    pub fn coordinate(&self, deadline: Instant) -> Result<Coordination<'_>> {
        let dir = self.repo.common_dir().join("tower");
        std::fs::create_dir_all(&dir).map_err(Error::repo)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("coordination"))
            .map_err(Error::repo)?;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(Error::Deadline)?;
            match file.try_lock() {
                Ok(()) => {
                    return Ok(Coordination {
                        store: self,
                        _lock: file,
                    });
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(remaining.min(Duration::from_millis(10)));
                }
                Err(std::fs::TryLockError::Error(err)) => return Err(Error::repo(err)),
            }
        }
    }

    /// Read-only: absent until allocation or migration initializes the counter.
    pub fn counter(&self) -> Result<Option<Counter>> {
        ref_target(&self.repo, SEQ_REF)?
            .map(|tip| inspect(&self.repo, tip))
            .transpose()
    }
}

impl Coordination<'_> {
    /// Initialize a unique domain. Even two empty repositories start distinct lineages.
    pub fn initialize(&self) -> Result<Counter> {
        if let Some(counter) = self.store.counter()? {
            return Ok(counter);
        }
        if self
            .store
            .read_all()?
            .iter()
            .any(|event| matches!(event.kind, Kind::Numbered { .. }))
        {
            return Err(invalid("number claims exist but refs/tower/seq is missing"));
        }
        let nonce = super::fresh_writer_name(&self.store.repo);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(Error::repo)?
            .as_nanos();
        let tip = write(
            &self.store.repo,
            0,
            None,
            &[],
            Some(&format!("{nonce}-{}-{nanos}", std::process::id())),
        )?;
        move_ref(&self.store.repo, SEQ_REF, tip, None)?;
        inspect(&self.store.repo, tip)
    }

    /// Reserve a local range and record it. Recovery precedes every new reservation.
    /// This is deliberately explicit: raw log appends do not allocate numbers.
    pub fn claim_local(&self, flights: &[EventId]) -> Result<Vec<(EventId, u64)>> {
        self.recover_local()?;
        let fold = crate::board::fold(&self.store.read_all()?);
        let mut seen = HashSet::new();
        let mut wanted = Vec::new();
        for id in flights {
            if !seen.insert(id) {
                return Err(invalid("a reservation repeats a flight"));
            }
            let flight = fold
                .flights
                .iter()
                .find(|flight| &flight.id == id)
                .ok_or_else(|| invalid(format!("flight {id} has not been filed")))?;
            if flight.global_number.is_none() {
                wanted.push(id.clone());
            }
        }
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let counter = self.initialize()?;
        let reservation = self.prepare(&counter, &wanted)?;
        move_ref(
            &self.store.repo,
            SEQ_REF,
            reservation.tip,
            Some(counter.tip),
        )?;
        self.record(&reservation)?;
        Ok(reservation.assignments().collect())
    }

    /// Root the reservation before updating a counter. No network occurs here.
    pub fn prepare(&self, counter: &Counter, flights: &[EventId]) -> Result<Reservation> {
        if flights.is_empty()
            || flights.iter().any(|id| id.seq == 0)
            || flights.iter().collect::<HashSet<_>>().len() != flights.len()
        {
            return Err(invalid("a reservation needs distinct flight IDs"));
        }
        for id in flights {
            super::validate_component("writer", &id.writer)?;
        }
        let checked = inspect(&self.store.repo, counter.tip)?;
        if checked.value != counter.value || checked.lineage != counter.lineage {
            return Err(invalid("reservation context does not match its counter"));
        }
        let value = counter
            .value
            .checked_add(flights.len() as u64)
            .ok_or_else(|| invalid("the counter is exhausted"))?;
        let pending = self.pending_name()?;
        if ref_target(&self.store.repo, &pending)?.is_some() {
            return Err(invalid("an earlier reservation still needs recovery"));
        }
        let tip = write(&self.store.repo, value, Some(counter.tip), flights, None)?;
        move_ref(&self.store.repo, &pending, tip, None)?;
        Ok(Reservation {
            tip,
            parent: counter.tip,
            first: counter.value + 1,
            flights: flights.to_vec(),
        })
    }

    /// A pending local reservation is confirmed only by ancestry, never by its existence.
    pub fn recover_local(&self) -> Result<()> {
        if self
            .store
            .repo
            .config_snapshot()
            .string("tower.remote")
            .is_some_and(|remote| !remote.is_empty())
        {
            return Err(invalid(
                "local recovery requires tower.remote to be unset; shared reservations need remote confirmation",
            ));
        }
        let pending = self.pending_name()?;
        let Some(tip) = ref_target(&self.store.repo, &pending)? else {
            return Ok(());
        };
        let reservation = reservation(&self.store.repo, tip)?;
        if let Some(counter) = self.store.counter()?
            && contains(&self.store.repo, counter.tip, tip)?
        {
            return self.record(&reservation);
        }
        self.clear_pending(tip)
    }

    pub(super) fn pending_name(&self) -> Result<String> {
        Ok(format!(
            "refs/tower/pending/{}",
            self.store.writer_or_mint()?
        ))
    }

    /// Record a confirmed reservation once. Callers must establish confirmation first.
    pub(super) fn record(&self, reservation: &Reservation) -> Result<()> {
        self.record_until(reservation, None)
    }

    pub(super) fn record_until(
        &self,
        reservation: &Reservation,
        deadline: Option<Instant>,
    ) -> Result<()> {
        let events = self.store.read_all()?;
        let kinds: Vec<Kind> = reservation
            .assignments()
            .filter(|(id, number)| {
                !events.iter().any(|event| {
                    matches!(&event.kind,
                Kind::Numbered { flight, number: assigned, reservation: claim } if flight == id && assigned == number && claim.as_deref() == Some(reservation.tip.to_string().as_str()))
                })
            })
            .map(|(flight, number)| Kind::Numbered { flight, number, reservation: Some(reservation.tip.to_string()) })
            .collect();
        if !kinds.is_empty() {
            self.store
                .append_with_deadline(|_| kinds.clone(), deadline)?;
        }
        self.clear_pending(reservation.tip)
    }

    pub(super) fn clear_pending(&self, tip: gix::ObjectId) -> Result<()> {
        use gix::refs::transaction::{Change, RefEdit, RefLog};
        self.store
            .repo
            .edit_reference(RefEdit {
                change: Change::Delete {
                    expected: PreviousValue::MustExistAndMatch(gix::refs::Target::Object(tip)),
                    log: RefLog::AndReference,
                },
                name: self
                    .pending_name()?
                    .as_str()
                    .try_into()
                    .map_err(Error::repo)?,
                deref: false,
            })
            .map_err(Error::repo)?;
        Ok(())
    }
}

struct Node {
    parent: Option<gix::ObjectId>,
    value: u64,
    flights: Vec<EventId>,
}

fn invalid(detail: impl Into<String>) -> Error {
    Error::Counter {
        detail: detail.into(),
    }
}

fn decode(repo: &gix::Repository, id: gix::ObjectId) -> Result<Node> {
    let object = repo.find_object(id).map_err(Error::repo)?;
    if object.kind != gix::objs::Kind::Commit {
        return Err(invalid(format!("{id} is not a commit")));
    }
    let commit = gix::objs::CommitRef::from_bytes(&object.data).map_err(Error::repo)?;
    if commit.author.name != chain::ATC_NAME
        || commit.author.email != chain::ATC_EMAIL
        || commit.committer.name != chain::ATC_NAME
        || commit.committer.email != chain::ATC_EMAIL
    {
        return Err(invalid(format!("{id} does not have tower's identity")));
    }
    let message =
        std::str::from_utf8(commit.message).map_err(|_| invalid("counter message is not UTF-8"))?;
    let mut lines = message.lines();
    let value = lines
        .next()
        .filter(|line| !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|line| line.parse::<u64>().ok())
        .ok_or_else(|| invalid("counter message must start with its decimal value"))?;
    if lines
        .filter(|line| line.starts_with("tower-counter:"))
        .collect::<Vec<_>>()
        != [VERSION]
    {
        return Err(invalid("unsupported or missing counter format"));
    }
    let parents: Vec<_> = commit.parents().collect();
    if parents.len() > 1 {
        return Err(invalid("counter commits cannot merge"));
    }
    let object = repo.find_object(commit.tree()).map_err(Error::repo)?;
    if object.kind != gix::objs::Kind::Tree {
        return Err(invalid("counter tree is not a tree"));
    }
    let tree = gix::objs::TreeRef::from_bytes(&object.data).map_err(Error::repo)?;
    if tree.entries.len() != 1
        || tree.entries[0].filename != RESERVATION_FILE
        || tree.entries[0].mode.kind() != gix::objs::tree::EntryKind::Blob
    {
        return Err(invalid("counter tree must contain reservation.json alone"));
    }
    let blob = repo.find_object(tree.entries[0].oid).map_err(Error::repo)?;
    if blob.kind != gix::objs::Kind::Blob {
        return Err(invalid("reservation is not a blob"));
    }
    let flights: Vec<EventId> =
        serde_json::from_slice(&blob.data).map_err(|err| invalid(err.to_string()))?;
    if flights.iter().any(|id| id.seq == 0)
        || flights.iter().collect::<HashSet<_>>().len() != flights.len()
    {
        return Err(invalid(
            "reservation flight IDs must be positive and distinct",
        ));
    }
    for id in &flights {
        super::validate_component("writer", &id.writer).map_err(|err| invalid(err.to_string()))?;
    }
    if parents.is_empty() {
        if value != 0
            || !flights.is_empty()
            || !message.lines().any(|line| {
                line.strip_prefix("tower-domain: ")
                    .is_some_and(|nonce| !nonce.is_empty())
            })
        {
            return Err(invalid(
                "counter root must be zero with a domain nonce and no reservation",
            ));
        }
    } else if flights.is_empty() {
        return Err(invalid("counter successor has an empty reservation"));
    }
    Ok(Node {
        parent: parents.first().copied(),
        value,
        flights,
    })
}

/// Validate every ancestor, including range sizes. Suitable for a staged fetched ref.
pub fn inspect(repo: &gix::Repository, tip: gix::ObjectId) -> Result<Counter> {
    let mut cursor = tip;
    let mut node = decode(repo, cursor)?;
    let value = node.value;
    while let Some(parent) = node.parent {
        let previous = decode(repo, parent)?;
        if previous.value.checked_add(node.flights.len() as u64) != Some(node.value) {
            return Err(invalid(
                "counter increase does not match reservation length",
            ));
        }
        node = previous;
        cursor = parent;
    }
    Ok(Counter {
        tip,
        lineage: cursor,
        value,
    })
}

pub(super) fn reservation(repo: &gix::Repository, tip: gix::ObjectId) -> Result<Reservation> {
    let node = decode(repo, tip)?;
    let parent = node
        .parent
        .ok_or_else(|| invalid("a counter root is not a reservation"))?;
    let previous = decode(repo, parent)?;
    if previous.value.checked_add(node.flights.len() as u64) != Some(node.value) {
        return Err(invalid(
            "counter increase does not match reservation length",
        ));
    }
    Ok(Reservation {
        tip,
        parent,
        first: node.value - node.flights.len() as u64 + 1,
        flights: node.flights,
    })
}

pub(super) fn contains(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    ancestor: gix::ObjectId,
) -> Result<bool> {
    let mut cursor = Some(tip);
    while let Some(id) = cursor {
        if id == ancestor {
            return Ok(true);
        }
        cursor = decode(repo, id)?.parent;
    }
    Ok(false)
}

pub(super) fn move_ref(
    repo: &gix::Repository,
    name: &str,
    tip: gix::ObjectId,
    previous: Option<gix::ObjectId>,
) -> Result<()> {
    let expected = previous
        .map(|id| PreviousValue::MustExistAndMatch(gix::refs::Target::Object(id)))
        .unwrap_or(PreviousValue::MustNotExist);
    match chain::move_ref(repo, name, tip, expected, wall_clock(), "tower counter")? {
        chain::EditOutcome::Applied => Ok(()),
        chain::EditOutcome::Contended => Err(Error::Contended {
            writer: "counter".to_string(),
        }),
    }
}

fn write(
    repo: &gix::Repository,
    value: u64,
    parent: Option<gix::ObjectId>,
    flights: &[EventId],
    nonce: Option<&str>,
) -> Result<gix::ObjectId> {
    use gix::objs::tree::{Entry, EntryKind};
    let blob = repo
        .write_blob(serde_json::to_vec(flights).map_err(Error::repo)?)
        .map_err(Error::repo)?
        .detach();
    let tree = repo
        .write_object(&gix::objs::Tree {
            entries: vec![Entry {
                mode: EntryKind::Blob.into(),
                filename: RESERVATION_FILE.into(),
                oid: blob,
            }],
        })
        .map_err(Error::repo)?
        .detach();
    let signature = gix::actor::Signature {
        name: chain::ATC_NAME.into(),
        email: chain::ATC_EMAIL.into(),
        time: gix::date::Time {
            seconds: wall_clock(),
            offset: 0,
        },
    };
    let mut message = format!("{value}\n\n{VERSION}\n");
    if let Some(nonce) = nonce {
        message.push_str(&format!("tower-domain: {nonce}\n"));
    }
    repo.write_object(&gix::objs::Commit {
        tree,
        parents: parent.into_iter().collect::<Vec<_>>().into(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: message.into(),
        extra_headers: Vec::new(),
    })
    .map(|id| id.detach())
    .map_err(Error::repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atc_testsupport::Repo;

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }

    fn filing(store: &Store, subject: &str) -> EventId {
        store
            .append(vec![Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: String::new(),
                status: "ready".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            }])
            .expect("file")
            .remove(0)
    }

    #[test]
    fn opening_and_reading_do_not_initialize_a_counter() {
        let fixture = Repo::new();
        let store = Store::open(fixture.path()).expect("open");
        assert!(store.counter().expect("read").is_none());
        assert!(store.writer().is_none());
    }

    #[test]
    fn independent_empty_boards_have_distinct_durable_lineages() {
        let a = Repo::new();
        let b = Repo::new();
        let a = Store::open(a.path()).expect("a");
        let b = Store::open(b.path()).expect("b");
        let first = a
            .coordinate(deadline())
            .expect("lock")
            .initialize()
            .expect("root");
        let second = b
            .coordinate(deadline())
            .expect("lock")
            .initialize()
            .expect("root");
        assert_ne!(first.lineage, second.lineage);
        assert_eq!(first.tip, first.lineage);
        assert_eq!(first.value, 0);
        assert_eq!(
            a.counter().expect("read").expect("counter").lineage,
            first.lineage
        );
    }

    #[test]
    fn range_claims_are_recorded_once_and_the_counter_is_readable_by_git() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let ids = vec![
            filing(&store, "parent"),
            filing(&store, "one"),
            filing(&store, "two"),
        ];
        let held = store.coordinate(deadline()).expect("lock");
        let claimed = held.claim_local(&ids).expect("claim");
        assert_eq!(claimed, ids.iter().cloned().zip(1..=3).collect::<Vec<_>>());
        assert!(held.claim_local(&ids).expect("retry").is_empty());
        let events = store.read_all().expect("events");
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e.kind, Kind::Numbered { .. }))
                .count(),
            3
        );
        assert_eq!(store.counter().expect("read").expect("counter").value, 3);
        assert_eq!(
            fixture.git(&["log", "-1", "--format=%s", SEQ_REF]).trim(),
            "3"
        );
        assert!(
            ref_target(&store.repo, "refs/tower/pending/pi")
                .expect("pending")
                .is_none()
        );
    }

    #[test]
    fn a_confirmed_reservation_recovers_after_a_crash_before_log_append() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let id = filing(&store, "recover");
        let root;
        {
            let held = store.coordinate(deadline()).expect("lock");
            let counter = held.initialize().expect("initialize");
            root = counter.lineage;
            let reservation = held
                .prepare(&counter, std::slice::from_ref(&id))
                .expect("prepare");
            move_ref(&store.repo, SEQ_REF, reservation.tip, Some(counter.tip)).expect("reserve");
        }
        assert!(
            crate::board::fold(&store.read_all().expect("events")).flights[0]
                .global_number
                .is_none()
        );
        let reopened = Store::open(fixture.path()).expect("reopen");
        let held = reopened.coordinate(deadline()).expect("lock");
        held.recover_local().expect("recover");
        held.recover_local().expect("idempotent");
        let events = reopened.read_all().expect("events");
        assert_eq!(
            crate::board::fold(&events).flights[0].global_number,
            Some(1)
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e.kind, Kind::Numbered { .. }))
                .count(),
            1
        );
        let counter = reopened.counter().expect("read").expect("counter");
        assert_eq!(counter.lineage, root);
        assert_eq!(counter.value, 1);
    }

    #[test]
    fn recovery_after_log_append_does_not_append_again() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let id = filing(&store, "already recorded");
        let held = store.coordinate(deadline()).expect("lock");
        let counter = held.initialize().expect("root");
        let reservation = held
            .prepare(&counter, std::slice::from_ref(&id))
            .expect("prepare");
        move_ref(&store.repo, SEQ_REF, reservation.tip, Some(counter.tip)).expect("reserve");
        store
            .append(vec![Kind::Numbered {
                flight: id,
                number: 1,
                reservation: Some(reservation.tip.to_string()),
            }])
            .expect("record before crash");
        let before = store.read_all().expect("before").len();
        held.recover_local().expect("recover");
        assert_eq!(store.read_all().expect("after").len(), before);
    }

    #[test]
    fn an_old_domains_equal_number_does_not_suppress_the_new_reservation_record() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).unwrap();
        let id = filing(&store, "same digits, different claim");
        let held = store.coordinate(deadline()).unwrap();
        let root = held.initialize().unwrap();
        store
            .append(vec![Kind::Numbered {
                flight: id.clone(),
                number: 1,
                reservation: None,
            }])
            .unwrap();
        let reservation = held.prepare(&root, &[id]).unwrap();
        move_ref(&store.repo, SEQ_REF, reservation.tip, Some(root.tip)).unwrap();
        held.recover_local().unwrap();
        let events = store.read_all().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e.kind, Kind::Numbered { .. }))
                .count(),
            2
        );
        held.recover_local().unwrap();
        assert_eq!(store.read_all().unwrap().len(), events.len());
    }

    #[test]
    fn renumbering_replaces_an_observed_claim_from_a_faster_clock() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).unwrap();
        let flight = filing(&store, "renumber");
        let held = store.coordinate(deadline()).unwrap();
        let root = held.initialize().unwrap();
        let future = wall_clock() + 3600;
        let event = super::super::Event {
            id: "zeta.1".parse().unwrap(),
            author: "other@local".into(),
            writer: "zeta".into(),
            time: future,
            session: None,
            callsign: None,
            kind: Kind::Numbered {
                flight: flight.clone(),
                number: 100,
                reservation: None,
            },
        };
        let tip = chain::write_events(&store.repo, &[event], 2, None, future).unwrap();
        move_ref(&store.repo, "refs/tower/log/other@local/zeta", tip, None).unwrap();
        let reservation = held.prepare(&root, std::slice::from_ref(&flight)).unwrap();
        move_ref(&store.repo, SEQ_REF, reservation.tip, Some(root.tip)).unwrap();
        held.recover_local().unwrap();
        assert_eq!(
            crate::board::flight(&crate::board::fold(&store.read_all().unwrap()), &flight)
                .global_number,
            Some(1)
        );
    }

    #[test]
    fn uncommitted_pending_ranges_are_not_claims() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let id = filing(&store, "unconfirmed");
        let held = store.coordinate(deadline()).expect("lock");
        let counter = held.initialize().expect("root");
        held.prepare(&counter, std::slice::from_ref(&id))
            .expect("prepare only");
        held.recover_local().expect("recover");
        assert_eq!(store.counter().expect("read").expect("counter").value, 0);
        assert!(
            crate::board::fold(&store.read_all().expect("events")).flights[0]
                .global_number
                .is_none()
        );
        assert_eq!(held.claim_local(&[id]).expect("claim")[0].1, 1);
    }

    #[test]
    fn reserved_gaps_are_not_reused() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let id = filing(&store, "after gap");
        let held = store.coordinate(deadline()).expect("lock");
        let root = held.initialize().expect("root");
        let lost = write(
            &store.repo,
            2,
            Some(root.tip),
            &["lost.1".parse().expect("id"), "lost.2".parse().expect("id")],
            None,
        )
        .expect("objects");
        move_ref(&store.repo, SEQ_REF, lost, Some(root.tip)).expect("reserved but no record");
        assert_eq!(held.claim_local(&[id]).expect("claim")[0].1, 3);
    }

    #[test]
    fn counter_validation_rejects_malformed_ranges_and_foreign_commits() {
        let fixture = Repo::new();
        let store = Store::open(fixture.path()).expect("open");
        let held = store.coordinate(deadline()).expect("lock");
        let root = held.initialize().expect("root");
        let id: EventId = "pi.1".parse().expect("id");
        for (value, ids) in [
            (0, vec![id.clone()]),
            (2, vec![id.clone()]),
            (2, vec![id.clone(), id.clone()]),
            (1, vec![]),
        ] {
            let tip = write(&store.repo, value, Some(root.tip), &ids, None).expect("objects");
            assert!(
                inspect(&store.repo, tip).is_err(),
                "accepted value {value} with {ids:?}"
            );
        }
        let foreign = store.repo.head_id().expect("head").detach();
        assert!(inspect(&store.repo, foreign).is_err());
        assert_eq!(
            store.counter().expect("read").expect("unchanged").tip,
            root.tip
        );
    }

    #[test]
    fn a_waiter_observes_its_total_deadline_and_drop_releases_ownership() {
        let fixture = Repo::new();
        let store = Store::open(fixture.path()).expect("open");
        let other = Store::open(fixture.path()).expect("other");
        let held = store.coordinate(deadline()).expect("lock");
        let start = Instant::now();
        assert!(matches!(
            other.coordinate(start + Duration::from_millis(60)),
            Err(Error::Deadline)
        ));
        assert!(start.elapsed() < Duration::from_millis(500));
        drop(held);
        let _reacquired = other.coordinate(deadline()).expect("released");
    }

    #[test]
    fn invalid_reservation_inputs_leave_no_pending_ref() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let held = store.coordinate(deadline()).expect("lock");
        let root = held.initialize().expect("root");
        for ids in [
            vec![],
            vec!["pi.0".parse().expect("id")],
            vec!["pi.1".parse().expect("id"); 2],
            vec![EventId {
                writer: "bad/writer".to_string(),
                seq: 1,
            }],
        ] {
            assert!(held.prepare(&root, &ids).is_err());
            assert!(
                ref_target(&store.repo, "refs/tower/pending/pi")
                    .expect("pending")
                    .is_none()
            );
            assert_eq!(store.counter().expect("read").expect("counter").value, 0);
        }
    }

    #[test]
    fn concurrent_local_claims_have_disjoint_ranges() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let ids: Vec<_> = (0..8)
            .map(|i| filing(&store, &format!("flight {i}")))
            .collect();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(ids.len()));
        let threads: Vec<_> = ids
            .into_iter()
            .map(|id| {
                let path = fixture.path().to_path_buf();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let store = Store::open(&path).expect("open");
                    barrier.wait();
                    store
                        .coordinate(deadline())
                        .expect("lock")
                        .claim_local(&[id])
                        .expect("claim")[0]
                        .1
                })
            })
            .collect();
        let mut numbers: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("join"))
            .collect();
        numbers.sort_unstable();
        assert_eq!(numbers, (1..=8).collect::<Vec<_>>());
        assert_eq!(store.counter().expect("read").expect("counter").value, 8);
    }

    #[test]
    fn local_recovery_never_discards_a_shared_reservation() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let store = Store::open(fixture.path()).expect("open");
        let id = filing(&store, "pending shared claim");
        let tip = {
            let held = store.coordinate(deadline()).expect("lock");
            let root = held.initialize().expect("root");
            held.prepare(&root, std::slice::from_ref(&id))
                .expect("prepare")
                .tip
        };
        fixture.git(&["config", "tower.remote", "origin"]);
        let shared = Store::open(fixture.path()).expect("reopen");
        let held = shared.coordinate(deadline()).expect("lock");
        assert!(held.recover_local().is_err());
        assert!(held.claim_local(&[id]).is_err());
        assert_eq!(
            ref_target(&shared.repo, "refs/tower/pending/pi").expect("pending"),
            Some(tip)
        );
        assert_eq!(shared.counter().expect("read").expect("counter").value, 0);
    }

    #[test]
    fn different_writers_share_one_numbering_domain() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let pi = Store::open(fixture.path()).expect("pi");
        let a = filing(&pi, "pi flight");
        fixture.pin_writer("qi");
        let qi = Store::open(fixture.path()).expect("qi");
        let b = filing(&qi, "qi flight");
        pi.coordinate(deadline())
            .expect("lock")
            .claim_local(&[a])
            .expect("claim pi");
        qi.coordinate(deadline())
            .expect("lock")
            .claim_local(&[b])
            .expect("claim qi");
        let fold = crate::board::fold_numbered(&pi.read_all().expect("union"), 2);
        assert_eq!(
            fold.flights
                .iter()
                .map(|flight| flight.number)
                .collect::<Vec<_>>(),
            [1, 1]
        );
        let mut globals: Vec<_> = fold
            .flights
            .iter()
            .map(|flight| flight.global_number.expect("claimed"))
            .collect();
        globals.sort_unstable();
        assert_eq!(globals, [1, 2]);
        assert_eq!(
            pi.counter().expect("counter").expect("present").lineage,
            qi.counter().expect("counter").expect("present").lineage
        );
    }

    #[test]
    fn linked_worktrees_share_counter_lock_and_lineage() {
        let fixture = Repo::new();
        fixture.pin_writer("pi");
        let linked = fixture.path().join("linked");
        fixture.git(&[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().expect("path"),
        ]);
        let main = Store::open(fixture.path()).expect("main");
        let other = Store::open(&linked).expect("linked");
        let held = main.coordinate(deadline()).expect("lock");
        let root = held.initialize().expect("root");
        assert!(matches!(
            other.coordinate(Instant::now() + Duration::from_millis(30)),
            Err(Error::Deadline)
        ));
        drop(held);
        let id = filing(&other, "linked filing");
        other
            .coordinate(deadline())
            .expect("lock")
            .claim_local(&[id])
            .expect("claim");
        let counter = main.counter().expect("read").expect("counter");
        assert_eq!(counter.lineage, root.lineage);
        assert_eq!(counter.value, 1);
    }
}

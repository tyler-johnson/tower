//! The chain: hand-built commits on `refs/tower/log/<author>/<writer>`.
//!
//! One commit per append, one parent per commit (the root has none), one
//! tree entry: `events.json`. The message carries two trailers —
//! `tower-v`, the chain format checked before the payload is read, and
//! `tower-seq`, the next unused sequence number, which makes append O(1)
//! instead of a count of the chain.
//!
//! What fufu's op log needs and this chain does not: no trailer/payload
//! split (the board folds every event, so there is no fast path to feed),
//! no stated-prev walk (a true orphan's one parent *is* prev), and no gc
//! reflog-expiry guard (every event is reachable from the tip, and the ref
//! is the gc root). `force_create_reflog` stays on for forensics only;
//! nothing depends on those lines surviving.

use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};

use crate::log::error::{Error, Result};
use crate::log::event::Event;

/// The fixed identity every log commit bears as both author and committer.
/// A write can never depend on the user's git config, and the signature
/// doubles as the decode gate's first check — necessary, never sufficient.
pub const TOWER_NAME: &str = "tower";
pub const TOWER_EMAIL: &str = "tower@local";

/// The chain format this tower writes and reads.
pub const CHAIN_VERSION: u32 = 1;

/// The one entry in every log commit's tree.
const EVENTS_FILE: &str = "events.json";

/// Every log chain lives under here; the fold unions the lot.
pub(crate) const LOG_PREFIX: &str = "refs/tower/log/";

pub(crate) fn log_ref(author: &str, writer: &str) -> String {
    format!("{LOG_PREFIX}{author}/{writer}")
}

fn is_tower_commit(commit: &gix::objs::CommitRef<'_>) -> bool {
    commit.author.name == TOWER_NAME
        && commit.author.email == TOWER_EMAIL
        && commit.committer.name == TOWER_NAME
        && commit.committer.email == TOWER_EMAIL
}

fn build_message(count: usize, next_seq: u64) -> String {
    format!("{count} event(s)\n\ntower-v: {CHAIN_VERSION}\ntower-seq: {next_seq}\n")
}

/// The value of one `key: value` line, wherever it stands in the message.
/// Tolerant on position on purpose: the trailer block is the contract, its
/// line order is not.
fn trailer<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}: ");
    text.lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}

/// One decoded log commit.
pub(crate) struct Decoded {
    /// The only parent; `None` at the root.
    pub parent: Option<gix::ObjectId>,
    /// The next unused sequence number, read off the trailer.
    pub next_seq: u64,
    /// Committer seconds — the floor the next append's clock is clamped to.
    pub time: i64,
    /// The payload, in append order.
    pub events: Vec<Event>,
}

/// Decode a log commit. Anything that is not one is refused — the guard
/// and the decode are the same step, so no caller can walk past a commit
/// tower did not write and fold it into a board. The gate, in order: it is
/// a commit, its signature is tower's, its `tower-v` is a version this
/// tower reads, then `events.json` parses.
pub(crate) fn decode(repo: &gix::Repository, id: gix::ObjectId) -> Result<Decoded> {
    let refused = || Error::NotTowerLog { id: id.to_string() };

    let (parent, tree_id, next_seq, time) = {
        let obj = repo
            .try_find_object(id)
            .map_err(Error::repo)?
            .filter(|obj| obj.kind == gix::objs::Kind::Commit)
            .ok_or_else(refused)?;
        let commit = gix::objs::CommitRef::from_bytes(&obj.data).map_err(Error::repo)?;
        if !is_tower_commit(&commit) {
            return Err(refused());
        }
        let text = String::from_utf8_lossy(commit.message);
        let found: u32 = trailer(&text, "tower-v")
            .and_then(|value| value.trim().parse().ok())
            .ok_or_else(refused)?;
        if found != CHAIN_VERSION {
            return Err(Error::Version {
                id: id.to_string(),
                found,
                reads: CHAIN_VERSION,
            });
        }
        let next_seq: u64 = trailer(&text, "tower-seq")
            .and_then(|value| value.trim().parse().ok())
            .ok_or_else(refused)?;
        let time = commit.committer.time().map_err(Error::repo)?.seconds;
        (commit.parents().next(), commit.tree(), next_seq, time)
    };

    let blob_id = {
        let obj = repo
            .try_find_object(tree_id)
            .map_err(Error::repo)?
            .filter(|obj| obj.kind == gix::objs::Kind::Tree)
            .ok_or_else(refused)?;
        let tree = gix::objs::TreeRef::from_bytes(&obj.data).map_err(Error::repo)?;
        tree.entries
            .iter()
            .find(|entry| entry.filename == EVENTS_FILE)
            .map(|entry| entry.oid.to_owned())
            .ok_or_else(refused)?
    };

    let obj = repo
        .try_find_object(blob_id)
        .map_err(Error::repo)?
        .filter(|obj| obj.kind == gix::objs::Kind::Blob)
        .ok_or_else(refused)?;
    let events: Vec<Event> = serde_json::from_slice(&obj.data).map_err(|_| refused())?;

    Ok(Decoded {
        parent,
        next_seq,
        time,
        events,
    })
}

/// Write one append's objects: blob → tree → commit. `write_blob`
/// pre-hashes and skips an existing object, so identical payloads dedup
/// for free. Nothing here moves a ref.
pub(crate) fn write_events(
    repo: &gix::Repository,
    events: &[Event],
    next_seq: u64,
    parent: Option<gix::ObjectId>,
    now: i64,
) -> Result<gix::ObjectId> {
    let json = serde_json::to_vec_pretty(events).map_err(Error::repo)?;
    let blob = repo.write_blob(&json).map_err(Error::repo)?.detach();

    use gix::objs::tree::{Entry as TreeEntry, EntryKind};
    let tree = gix::objs::Tree {
        entries: vec![TreeEntry {
            mode: EntryKind::Blob.into(),
            filename: EVENTS_FILE.into(),
            oid: blob,
        }],
    };
    let tree_id = repo.write_object(&tree).map_err(Error::repo)?.detach();

    let sig = gix::actor::Signature {
        name: TOWER_NAME.into(),
        email: TOWER_EMAIL.into(),
        time: gix::date::Time {
            seconds: now,
            offset: 0,
        },
    };
    let commit = gix::objs::Commit {
        tree: tree_id,
        parents: parent.into_iter().collect::<Vec<_>>().into(),
        author: sig.clone(),
        committer: sig,
        encoding: None,
        message: build_message(events.len(), next_seq).into(),
        extra_headers: Vec::new(),
    };
    Ok(repo.write_object(&commit).map_err(Error::repo)?.detach())
}

/// Whether an edit failure is contention — a held lock or a lost CAS —
/// rather than a real error. fufu's classification, ported from its
/// `refs.rs`.
fn is_contended(err: &gix::reference::edit::Error) -> bool {
    use gix::refs::file::transaction::prepare::Error as Prepare;
    match err {
        gix::reference::edit::Error::FileTransactionPrepare(err) => matches!(
            err,
            Prepare::LockAcquire { .. }
                | Prepare::MustNotExist { .. }
                | Prepare::MustExist { .. }
                | Prepare::ReferenceOutOfDate { .. }
        ),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditOutcome {
    Applied,
    Contended,
}

/// Move the chain ref, CAS-guarded. The lock is the exclusion; this
/// expectation is the second line, catching a foreign writer or a stale
/// tip the lock has nothing to say about. Contention is a value, not an
/// error, so the caller picks the retry policy.
pub(crate) fn move_ref(
    repo: &gix::Repository,
    name: &str,
    target: gix::ObjectId,
    expected: PreviousValue,
    now: i64,
    message: &str,
) -> Result<EditOutcome> {
    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                // Custom namespaces get no reflog by default, and silently
                // so. Forensics only; nothing depends on the lines
                // surviving expiry.
                force_create_reflog: true,
                message: message.into(),
            },
            expected,
            new: gix::refs::Target::Object(target),
        },
        name: name.try_into().map_err(Error::repo)?,
        deref: false,
    };
    let time_str = format!("{now} +0000");
    let committer = gix::actor::SignatureRef {
        name: TOWER_NAME.into(),
        email: TOWER_EMAIL.into(),
        time: &time_str,
    };
    match repo.edit_references_as(Some(edit), Some(committer)) {
        Ok(_) => Ok(EditOutcome::Applied),
        Err(err) if is_contended(&err) => Ok(EditOutcome::Contended),
        Err(err) => Err(Error::repo(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CAS second line: an expectation the ref has moved past comes
    /// back [`EditOutcome::Contended`] — not applied, not an error — and
    /// the ref does not move. This is the shape a foreign mover leaves
    /// behind, and the one thing the lock has nothing to say about.
    #[test]
    fn a_stale_expectation_is_contention_not_a_clobber() {
        let fixture = ff_tower_testsupport::Repo::new();
        let repo = gix::discover(fixture.path()).expect("open");
        let name = "refs/tower/log/tests@tower.invalid/pi";

        let root = write_events(&repo, &[], 1, None, 100).expect("root");
        assert_eq!(
            move_ref(&repo, name, root, PreviousValue::MustNotExist, 100, "root").expect("edit"),
            EditOutcome::Applied,
        );

        let fork = write_events(&repo, &[], 2, Some(root), 101).expect("fork");
        let stale = PreviousValue::MustExistAndMatch(gix::refs::Target::Object(fork));
        assert_eq!(
            move_ref(&repo, name, fork, stale, 101, "stale").expect("edit"),
            EditOutcome::Contended,
        );
        assert_eq!(fixture.git(&["rev-parse", name]).trim(), root.to_string());
    }
}

//! The one lock a writer's chain is appended under.
//!
//! Ported from fufu's `ops/lock.rs`, reasoning included, because the bug
//! it guards against is gix's rather than fufu's:
//! `file::Transaction::prepare` reads each reference's existing value
//! *before* it acquires that reference's lock, and compares
//! `MustExistAndMatch` against the value it read. Two writers can both
//! read the same tip, both pass the check, then take the lock in turn and
//! both apply — the loser's append reports success, and the winner writes
//! over it naming the tip from before either of them. The event vanishes
//! from the log while staying in the reflog. fufu reproduced this about
//! once in thirty racing appends.
//!
//! So the CAS is the second line, and this file is the first: tower's own
//! lock, held across read-tip → write-objects → move-ref. It matters more
//! here than in fufu, whose chains are keyed per worktree and never
//! contend — every tower bay on one machine writes the *same* ref, so
//! contention is the normal case rather than the rare one.
//!
//! The lock is tower's own file rather than the ref's `.lock`: gix takes
//! that one itself inside the transaction, and a writer holding it would
//! deadlock against its own append. Nothing outside tower writes the log,
//! so a lock only tower observes loses nothing.

use std::time::Duration;

use crate::log::error::{Error, Result};

/// How long an append waits for another writer. Long enough to cover an
/// append already in flight — a few small object writes and one ref edit —
/// and short enough that a stale lock file is noticed rather than waited
/// out.
const APPEND_WAIT: Duration = Duration::from_secs(2);

/// Held from the read of the tip until the ref has moved. Dropping it
/// releases the lock; there is nothing to commit, because the lock names a
/// resource rather than staging a new value for one. Bind it to a named
/// local — `_` drops it on the spot.
pub(crate) struct Guard(#[allow(dead_code)] gix::lock::Marker);

/// Take the write lock on one writer's chain, waiting briefly. A wait
/// that expires is contention, reported as such: unlike fufu's captures,
/// tower has no caller for whom giving up silently is the right answer.
pub(crate) fn acquire(repo: &gix::Repository, writer: &str) -> Result<Guard> {
    let dir = repo.common_dir().join("tower");
    let name = format!("log-{writer}");
    match gix::lock::Marker::acquire_to_hold_resource(
        dir.join(name),
        gix::lock::acquire::Fail::AfterDurationWithBackoff(APPEND_WAIT),
        Some(dir),
    ) {
        Ok(marker) => Ok(Guard(marker)),
        Err(gix::lock::acquire::Error::PermanentlyLocked { .. }) => Err(Error::Contended {
            writer: writer.to_string(),
        }),
        Err(err) => Err(Error::repo(err)),
    }
}

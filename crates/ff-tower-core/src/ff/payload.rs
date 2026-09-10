//! The payloads tower reads, mirrored as tower's own types.
//!
//! These are deliberately *not* fufu's structs. tower holds no fufu type —
//! that is the seam's whole point — so `data` is parsed into shapes declared
//! here, from the same JSON any other extension gets.
//!
//! Two rules keep the mirror from becoming a maintenance tax:
//!
//! **A field appears here when a tower caller reads it, and not before.**
//! `ff status --json` carries a dozen more than this — `futures`, `upstream`,
//! `operation`, `foreign`, `parent` — and mirroring them ahead of a reader
//! would be signing up to chase refactors in fields nothing consults.
//!
//! **Unknown fields are ignored, unknown enum variants are not fatal.** fufu
//! adds to its payloads between releases and the contract version does not
//! move for it. A tower that failed to parse a board because fufu grew a
//! field would be broken by an upgrade that broke nothing.

use serde::Deserialize;

/// `ff status --json` — the state of one worktree.
#[derive(Debug, Clone, Deserialize)]
pub struct Status {
    pub head: Head,
    pub open: Open,
    pub changes: Vec<FileStat>,
    pub insertions: u32,
    pub deletions: u32,
    /// Paths standing in conflict in the working tree right now.
    pub conflicts: Vec<String>,
    /// A rewrite stopped and waiting for a person. fufu's `held`, which is
    /// the state tower's own `hold` is named after and inherits principle 8
    /// from — but this one is fufu's, about a replay, not a flight's.
    pub held: Option<Held>,
    /// **Not the session tag.** fufu's `session` field is an `ff edit`
    /// session — a commit being edited on a branch of its own. The tag tower
    /// rides on every call (`--session <flight>`) is a different thing that
    /// happens to share the word, and it never appears here. Renamed on the
    /// way in so no board code can confuse the two.
    #[serde(rename = "session")]
    pub editing: Option<Editing>,
}

/// Where HEAD points.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Head {
    /// A branch with no commits yet.
    Unborn {
        r#ref: String,
    },
    Branch {
        name: String,
        r#ref: String,
        commit: String,
    },
    Detached {
        commit: String,
    },
}

impl Head {
    /// The branch name, when HEAD is on one. The board's most common
    /// question about a worktree, and the one that links a bay to a flight.
    pub fn branch(&self) -> Option<&str> {
        match self {
            Head::Branch { name, .. } => Some(name.as_str()),
            Head::Unborn { .. } | Head::Detached { .. } => None,
        }
    }
}

/// The open change: work fufu is holding that is not a commit yet.
///
/// This is the field the whole thesis rests on. An agent that has edited for
/// twenty minutes and committed nothing shows up here — `clean: false` with
/// a `time` — and nowhere else.
#[derive(Debug, Clone, Deserialize)]
pub struct Open {
    pub id: Option<String>,
    pub id_letters: Option<String>,
    pub pending: Option<String>,
    pub subject: Option<String>,
    pub clean: bool,
    pub base: Option<String>,
    /// Unix seconds of the newest snapshot.
    pub time: Option<i64>,
}

/// One file's contribution to the open change.
#[derive(Debug, Clone, Deserialize)]
pub struct FileStat {
    pub path: String,
    /// Source path, for a rename or a copy.
    pub from: Option<String>,
    pub kind: ChangeKind,
    pub insertions: u32,
    pub deletions: u32,
    pub binary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    TypeChange,
    Renamed,
    Copied,
    IntentToAdd,
    /// A kind fufu grew after this was written. Every reader here cares
    /// which files moved rather than how, so an unrecognized kind is a
    /// detail to carry, not a parse to fail.
    #[serde(other)]
    Unknown,
}

/// A fufu rewrite held: nothing moved, and a person has to decide.
#[derive(Debug, Clone, Deserialize)]
pub struct Held {
    /// The verb that recorded it: restack, done, absorb or lift.
    pub verb: String,
    pub at: At,
    pub paths: Vec<String>,
    pub time: i64,
}

/// Where a held replay stopped.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "what", rename_all = "kebab-case")]
pub enum At {
    Commit { id: String, subject: String },
    OpenChange,
}

/// An `ff edit` session running on the branch underfoot. See the note on
/// `Status::editing` about the word.
#[derive(Debug, Clone, Deserialize)]
pub struct Editing {
    /// The session branch — the one HEAD is on.
    pub branch: String,
    /// The commit being edited, full sha.
    pub editing: String,
    pub subject: String,
    /// The branch its commits replay onto when the session ends.
    pub onto: String,
}

/// `ff collide --json` — the sideways axis, and tower's earned existence.
///
/// One pair, judged. Every discovered conflict, every land order, and the
/// conflict-free set `ff tower next -n <k>` hands out is a fold over these
/// verdicts — the fold is tower's, the verdict is fufu's, and tower does
/// not reimplement the merge to get one. The probe runs in an object-memory
/// clone and writes nothing, and it judges each side on the open change's
/// tree when that differs from the tip's — so a branch an agent is editing
/// right now, with nothing committed, still answers.
#[derive(Debug, Clone, Deserialize)]
pub struct Collision {
    pub a: Side,
    pub b: Side,
    pub pairing: Pairing,
}

/// One branch, as the probe judged it. The ids are what make a verdict
/// cacheable: it is a pure function of the two trees and the base between
/// them, so a verdict stays good until one of these moves.
#[derive(Debug, Clone, Deserialize)]
pub struct Side {
    pub name: String,
    /// The branch tip, lowercase hex.
    pub tip: String,
    /// The tree it was judged on.
    pub tree: String,
    /// True when that tree is uncommitted work the operation log holds.
    pub open: bool,
}

/// How two branches answer each other.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Pairing {
    /// A three-way merge leaves no conflicts.
    Clear,
    /// It would conflict, on exactly these paths.
    Collide { paths: Vec<String> },
    /// No base to merge against: refused, not guessed. Distinct from
    /// `Clear`, and tower must never round it down to one — an unknown
    /// pairing is a reason to leave a flight out of a fan-out set.
    Unknown { reason: UnknownReason },
}

impl Pairing {
    /// True only for a verdict that says these two do not touch. `Unknown`
    /// is not clear; it is unanswered.
    pub fn is_clear(&self) -> bool {
        matches!(self, Pairing::Clear)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnknownReason {
    UnrelatedHistories,
    MergeCommits,
    TooManyCommits,
    #[serde(other)]
    Other,
}

/// `ff op log --json` — one operation row.
///
/// The row that ties a flight to a branch: `session` is the tag tower rode
/// on the call that captured it, `branch` is where HEAD stood when it did.
/// `id`, `verb`, `summary` and the rest go unmirrored until something in
/// tower reads them.
#[derive(Debug, Clone, Deserialize)]
pub struct OpEntry {
    /// `@detached` is a literal fufu emits for a detached HEAD, carried
    /// as-is rather than decoded here.
    pub branch: Option<String>,
    pub session: Option<String>,
    /// Unix seconds.
    pub time: i64,
}

/// `ff op log --json` — the payload around the rows.
#[derive(Debug, Clone, Deserialize)]
pub struct OpLog {
    pub ops: Vec<OpEntry>,
}

/// `ff worktree list --json` — one worktree row: a bay, or the main
/// worktree itself.
///
/// `path` is null for a bare repository's main row — tower never usefully
/// runs bare, but the payload parses what fufu emits. A null `branch` is a
/// detached HEAD. `chain` and `tip` stay unmirrored: branch tips come from
/// the `branch list` join, and nothing in tower reads a chain ref by name.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeInfo {
    /// fufu's worktree id — `main` for the main worktree.
    pub id: String,
    pub path: Option<String>,
    pub branch: Option<String>,
    /// True on the row the invoking worktree answered for.
    pub current: bool,
}

/// One orphan chain in the survey: a bay torn down whose chain remains.
/// Every `bay release` leaves one by design — fufu guarantees the chain
/// outlives the bay — so an orphan is a fact, not a fault. `chain` stays
/// unmirrored: nothing in tower reads a chain ref by name.
#[derive(Debug, Clone, Deserialize)]
pub struct OrphanInfo {
    /// The worktree id the chain answered to.
    pub id: String,
    /// The chain's newest operation, `ff restore --at-op`'s address.
    pub tip: Option<String>,
    /// The branch the bay stood on when it was torn down.
    pub branch: Option<String>,
    /// Unix seconds of the tip operation.
    pub time: Option<i64>,
}

/// `ff worktree list --json` — the survey: every worktree, main first,
/// and the orphan chains beside them. `orphans` defaults so an older
/// fufu's payload without the key still parses.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeList {
    pub worktrees: Vec<WorktreeInfo>,
    #[serde(default)]
    pub orphans: Vec<OrphanInfo>,
}

/// `ff worktree add --json` — the envelope wrapper around what was made.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeAdd {
    pub added: WorktreeAdded,
}

/// The worktree `ff worktree add` made.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeAdded {
    pub id: String,
    pub path: String,
    pub branch: String,
}

/// `ff worktree remove --json` — the envelope wrapper around what was
/// torn down.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeRemove {
    pub removed: WorktreeRemoved,
}

/// The worktree `ff worktree remove` tore down. The capture came first —
/// that is why the verb needs no `--force` — and `capture` is its op id,
/// null when the tree held nothing to keep.
#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeRemoved {
    pub id: String,
    pub path: String,
    pub branch: Option<String>,
    pub capture: Option<String>,
}

/// `ff branch list --json` — one branch, with fufu's holds on it.
///
/// fufu's `session` field is deliberately unmirrored: it is an *editing*
/// session, the same naming trap `Status::editing` renames away, and
/// nothing in tower reads it.
#[derive(Debug, Clone, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    /// `None` for the unborn current branch, which the list still carries.
    pub tip: Option<String>,
    pub held: bool,
    pub resolving: bool,
}

/// `ff branch list --json` — every branch fufu knows.
#[derive(Debug, Clone, Deserialize)]
pub struct BranchList {
    pub named: Vec<BranchInfo>,
    pub anonymous: Vec<BranchInfo>,
}

/// `ff version --json` — what is actually installed. Repo-independent,
/// and the doctor's drift check: the call itself is what surfaces an
/// [`Error::Contract`](super::Error::Contract) when the `ff` on PATH and
/// tower have moved apart.
#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_survey_with_orphans_parses() {
        let list: WorktreeList = serde_json::from_str(
            r#"{"worktrees":[{"id":"main","path":"/repo","branch":"main","chain":"refs/fufu/wt/main/ops","tip":"abc","current":true}],"orphans":[{"id":"bay1","chain":"refs/fufu/wt/bay1/ops","tip":"def","branch":"feather","time":1700000000}]}"#,
        )
        .expect("parse");
        assert_eq!(list.worktrees.len(), 1);
        assert_eq!(list.orphans.len(), 1);
        let orphan = &list.orphans[0];
        assert_eq!(orphan.id, "bay1");
        assert_eq!(orphan.tip.as_deref(), Some("def"));
        assert_eq!(orphan.branch.as_deref(), Some("feather"));
        assert_eq!(orphan.time, Some(1_700_000_000));
    }

    #[test]
    fn a_survey_without_the_orphans_key_parses() {
        // An older fufu's payload — the key defaults instead of failing.
        let list: WorktreeList = serde_json::from_str(r#"{"worktrees":[]}"#).expect("parse");
        assert!(list.orphans.is_empty());
    }

    #[test]
    fn an_orphan_with_only_an_id_parses() {
        // The optional fields are absence-tolerant, unknown ones ignored.
        let orphan: OrphanInfo =
            serde_json::from_str(r#"{"id":"bay1","a_field_from_next_year":true}"#).expect("parse");
        assert_eq!(orphan.id, "bay1");
        assert!(orphan.tip.is_none() && orphan.branch.is_none() && orphan.time.is_none());
    }

    #[test]
    fn the_version_payload_parses_and_ignores_unknown_fields() {
        let version: Version = serde_json::from_str(
            r#"{"version":"0.9.0","commit":"ae91532","date":"2026-08-27","update":{"status":"unofficial","latest":null}}"#,
        )
        .expect("parse");
        assert_eq!(version.version, "0.9.0");
    }
}

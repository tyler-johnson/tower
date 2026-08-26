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

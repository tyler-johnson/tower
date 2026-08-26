//! What can go wrong in the store.
//!
//! On slice 1's shape: a plain thiserror enum with a `Result` alias, no
//! code registry. The split that matters here is refusal against
//! corruption. `Identity`, `RefName`, and `Contended` are tower declining
//! to write — the repository is fine and the caller can fix the input or
//! try again. `NotTowerLog` and `Version` are the decode gate refusing to
//! *read*: something on a tower ref is not tower's, and folding it into a
//! board would launder it into authored intent. `Repo` is gix failing
//! underneath either half.

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No `user.email` to file events under. Fatal by design, the same
    /// stance as fufu's `identity/missing`: tower's own machinery signs as
    /// `tower <tower@local>`, but events belong to a person.
    #[error("no author identity configured: set it with `git config user.email <email>`")]
    Identity,

    /// An author or writer that cannot be a ref-name component, so the
    /// chain it names could never exist.
    #[error("{what} `{value}` cannot name a ref under refs/tower/log/: {detail}")]
    RefName {
        what: &'static str,
        value: String,
        detail: String,
    },

    /// Another writer held the log through the whole wait, or the CAS kept
    /// losing through every retry. Both mean the same thing to a caller:
    /// try again when the other writer is done.
    #[error("another writer holds the tower log for `{writer}`")]
    Contended { writer: String },

    /// A commit on a tower ref that tower did not write, or one whose
    /// payload does not decode. Refused whole: the guard and the decode
    /// are the same step, and nothing downstream sees a commit that failed
    /// it.
    #[error("{id} is not a tower log commit")]
    NotTowerLog { id: String },

    /// A chain format from the future. Checked before the payload is
    /// touched, the same discipline as the seam's contract number.
    #[error("commit {id} carries chain format {found}; this tower reads {reads} — upgrade tower")]
    Version { id: String, found: u32, reads: u32 },

    /// gix, underneath everything else.
    #[error("git error: {0}")]
    Repo(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    pub(crate) fn repo(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
        Error::Repo(err.into())
    }
}

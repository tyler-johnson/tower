//! What a write verb can refuse with, and the id every refusal answers
//! to.
//!
//! One table for both surfaces: the CLI wraps this whole in its
//! `CliError` and the server in its `ApiError`, and each reads id,
//! message, and exits from here, so the two envelopes cannot drift. The
//! wrapped errors keep naming themselves — a resolver, log, or registry
//! refusal carries its own id and exits through transparently.
//!
//! The exits below are what the CLI envelope actually carries today, the
//! raise site's or the registry's — stated here because the server has no
//! registry to fall back on, and the fallback rule on both surfaces is
//! "the site's exits when it has any."

use crate::board::{QueryError, ResolveError};
use crate::{log, procedure};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A flight reference refused by the resolver — its own id and
    /// exits, carried through.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Log(#[from] log::Error),
    /// A definition the loader or the lookup declined — its own id,
    /// carried through.
    #[error(transparent)]
    Procedure(#[from] procedure::Error),
    /// A query the codec declined — its own id and exits, carried
    /// through.
    #[error(transparent)]
    Query(#[from] QueryError),

    #[error("the subject is empty")]
    EmptySubject,
    /// `decompose` with no procedure and no subjects — nothing to mint.
    #[error("there is nothing to split into")]
    NoParts,
    #[error("the procedure name is empty")]
    EmptyProcedure,
    #[error("no question given")]
    NeedsQuestion,
    #[error("no answer given")]
    NeedsAnswer,
    #[error("no note given")]
    NeedsNote,
    /// A word the status vocabulary does not carry.
    #[error(
        "`{word}` is not a status — triage, waiting, ready, in_progress, held, done, or canceled"
    )]
    BadStatus { word: String },
    /// `waiting` typed as a move — the fold derives it from the edges.
    #[error(
        "`{display}` cannot be set waiting — waiting comes from links: `ff tower link <flight> <dependency>`"
    )]
    StatusWaiting { display: String },
    /// `held` typed as a move — the fold derives it from a question.
    #[error(
        "`{display}` cannot be set held — held comes from a question: `ff tower hold <flight> -m <question>`"
    )]
    StatusHold { display: String },
    /// A word the assignee vocabulary does not carry.
    #[error("`{word}` is not a lane — me, agent, or none")]
    BadAssignee { word: String },
    /// A lifecycle verb reaching a flight that is already off the board.
    #[error("`{display}` is done — the log keeps its record")]
    FlightDone { display: String },
    /// `done` twice — the same id, with "already" wording.
    #[error("`{display}` is already done")]
    AlreadyDone { display: String },
    /// A status move over an open question — only `done` and `canceled`
    /// may override it.
    #[error("`{display}` is held on a question: {question}")]
    StatusHeld { display: String, question: String },
    #[error("`{display}` is already held: {question}")]
    AlreadyHeld { display: String, question: String },
    #[error("`{display}` has no open question")]
    NotHeld { display: String },

    /// `view save` with a blank name.
    #[error("the view name is empty")]
    EmptyName,
    /// A view reference that is not a wire id.
    #[error("`{text}` is not a view id — `<writer>.<seq>`")]
    BadView { text: String },
    /// A view id naming nothing this viewer sees — another author's
    /// personal view lands here too, since personal is a rendering
    /// rule and never a permission.
    #[error("no view `{text}` is visible to you")]
    ViewNotFound { text: String },
    /// `view edit` with every field left unsaid.
    #[error("the edit changes nothing — give a name, a query, or shared")]
    NeedsViewEdit,
}

impl Error {
    /// The stable id, tower's `category/kebab-case`.
    pub fn id(&self) -> &'static str {
        match self {
            Error::Resolve(err) => err.id(),
            Error::Log(err) => err.id(),
            Error::Procedure(err) => err.id(),
            Error::Query(err) => err.id(),
            Error::EmptySubject => "usage/empty-subject",
            Error::NoParts => "usage/no-parts",
            Error::EmptyProcedure => "usage/empty-procedure",
            Error::NeedsQuestion | Error::NeedsAnswer | Error::NeedsNote => "usage/needs-message",
            Error::BadStatus { .. } => "usage/bad-status",
            Error::StatusWaiting { .. } => "usage/status-waiting",
            Error::StatusHold { .. } => "usage/status-held",
            Error::BadAssignee { .. } => "usage/bad-assignee",
            Error::FlightDone { .. } | Error::AlreadyDone { .. } => "flight/done",
            Error::StatusHeld { .. } => "status/held",
            Error::AlreadyHeld { .. } => "hold/exists",
            Error::NotHeld { .. } => "answer/not-held",
            Error::EmptyName => "usage/empty-name",
            Error::BadView { .. } => "usage/bad-view",
            Error::ViewNotFound { .. } => "view/not-found",
            Error::NeedsViewEdit => "usage/needs-edit",
        }
    }

    /// Commands that lead out of it. Empty where no command helps — the
    /// envelope carries `[]`, never null.
    pub fn exits(&self) -> Vec<String> {
        let exits: &[&str] = match self {
            Error::Resolve(err) => return err.exits(),
            Error::Log(err) => return err.exits(),
            Error::Procedure(err) => return err.exits(),
            Error::Query(err) => return err.exits(),
            Error::EmptySubject | Error::NoParts | Error::EmptyName | Error::BadView { .. } => &[],
            Error::EmptyProcedure => &["ff tower procedures"],
            Error::NeedsQuestion | Error::StatusHold { .. } => {
                &["ff tower hold <flight> -m <question>"]
            }
            Error::StatusWaiting { .. } => &["ff tower link <flight> <dependency>"],
            Error::NeedsAnswer | Error::AlreadyHeld { .. } | Error::StatusHeld { .. } => {
                &["ff tower answer <flight> -m <answer>"]
            }
            Error::NeedsNote => &["ff tower comment <flight> -m <note>"],
            Error::FlightDone { .. }
            | Error::AlreadyDone { .. }
            | Error::NotHeld { .. }
            | Error::ViewNotFound { .. }
            | Error::NeedsViewEdit => &["ff tower"],
            Error::BadStatus { .. } | Error::BadAssignee { .. } => &[],
        };
        exits.iter().map(|exit| (*exit).to_string()).collect()
    }
}

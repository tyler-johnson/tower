//! The saved view: a name, a query as [`Query::render`](super::Query::render)
//! spells it, and whether it is shared.
//!
//! A view is a `view_saved` event on the log, so it is persisted,
//! carried by the repository, and readable without tower. The fold
//! minted it beside the flights — a save naming an earlier view
//! replaces its three fields wholesale, last-wins, and a `view_deleted`
//! is final. The owner is the minting event's author, the git
//! `user.email` the store stamps, and the viewer is the process's own
//! author: there is no request-scoped viewer anywhere.
//!
//! Personal is a rendering rule, not a privacy one. The log is shared by
//! construction, so a personal view's event is readable by anyone with
//! the repository; it renders for its author and not for others, and
//! that is the whole of what the word promises.

use serde::Serialize;

use super::Fold;
use crate::log::EventId;

/// One saved view, as the fold holds it after every save and delete.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    /// The minting event's id — the view's identity, the reference every
    /// later save and delete names.
    pub id: EventId,
    pub name: String,
    /// The rendered query — canonical, since the verb stores
    /// `Query::render` of what it parsed.
    pub query: String,
    pub shared: bool,
    /// The owner: the minting event's author.
    pub author: String,
    /// The last save's author.
    pub saved_by: String,
    /// The last save's time.
    pub saved_at: i64,
}

impl View {
    /// Whether `viewer` sees this view: shared, or their own.
    pub fn visible_to(&self, viewer: &str) -> bool {
        self.shared || self.author == viewer
    }
}

/// The views `viewer` sees, in minting order.
pub fn views(fold: &Fold, viewer: &str) -> Vec<View> {
    fold.views
        .iter()
        .filter(|view| view.visible_to(viewer))
        .cloned()
        .collect()
}

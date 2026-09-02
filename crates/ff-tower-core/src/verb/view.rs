//! `view list`, `view save`, `view edit`, `view delete` — the saved
//! views, as the log holds them.
//!
//! No CLI verb family yet: serve mounts these as `/api/views` and the
//! routes' `cmd` names are the wire names above. A view is three fields
//! — a name, a query, personal or shared — and the log holds one
//! spelling of the query: the verb parses what it is handed and stores
//! [`Query::render`] of it, so the first place a query can be spelled
//! wrong is also the last.
//!
//! The viewer is the store's author, git `user.email`: there is no
//! request-scoped viewer anywhere in serve, so the process's identity is
//! who sees what. A reference resolves against the visible set, which
//! is why another author's personal view is `view/not-found` and never a
//! permission refusal — personal is a rendering rule, not a privacy one.

use serde::Serialize;

use crate::board::{self, Fold, Query, View};
use crate::log::{Event, EventId, Kind, Store};

use super::{Error, appended};

/// The envelope's `data` for `view list`: what this viewer sees.
#[derive(Serialize)]
pub struct Views {
    pub views: Vec<View>,
}

pub fn views(store: &Store) -> Result<Views, Error> {
    let fold = board::fold(&store.read_all()?);
    Ok(Views {
        views: board::views(&fold, store.author()),
    })
}

/// The envelope's `data` for `view save` and `view edit`: the view as
/// the fold holds it after the append — store-assigned time included.
#[derive(Serialize)]
pub struct Saved {
    pub view: View,
}

/// The outcome of a save or an edit.
pub struct Save {
    pub payload: Saved,
}

/// `view save` — mint a view. The name is trimmed and refused empty; the
/// query is parsed and stored canonical.
pub fn save(store: &Store, name: &str, query: &str, shared: bool) -> Result<Save, Error> {
    let (name, query) = validate(name, query)?;
    let ids = store.append(vec![Kind::ViewSaved {
        view: None,
        name,
        query,
        shared,
    }])?;
    let id = ids.into_iter().next().expect("one view_saved event");
    folded(store, &id)
}

/// `view edit` — replace a view wholesale, the fields left `None` filled
/// from the standing view. All three `None` is refused: the event would
/// change nothing.
pub fn edit(
    store: &Store,
    view: &str,
    name: Option<String>,
    query: Option<String>,
    shared: Option<bool>,
) -> Result<Save, Error> {
    if name.is_none() && query.is_none() && shared.is_none() {
        return Err(Error::NeedsViewEdit);
    }
    let fold = board::fold(&store.read_all()?);
    let standing = resolve(&fold, store.author(), view)?;
    let name = name.unwrap_or_else(|| standing.name.clone());
    let query = query.unwrap_or_else(|| standing.query.clone());
    let (name, query) = validate(&name, &query)?;
    let ids = store.append(vec![Kind::ViewSaved {
        view: Some(standing.id.clone()),
        name,
        query,
        shared: shared.unwrap_or(standing.shared),
    }])?;
    ids.into_iter().next().expect("one view_saved event");
    folded(store, &standing.id)
}

/// The envelope's `data` for `view delete`: the delete, as the log holds
/// it.
#[derive(Serialize)]
pub struct Deleted {
    pub deleted: Event,
}

/// The outcome: the payload, plus the name a human echo needs.
pub struct Delete {
    pub payload: Deleted,
    pub name: String,
}

/// `view delete` — remove a view. Final: the fold folds a later save
/// naming it as nothing.
pub fn delete(store: &Store, view: &str) -> Result<Delete, Error> {
    let fold = board::fold(&store.read_all()?);
    let standing = resolve(&fold, store.author(), view)?;
    let ids = store.append(vec![Kind::ViewDeleted {
        view: standing.id.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one view_deleted event");
    Ok(Delete {
        payload: Deleted {
            deleted: appended(store, &id)?,
        },
        name: standing.name,
    })
}

/// The name trimmed and non-empty, the query parsed and rendered back
/// — the one spelling the log holds.
fn validate(name: &str, query: &str) -> Result<(String, String), Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::EmptyName);
    }
    let query = Query::parse(query)?.render();
    Ok((name.to_string(), query))
}

/// A view reference — the full wire id — against what `viewer` sees.
fn resolve(fold: &Fold, viewer: &str, text: &str) -> Result<View, Error> {
    let id: EventId = text.parse().map_err(|_| Error::BadView {
        text: text.to_string(),
    })?;
    board::views(fold, viewer)
        .into_iter()
        .find(|view| view.id == id)
        .ok_or_else(|| Error::ViewNotFound {
            text: text.to_string(),
        })
}

/// Re-fold and return the view by id, so the payload is what the log
/// holds — the store-assigned time included — and not a reconstruction.
fn folded(store: &Store, id: &EventId) -> Result<Save, Error> {
    let fold = board::fold(&store.read_all()?);
    let view = fold
        .views
        .into_iter()
        .find(|view| &view.id == id)
        .expect("the saved view is on the chain");
    Ok(Save {
        payload: Saved { view },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff_tower_testsupport::Repo;

    fn store() -> (Repo, Store) {
        let repo = Repo::new();
        repo.pin_writer("pi");
        let store = Store::open(repo.path()).expect("open");
        (repo, store)
    }

    fn pinned(err: &Error, id: &str, message: &str, exits: &[&str]) {
        assert_eq!(err.id(), id);
        assert_eq!(err.to_string(), message);
        assert_eq!(err.exits(), exits);
    }

    #[test]
    fn a_save_stores_the_canonical_query_and_the_trimmed_name() {
        let (_repo, store) = store();
        let saved = save(&store, "  mine ", "?status=ready&closed=3", false).expect("save");
        let view = saved.payload.view;
        assert_eq!(view.id.to_string(), "pi.1");
        assert_eq!(view.name, "mine");
        assert_eq!(
            view.query, "status=ready",
            "the `?` and the default window are not spelled"
        );
        assert!(!view.shared);
        assert_eq!(view.author, "tests@tower.invalid");
        assert_eq!(view.saved_by, "tests@tower.invalid");
        let listed = views(&store).expect("list").views;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].query, "status=ready");
    }

    #[test]
    fn an_empty_name_or_a_bad_query_refuses_before_the_store() {
        let (_repo, store) = store();
        pinned(
            &save(&store, "  ", "", false).err().expect("empty name"),
            "usage/empty-name",
            "the view name is empty",
            &[],
        );
        let err = save(&store, "mine", "bogus=1", false)
            .err()
            .expect("not a field");
        assert_eq!(err.id(), "usage/unknown-field");
        assert_eq!(err.exits(), ["ff tower"]);
        assert!(views(&store).expect("list").views.is_empty());
    }

    #[test]
    fn an_edit_keeps_the_fields_left_unsaid() {
        let (_repo, store) = store();
        let minted = save(&store, "mine", "assignee=me", true).expect("save");
        let id = minted.payload.view.id.to_string();
        let edited = edit(&store, &id, Some("renamed".to_string()), None, None).expect("edit");
        let view = edited.payload.view;
        assert_eq!(view.id.to_string(), id);
        assert_eq!(view.name, "renamed");
        assert_eq!(view.query, "assignee=me");
        assert!(view.shared);
        let edited = edit(&store, &id, None, None, Some(false)).expect("edit");
        assert_eq!(edited.payload.view.name, "renamed");
        assert!(!edited.payload.view.shared);
    }

    #[test]
    fn an_edit_with_nothing_to_change_refuses() {
        let (_repo, store) = store();
        let minted = save(&store, "mine", "", false).expect("save");
        let id = minted.payload.view.id.to_string();
        pinned(
            &edit(&store, &id, None, None, None).err().expect("nothing"),
            "usage/needs-edit",
            "the edit changes nothing — give a name, a query, or shared",
            &["ff tower"],
        );
    }

    #[test]
    fn a_bad_reference_and_an_unknown_view_refuse_apart() {
        let (_repo, store) = store();
        pinned(
            &delete(&store, "1").err().expect("not an id"),
            "usage/bad-view",
            "`1` is not a view id — `<writer>.<seq>`",
            &[],
        );
        pinned(
            &delete(&store, "pi.9").err().expect("no such view"),
            "view/not-found",
            "no view `pi.9` is visible to you",
            &["ff tower"],
        );
    }

    #[test]
    fn a_delete_empties_the_list() {
        let (_repo, store) = store();
        let minted = save(&store, "mine", "", false).expect("save");
        let id = minted.payload.view.id.to_string();
        let gone = delete(&store, &id).expect("delete");
        assert_eq!(gone.name, "mine");
        assert!(matches!(
            gone.payload.deleted.kind,
            Kind::ViewDeleted { .. }
        ));
        assert!(views(&store).expect("list").views.is_empty());
        pinned(
            &delete(&store, &id).err().expect("gone"),
            "view/not-found",
            "no view `pi.1` is visible to you",
            &["ff tower"],
        );
    }

    #[test]
    fn another_author_sees_the_shared_view_and_not_the_personal_one() {
        let (repo, store) = store();
        let personal = save(&store, "mine", "assignee=me", false)
            .expect("save")
            .payload
            .view
            .id
            .to_string();
        let shared = save(&store, "ours", "status=ready", true)
            .expect("save")
            .payload
            .view
            .id
            .to_string();

        repo.git(&["config", "user.email", "other@tower.invalid"]);
        let other = Store::open(repo.path()).expect("open");
        let seen: Vec<String> = views(&other)
            .expect("list")
            .views
            .iter()
            .map(|view| view.id.to_string())
            .collect();
        assert_eq!(seen, std::slice::from_ref(&shared));
        pinned(
            &edit(&other, &personal, Some("theirs".to_string()), None, None)
                .err()
                .expect("not visible"),
            "view/not-found",
            &format!("no view `{personal}` is visible to you"),
            &["ff tower"],
        );
        let edited = edit(&other, &shared, Some("everyone's".to_string()), None, None)
            .expect("a shared view takes anyone's edit");
        assert_eq!(edited.payload.view.name, "everyone's");
        assert_eq!(edited.payload.view.author, "tests@tower.invalid");
        assert_eq!(edited.payload.view.saved_by, "other@tower.invalid");
    }
}

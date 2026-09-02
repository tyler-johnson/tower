//! `link <a> <b>` — declare that `a` depends on `b`; `unlink <a> <b>`
//! — take the edge back.
//!
//! Stored intent, one edge per event. The identical edge declared twice
//! is refused: the fold would render it twice, and nothing in the log
//! means it twice. With Waiting derived from the edges, `unlink` is the
//! only way to disagree with one — the fold drops the edge and the
//! dependent derives Ready at the next render with no further event.
//! An edge that is not on the record is refused: there is nothing to
//! take back. No `ensure_active` on either: a closed flight's edges are
//! still its record.

use serde::Serialize;

use crate::board::{self, display};
use crate::log::{Event, Kind, Store};

use super::{Error, appended};

/// The envelope's `data`: the edge, as the log holds it.
#[derive(Serialize)]
pub struct Linked {
    pub linked: Event,
}

/// The outcome: the payload, plus the two displays the echo names.
pub struct Link {
    pub payload: Linked,
    pub from: String,
    pub to: String,
}

pub fn link(store: &Store, a: &str, b: &str) -> Result<Link, Error> {
    board::parse_ref(a)?;
    board::parse_ref(b)?;
    let fold = board::fold(&store.read_all()?);
    let from = board::resolve(&fold, a)?;
    let to = board::resolve(&fold, b)?;
    // Self-link fires on the resolved ids — `link 3 pi.3` naming one
    // flight twice is a self-link only resolution can see.
    if from == to {
        return Err(Error::SelfLink {
            display: display(&fold, &from),
        });
    }
    if board::flight(&fold, &from).depends_on.contains(&to) {
        return Err(Error::LinkExists {
            from: display(&fold, &from),
            to: display(&fold, &to),
        });
    }

    let ids = store.append(vec![Kind::Linked {
        from: from.clone(),
        to: to.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one linked event");

    Ok(Link {
        payload: Linked {
            linked: appended(store, &id)?,
        },
        from: display(&fold, &from),
        to: display(&fold, &to),
    })
}

/// The envelope's `data`: the retraction, as the log holds it.
#[derive(Serialize)]
pub struct Unlinked {
    pub unlinked: Event,
}

/// The outcome: the payload, plus the two displays the echo names.
pub struct Unlink {
    pub payload: Unlinked,
    pub from: String,
    pub to: String,
}

pub fn unlink(store: &Store, a: &str, b: &str) -> Result<Unlink, Error> {
    board::parse_ref(a)?;
    board::parse_ref(b)?;
    let fold = board::fold(&store.read_all()?);
    let from = board::resolve(&fold, a)?;
    let to = board::resolve(&fold, b)?;
    // No self-link check: a self-edge can never be on the record, so
    // it falls to the one refusal.
    if !board::flight(&fold, &from).depends_on.contains(&to) {
        return Err(Error::LinkMissing {
            from: display(&fold, &from),
            to: display(&fold, &to),
        });
    }

    let ids = store.append(vec![Kind::Unlinked {
        from: from.clone(),
        to: to.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one unlinked event");

    Ok(Unlink {
        payload: Unlinked {
            unlinked: appended(store, &id)?,
        },
        from: display(&fold, &from),
        to: display(&fold, &to),
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

    fn filed(store: &Store, subject: &str) {
        store
            .append(vec![Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: String::new(),
                status: "triage".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            }])
            .expect("append");
    }

    fn pinned(err: &Error, id: &str, message: &str, exits: &[&str]) {
        assert_eq!(err.id(), id);
        assert_eq!(err.to_string(), message);
        assert_eq!(err.exits(), exits);
    }

    #[test]
    fn a_self_link_refuses_on_the_resolved_ids() {
        let (_repo, store) = store();
        filed(&store, "alone");
        pinned(
            &link(&store, "1", "pi.1").err().expect("a self-link"),
            "usage/self-link",
            "`#1` cannot depend on itself",
            &[],
        );
    }

    #[test]
    fn the_same_edge_twice_refuses_with_the_brief_exit() {
        let (_repo, store) = store();
        filed(&store, "the dependency");
        filed(&store, "the dependent");
        link(&store, "2", "1").expect("the edge lands");
        pinned(
            &link(&store, "2", "1").err().expect("declared already"),
            "link/exists",
            "`#2` already depends on `#1`",
            &["ff tower brief <flight>"],
        );
    }

    #[test]
    fn an_unlink_of_no_edge_refuses_and_a_self_edge_falls_there_too() {
        let (_repo, store) = store();
        filed(&store, "the dependency");
        filed(&store, "the dependent");
        pinned(
            &unlink(&store, "2", "1").err().expect("no edge"),
            "link/missing",
            "`#2` does not depend on `#1`",
            &["ff tower brief <flight>"],
        );
        assert_eq!(
            unlink(&store, "1", "1")
                .err()
                .expect("never on the record")
                .id(),
            "link/missing"
        );
    }

    #[test]
    fn a_link_then_unlink_leaves_no_edge() {
        let (_repo, store) = store();
        filed(&store, "the dependency");
        filed(&store, "the dependent");
        let linked = link(&store, "2", "1").expect("the edge lands");
        assert_eq!((linked.from.as_str(), linked.to.as_str()), ("#2", "#1"));
        let fold = board::fold(&store.read_all().expect("read"));
        assert_eq!(fold.flights[1].depends_on.len(), 1);
        let unlinked = unlink(&store, "2", "1").expect("the edge goes");
        assert!(matches!(
            unlinked.payload.unlinked.kind,
            Kind::Unlinked { .. }
        ));
        let fold = board::fold(&store.read_all().expect("read"));
        assert!(fold.flights[1].depends_on.is_empty());
    }
}

//! `decompose <flight> [<procedure> | <part>…]` — make a flight a
//! parent.
//!
//! Two forms, told apart by the arguments: exactly one argument that
//! names an installed procedure mints the definition's flights beneath
//! the parent; anything else is the by-hand form, one subject per
//! argument. Either way the parts are born cleared — the person
//! decomposing is the person clearing them, and a bare `file` alone
//! lands in Triage — and the fold derives Waiting for any part whose
//! edges say so. Either way the children ride ordinary
//! `linked` edges, so a sub-flight is indistinguishable from a
//! hand-declared dependency, and the filings and the edges land in one
//! `append_with` — two appends would leave a window where the parent is
//! live, unlinked, and pullable.

use serde::Serialize;

use crate::board;
use crate::log::{Event, EventId, Kind, Store};
use crate::procedure;

use super::{Error, Fields, Parent, appended_all, classify, ensure_active};

/// The envelope's `data`. Struct fields serialize in declaration order,
/// and this order — `filed, linked, parent` — is the alphabetical one
/// the CLI's `json!` emitted before the payload moved here, so the bytes
/// on the wire never changed.
#[derive(Serialize)]
pub struct Decomposed {
    pub filed: Vec<Event>,
    pub linked: Vec<Event>,
    pub parent: String,
}

/// The outcome: the payload, plus the ids a human render echoes — it
/// re-folds for the display numbers, so the machine path never has to.
pub struct Decompose {
    pub payload: Decomposed,
    pub parent: EventId,
    pub filed_ids: Vec<EventId>,
}

pub fn decompose(store: &Store, flight: &str, parts: &[String]) -> Result<Decompose, Error> {
    board::parse_ref(flight)?;
    if parts.is_empty() {
        return Err(Error::NoParts);
    }
    let subjects: Vec<String> = parts.iter().map(|part| part.trim().to_string()).collect();
    if subjects.iter().any(String::is_empty) {
        return Err(Error::EmptySubject);
    }

    let fold = board::fold(&store.read_all()?);
    let parent = board::resolve(&fold, flight)?;
    let parent_row = ensure_active(&fold, &parent)?;
    let procedure = parent_row.procedure.clone();
    let subject = parent_row.subject.clone();

    // The procedure form: one argument, and it names an installed
    // definition. The lookup is soft where `file`'s is `require` — a
    // subject that happens to collide with a procedure name must fall
    // through to the by-hand form, spelled around by decomposing with
    // two arguments or a different name, which is what the help
    // documents.
    let installed = procedure::registry(store.main_worktree().as_deref())?;
    if let [name] = subjects.as_slice()
        && let Some(definition) = installed.get(name)
    {
        let ids = store.append_with(|mint| {
            classify(
                definition,
                &subject,
                &Fields::default(),
                Parent::Existing(parent.clone()),
                mint,
            )
        })?;
        // `Parent::Existing` mints no parent event, so the filings are
        // the whole head of the batch and every edge — parent to child,
        // and the definition's own `after` — is the tail.
        let (filed, linked) = ids.split_at(definition.flights.len());
        return outcome(store, parent, filed, linked);
    }

    // The by-hand form: the filings first, then one edge per sub-flight
    // naming the id its filing is about to take — which is why this is
    // `append_with` and not a batch built ahead of it.
    let ids = store.append_with(|mint| {
        let mut kinds: Vec<Kind> = subjects
            .iter()
            .map(|subject| Kind::Filed {
                // Provenance follows the parent; the fields are a bare
                // filing's — no lane, defaults — born cleared.
                procedure: procedure.clone(),
                subject: subject.clone(),
                body: String::new(),
                status: "ready".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            })
            .collect();
        kinds.extend((0..subjects.len()).map(|offset| Kind::Linked {
            from: parent.clone(),
            to: mint(offset),
        }));
        kinds
    })?;
    let (filed, linked) = ids.split_at(subjects.len());
    outcome(store, parent, filed, linked)
}

/// The shared tail: the appended events read back off the chain, and the
/// echo facts beside them.
fn outcome(
    store: &Store,
    parent: EventId,
    filed: &[EventId],
    linked: &[EventId],
) -> Result<Decompose, Error> {
    Ok(Decompose {
        payload: Decomposed {
            filed: appended_all(store, filed)?,
            linked: appended_all(store, linked)?,
            parent: parent.to_string(),
        },
        parent,
        filed_ids: filed.to_vec(),
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

    /// The engine ships empty, so the procedure form needs a definition
    /// installed first — the repository layer, `docs/procedures/`'s
    /// review shape.
    fn install_review(repo: &Repo) {
        repo.write(
            ".tower/procedures/review.toml",
            r#"
name    = "review"
subject = "branch"

[[flight]]
id       = "pass"
assignee = "agent"
skill    = "review"

[[flight]]
id       = "smoke"
assignee = "me"
bay      = "warm"

[[flight]]
id       = "verdict"
assignee = "me"
after    = ["pass", "smoke"]
"#,
        );
    }

    fn filed(store: &Store, subject: &str) {
        store
            .append(vec![Kind::Filed {
                procedure: Some("chore".to_string()),
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

    fn folded(store: &Store) -> board::Fold {
        board::fold(&store.read_all().expect("read"))
    }

    #[test]
    fn the_by_hand_form_files_bare_children_on_the_parents_edges() {
        let (_repo, store) = store();
        filed(&store, "a broad task");
        let outcome = decompose(
            &store,
            "1",
            &["part one".to_string(), "  part two  ".to_string()],
        )
        .expect("decomposes");
        assert_eq!(outcome.payload.parent, "pi.1");
        assert_eq!(outcome.filed_ids.len(), 2);
        assert_eq!(outcome.payload.filed.len(), 2);
        assert_eq!(outcome.payload.linked.len(), 2, "one edge per child");

        let fold = folded(&store);
        let child = |id: &EventId| {
            fold.flights
                .iter()
                .find(|flight| &flight.id == id)
                .expect("minted")
        };
        let first = child(&outcome.filed_ids[0]);
        assert_eq!(first.subject, "part one", "the subject is trimmed");
        assert_eq!(child(&outcome.filed_ids[1]).subject, "part two");
        assert_eq!(first.status, "ready", "a part is born cleared");
        assert!(first.assignee.is_none());
        assert_eq!(
            first.procedure.as_deref(),
            Some("chore"),
            "provenance follows the parent"
        );
        let parent = child(&outcome.parent);
        assert_eq!(parent.depends_on.len(), 2);
        assert_eq!(parent.status, "triage", "the mint never re-stamps");
    }

    #[test]
    fn the_procedure_form_mints_the_definitions_flights_beneath_it() {
        let (repo, store) = store();
        install_review(&repo);
        filed(&store, "look this over");
        let outcome =
            decompose(&store, "1", &["review".to_string()]).expect("the definition is installed");
        assert_eq!(outcome.filed_ids.len(), 3);
        assert_eq!(
            outcome.payload.linked.len(),
            5,
            "three parent edges and the definition's two"
        );

        let fold = folded(&store);
        let by_subject = |tail: &str| {
            fold.flights
                .iter()
                .find(|flight| flight.subject.ends_with(tail))
                .expect("minted")
        };
        let pass = by_subject("· pass");
        assert_eq!(pass.status, "ready", "no after — born Ready");
        assert_eq!(pass.assignee.as_deref(), Some("agent"));
        assert_eq!(by_subject("· verdict").status, "waiting");
    }

    #[test]
    fn a_name_that_is_not_installed_falls_through_to_one_sub_flight() {
        let (_repo, store) = store();
        filed(&store, "a broad task");
        let outcome = decompose(&store, "1", &["review the docs".to_string()])
            .expect("the by-hand form takes it");
        assert_eq!(outcome.filed_ids.len(), 1);
        let fold = folded(&store);
        let child = fold
            .flights
            .iter()
            .find(|flight| flight.id == outcome.filed_ids[0])
            .expect("minted");
        assert_eq!(child.subject, "review the docs");
        assert_eq!(child.status, "ready");
    }

    #[test]
    fn nothing_to_split_into_and_a_blank_subject_refuse_before_the_append() {
        let (_repo, store) = store();
        filed(&store, "a broad task");
        let err = decompose(&store, "1", &[]).err().expect("no parts");
        assert_eq!(err.id(), "usage/no-parts");
        assert_eq!(err.to_string(), "there is nothing to split into");
        assert!(err.exits().is_empty());

        let err = decompose(&store, "1", &["part one".to_string(), "  ".to_string()])
            .err()
            .expect("a blank subject");
        assert_eq!(err.id(), "usage/empty-subject");
        assert_eq!(
            folded(&store).flights.len(),
            1,
            "the refusals land before the append"
        );
    }
}

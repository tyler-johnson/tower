//! `file [<procedure>] <subject> [flags]` — put work on the board.
//!
//! A bare filing carries no procedure and mints one flight in **Triage**
//! — nothing clears work to Ready but a procedure or a person's own
//! `status` move. Every stored field is a flag: `-m` the body, priority,
//! labels, skill, assignee, bay, copied onto the filing as given.
//!
//! Under a procedure, the named definition is looked up in the registry,
//! refused when it is not installed, and its flights are minted with it.
//! The definition is read here and never again — each flight's fields
//! are copied into the log, so editing a definition afterwards cannot
//! disturb a flight already in the air. Statuses fall out of the edges
//! at mint: no `after` is born Ready, dependencies are born Waiting, and
//! the parent waits on them all. **One flight** collapses onto the
//! filing itself — born Ready, the definition's fields under the
//! caller's flags — because `ff tower file "fix the typo"` must not cost
//! two flights to say one thing. **Two or more** file a parent plus one
//! flight each, on the same `linked` edges `decompose` writes. All of it
//! in one `append_with`: two appends would leave a window where the
//! parent is live, unlinked, and pullable.
//!
//! Still no fufu spawn. The registry's repository layer resolves through
//! `Store::main_worktree`, which reads the common dir and runs nothing.

use serde::Serialize;

use crate::log::{Event, EventId, Kind, Store};
use crate::model::Assignee;
use crate::procedure;

use super::{Error, Fields, Parent, appended, appended_all, classify};

/// The envelope's `data`. Struct fields serialize in declaration order,
/// and this order — `filed, linked, parts` — is the alphabetical one the
/// CLI's `json!` emitted before the payload moved here, so the bytes on
/// the wire never changed.
#[derive(Serialize)]
pub struct Filed {
    pub filed: Event,
    pub linked: Vec<Event>,
    pub parts: Vec<Event>,
}

/// The outcome: the payload, plus the ids a human render echoes — it
/// re-folds for the display numbers, so the machine path never has to.
pub struct File {
    pub payload: Filed,
    pub parent: EventId,
    pub part_ids: Vec<EventId>,
}

pub fn file(
    store: &Store,
    subject: &str,
    fields: Fields,
    procedure: Option<&str>,
) -> Result<File, Error> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(Error::EmptySubject);
    }
    let fields = Fields {
        assignee: lane(fields.assignee)?,
        ..fields
    };

    let Some(name) = procedure else {
        // The bare filing: no procedure, one flight, Triage.
        let ids = store.append(vec![Kind::Filed {
            procedure: None,
            subject: subject.to_string(),
            body: fields.message.clone().unwrap_or_default(),
            status: "triage".to_string(),
            assignee: fields.assignee.clone(),
            priority: fields
                .priority
                .clone()
                .unwrap_or_else(|| "none".to_string()),
            labels: fields.labels.clone(),
            skill: fields.skill.clone(),
            bay: fields.bay.clone(),
            done: "asserted".to_string(),
            branch: None,
        }])?;
        let id = ids.into_iter().next().expect("one filed event");
        return Ok(File {
            payload: Filed {
                filed: appended(store, &id)?,
                linked: Vec::new(),
                parts: Vec::new(),
            },
            parent: id,
            part_ids: Vec::new(),
        });
    };

    let name = name.trim();
    if name.is_empty() {
        return Err(Error::EmptyProcedure);
    }
    let installed = procedure::registry(store.main_worktree().as_deref())?;
    let definition = installed.require(name)?;

    let ids =
        store.append_with(|mint| classify(definition, subject, &fields, Parent::Mint, mint))?;
    let (parent, rest) = ids.split_first().expect("the parent is the first event");
    let parts = if definition.flights.len() == 1 {
        0
    } else {
        definition.flights.len()
    };
    let (filed, linked) = rest.split_at(parts);

    Ok(File {
        payload: Filed {
            filed: appended(store, parent)?,
            linked: appended_all(store, linked)?,
            parts: appended_all(store, filed)?,
        },
        parent: parent.clone(),
        part_ids: filed.to_vec(),
    })
}

/// The caller's `--assignee`, validated: a lane name passes through,
/// `none` is the absent lane spelled out, and anything else refuses —
/// the closed vocabulary lives here, not on the wire.
fn lane(assignee: Option<String>) -> Result<Option<String>, Error> {
    match assignee.as_deref() {
        None | Some("none") => Ok(None),
        Some(word) => match Assignee::parse(word) {
            Some(lane) => Ok(Some(lane.name().to_string())),
            None => Err(Error::BadAssignee {
                word: word.to_string(),
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use ff_tower_testsupport::Repo;

    fn store() -> (Repo, Store) {
        let repo = Repo::new();
        repo.pin_writer("pi");
        let store = Store::open(repo.path()).expect("open");
        (repo, store)
    }

    /// The engine ships empty, so a filing under a procedure needs one
    /// installed first: the repository layer, which beats whatever the
    /// machine's own user layer happens to hold.
    fn install(repo: &Repo, name: &str, text: &str) {
        repo.write(format!(".tower/procedures/{name}.toml"), text);
    }

    /// `docs/procedures/review.toml`'s shape: `pass` and `smoke` fly
    /// together, and `verdict` waits on both.
    const REVIEW: &str = r#"
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
"#;

    /// `docs/procedures/ticket.toml`'s: one flight, the collapse case.
    const TICKET: &str = r#"
name = "ticket"

[[flight]]
id       = "work"
assignee = "me"
done     = "asserted"
"#;

    fn folded(store: &Store) -> board::Fold {
        board::fold(&store.read_all().expect("read"))
    }

    #[test]
    fn a_bare_filing_lands_in_triage_with_its_flags() {
        let (_repo, store) = store();
        let outcome = file(
            &store,
            "fix the login redirect",
            Fields {
                message: Some("the redirect loops".to_string()),
                priority: Some("high".to_string()),
                labels: vec!["web".to_string()],
                skill: Some("debug".to_string()),
                assignee: Some("agent".to_string()),
                bay: Some("warm".to_string()),
            },
            None,
        )
        .expect("files");
        assert!(outcome.part_ids.is_empty());
        assert!(outcome.payload.linked.is_empty());

        let fold = folded(&store);
        let flight = &fold.flights[0];
        assert!(flight.procedure.is_none(), "bare carries no procedure");
        assert_eq!(flight.status, "triage", "only a procedure clears work");
        assert_eq!(flight.assignee.as_deref(), Some("agent"));
        assert_eq!(flight.priority, "high");
        assert_eq!(flight.labels, ["web"]);
        assert_eq!(flight.skill.as_deref(), Some("debug"));
        assert_eq!(flight.bay.as_deref(), Some("warm"));
        assert_eq!(flight.body, "the redirect loops");
        assert!(
            !flight.pullable(),
            "agent-laned but Triage — the lane alone clears nothing"
        );
    }

    #[test]
    fn a_multi_flight_procedure_mints_ready_waiting_and_a_waiting_parent() {
        let (repo, store) = store();
        install(&repo, "review", REVIEW);
        let outcome = file(
            &store,
            "feather",
            Fields {
                priority: Some("urgent".to_string()),
                ..Fields::default()
            },
            Some("review"),
        )
        .expect("files");
        assert_eq!(outcome.part_ids.len(), 3);

        let fold = folded(&store);
        let by_subject = |tail: &str| {
            fold.flights
                .iter()
                .find(|flight| flight.subject.ends_with(tail))
                .expect("minted")
        };
        let parent = fold
            .flights
            .iter()
            .find(|flight| flight.id == outcome.parent)
            .expect("the parent is filed");
        assert_eq!(parent.status, "waiting", "the parent waits on them all");
        assert_eq!(parent.priority, "urgent", "caller flags land on the parent");
        assert_eq!(parent.depends_on.len(), 3);

        let pass = by_subject("· pass");
        assert_eq!(pass.status, "ready", "no after — born Ready");
        assert_eq!(pass.assignee.as_deref(), Some("agent"));
        assert_eq!(pass.skill.as_deref(), Some("review"));
        assert_eq!(
            pass.branch_stamp.as_deref(),
            Some("feather"),
            "the subject rule resolves once, at file time"
        );
        assert!(pass.pullable());

        let smoke = by_subject("· smoke");
        assert_eq!(smoke.status, "ready");
        assert_eq!(smoke.assignee.as_deref(), Some("me"));
        assert_eq!(smoke.bay.as_deref(), Some("warm"));

        let verdict = by_subject("· verdict");
        assert_eq!(verdict.status, "waiting", "dependencies — born Waiting");
        assert_eq!(verdict.depends_on.len(), 2);
    }

    #[test]
    fn a_single_flight_procedure_collapses_ready_and_the_caller_wins() {
        let (repo, store) = store();
        install(&repo, "ticket", TICKET);
        let outcome = file(
            &store,
            "fix the typo",
            Fields {
                assignee: Some("agent".to_string()),
                labels: vec!["chore".to_string()],
                ..Fields::default()
            },
            Some("ticket"),
        )
        .expect("files");
        assert!(outcome.part_ids.is_empty(), "one flight, no parent");

        let fold = folded(&store);
        let flight = &fold.flights[0];
        assert_eq!(flight.procedure.as_deref(), Some("ticket"));
        assert_eq!(flight.subject, "fix the typo");
        assert_eq!(flight.status, "ready", "the collapse is born Ready");
        assert_eq!(
            flight.assignee.as_deref(),
            Some("agent"),
            "the caller's lane beats the definition's"
        );
        assert_eq!(flight.labels, ["chore"]);
    }

    #[test]
    fn a_definitions_done_word_rides_onto_the_filed_event() {
        // The enum is closed in the loader and open in the log: what
        // `done` parsed to is copied onto the filing as the free string
        // the log wants, so a non-default value has to survive the mint.
        let (repo, store) = store();
        install(
            &repo,
            "landing",
            r#"
name = "landing"

[[flight]]
id       = "ship"
assignee = "me"
done     = "landed"
"#,
        );
        file(&store, "the release", Fields::default(), Some("landing")).expect("files");

        let fold = folded(&store);
        assert_eq!(fold.flights[0].done_kind, "landed");
    }
}

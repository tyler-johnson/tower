//! `file [<procedure>] <subject> [flags]` — put work on the board.
//!
//! A bare filing carries no procedure and mints one flight where
//! `tower.defaultFileStatus` says — **Ready** unless the setting says
//! otherwise, because most filings are work already decided on. `backlog`
//! parks it for a person instead. `--status` overrides the setting for
//! one filing; the words it takes are the ones a flight can be filed
//! with — backlog, ready, in_progress — never the derived or closed ones.
//! Every stored field is a flag: `-m` the body, priority, labels, skill,
//! assignee, bay, copied onto the filing as given.
//!
//! **A bare filing is matched once, here.** The registry loads, and the
//! first rule whose predicates all hold against the caller's fields —
//! registry name order, declaration order within a definition — chooses
//! its procedure; a `status` predicate matches the word the filing lands
//! with, flag or setting. The filing is minted exactly as naming that
//! procedure would mint it, and one `routed` event in the same batch
//! records which rule chose it and why. A rule keyed on `source`/`event`
//! is inert, because no adapter exists to carry provenance. The match
//! runs on the filing machine against the procedures it has, so a pushed
//! log never gets a teammate's flight restamped under rules only this
//! machine holds; moving a flight to Backlog later, or editing a label
//! on, never re-matches — `file <procedure>` by name, or `decompose`, is
//! how a flight gets a shape later. A routed filing lands where the named
//! filing would: Ready under the default setting, parked under `backlog`,
//! because the rule decides the shape and the setting the clearance.
//! Since every bare filing now reads the registry, a rule file that does
//! not parse refuses the filing by path, the way `procedures` and a
//! named `file` already do.
//!
//! Under a procedure, the named definition is looked up in the registry,
//! refused when it is not installed, and its flights are minted with it.
//! The definition is read here and never again — each flight's fields
//! are copied into the log, so editing a definition afterwards cannot
//! disturb a flight already in the air. A flight's status is what its
//! definition declares, the setting's word when it declares nothing, and
//! then the edges have their say: dependencies fold Waiting, and the
//! parent waits on them all. **One flight** collapses onto the filing
//! itself — the definition's fields under the caller's flags, `--status`
//! included — because `ff tower file "fix the typo"` must not cost two
//! flights to say one thing. **Two or more** file a parent plus one
//! flight each, on the same `linked` edges `decompose` writes; `--status`
//! lands on the parent. All of it in one `append_with`: two appends would
//! leave a window where the parent is live, unlinked, and pullable.
//!
//! Still no fufu spawn. The registry's repository layer resolves through
//! `Store::main_worktree`, which reads the common dir and runs nothing.

use serde::Serialize;

use crate::config;
use crate::log::{Event, EventId, Kind, Store};
use crate::model::{Assignee, Status};
use crate::procedure::{self, Definition, Match, Registry};

use super::{Error, Fields, Parent, appended, appended_all, classify};

/// The envelope's `data`. Struct fields serialize in declaration order,
/// and this order — `filed, linked, parts` — is the alphabetical one the
/// CLI's `json!` emitted before the payload moved here, so the bytes on
/// the wire never changed. `routed` is last and absent when no rule
/// fired, so an unrouted filing's bytes never changed either.
#[derive(Serialize)]
pub struct Filed {
    pub filed: Event,
    pub linked: Vec<Event>,
    pub parts: Vec<Event>,
    /// The `routed` event, when a match rule chose the procedure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routed: Option<Event>,
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
        status: born(fields.status)?,
        ..fields
    };
    // Read once, ahead of the branch: the bare filing and a procedure's
    // undeclared rows take the same word.
    let default = config::default_file_status(&store.config());

    let Some(name) = procedure else {
        let installed = procedure::registry(store.main_worktree().as_deref())?;
        if let Some((definition, rule)) = matched(&installed, &fields, default) {
            return minted(store, definition, subject, &fields, default, Some(rule));
        }
        // The bare filing: no rule covers it, one flight, the setting's
        // word unless `--status` said one.
        let ids = store.append(vec![Kind::Filed {
            procedure: None,
            subject: subject.to_string(),
            body: fields.message.clone().unwrap_or_default(),
            status: fields.status.clone().unwrap_or_else(|| default.to_string()),
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
                routed: None,
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
    minted(store, definition, subject, &fields, default, None)
}

/// The mint under a definition, named or matched: `classify`'s batch in
/// one `append_with`, and — when a rule chose the definition — one
/// `routed` event on its tail naming the head mint, so the record of
/// judgment lands in the same commit as the flights it explains. The
/// event carries no overlay: the flights were minted with the
/// definition's fields already on them.
fn minted(
    store: &Store,
    definition: &Definition,
    subject: &str,
    fields: &Fields,
    default: &str,
    rule: Option<&Match>,
) -> Result<File, Error> {
    let ids = store.append_with(|mint| {
        let mut kinds = classify(definition, subject, fields, Parent::Mint, default, mint);
        if let Some(rule) = rule {
            kinds.push(Kind::Routed {
                flight: mint(0),
                procedure: definition.name.clone(),
                rule: rule.name.clone(),
                because: because(rule),
                status: None,
                assignee: None,
                priority: None,
                labels: None,
                skill: None,
                bay: None,
                done: None,
                branch: None,
            });
        }
        kinds
    })?;
    let (minted, routed) = match rule {
        Some(_) => {
            let (routed, minted) = ids.split_last().expect("the routing is the last event");
            (minted, Some(routed))
        }
        None => (ids.as_slice(), None),
    };
    let (parent, rest) = minted.split_first().expect("the parent is the first event");
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
            routed: routed.map(|id| appended(store, id)).transpose()?,
        },
        parent: parent.clone(),
        part_ids: filed.to_vec(),
    })
}

/// The first rule that covers the filing, with its definition: registry
/// name order, then declaration order, first match wins.
fn matched<'a>(
    installed: &'a Registry,
    fields: &Fields,
    default: &str,
) -> Option<(&'a Definition, &'a Match)> {
    installed.definitions().find_map(|definition| {
        definition
            .matches
            .iter()
            .find(|rule| covers(rule, fields, default))
            .map(|rule| (definition, rule))
    })
}

/// Whether one rule covers the caller's fields. Every present predicate
/// must hold, and a rule keyed on `source`/`event` can never match —
/// a filing carries no adapter provenance, so adapter rules stay
/// honestly inert per-rule. A rule with no predicates matches nothing;
/// the loader refuses one, but a hand-built rule must not cover the
/// world. An unset priority is `none`, and an unset status is the
/// setting's word — the words the filing stores.
fn covers(rule: &Match, fields: &Fields, default: &str) -> bool {
    if !rule.has_predicates() || rule.source.is_some() || rule.event.is_some() {
        return false;
    }
    rule.label
        .as_ref()
        .is_none_or(|label| fields.labels.contains(label))
        && rule
            .priority
            .as_deref()
            .is_none_or(|priority| fields.priority.as_deref().unwrap_or("none") == priority)
        && rule
            .skill
            .as_ref()
            .is_none_or(|skill| fields.skill.as_ref() == Some(skill))
        && rule
            .assignee
            .as_ref()
            .is_none_or(|assignee| fields.assignee.as_ref() == Some(assignee))
        && rule
            .status
            .as_deref()
            .is_none_or(|status| fields.status.as_deref().unwrap_or(default) == status)
}

/// The render-ready explanation the routing event stores — "matched
/// label chore", every present predicate named.
fn because(rule: &Match) -> String {
    let mut phrases = Vec::new();
    if let Some(label) = &rule.label {
        phrases.push(format!("label {label}"));
    }
    if let Some(priority) = &rule.priority {
        phrases.push(format!("priority {priority}"));
    }
    if let Some(skill) = &rule.skill {
        phrases.push(format!("skill {skill}"));
    }
    if let Some(assignee) = &rule.assignee {
        phrases.push(format!("assignee {assignee}"));
    }
    if let Some(status) = &rule.status {
        phrases.push(format!("status {status}"));
    }
    format!("matched {}", phrases.join(", "))
}

/// The caller's `--status`, validated: a word a flight can be filed
/// with passes through, anything else refuses — a word that is not a
/// status, one the fold derives, or one that is closed, all by the same
/// rule, so the log never learns a word this binary would not write.
fn born(status: Option<String>) -> Result<Option<String>, Error> {
    match status.as_deref() {
        None => Ok(None),
        Some(word) => match Status::fileable(word) {
            Some(status) => Ok(Some(status.name().to_string())),
            None => Err(Error::FileStatus {
                word: word.to_string(),
            }),
        },
    }
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

    /// Set `tower.defaultFileStatus` and reopen: a store reads config at
    /// open, the way every process does, so a change lands on the next
    /// one.
    fn with_default(repo: &Repo, word: &str) -> Store {
        repo.git(&["config", "tower.defaultFileStatus", word]);
        Store::open(repo.path()).expect("reopen")
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

    /// A one-flight definition with a label rule: the intake case.
    const CHORES: &str = r#"
name = "chores"
[[match]]
name  = "chore-label"
label = "chore"
[[flight]]
id       = "work"
assignee = "me"
skill    = "tidy"
priority = "low"
done     = "committed"
"#;

    fn folded(store: &Store) -> board::Fold {
        board::fold(&store.read_all().expect("read"))
    }

    fn labeled(label: &str) -> Fields {
        Fields {
            labels: vec![label.to_string()],
            ..Fields::default()
        }
    }

    #[test]
    fn a_bare_filing_lands_ready_with_its_flags() {
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
                status: None,
            },
            None,
        )
        .expect("files");
        assert!(outcome.part_ids.is_empty());
        assert!(outcome.payload.linked.is_empty());
        assert!(outcome.payload.routed.is_none());

        let fold = folded(&store);
        let flight = &fold.flights[0];
        assert!(flight.procedure.is_none(), "bare carries no procedure");
        assert_eq!(flight.status, "ready", "the default clears it at once");
        assert_eq!(flight.assignee.as_deref(), Some("agent"));
        assert_eq!(flight.priority, "high");
        assert_eq!(flight.labels, ["web"]);
        assert_eq!(flight.skill.as_deref(), Some("debug"));
        assert_eq!(flight.bay.as_deref(), Some("warm"));
        assert_eq!(flight.body, "the redirect loops");
        assert!(flight.pullable(), "agent-laned and Ready — pullable");
    }

    #[test]
    fn the_setting_moves_the_bare_default_and_the_flag_beats_it() {
        let (repo, _) = store();
        let store = with_default(&repo, "backlog");
        file(&store, "parked", Fields::default(), None).expect("files");
        file(
            &store,
            "moving",
            Fields {
                status: Some("in_progress".to_string()),
                ..Fields::default()
            },
            None,
        )
        .expect("files");

        let fold = folded(&store);
        assert_eq!(fold.flights[0].status, "backlog", "the setting's word");
        assert_eq!(fold.flights[1].status, "in_progress", "--status wins");
        assert!(
            !fold.flights[0].pullable(),
            "Backlog — the lane alone clears nothing"
        );
    }

    #[test]
    fn an_unfileable_status_refuses_before_the_store() {
        let (_repo, store) = store();
        for word in ["held", "waiting", "done", "canceled", "claimed"] {
            let err = file(
                &store,
                "no",
                Fields {
                    status: Some(word.to_string()),
                    ..Fields::default()
                },
                None,
            )
            .err()
            .expect("cannot be filed");
            assert_eq!(err.id(), "usage/file-status");
            assert_eq!(
                err.to_string(),
                format!("`{word}` cannot be filed — backlog, ready, or in_progress")
            );
        }
        assert!(folded(&store).flights.is_empty(), "nothing was written");
    }

    #[test]
    fn a_procedure_flight_declaring_a_status_keeps_it_and_siblings_take_the_default() {
        let (repo, store) = store();
        install(
            &repo,
            "staged",
            r#"
name = "staged"

[[flight]]
id       = "look"
assignee = "me"
status   = "backlog"

[[flight]]
id       = "do"
assignee = "agent"
"#,
        );
        file(&store, "the thing", Fields::default(), Some("staged")).expect("files");

        let fold = folded(&store);
        let by_subject = |tail: &str| {
            fold.flights
                .iter()
                .find(|flight| flight.subject.ends_with(tail))
                .expect("minted")
        };
        assert_eq!(by_subject("· look").status, "backlog", "declared, kept");
        assert_eq!(
            by_subject("· do").status,
            "ready",
            "undeclared, the default"
        );

        // Under `backlog` as the default, the declared word still wins
        // and the sibling follows the setting; `--status` lands on the
        // parent, and In Progress is not a word the edges gate.
        let store = with_default(&repo, "backlog");
        file(
            &store,
            "again",
            Fields {
                status: Some("in_progress".to_string()),
                ..Fields::default()
            },
            Some("staged"),
        )
        .expect("files");
        let fold = folded(&store);
        let by_subject = |tail: &str| {
            fold.flights
                .iter()
                .find(|flight| flight.subject == tail)
                .expect("minted")
        };
        assert_eq!(by_subject("again · look").status, "backlog");
        assert_eq!(
            by_subject("again · do").status,
            "backlog",
            "the setting's word"
        );
        assert_eq!(
            by_subject("again").status,
            "in_progress",
            "--status lands on the parent"
        );
    }

    #[test]
    fn the_collapse_takes_the_flag_over_the_definition_over_the_default() {
        let (repo, store) = store();
        install(
            &repo,
            "parked",
            r#"
name = "parked"

[[flight]]
id       = "work"
assignee = "me"
status   = "backlog"
"#,
        );
        file(&store, "declared", Fields::default(), Some("parked")).expect("files");
        file(
            &store,
            "flagged",
            Fields {
                status: Some("ready".to_string()),
                ..Fields::default()
            },
            Some("parked"),
        )
        .expect("files");
        install(&repo, "ticket", TICKET);
        file(&store, "defaulted", Fields::default(), Some("ticket")).expect("files");

        let fold = folded(&store);
        assert_eq!(fold.flights[0].status, "backlog", "the definition's word");
        assert_eq!(fold.flights[1].status, "ready", "--status beats it");
        assert_eq!(
            fold.flights[2].status, "ready",
            "nothing declared — the default"
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

    #[test]
    fn a_label_rule_routes_a_bare_filing_as_the_named_filing_with_the_right_because() {
        let (repo, store) = store();
        install(&repo, "chores", CHORES);
        let outcome = file(&store, "sweep the logs", labeled("chore"), None).expect("files");
        assert!(outcome.part_ids.is_empty(), "one flight collapses");
        let routed = outcome.payload.routed.as_ref().expect("a routed event");
        let Kind::Routed {
            flight,
            procedure,
            rule,
            because,
            status,
            assignee,
            priority,
            labels,
            skill,
            bay,
            done,
            branch,
        } = &routed.kind
        else {
            panic!("expected a routing, got {:?}", routed.kind);
        };
        assert_eq!(flight, &outcome.parent, "the routing names the filing");
        assert_eq!(procedure, "chores");
        assert_eq!(rule, "chore-label");
        assert_eq!(because, "matched label chore");
        assert!(
            status.is_none()
                && assignee.is_none()
                && priority.is_none()
                && labels.is_none()
                && skill.is_none()
                && bay.is_none()
                && done.is_none()
                && branch.is_none(),
            "the event is the record of judgment alone"
        );

        // Minted exactly as `file chores "sweep the logs" --label chore`
        // would be: the definition's fields under the caller's flags,
        // born Ready under the default setting.
        let fold = folded(&store);
        let flight = &fold.flights[0];
        assert_eq!(flight.procedure.as_deref(), Some("chores"));
        assert_eq!(flight.status, "ready");
        assert_eq!(flight.assignee.as_deref(), Some("me"));
        assert_eq!(flight.skill.as_deref(), Some("tidy"));
        assert_eq!(flight.priority, "low");
        assert_eq!(flight.labels, ["chore"], "the caller's label stays");
        assert_eq!(flight.done_kind, "committed");
        assert!(fold.unrouted.is_empty());
    }

    #[test]
    fn a_routed_filing_parks_under_the_backlog_setting_like_any_other() {
        // The rule decides the shape, the setting decides the clearance:
        // a matched filing lands where a named one would.
        let (repo, _) = store();
        install(&repo, "chores", CHORES);
        let store = with_default(&repo, "backlog");
        file(&store, "sweep the logs", labeled("chore"), None).expect("files");
        let fold = folded(&store);
        assert_eq!(fold.flights[0].procedure.as_deref(), Some("chores"));
        assert_eq!(fold.flights[0].status, "backlog");
    }

    #[test]
    fn a_filing_no_rule_covers_lands_bare_and_a_named_filing_is_never_matched() {
        let (repo, store) = store();
        install(&repo, "chores", CHORES);
        install(&repo, "ticket", TICKET);
        let plain = file(&store, "plain", labeled("ops"), None).expect("files");
        assert!(plain.payload.routed.is_none());
        let named = file(&store, "named", labeled("chore"), Some("ticket")).expect("files");
        assert!(named.payload.routed.is_none(), "the name was typed");

        let fold = folded(&store);
        assert!(fold.flights[0].procedure.is_none());
        assert_eq!(fold.flights[1].procedure.as_deref(), Some("ticket"));
        assert_eq!(
            fold.flights[1].labels,
            ["chore"],
            "the label rides, the rule does not fire"
        );
        assert!(
            store
                .read_all()
                .expect("read")
                .iter()
                .all(|event| !matches!(event.kind, Kind::Routed { .. })),
            "no routing on the record"
        );
    }

    #[test]
    fn an_adapter_keyed_rule_is_inert() {
        let (repo, store) = store();
        install(
            &repo,
            "review",
            r#"
name = "review"
[[match]]
name   = "github-reviews"
source = "github"
event  = "review_requested"
[[flight]]
id       = "work"
assignee = "me"
"#,
        );
        let outcome = file(&store, "s", labeled("chore"), None).expect("files");
        assert!(outcome.payload.routed.is_none());
        assert!(folded(&store).flights[0].procedure.is_none());
    }

    #[test]
    fn predicates_all_and() {
        let (repo, store) = store();
        install(
            &repo,
            "narrow",
            r#"
name = "narrow"
[[match]]
name     = "labeled-high"
label    = "chore"
priority = "high"
[[flight]]
id       = "work"
assignee = "me"
"#,
        );
        // The label matches, the priority does not: bare.
        let half = file(&store, "half", labeled("chore"), None).expect("files");
        assert!(half.payload.routed.is_none());
        // Both hold: routed.
        let both = file(
            &store,
            "both",
            Fields {
                priority: Some("high".to_string()),
                ..labeled("chore")
            },
            None,
        )
        .expect("files");
        assert!(both.payload.routed.is_some());
        let fold = folded(&store);
        assert!(fold.flights[0].procedure.is_none());
        assert_eq!(fold.flights[1].procedure.as_deref(), Some("narrow"));
    }

    /// A one-flight definition with a status rule: the parked case.
    const PARKED: &str = r#"
name = "parked"
[[match]]
name   = "parked"
status = "backlog"
[[flight]]
id       = "work"
assignee = "me"
"#;

    fn parked(status: &str) -> Fields {
        Fields {
            status: Some(status.to_string()),
            ..Fields::default()
        }
    }

    #[test]
    fn a_status_rule_reads_the_flag_over_the_setting() {
        let (repo, store) = store();
        install(&repo, "parked", PARKED);
        // Under the default Ready, `--status backlog` is what the filing
        // stores, so the rule covers it; a filing with no flag is bare.
        let flagged = file(&store, "flagged", parked("backlog"), None).expect("files");
        assert!(flagged.payload.routed.is_some());
        let plain = file(&store, "plain", Fields::default(), None).expect("files");
        assert!(plain.payload.routed.is_none());

        let fold = folded(&store);
        assert_eq!(fold.flights[0].procedure.as_deref(), Some("parked"));
        assert_eq!(fold.flights[0].status, "backlog");
        assert!(fold.flights[1].procedure.is_none());
        assert_eq!(fold.flights[1].status, "ready");
    }

    #[test]
    fn a_status_rule_reads_the_settings_word_when_no_flag_is_given() {
        let (repo, _) = store();
        install(&repo, "parked", PARKED);
        let store = with_default(&repo, "backlog");
        let plain = file(&store, "plain", Fields::default(), None).expect("files");
        let routed = plain.payload.routed.as_ref().expect("a routed event");
        let Kind::Routed { because, .. } = &routed.kind else {
            panic!("expected a routing, got {:?}", routed.kind);
        };
        assert_eq!(because, "matched status backlog");
        // `--status ready` beats the setting, so the same filing is bare.
        let cleared = file(&store, "cleared", parked("ready"), None).expect("files");
        assert!(cleared.payload.routed.is_none());

        let fold = folded(&store);
        assert_eq!(fold.flights[0].procedure.as_deref(), Some("parked"));
        assert_eq!(fold.flights[0].status, "backlog", "lands parked");
        assert!(fold.flights[1].procedure.is_none());
        assert_eq!(fold.flights[1].status, "ready");
    }

    #[test]
    fn a_status_rule_naming_an_unfileable_word_installs_and_never_matches() {
        let (repo, store) = store();
        for word in ["held", "waiting"] {
            install(
                &repo,
                word,
                &format!(
                    r#"
name   = "{word}"
[[match]]
name   = "{word}"
status = "{word}"
[[flight]]
id       = "work"
assignee = "me"
"#
                ),
            );
        }
        let ready = file(&store, "ready", Fields::default(), None).expect("files");
        assert!(ready.payload.routed.is_none());
        let store = with_default(&repo, "backlog");
        let parked = file(&store, "parked", Fields::default(), None).expect("files");
        assert!(parked.payload.routed.is_none());
        assert!(
            folded(&store)
                .flights
                .iter()
                .all(|flight| flight.procedure.is_none()),
            "no filing carries a word a rule cannot see"
        );
    }

    #[test]
    fn first_match_wins_within_and_across_definitions() {
        let (repo, store) = store();
        install(
            &repo,
            "alpha",
            r#"
name = "alpha"
[[match]]
name  = "first"
label = "chore"
[[match]]
name  = "second"
label = "chore"
[[flight]]
id       = "work"
assignee = "me"
"#,
        );
        install(
            &repo,
            "beta",
            r#"
name = "beta"
[[match]]
name  = "also"
label = "chore"
[[flight]]
id       = "work"
assignee = "me"
"#,
        );
        let outcome = file(&store, "s", labeled("chore"), None).expect("files");
        // `alpha` sorts first in the registry, and its first rule beats
        // its second.
        let Some(Event {
            kind: Kind::Routed {
                procedure, rule, ..
            },
            ..
        }) = &outcome.payload.routed
        else {
            panic!("expected a routing");
        };
        assert_eq!(procedure, "alpha");
        assert_eq!(rule, "first");
    }

    #[test]
    fn a_broken_rule_file_refuses_the_bare_filing_by_path() {
        let (repo, store) = store();
        install(&repo, "broken", "name = \"broken\"\n");
        let err = file(&store, "anything", Fields::default(), None)
            .err()
            .expect("the registry refuses");
        assert_eq!(err.id(), "procedure/no-parts");
        assert!(folded(&store).flights.is_empty(), "nothing was written");
    }
}

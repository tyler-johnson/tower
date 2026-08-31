//! The lazy pass: the board's two automated transitions, run whenever
//! anyone asks for anything.
//!
//! DESIGN.md promises exactly two moves without a hand on them, both
//! deterministic, attributed, and explained in the history: a match rule
//! routing an arrival out of Triage, and the Waiting → Ready advance
//! when a flight's last live dependency closes. Both run here — any
//! invocation, and each of serve's refolds, calls [`pass`], which
//! examines what the rules cover and appends what it concludes — so the
//! board catches up with no standing process.
//!
//! [`conclusions`] is the pure half: what the pass would do, off a fold
//! and the installed registry alone. [`pass`] decides *outside* the
//! append and only then takes the writer — `append_with` mints the
//! writer and takes the lock before its plan runs, so this split is
//! what keeps `ff tower board` on a quiet repository truly read-only.
//! When it does conclude, one atomic `append_with` whose plan recomputes
//! the conclusions from a fresh read: a lost CAS re-runs the plan, and a
//! concurrent pass may already have concluded the same things.
//!
//! No fixpoint is needed. Routing closes nothing, so it cannot enable an
//! advance; an advance routes nothing. One batch settles the board, and
//! a second pass concludes nothing — the idempotence that terminates
//! serve's watcher loop.

use crate::board::{self, Flight, Fold};
use crate::log::{self, EventId, Kind, Store};
use crate::procedure::{Definition, Done, Match, Registry};

use super::{Fields, Parent, classify};

/// One thing the pass would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conclusion {
    /// A Triage flight a rule covers, routed under the rule's procedure.
    Route {
        flight: EventId,
        procedure: String,
        rule: String,
        because: String,
    },
    /// A Waiting flight whose every declared dependency is done.
    Advance { flight: EventId, reason: String },
}

/// What the pass would conclude, pure: routes first — each Triage flight
/// in filed order, procedures in registry (name) order, rules in
/// declaration order, first match wins, one conclusion per flight — then
/// advances, for each Waiting flight whose dependency list is non-empty
/// and entirely done. A hand-parked Waiting flight with no edges is left
/// alone: advancing it would fight the person's gesture every pass.
pub fn conclusions(fold: &Fold, installed: &Registry) -> Vec<Conclusion> {
    let mut concluded = Vec::new();

    for flight in &fold.flights {
        if flight.status != "triage" {
            continue;
        }
        'flight: for definition in installed.definitions() {
            for rule in &definition.matches {
                if covers(rule, flight) {
                    concluded.push(Conclusion::Route {
                        flight: flight.id.clone(),
                        procedure: definition.name.clone(),
                        rule: rule.name.clone(),
                        because: because(rule),
                    });
                    break 'flight;
                }
            }
        }
    }

    for flight in &fold.flights {
        if flight.status != "waiting" || flight.depends_on.is_empty() {
            continue;
        }
        // Done exactly, `pick`'s belt rule: a canceled dependency does
        // not satisfy its waiters — the flight stays Waiting, and the
        // canceled part surfaces on the brief's per-dependency status.
        let all_done = flight.depends_on.iter().all(|dep| {
            fold.flights
                .iter()
                .find(|other| &other.id == dep)
                .is_some_and(|other| other.status == "done")
        });
        if all_done {
            concluded.push(Conclusion::Advance {
                flight: flight.id.clone(),
                reason: reason(&flight.depends_on),
            });
        }
    }

    concluded
}

/// Whether one rule matches one folded flight. Every present predicate
/// must hold, and a rule keyed on `source`/`event` can never match —
/// flights store no adapter provenance, so adapter rules stay honestly
/// inert per-rule. A rule with no predicates matches nothing; the loader
/// refuses one, but a hand-built rule must not cover the world.
fn covers(rule: &Match, flight: &Flight) -> bool {
    if !rule.has_predicates() || rule.source.is_some() || rule.event.is_some() {
        return false;
    }
    rule.label
        .as_ref()
        .is_none_or(|label| flight.labels.contains(label))
        && rule
            .priority
            .as_ref()
            .is_none_or(|priority| &flight.priority == priority)
        && rule
            .skill
            .as_ref()
            .is_none_or(|skill| flight.skill.as_ref() == Some(skill))
        && rule
            .assignee
            .as_ref()
            .is_none_or(|assignee| flight.assignee.as_ref() == Some(assignee))
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
    format!("matched {}", phrases.join(", "))
}

/// The advance's stored explanation, naming the closed dependencies.
fn reason(deps: &[EventId]) -> String {
    let named: Vec<String> = deps.iter().map(ToString::to_string).collect();
    if named.len() == 1 {
        format!("dependency {} done", named[0])
    } else {
        format!("dependencies {} done", named.join(", "))
    }
}

/// The conclusions as one append batch: routing kinds first, then
/// advances — `conclusions` already builds them in that order, and the
/// order is load-bearing only for the mint offsets `classify` hands out.
fn kinds(
    fold: &Fold,
    installed: &Registry,
    concluded: &[Conclusion],
    mint: &dyn Fn(usize) -> EventId,
) -> Vec<Kind> {
    let mut kinds: Vec<Kind> = Vec::new();
    for conclusion in concluded {
        match conclusion {
            Conclusion::Route {
                flight,
                procedure,
                rule,
                because,
            } => {
                let definition = installed
                    .get(procedure)
                    .expect("the conclusion named an installed procedure");
                let flight = fold
                    .flights
                    .iter()
                    .find(|folded| &folded.id == flight)
                    .expect("the conclusion named a folded flight");
                if let [only] = definition.flights.as_slice() {
                    // The collapse rule: a single-flight definition
                    // routes onto the flight itself, born Ready, the
                    // overlay resolved the way `classify` resolves
                    // caller flags — the flight's own non-default
                    // fields win, the definition fills the gaps, and an
                    // unchanged field stays `None` on the wire.
                    kinds.push(Kind::Routed {
                        flight: flight.id.clone(),
                        procedure: procedure.clone(),
                        rule: rule.clone(),
                        because: because.clone(),
                        status: Some("ready".to_string()),
                        assignee: flight
                            .assignee
                            .is_none()
                            .then(|| only.assignee.name().to_string()),
                        priority: (flight.priority == "none")
                            .then(|| only.priority.clone())
                            .flatten(),
                        labels: (flight.labels.is_empty() && !only.labels.is_empty())
                            .then(|| only.labels.clone()),
                        skill: flight.skill.is_none().then(|| only.skill.clone()).flatten(),
                        bay: flight
                            .bay
                            .is_none()
                            .then(|| only.bay.map(|bay| bay.name().to_string()))
                            .flatten(),
                        done: (flight.done_kind == "asserted" && only.done != Done::Asserted)
                            .then(|| only.done.name().to_string()),
                        branch: branch(definition, flight),
                    });
                } else {
                    // Multi-flight: the flight becomes the parent, born
                    // Waiting with no field overlay — the caller-flags
                    // rule, a parent keeps its own fields — and the
                    // children with their edges ride the same batch,
                    // their mint indices shifted by what is queued.
                    kinds.push(Kind::Routed {
                        flight: flight.id.clone(),
                        procedure: procedure.clone(),
                        rule: rule.clone(),
                        because: because.clone(),
                        status: Some("waiting".to_string()),
                        assignee: None,
                        priority: None,
                        labels: None,
                        skill: None,
                        bay: None,
                        done: None,
                        branch: None,
                    });
                    let base = kinds.len();
                    kinds.extend(classify(
                        definition,
                        &flight.subject,
                        &Fields::default(),
                        Parent::Existing(flight.id.clone()),
                        &|offset| mint(base + offset),
                    ));
                }
            }
            // The existing status vocabulary, zero new fold surface:
            // attributed to the invoker, explained by the reason.
            Conclusion::Advance { flight, reason } => kinds.push(Kind::Status {
                flight: flight.clone(),
                status: "ready".to_string(),
                reason: Some(reason.clone()),
            }),
        }
    }
    kinds
}

/// A definition's `subject = "branch"` rule, resolved at pass time the
/// way `file` resolves it at file time — from the flight's own subject,
/// and only where no stamp already stands.
fn branch(definition: &Definition, flight: &Flight) -> Option<String> {
    (definition.subject.as_deref() == Some("branch") && flight.branch_stamp.is_none())
        .then(|| flight.subject.clone())
}

/// Run the pass: read, fold, and — only when something concludes — one
/// atomic append. The quiet path returns before any writer is minted or
/// lock taken. The plan recomputes the conclusions from a fresh read on
/// every attempt; a re-read failure returns an empty batch, and the next
/// pass catches up.
pub fn pass(store: &Store, installed: &Registry) -> Result<Vec<EventId>, log::Error> {
    let events = store.read_all()?;
    if conclusions(&board::fold(&events), installed).is_empty() {
        return Ok(Vec::new());
    }
    store.append_with(|mint| {
        let Ok(events) = store.read_all() else {
            return Vec::new();
        };
        let fold = board::fold(&events);
        let concluded = conclusions(&fold, installed);
        kinds(&fold, installed, &concluded, mint)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::Event;
    use crate::procedure::{self, Source};
    use std::path::PathBuf;

    fn event(id: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind,
        }
    }

    /// A hand filing in Triage carrying the given labels.
    fn filed(id: &str, time: i64, subject: &str, labels: &[&str]) -> Event {
        stored(id, time, subject, "triage", labels)
    }

    fn stored(id: &str, time: i64, subject: &str, status: &str, labels: &[&str]) -> Event {
        event(
            id,
            time,
            Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: String::new(),
                status: status.to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: labels.iter().map(|label| label.to_string()).collect(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        )
    }

    fn moved(id: &str, time: i64, flight: &str, to: &str) -> Event {
        event(
            id,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
            },
        )
    }

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        event(
            id,
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
            },
        )
    }

    /// A registry over hand-written definitions, loaded through the real
    /// loader so a test rule is always one the validator accepts. The
    /// layer is the repository's, which is where a rule file lives.
    fn registry(definitions: &[&str]) -> Registry {
        let mut installed = Registry::default();
        for (index, text) in definitions.iter().enumerate() {
            let source = Source::Repo(PathBuf::from(format!("rule-{index}.toml")));
            installed.insert(procedure::load(text, source).expect("loads"));
        }
        installed
    }

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

    fn folded(events: &[Event]) -> Fold {
        board::fold(events)
    }

    #[test]
    fn a_label_rule_routes_a_triage_flight_with_the_right_because() {
        let fold = folded(&[filed("pi.1", 10, "sweep the logs", &["chore"])]);
        let concluded = conclusions(&fold, &registry(&[CHORES]));
        assert_eq!(
            concluded,
            [Conclusion::Route {
                flight: "pi.1".parse().expect("id"),
                procedure: "chores".to_string(),
                rule: "chore-label".to_string(),
                because: "matched label chore".to_string(),
            }]
        );
    }

    #[test]
    fn only_triage_is_covered_by_routing() {
        let fold = folded(&[
            stored("pi.1", 10, "a", "ready", &["chore"]),
            stored("pi.2", 20, "b", "waiting", &["chore"]),
            stored("pi.3", 30, "c", "held", &["chore"]),
            stored("pi.4", 40, "d", "in_progress", &["chore"]),
            moved("pi.5", 50, "pi.4", "done"),
        ]);
        assert!(conclusions(&fold, &registry(&[CHORES])).is_empty());
    }

    #[test]
    fn an_adapter_keyed_rule_is_inert() {
        let github = r#"
name = "review"
[[match]]
name   = "github-reviews"
source = "github"
event  = "review_requested"
[[flight]]
id       = "work"
assignee = "me"
"#;
        let fold = folded(&[filed("pi.1", 10, "s", &["chore"])]);
        assert!(conclusions(&fold, &registry(&[github])).is_empty());
    }

    #[test]
    fn predicates_all_and() {
        let both = r#"
name = "narrow"
[[match]]
name     = "labeled-high"
label    = "chore"
priority = "high"
[[flight]]
id       = "work"
assignee = "me"
"#;
        // The label matches, the priority does not: no conclusion.
        let fold = folded(&[filed("pi.1", 10, "s", &["chore"])]);
        assert!(conclusions(&fold, &registry(&[both])).is_empty());
    }

    #[test]
    fn first_match_wins_within_and_across_definitions() {
        let two_rules = r#"
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
"#;
        let later = r#"
name = "beta"
[[match]]
name  = "also"
label = "chore"
[[flight]]
id       = "work"
assignee = "me"
"#;
        let fold = folded(&[filed("pi.1", 10, "s", &["chore"])]);
        let concluded = conclusions(&fold, &registry(&[later, two_rules]));
        // One conclusion per flight; `alpha` sorts first in the
        // registry, and its first rule beats its second.
        match concluded.as_slice() {
            [
                Conclusion::Route {
                    procedure, rule, ..
                },
            ] => {
                assert_eq!(procedure, "alpha");
                assert_eq!(rule, "first");
            }
            other => panic!("expected one route, got {other:?}"),
        }
    }

    #[test]
    fn an_all_done_dependency_list_advances_with_the_reason() {
        let fold = folded(&[
            stored("pi.1", 10, "dep a", "ready", &[]),
            stored("pi.2", 20, "dep b", "ready", &[]),
            stored("pi.3", 30, "waiter", "waiting", &[]),
            linked("pi.4", 40, "pi.3", "pi.1"),
            linked("pi.5", 50, "pi.3", "pi.2"),
            moved("pi.6", 60, "pi.1", "done"),
            moved("pi.7", 70, "pi.2", "done"),
        ]);
        assert_eq!(
            conclusions(&fold, &Registry::default()),
            [Conclusion::Advance {
                flight: "pi.3".parse().expect("id"),
                reason: "dependencies pi.1, pi.2 done".to_string(),
            }]
        );
    }

    #[test]
    fn a_canceled_or_live_dependency_concludes_nothing() {
        let canceled = folded(&[
            stored("pi.1", 10, "dep", "ready", &[]),
            stored("pi.2", 20, "waiter", "waiting", &[]),
            linked("pi.3", 30, "pi.2", "pi.1"),
            moved("pi.4", 40, "pi.1", "canceled"),
        ]);
        assert!(conclusions(&canceled, &Registry::default()).is_empty());

        let mixed = folded(&[
            stored("pi.1", 10, "dep a", "ready", &[]),
            stored("pi.2", 20, "dep b", "ready", &[]),
            stored("pi.3", 30, "waiter", "waiting", &[]),
            linked("pi.4", 40, "pi.3", "pi.1"),
            linked("pi.5", 50, "pi.3", "pi.2"),
            moved("pi.6", 60, "pi.1", "done"),
        ]);
        assert!(conclusions(&mixed, &Registry::default()).is_empty());
    }

    #[test]
    fn a_hand_parked_waiting_flight_with_no_edges_is_left_alone() {
        let fold = folded(&[stored("pi.1", 10, "parked", "waiting", &[])]);
        assert!(conclusions(&fold, &Registry::default()).is_empty());
    }

    #[test]
    fn held_triage_and_ready_flights_are_never_advanced() {
        let fold = folded(&[
            stored("pi.1", 10, "dep", "ready", &[]),
            stored("pi.2", 20, "held", "held", &[]),
            stored("pi.3", 30, "triage", "triage", &[]),
            stored("pi.4", 40, "ready", "ready", &[]),
            linked("pi.5", 50, "pi.2", "pi.1"),
            linked("pi.6", 60, "pi.3", "pi.1"),
            linked("pi.7", 70, "pi.4", "pi.1"),
            moved("pi.8", 80, "pi.1", "done"),
        ]);
        assert!(conclusions(&fold, &Registry::default()).is_empty());
    }

    #[test]
    fn a_single_flight_route_resolves_the_overlay_and_the_flights_fields_win() {
        let branchy = r#"
name    = "chores"
subject = "branch"
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
        let installed = registry(&[branchy]);
        let mut filing = filed("pi.1", 10, "sweep the logs", &["chore"]);
        // The flight carries its own priority: the definition must not
        // overwrite it.
        if let Kind::Filed { priority, .. } = &mut filing.kind {
            "high".clone_into(priority);
        }
        let fold = folded(&[filing]);
        let concluded = conclusions(&fold, &installed);
        let batch = kinds(&fold, &installed, &concluded, &|offset| EventId {
            writer: "pi".to_string(),
            seq: 100 + offset as u64,
        });
        match batch.as_slice() {
            [
                Kind::Routed {
                    status,
                    assignee,
                    priority,
                    labels,
                    skill,
                    bay,
                    done,
                    branch,
                    ..
                },
            ] => {
                assert_eq!(status.as_deref(), Some("ready"));
                assert_eq!(assignee.as_deref(), Some("me"));
                assert!(priority.is_none(), "the flight's own priority wins");
                assert!(labels.is_none(), "the definition has none to fill");
                assert_eq!(skill.as_deref(), Some("tidy"));
                assert!(bay.is_none());
                assert_eq!(done.as_deref(), Some("committed"));
                assert_eq!(branch.as_deref(), Some("sweep the logs"));
            }
            other => panic!("expected one routed kind, got {other:?}"),
        }
    }

    #[test]
    fn a_multi_flight_route_mints_the_children_on_shifted_indices() {
        let review = r#"
name = "review"
[[match]]
name  = "chore-label"
label = "chore"
[[flight]]
id       = "pass"
assignee = "agent"
[[flight]]
id       = "verdict"
assignee = "me"
after    = ["pass"]
"#;
        let installed = registry(&[review]);
        let fold = folded(&[filed("pi.1", 10, "feather", &["chore"])]);
        let concluded = conclusions(&fold, &installed);
        let batch = kinds(&fold, &installed, &concluded, &|offset| EventId {
            writer: "pi".to_string(),
            seq: 100 + offset as u64,
        });
        // One routed parent, two children, the parent's two edges, and
        // the after edge — with every link naming the batch-relative
        // mints shifted past the routed kind.
        assert_eq!(batch.len(), 6);
        let Kind::Routed { status, .. } = &batch[0] else {
            panic!("the routing leads the batch");
        };
        assert_eq!(status.as_deref(), Some("waiting"));
        assert!(matches!(&batch[1], Kind::Filed { status, .. } if status == "ready"));
        assert!(matches!(&batch[2], Kind::Filed { status, .. } if status == "waiting"));
        let edge = |kind: &Kind| match kind {
            Kind::Linked { from, to } => (from.to_string(), to.to_string()),
            other => panic!("expected an edge, got {other:?}"),
        };
        assert_eq!(edge(&batch[3]), ("pi.1".to_string(), "pi.101".to_string()));
        assert_eq!(edge(&batch[4]), ("pi.1".to_string(), "pi.102".to_string()));
        assert_eq!(
            edge(&batch[5]),
            ("pi.102".to_string(), "pi.101".to_string())
        );
    }

    #[test]
    fn applying_the_conclusions_settles_the_board() {
        // Idempotence, the property serve's loop terminates on: apply
        // one pass's batch, re-fold, and the next pass concludes
        // nothing.
        let installed = registry(&[CHORES]);
        let mut events = vec![
            filed("pi.1", 10, "sweep the logs", &["chore"]),
            stored("pi.2", 20, "dep", "ready", &[]),
            stored("pi.3", 30, "waiter", "waiting", &[]),
            linked("pi.4", 40, "pi.3", "pi.2"),
            moved("pi.5", 50, "pi.2", "done"),
        ];
        let fold = folded(&events);
        let concluded = conclusions(&fold, &installed);
        assert_eq!(concluded.len(), 2, "one route and one advance");
        let batch = kinds(&fold, &installed, &concluded, &|offset| EventId {
            writer: "pi".to_string(),
            seq: 100 + offset as u64,
        });
        events.extend(
            batch
                .into_iter()
                .enumerate()
                .map(|(offset, kind)| event(&format!("pi.{}", 100 + offset), 60, kind)),
        );
        assert!(
            conclusions(&folded(&events), &installed).is_empty(),
            "a second pass concludes nothing"
        );
    }
}

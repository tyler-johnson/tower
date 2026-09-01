//! The board: the fold's flights, enriched with the repository's answer
//! to "where is each one" and grouped by the status each record derives.
//!
//! `enrich` is pure — it runs over a [`Reads`] the caller already
//! gathered, plus the two scalars it refuses to read for itself: `now`,
//! and the threshold behind the stale line. A flight sits in the group
//! its derived status names — the projection the fold already made over
//! the stored facts, the open question and the edges — and `enrich`
//! moves it nowhere further; `held`/`resolving` stay fufu's branch
//! verdicts, printed on the row, deciding no section.
//!
//! What the repository knows lands beside the fields as two independent
//! facts, never joined under one word: a flight In Progress that its
//! branch has forgotten, and a Ready flight whose branch moved after it
//! was set Ready. One is staleness and one is its opposite.
//!
//! Above the groups sits the inbox — questions and yours — a view of the
//! same rows rather than a seventh group. Having a parent is not a
//! grouping fact either: a sub-flight is a flight, and it files beside
//! every other row.

use std::collections::HashMap;

use serde::Serialize;

use crate::ff::Pairing;
use crate::log::Event;

use super::flight::{Flight, Fold};
use super::reads::{Reads, Verdicts};

/// How much of the closed group a render carries.
///
/// A count rather than a span by default: three rows hold their size
/// whatever the week did, where a span shows nothing on a quiet Monday
/// and a wall after a Friday sweep. Compiled in and not a config key —
/// the window is a render's memory of the week, not a preference, and
/// the log was always the full record regardless. The CLI's `--closed`
/// overrides it for one render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedWindow {
    /// Every closed flight, however old.
    All,
    /// No closed group at all.
    None,
    /// The `n` newest.
    Count(usize),
    /// Everything closed inside the last `n` seconds.
    Span(i64),
}

/// The compiled-in window: the three newest closed flights.
pub const DEFAULT_CLOSED: ClosedWindow = ClosedWindow::Count(3);

impl Default for ClosedWindow {
    fn default() -> Self {
        DEFAULT_CLOSED
    }
}

/// Parse a closed window, modeled on [`crate::config::parse_cadence`]:
/// `true` or `all` for everything, `false` or `none` for nothing, a
/// suffixed duration (`7d`, `12h`, `2w`) for a span, and a bare integer
/// for a count. Anything else is `None`, and the caller refuses.
///
/// The suffix grammar is the one this project already has,
/// `config::parse_duration`, so `30s` parses here too — a harmless
/// superset, and better than a second spelling of the same thing. A bare
/// integer never reaches it: here a number is a count, where the cadence
/// grammar reads it as days.
pub fn parse_closed(raw: &str) -> Option<ClosedWindow> {
    let raw = raw.trim();
    match raw.to_ascii_lowercase().as_str() {
        "true" | "all" => return Some(ClosedWindow::All),
        "false" | "none" => return Some(ClosedWindow::None),
        _ => {}
    }
    if raw.ends_with(['s', 'm', 'h', 'd', 'w']) {
        return crate::config::parse_duration(raw).map(ClosedWindow::Span);
    }
    raw.parse().ok().map(ClosedWindow::Count)
}

/// The derived model as an envelope: the inbox, then one group per
/// status in lifecycle order, then what the fold could not route.
///
/// A flight appears in exactly one status group — the one its `status`
/// field names — and a status string this binary has never heard of
/// routes nowhere rather than being invented into a group. `closed`
/// carries done and canceled for as much of the [`ClosedWindow`] the
/// caller asked for; the log keeps the rest.
#[derive(Debug, Serialize)]
pub struct Board {
    pub waiting_on_you: WaitingOnYou,
    pub triage: Vec<FlightView>,
    pub waiting: Vec<FlightView>,
    pub ready: Vec<FlightView>,
    pub in_progress: Vec<FlightView>,
    pub held: Vec<FlightView>,
    /// Done and canceled, newest first, cut to the [`ClosedWindow`].
    pub closed: Vec<FlightView>,
    pub unrouted: Vec<Event>,
    /// Kinds tower retired: carried for the machine envelope, and never
    /// warned about, because no command routes them.
    pub retired: Vec<Event>,
}

/// Pinned above the status groups: what needs a person now. A view of
/// the same rows — a flight here still appears in its status group.
#[derive(Debug, Serialize)]
pub struct WaitingOnYou {
    /// Held with an open question — an agent is stopped on you. Oldest
    /// ask first, so the longest-blocked agent takes the top row.
    pub questions: Vec<FlightView>,
    /// Ready in the `me` lane — the todo list. Narrower than
    /// `Picks::yours`, which counts every Ready flight outside the agent
    /// lane: an unassigned flight is nobody's claim, and it still stands
    /// in the `ready` group.
    pub yours: Vec<FlightView>,
}

/// One flight, as a render sees it.
#[derive(Debug, Clone, Serialize)]
pub struct FlightView {
    pub id: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    /// Provenance only: the procedure the filing was minted under, or
    /// the pass routed it under.
    pub procedure: Option<String>,
    pub subject: String,
    /// The filing's prose, verbatim. Carried on the row because the
    /// query filters over it — `body=contains:…` cannot be answered
    /// from a row that has no body — and never rendered as a column.
    pub body: String,
    pub filed_by: String,
    /// Raw epoch; relative age is the render's concern.
    pub filed_at: i64,
    pub comments: usize,
    pub depends_on: Vec<String>,
    pub blocks: Vec<String>,
    /// The derived status: the fold's projection of the facts a status
    /// word assigns, the open question, and the edges.
    pub status: String,
    /// Who made the gesture the status rests on, and when — the mover,
    /// the asker, the answerer, or the closer of the dependency that
    /// released it. `null` while the flight still stands where it was
    /// filed.
    pub status_by: Option<String>,
    pub status_at: Option<i64>,
    /// Why the mark is someone else's gesture: "dependency <id> done"
    /// or "… canceled" when a dependency's closing made the flight
    /// Ready. `null` when the mark is the flight's own.
    pub status_reason: Option<String>,
    pub assignee: Option<String>,
    pub priority: String,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    /// The bay a filing or an edit stamped this flight for — a stored
    /// field, and not the pool's own answer about occupancy.
    pub bay: Option<String>,
    /// `@detached` is a real literal value here, carried as fufu emitted
    /// it; a render decides how to print it.
    pub branch: Option<String>,
    /// That branch's tip, when the branch resolves to a row in the index.
    pub tip: Option<String>,
    /// The freshest session-tagged capture on this flight's branch — the
    /// repository's fact, and nothing the record itself did.
    pub last_change: Option<i64>,
    /// In Progress, and the branch has not changed for the threshold.
    pub stale: bool,
    /// Ready, and the branch changed after the flight was set Ready.
    pub changed_since_ready: bool,
    /// Closed children over total, whenever this flight has children at
    /// all — what a render prints as `(2/6)`.
    pub progress: Option<(usize, usize)>,
    pub held: bool,
    pub resolving: bool,
    /// The branch is the one this render's own worktree sits on.
    pub current: bool,
    /// The open question tower's own `hold` attached — distinct from
    /// `held`/`resolving`, which stay fufu's branch verdicts.
    pub question: Option<String>,
    pub asked_at: Option<i64>,
    /// Flights this one would conflict with, and where. Filed order.
    pub collides: Vec<CollideView>,
    /// Flights whose pairing fufu could not judge — unknown never rounds
    /// down to clear. Filed order.
    pub unanswered: Vec<String>,
}

/// One discovered conflict, as a flight's row carries it.
#[derive(Debug, Clone, Serialize)]
pub struct CollideView {
    /// The other flight's id.
    pub with: String,
    /// fufu's verdict, verbatim.
    pub paths: Vec<String>,
}

/// The fold's flights as flat rows, enriched with what the repository
/// knows — everything [`enrich`] does before it sections anything.
///
/// The flat half exists on its own because two folds read it: the
/// board's sectioning below, and [`Query::fold`](super::Query::fold),
/// which groups the same rows by whatever axis a query names. Rows
/// arrive in filed order, which every sort downstream is stable over.
#[derive(Debug)]
pub struct Rows {
    pub flights: Vec<FlightView>,
    pub unrouted: Vec<Event>,
    pub retired: Vec<Event>,
}

/// Enrich every folded flight into a row, routing nothing.
///
/// `now` and `stale_after` are arguments so the module stays pure and
/// reads no clock and no config: rows are a function of their inputs,
/// and `stale_after` of `0` turns the stale line off entirely.
///
/// Every flight is enriched the same way — branch from the freshest op
/// row (`@detached` carried literal), tip and holds from the branch row,
/// `last_change` from the op row's time.
///
/// Verdicts land on every live flight against every other live flight on
/// a distinct branch — the facts are orthogonal to any grouping.
/// `Collide` becomes a `collides` entry, `Unknown` an `unanswered` one,
/// and `Clear` or an unprobed pair adds nothing; entries keep filed
/// order, so a render is deterministic.
pub fn rows(fold: Fold, reads: &Reads, verdicts: &Verdicts, now: i64, stale_after: i64) -> Rows {
    let freshest = reads.freshest();
    let branches = reads.branch_index();

    // The live flight-to-branch assignments, in filed order — what each
    // flight's verdicts are computed against.
    let assignments: Vec<(String, String)> = fold
        .flights
        .iter()
        .filter(|flight| !flight.closed())
        .filter_map(|flight| {
            let id = flight.id.to_string();
            let branch = freshest.get(id.as_str())?.branch.as_deref()?;
            if branch == "@detached" {
                return None;
            }
            Some((id, branch.to_string()))
        })
        .collect();

    // The progress marks and the since lines, taken before the flights
    // are consumed and carried as owned rows.
    let marks: HashMap<String, (usize, usize)> = fold
        .flights
        .iter()
        .filter_map(|flight| Some((flight.id.to_string(), progress(&fold, flight)?)))
        .collect();
    let mut reasons: HashMap<String, String> = fold
        .flights
        .iter()
        .filter_map(|flight| Some((flight.id.to_string(), status_reason(&fold, flight)?)))
        .collect();

    let mut flights = Vec::with_capacity(fold.flights.len());
    for flight in fold.flights {
        let id = flight.id.to_string();
        let op = freshest.get(id.as_str()).copied();
        let row = op.and_then(|op| {
            op.branch
                .as_deref()
                .filter(|name| *name != "@detached")
                .and_then(|name| branches.get(name).copied())
        });
        let last_change = op.map(|op| op.time);
        let status_at = flight.status_mark.as_ref().map(|mark| mark.at);
        let is_closed = flight.closed();

        // The first audit: In Progress and the branch has forgotten it.
        // With no capture at all the clock runs from the move itself —
        // a flight declared In Progress and never touched is exactly the
        // case the line exists for.
        let stale = flight.status == "in_progress"
            && stale_after > 0
            && now - last_change.or(status_at).unwrap_or(flight.filed_at) >= stale_after;
        // The second: Ready, and the branch moved *after* it was set
        // Ready. After, not merely at all — the answer that releases a
        // hold is the Ready mark, so every resumed hold carries a full
        // branch and a flat check would be pure noise.
        let changed_since_ready = flight.status == "ready"
            && matches!((last_change, status_at), (Some(change), Some(set)) if change > set);

        let mut collides = Vec::new();
        let mut unanswered = Vec::new();
        if !is_closed && let Some(branch) = op.and_then(|op| op.branch.as_deref()) {
            for (other, theirs) in &assignments {
                if *other == id || theirs == branch {
                    continue;
                }
                match verdicts.between(branch, theirs) {
                    Some(Pairing::Collide { paths }) => collides.push(CollideView {
                        with: other.clone(),
                        paths: paths.clone(),
                    }),
                    Some(Pairing::Unknown { .. }) => unanswered.push(other.clone()),
                    Some(Pairing::Clear) | None => {}
                }
            }
        }

        let mut view = view(
            flight,
            reasons.remove(&id),
            op.and_then(|op| op.branch.clone()),
            row.and_then(|row| row.tip.clone()),
            row.is_some_and(|row| row.held),
            row.is_some_and(|row| row.resolving),
            reads.current_branch.as_deref(),
        );
        view.last_change = last_change;
        view.stale = stale;
        view.changed_since_ready = changed_since_ready;
        view.collides = collides;
        view.unanswered = unanswered;
        view.progress = marks.get(&id).copied();
        flights.push(view);
    }

    Rows {
        flights,
        unrouted: fold.unrouted,
        retired: fold.retired,
    }
}

/// Group the fold's flights by their derived status, using
/// already-fetched reads.
///
/// `now`, `stale_after`, and `closed` are arguments so the module stays
/// pure and reads no clock, no config, and no command line: a board is a
/// function of its inputs, and `stale_after` of `0` turns the stale line
/// off entirely.
///
/// [`rows`] does the enrichment; what happens here is the routing. A row
/// lands in the group its `status` field names and a status string this
/// binary has never heard of routes nowhere rather than being invented
/// into a group. The inbox is a second view over the same rows: an open
/// question puts a flight in `questions`, Ready in the `me` lane puts it
/// in `yours`, and both keep their place in the status group below.
///
/// A sub-flight is a flight: it lands in its own status group beside
/// every other row, and nothing about having a parent moves or hides it.
/// What says a row is a family is the parent's progress mark, closed
/// children over total, which every parent carries. The family itself is
/// the projects view's shape, not this list's.
pub fn enrich(
    fold: Fold,
    reads: &Reads,
    verdicts: &Verdicts,
    now: i64,
    stale_after: i64,
    closed: ClosedWindow,
) -> Board {
    let rows = rows(fold, reads, verdicts, now, stale_after);

    let mut inbox = WaitingOnYou {
        questions: Vec::new(),
        yours: Vec::new(),
    };
    let mut triage = Vec::new();
    let mut waiting = Vec::new();
    let mut ready = Vec::new();
    let mut in_progress = Vec::new();
    let mut held = Vec::new();
    let mut group = Vec::new();
    for view in rows.flights {
        // The inbox, live rows only: a closed flight needs nobody, and
        // `done` does not clear a question the log still carries.
        let live = !closed_row(&view);
        let questioned = live && view.question.is_some();
        let mine = live && view.status == "ready" && view.assignee.as_deref() == Some("me");
        if questioned {
            inbox.questions.push(view.clone());
        } else if mine {
            inbox.yours.push(view.clone());
        }

        match view.status.as_str() {
            "triage" => triage.push(view),
            "waiting" => waiting.push(view),
            "ready" => ready.push(view),
            "in_progress" => in_progress.push(view),
            "held" => held.push(view),
            "done" | "canceled" => group.push(view),
            // A status this binary has never heard of routes nowhere.
            // Inventing a group for it would be the fold's tolerance
            // spent on a guess.
            _ => {}
        }
    }

    inbox.questions.sort_by_key(|view| view.asked_at);
    order(&mut inbox.yours);
    for group in [
        &mut triage,
        &mut waiting,
        &mut ready,
        &mut in_progress,
        &mut held,
    ] {
        order(group);
    }
    // The window is applied after the sort, never before: `Count` means
    // the newest n, and a truncation of what arrived first would answer
    // a different question.
    group.sort_by_key(|view| std::cmp::Reverse(closed_at(view)));
    match closed {
        ClosedWindow::All => {}
        ClosedWindow::None => group.clear(),
        ClosedWindow::Count(n) => group.truncate(n),
        ClosedWindow::Span(secs) => group.retain(|view| now - closed_at(view) <= secs),
    }

    Board {
        waiting_on_you: inbox,
        triage,
        waiting,
        ready,
        in_progress,
        held,
        closed: group,
        unrouted: rows.unrouted,
        retired: rows.retired,
    }
}

/// The since line under a derived Ready: the dependency whose closing
/// is the status mark, and how it closed — "dependency pi.2 done".
pub(super) fn status_reason(fold: &Fold, flight: &Flight) -> Option<String> {
    let dep = flight.status_dep.as_ref()?;
    let closed = fold
        .flights
        .iter()
        .find(|other| &other.id == dep)?
        .stand
        .closed
        .as_deref()?;
    Some(format!("dependency {dep} {closed}"))
}

/// Closed children over total, or `None` for a flight with no children.
/// Canceled counts as closed: the part is over, whatever it concluded.
pub(super) fn progress(fold: &Fold, flight: &Flight) -> Option<(usize, usize)> {
    if flight.depends_on.is_empty() {
        return None;
    }
    let closed = flight
        .depends_on
        .iter()
        .filter(|child| {
            fold.flights
                .iter()
                .find(|other| &other.id == *child)
                .is_some_and(Flight::closed)
        })
        .count();
    Some((closed, flight.depends_on.len()))
}

/// Off the board: done or canceled — [`Flight::closed`] read off the
/// row rather than the fold, for the surfaces that only have rows.
pub(super) fn closed_row(view: &FlightView) -> bool {
    view.status == "done" || view.status == "canceled"
}

/// When a closed flight closed: the status move that closed it, or the
/// filing for a flight that arrived closed.
pub(super) fn closed_at(view: &FlightView) -> i64 {
    view.status_at.unwrap_or(view.filed_at)
}

/// Within a group: priority first, then age oldest-first. The sort is
/// stable and the fold hands flights over in filed order, so equal rows
/// keep it.
fn order(views: &mut [FlightView]) {
    views.sort_by(|a, b| {
        rank(&a.priority)
            .cmp(&rank(&b.priority))
            .then(a.filed_at.cmp(&b.filed_at))
    });
}

/// The priority vocabulary, urgent first. A word this binary has never
/// heard of sorts after `none` rather than being invented into the middle
/// of the ladder.
pub(super) fn rank(priority: &str) -> u8 {
    match priority {
        "urgent" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        "none" => 4,
        _ => 5,
    }
}

fn view(
    flight: Flight,
    status_reason: Option<String>,
    branch: Option<String>,
    tip: Option<String>,
    held: bool,
    resolving: bool,
    current_branch: Option<&str>,
) -> FlightView {
    let current = match (branch.as_deref(), current_branch) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };
    let (question, asked_at) = match flight.question {
        Some(question) => (Some(question.text), Some(question.at)),
        None => (None, None),
    };
    FlightView {
        id: flight.id.to_string(),
        number: flight.number,
        procedure: flight.procedure,
        subject: flight.subject,
        body: flight.body,
        filed_by: flight.filed_by,
        filed_at: flight.filed_at,
        comments: flight.comments.len(),
        depends_on: flight.depends_on.iter().map(ToString::to_string).collect(),
        blocks: flight.blocks.iter().map(ToString::to_string).collect(),
        status: flight.status,
        status_by: flight.status_mark.as_ref().map(|mark| mark.by.clone()),
        status_at: flight.status_mark.as_ref().map(|mark| mark.at),
        status_reason,
        assignee: flight.assignee,
        priority: flight.priority,
        labels: flight.labels,
        skill: flight.skill,
        bay: flight.bay,
        branch,
        tip,
        last_change: None,
        stale: false,
        changed_since_ready: false,
        progress: None,
        held,
        resolving,
        current,
        question,
        asked_at,
        collides: Vec::new(),
        unanswered: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::super::reads::BranchPairing;
    use super::*;
    use crate::ff::{BranchInfo, BranchList, OpEntry};
    use crate::log::{Event, EventId, Kind};

    /// The tests' clock: far enough past every fixture time that an age
    /// is whatever the fixture says it is.
    const NOW: i64 = 1_000_000;
    /// The registry's default, in seconds.
    const TWO_DAYS: i64 = 2 * 24 * 60 * 60;

    fn filing(status: &str, priority: &str, assignee: Option<&str>, subject: &str) -> Kind {
        Kind::Filed {
            procedure: None,
            subject: subject.to_string(),
            body: String::new(),
            status: status.to_string(),
            assignee: assignee.map(str::to_string),
            priority: priority.to_string(),
            labels: Vec::new(),
            skill: None,
            bay: None,
            done: "asserted".to_string(),
            branch: None,
        }
    }

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

    fn filed(id: &str, time: i64) -> Event {
        event(
            id,
            time,
            filing("triage", "none", None, &format!("subject of {time}")),
        )
    }

    /// A filing carrying stored fields the grouping and the ordering read.
    fn filed_as(
        id: &str,
        time: i64,
        status: &str,
        priority: &str,
        assignee: Option<&str>,
    ) -> Event {
        event(
            id,
            time,
            filing(status, priority, assignee, &format!("subject of {time}")),
        )
    }

    fn subjected(id: &str, time: i64, subject: &str) -> Event {
        event(id, time, filing("triage", "none", None, subject))
    }

    fn lifecycle(id: &str, time: i64, kind: Kind) -> Event {
        event(id, time, kind)
    }

    fn moved(id: &str, time: i64, flight: &str, to: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
            },
        )
    }

    fn held(id: &str, time: i64, flight: &str, question: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Held {
                flight: flight.parse().expect("id"),
                question: question.to_string(),
            },
        )
    }

    fn answered(id: &str, time: i64, flight: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Answered {
                flight: flight.parse().expect("id"),
                answer: "an answer".to_string(),
            },
        )
    }

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
            },
        )
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        moved(id, time, flight, "done")
    }

    fn op(session: &str, branch: Option<&str>, time: i64) -> OpEntry {
        OpEntry {
            branch: branch.map(str::to_string),
            session: Some(session.to_string()),
            time,
        }
    }

    fn branch(name: &str, held: bool, resolving: bool) -> BranchInfo {
        BranchInfo {
            name: name.to_string(),
            tip: Some("3c8f91686a9e35a10ae8ebb6f0d6f9bbbfdd6940".to_string()),
            held,
            resolving,
        }
    }

    fn reads(ops: Vec<OpEntry>, named: Vec<BranchInfo>, current: Option<&str>) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named,
                anonymous: Vec::new(),
            },
            current_branch: current.map(str::to_string),
            worktrees: Vec::new(),
            orphans: Vec::new(),
        }
    }

    /// The common shape: no threshold, so the stale line never fires
    /// where a test is not about it.
    fn board(events: &[Event], reads: &Reads) -> Board {
        enrich(
            fold(events),
            reads,
            &Verdicts::default(),
            NOW,
            0,
            ClosedWindow::default(),
        )
    }

    /// The same board with the closed window named, where a test is
    /// about the window itself.
    fn windowed(events: &[Event], reads: &Reads, closed: ClosedWindow) -> Board {
        enrich(fold(events), reads, &Verdicts::default(), NOW, 0, closed)
    }

    fn ids(views: &[FlightView]) -> Vec<&str> {
        views.iter().map(|view| view.id.as_str()).collect()
    }

    #[test]
    fn every_status_routes_to_its_own_group_and_nothing_from_the_reads_moves_it() {
        // Waiting and Held are never words a filing sets: the edge and
        // the question are what put a row in those groups.
        let board = board(
            &[
                filed_as("pi.1", 10, "triage", "none", None),
                filed_as("pi.2", 20, "ready", "none", None),
                filed_as("pi.3", 30, "ready", "none", None),
                filed_as("pi.4", 40, "in_progress", "none", None),
                filed_as("pi.5", 50, "ready", "none", None),
                linked("pi.6", 60, "pi.2", "pi.1"),
                held("pi.7", 70, "pi.5", "which?"),
            ],
            // A held branch under every one of them: fufu's verdict is a
            // fact on the row, not a section.
            &reads(
                vec![op("pi.1", Some("work"), 60)],
                vec![branch("work", true, true)],
                None,
            ),
        );
        assert_eq!(ids(&board.triage), ["pi.1"]);
        assert_eq!(ids(&board.waiting), ["pi.2"]);
        assert_eq!(ids(&board.ready), ["pi.3"]);
        assert_eq!(ids(&board.in_progress), ["pi.4"]);
        assert_eq!(ids(&board.held), ["pi.5"]);
        assert!(board.closed.is_empty());
        assert!(board.triage[0].held && board.triage[0].resolving);
    }

    #[test]
    fn an_unknown_status_routes_nowhere() {
        let board = board(
            &[filed("pi.1", 10), moved("pi.2", 20, "pi.1", "parked")],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert!(board.triage.is_empty());
        assert!(board.waiting.is_empty());
        assert!(board.ready.is_empty());
        assert!(board.in_progress.is_empty());
        assert!(board.held.is_empty());
        assert!(board.closed.is_empty());
    }

    #[test]
    fn a_group_sorts_by_priority_then_oldest_first() {
        let board = board(
            &[
                filed_as("pi.1", 10, "triage", "low", None),
                filed_as("pi.2", 20, "triage", "urgent", None),
                filed_as("pi.3", 30, "triage", "none", None),
                filed_as("pi.4", 40, "triage", "high", None),
                filed_as("pi.5", 50, "triage", "urgent", None),
                filed_as("pi.6", 60, "triage", "medium", None),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(
            ids(&board.triage),
            ["pi.2", "pi.5", "pi.4", "pi.6", "pi.1", "pi.3"],
            "urgent oldest-first, then high, medium, low, none"
        );
    }

    #[test]
    fn an_unknown_priority_sorts_after_none() {
        let board = board(
            &[
                filed_as("pi.1", 10, "triage", "blocker", None),
                filed_as("pi.2", 20, "triage", "none", None),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(ids(&board.triage), ["pi.2", "pi.1"]);
    }

    #[test]
    fn the_closed_group_carries_the_three_newest_by_default() {
        let board = board(
            &[
                filed("pi.1", 10),
                filed("pi.2", 20),
                filed("pi.3", 30),
                filed("pi.4", 40),
                done("pi.5", NOW - 3_600, "pi.1"),
                moved("pi.6", NOW - 60, "pi.2", "canceled"),
                done("pi.7", NOW - 600, "pi.3"),
                done("pi.8", NOW - 10, "pi.4"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(
            ids(&board.closed),
            ["pi.4", "pi.2", "pi.3"],
            "the three newest, newest first — the fourth is dropped by the count, \
             minutes old though it is"
        );
        assert_eq!(board.closed[1].status, "canceled");
        assert!(board.triage.is_empty(), "a closed flight leaves its group");
    }

    #[test]
    fn all_carries_every_closed_flight_and_none_carries_no_group() {
        // Closed at 100, older than any span a person would type.
        let events = [
            filed("pi.1", 10),
            filed("pi.2", 20),
            done("pi.3", 100, "pi.1"),
            done("pi.4", NOW - 60, "pi.2"),
        ];
        let reads = reads(Vec::new(), Vec::new(), None);

        let board = windowed(&events, &reads, ClosedWindow::All);
        assert_eq!(ids(&board.closed), ["pi.2", "pi.1"]);

        let board = windowed(&events, &reads, ClosedWindow::None);
        assert!(board.closed.is_empty());
    }

    #[test]
    fn a_count_takes_the_newest_and_not_the_first_seen() {
        // Filed order is the reverse of closed order on purpose: a
        // truncation before the sort would keep pi.1 and pi.2.
        let events = [
            filed("pi.1", 10),
            filed("pi.2", 20),
            filed("pi.3", 30),
            done("pi.4", NOW - 3_000, "pi.1"),
            done("pi.5", NOW - 2_000, "pi.2"),
            done("pi.6", NOW - 1_000, "pi.3"),
        ];
        let reads = reads(Vec::new(), Vec::new(), None);

        let board = windowed(&events, &reads, ClosedWindow::Count(2));
        assert_eq!(ids(&board.closed), ["pi.3", "pi.2"]);

        let board = windowed(&events, &reads, ClosedWindow::Count(0));
        assert!(board.closed.is_empty());
    }

    #[test]
    fn a_span_keeps_what_closed_inside_it() {
        const DAY: i64 = 24 * 60 * 60;
        let board = windowed(
            &[
                filed("pi.1", 10),
                filed("pi.2", 20),
                done("pi.3", NOW - DAY + 1, "pi.1"),
                done("pi.4", NOW - DAY - 1, "pi.2"),
            ],
            &reads(Vec::new(), Vec::new(), None),
            ClosedWindow::Span(DAY),
        );
        assert_eq!(
            ids(&board.closed),
            ["pi.1"],
            "a second inside the edge stays, a second past it goes"
        );
    }

    #[test]
    fn parse_closed_reads_the_words_the_counts_and_the_spans() {
        assert_eq!(parse_closed("true"), Some(ClosedWindow::All));
        assert_eq!(parse_closed("TRUE"), Some(ClosedWindow::All));
        assert_eq!(parse_closed(" all "), Some(ClosedWindow::All));
        assert_eq!(parse_closed("false"), Some(ClosedWindow::None));
        assert_eq!(parse_closed("none"), Some(ClosedWindow::None));
        assert_eq!(
            parse_closed("10"),
            Some(ClosedWindow::Count(10)),
            "a bare number is a count of rows, and never ten days"
        );
        assert_eq!(parse_closed("0"), Some(ClosedWindow::Count(0)));
        assert_eq!(
            parse_closed("7d"),
            Some(ClosedWindow::Span(7 * 24 * 60 * 60))
        );
        assert_eq!(parse_closed("12h"), Some(ClosedWindow::Span(12 * 60 * 60)));
        assert_eq!(parse_closed("90m"), Some(ClosedWindow::Span(90 * 60)));
        assert_eq!(
            parse_closed("2w"),
            Some(ClosedWindow::Span(2 * 7 * 24 * 60 * 60))
        );
    }

    #[test]
    fn a_value_the_closed_grammar_does_not_cover_parses_to_nothing() {
        for raw in ["", "soon", "-1", "3x"] {
            assert_eq!(parse_closed(raw), None, "{raw}");
        }
    }

    #[test]
    fn the_inbox_holds_questions_oldest_first_and_the_me_lane() {
        let board = board(
            &[
                filed("pi.1", 10),
                filed("pi.2", 20),
                filed_as("pi.3", 30, "ready", "none", Some("me")),
                filed_as("pi.4", 40, "ready", "none", Some("agent")),
                filed_as("pi.5", 50, "ready", "none", None),
                held("pi.6", 70, "pi.2", "later"),
                held("pi.7", 60, "pi.1", "sooner"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(ids(&board.waiting_on_you.questions), ["pi.1", "pi.2"]);
        assert_eq!(
            ids(&board.waiting_on_you.yours),
            ["pi.3"],
            "the agent lane and the unassigned stay out"
        );
        assert_eq!(
            ids(&board.held),
            ["pi.1", "pi.2"],
            "the inbox is a view: the rows keep their group"
        );
        assert_eq!(ids(&board.ready), ["pi.3", "pi.4", "pi.5"]);
    }

    #[test]
    fn a_closed_flight_with_a_question_still_on_the_record_stays_out_of_the_inbox() {
        let board = board(
            &[
                filed("pi.1", 10),
                held("pi.2", 20, "pi.1", "which?"),
                done("pi.3", NOW - 60, "pi.1"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert!(board.waiting_on_you.questions.is_empty());
        assert_eq!(ids(&board.closed), ["pi.1"]);
    }

    #[test]
    fn last_change_is_the_op_row_alone_and_no_record_gesture() {
        // The audit's whole point: commenting on a stalled flight, or
        // moving its status, must not silence its own line.
        let board = board(
            &[
                filed("pi.1", 10),
                moved("pi.2", NOW - 10, "pi.1", "in_progress"),
                lifecycle(
                    "pi.3",
                    NOW - 5,
                    Kind::Edited {
                        target: "pi.1".parse().expect("id"),
                        subject: Some("reworded".to_string()),
                        body: None,
                        priority: None,
                        labels: None,
                        skill: None,
                        bay: None,
                    },
                ),
            ],
            &reads(
                vec![op("pi.1", Some("work"), NOW - 5_000)],
                vec![branch("work", false, false)],
                None,
            ),
        );
        assert_eq!(board.in_progress[0].last_change, Some(NOW - 5_000));
    }

    #[test]
    fn an_in_progress_flight_the_branch_forgot_is_stale() {
        let events = [
            filed("pi.1", 10),
            moved("pi.2", NOW - TWO_DAYS - 10, "pi.1", "in_progress"),
        ];
        let reads = reads(
            vec![op("pi.1", Some("work"), NOW - TWO_DAYS - 5)],
            vec![branch("work", false, false)],
            None,
        );
        let board = enrich(
            fold(&events),
            &reads,
            &Verdicts::default(),
            NOW,
            TWO_DAYS,
            ClosedWindow::default(),
        );
        assert!(board.in_progress[0].stale);

        // The threshold off: the same board says nothing.
        let board = enrich(
            fold(&events),
            &reads,
            &Verdicts::default(),
            NOW,
            0,
            ClosedWindow::default(),
        );
        assert!(!board.in_progress[0].stale);
    }

    #[test]
    fn an_in_progress_flight_never_captured_runs_the_clock_from_the_move() {
        let events = [
            filed("pi.1", 10),
            moved("pi.2", NOW - TWO_DAYS - 1, "pi.1", "in_progress"),
        ];
        let board = enrich(
            fold(&events),
            &reads(Vec::new(), Vec::new(), None),
            &Verdicts::default(),
            NOW,
            TWO_DAYS,
            ClosedWindow::default(),
        );
        assert!(board.in_progress[0].stale);
        assert!(board.in_progress[0].last_change.is_none());
    }

    #[test]
    fn an_edit_does_not_clear_staleness() {
        let board = enrich(
            fold(&[
                filed("pi.1", 10),
                moved("pi.2", NOW - TWO_DAYS - 10, "pi.1", "in_progress"),
                lifecycle(
                    "pi.3",
                    NOW - 1,
                    Kind::Edited {
                        target: "pi.1".parse().expect("id"),
                        subject: Some("reworded".to_string()),
                        body: None,
                        priority: None,
                        labels: None,
                        skill: None,
                        bay: None,
                    },
                ),
            ]),
            &reads(
                vec![op("pi.1", Some("work"), NOW - TWO_DAYS - 5)],
                vec![branch("work", false, false)],
                None,
            ),
            &Verdicts::default(),
            NOW,
            TWO_DAYS,
            ClosedWindow::default(),
        );
        assert!(board.in_progress[0].stale, "a reword is not a change");
    }

    #[test]
    fn a_ready_flight_the_branch_moved_under_says_so() {
        let board = board(
            &[filed("pi.1", 10), moved("pi.2", 100, "pi.1", "ready")],
            &reads(
                vec![op("pi.1", Some("work"), 200)],
                vec![branch("work", false, false)],
                None,
            ),
        );
        assert!(board.ready[0].changed_since_ready);
    }

    #[test]
    fn a_resumed_hold_does_not_flag_changes_since_ready() {
        // The answer is the Ready mark of a released hold, so the branch
        // is full of the work that preceded the question. Only a change
        // *after* the release is news.
        let board = board(
            &[
                filed("pi.1", 10),
                moved("pi.2", 50, "pi.1", "ready"),
                held("pi.3", 100, "pi.1", "which?"),
                answered("pi.4", 300, "pi.1"),
            ],
            &reads(
                vec![op("pi.1", Some("work"), 200)],
                vec![branch("work", false, false)],
                None,
            ),
        );
        assert_eq!(board.ready[0].status, "ready");
        assert_eq!(board.ready[0].status_at, Some(300));
        assert!(!board.ready[0].changed_since_ready);
    }

    #[test]
    fn a_canceled_child_lifts_the_parent_to_ready_with_the_reason() {
        let board = board(
            &[
                filed_as("pi.1", 10, "ready", "none", None),
                filed("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
                moved("pi.4", 40, "pi.2", "canceled"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(ids(&board.waiting), [] as [&str; 0]);
        let parent = &board.ready[0];
        assert_eq!(parent.id, "pi.1");
        assert_eq!(parent.status_at, Some(40));
        assert_eq!(
            parent.status_reason.as_deref(),
            Some("dependency pi.2 canceled")
        );
        assert_eq!(parent.progress, Some((1, 1)));
        assert_eq!(board.closed[0].id, "pi.2");
    }

    /// The flat board: having a parent moves a flight nowhere. Every
    /// generation files into the group its own status names — the
    /// middle one Waiting, because its edge to the live leaf is what the
    /// fold derives from — and the family is a view over these same
    /// rows rather than a filter on them.
    #[test]
    fn a_sub_flight_lands_in_its_own_status_group_beside_its_parent() {
        let board = board(
            &[
                subjected("pi.1", 10, "top"),
                event(
                    "pi.2",
                    20,
                    filing("ready", "none", Some("agent"), "top · middle"),
                ),
                subjected("pi.3", 30, "leaf"),
                linked("pi.4", 40, "pi.1", "pi.2"),
                linked("pi.5", 50, "pi.2", "pi.3"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(
            ids(&board.triage),
            ["pi.1", "pi.3"],
            "the parent and the grandchild, filed order within the group"
        );
        assert_eq!(ids(&board.waiting), ["pi.2"], "the child on its own row");
        assert_eq!(
            board.waiting[0].subject, "top · middle",
            "the subject is the stored one — nothing is prefixed onto it"
        );
    }

    /// A parent keeps its mark, which is what still says the row is a
    /// family, and a closed child is a row in the closed group like any
    /// other closed flight.
    #[test]
    fn a_parent_keeps_its_progress_mark() {
        let board = board(
            &[
                subjected("pi.1", 10, "a broad task"),
                subjected("pi.2", 20, "part one"),
                subjected("pi.3", 30, "part two"),
                linked("pi.4", 40, "pi.1", "pi.2"),
                linked("pi.5", 50, "pi.1", "pi.3"),
                done("pi.6", NOW - 60, "pi.2"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(ids(&board.triage), ["pi.1", "pi.3"]);
        assert_eq!(board.triage[0].progress, Some((1, 2)));
        assert!(
            board.triage[1].progress.is_none(),
            "a child with no children of its own carries no mark"
        );
        assert_eq!(ids(&board.closed), ["pi.2"]);
    }

    /// The inbox is still a second view over the same rows, and a
    /// sub-flight reaches it on the same terms as any flight.
    #[test]
    fn a_questioned_sub_flight_reaches_the_inbox_and_its_group() {
        let board = board(
            &[
                subjected("pi.1", 10, "check the PR"),
                subjected("pi.2", 20, "check the PR · verdict"),
                linked("pi.3", 30, "pi.1", "pi.2"),
                held("pi.4", 40, "pi.2", "which flow wins?"),
            ],
            &reads(Vec::new(), Vec::new(), None),
        );
        assert_eq!(ids(&board.waiting_on_you.questions), ["pi.2"]);
        assert_eq!(ids(&board.held), ["pi.2"]);
        assert_eq!(ids(&board.triage), ["pi.1"]);
    }

    #[test]
    fn a_flight_with_no_children_carries_no_progress_mark() {
        let board = board(&[filed("pi.1", 10)], &reads(Vec::new(), Vec::new(), None));
        assert!(board.triage[0].progress.is_none());
    }

    #[test]
    fn a_tagged_flight_keeps_its_branch_tip_and_current_mark() {
        let board = board(
            &[filed("pi.1", 10)],
            &reads(
                vec![op("pi.1", Some("main"), 50)],
                vec![branch("main", false, false)],
                Some("main"),
            ),
        );
        let view = &board.triage[0];
        assert_eq!(view.id, "pi.1");
        assert_eq!(view.number, 1);
        assert_eq!(view.branch.as_deref(), Some("main"));
        assert_eq!(view.tip.as_deref().map(|t| &t[..8]), Some("3c8f9168"));
        assert_eq!(view.last_change, Some(50));
        assert!(view.current);
    }

    #[test]
    fn an_untouched_flight_carries_its_stored_fields_and_nothing_else() {
        let board = board(
            &[filed("pi.1", 10)],
            &reads(Vec::new(), Vec::new(), Some("main")),
        );
        let view = &board.triage[0];
        assert!(view.branch.is_none() && view.tip.is_none() && view.last_change.is_none());
        assert!(!view.held && !view.resolving && !view.current);
        assert!(!view.stale && !view.changed_since_ready);
        assert_eq!(view.status, "triage");
        assert!(view.status_by.is_none() && view.status_at.is_none());
        assert!(view.assignee.is_none());
        assert_eq!(view.priority, "none");
        assert!(view.labels.is_empty());
        assert!(view.skill.is_none());
        assert!(view.procedure.is_none());
    }

    #[test]
    fn a_detached_flight_is_never_held() {
        // Even with a held branch in the index — `@detached` names nothing
        // and must not accidentally resolve to a row.
        let board = board(
            &[filed("pi.1", 10)],
            &reads(
                vec![op("pi.1", Some("@detached"), 50)],
                vec![branch("@detached", true, true)],
                None,
            ),
        );
        let view = &board.triage[0];
        assert_eq!(view.branch.as_deref(), Some("@detached"));
        assert!(view.tip.is_none());
        assert!(!view.held && !view.resolving);
    }

    #[test]
    fn the_freshest_op_row_wins_per_tag() {
        let board = board(
            &[filed("pi.1", 10)],
            &reads(
                vec![op("pi.1", Some("old"), 40), op("pi.1", Some("new"), 60)],
                vec![branch("new", false, false)],
                None,
            ),
        );
        assert_eq!(board.triage[0].branch.as_deref(), Some("new"));
        assert_eq!(board.triage[0].last_change, Some(60));
    }

    #[test]
    fn a_question_keeps_the_bay_warm() {
        let board = board(
            &[filed("pi.1", 10), held("pi.2", 60, "pi.1", "which?")],
            &reads(
                vec![op("pi.1", Some("work"), 50)],
                vec![branch("work", true, false)],
                None,
            ),
        );
        let view = &board.waiting_on_you.questions[0];
        assert_eq!(view.question.as_deref(), Some("which?"));
        assert_eq!(view.asked_at, Some(60));
        assert_eq!(view.status, "held", "the hold is a status move too");
        assert_eq!(view.branch.as_deref(), Some("work"));
        assert!(view.tip.is_some());
        assert!(view.held);
    }

    fn pairing(a: &str, b: &str, pairing: Pairing) -> BranchPairing {
        BranchPairing {
            a: a.to_string(),
            b: b.to_string(),
            pairing,
        }
    }

    fn collide(a: &str, b: &str, paths: &[&str]) -> BranchPairing {
        pairing(
            a,
            b,
            Pairing::Collide {
                paths: paths.iter().map(|p| p.to_string()).collect(),
            },
        )
    }

    fn probed(events: &[Event], reads: &Reads, verdicts: Verdicts) -> Board {
        enrich(
            fold(events),
            reads,
            &verdicts,
            NOW,
            0,
            ClosedWindow::default(),
        )
    }

    #[test]
    fn a_collide_lands_on_both_flights_views_with_its_paths() {
        let board = probed(
            &[filed("pi.1", 10), filed("pi.2", 20)],
            &reads(
                vec![op("pi.1", Some("left"), 50), op("pi.2", Some("right"), 60)],
                vec![branch("left", false, false), branch("right", false, false)],
                None,
            ),
            Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
        );
        let one = board.triage.iter().find(|v| v.id == "pi.1").unwrap();
        let two = board.triage.iter().find(|v| v.id == "pi.2").unwrap();
        assert_eq!(one.collides.len(), 1);
        assert_eq!(one.collides[0].with, "pi.2");
        assert_eq!(one.collides[0].paths, ["shared.txt"]);
        assert_eq!(two.collides.len(), 1);
        assert_eq!(two.collides[0].with, "pi.1");
        assert!(one.unanswered.is_empty() && two.unanswered.is_empty());
    }

    #[test]
    fn an_unknown_pairing_is_unanswered_never_a_collide() {
        let board = probed(
            &[filed("pi.1", 10), filed("pi.2", 20)],
            &reads(
                vec![op("pi.1", Some("left"), 50), op("pi.2", Some("right"), 60)],
                vec![branch("left", false, false), branch("right", false, false)],
                None,
            ),
            Verdicts {
                pairs: vec![pairing(
                    "left",
                    "right",
                    Pairing::Unknown {
                        reason: crate::ff::UnknownReason::Other,
                    },
                )],
            },
        );
        for view in &board.triage {
            assert!(view.collides.is_empty());
            assert_eq!(view.unanswered.len(), 1);
        }
        let one = board.triage.iter().find(|v| v.id == "pi.1").unwrap();
        assert_eq!(one.unanswered, ["pi.2"]);
    }

    #[test]
    fn clear_and_unprobed_pairs_add_nothing() {
        let board = probed(
            &[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)],
            &reads(
                vec![
                    op("pi.1", Some("a"), 50),
                    op("pi.2", Some("b"), 60),
                    op("pi.3", Some("c"), 70),
                ],
                vec![
                    branch("a", false, false),
                    branch("b", false, false),
                    branch("c", false, false),
                ],
                None,
            ),
            // (a, b) clear; (a, c) and (b, c) never probed.
            Verdicts {
                pairs: vec![pairing("a", "b", Pairing::Clear)],
            },
        );
        for view in &board.triage {
            assert!(view.collides.is_empty(), "{view:?}");
            assert!(view.unanswered.is_empty(), "{view:?}");
        }
    }

    #[test]
    fn two_flights_on_one_branch_get_no_entries_against_each_other() {
        // A same-name verdict row would be a caller bug; even with one
        // present, same-branch neighbors are one tree and never listed.
        let board = probed(
            &[filed("pi.1", 10), filed("pi.2", 20)],
            &reads(
                vec![op("pi.1", Some("work"), 50), op("pi.2", Some("work"), 60)],
                vec![branch("work", false, false)],
                None,
            ),
            Verdicts {
                pairs: vec![collide("work", "work", &["shared.txt"])],
            },
        );
        for view in &board.triage {
            assert!(view.collides.is_empty());
            assert!(view.unanswered.is_empty());
        }
    }

    #[test]
    fn a_questioned_flight_keeps_its_collides() {
        let board = probed(
            &[
                filed("pi.1", 10),
                filed("pi.2", 20),
                held("pi.3", 70, "pi.1", "which?"),
            ],
            &reads(
                vec![op("pi.1", Some("left"), 50), op("pi.2", Some("right"), 60)],
                vec![branch("left", false, false), branch("right", false, false)],
                None,
            ),
            Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
        );
        assert_eq!(board.waiting_on_you.questions[0].collides[0].with, "pi.2");
        assert_eq!(board.triage[0].collides[0].with, "pi.1");
    }

    #[test]
    fn a_closed_flight_carries_no_collides() {
        let board = probed(
            &[
                filed("pi.1", 10),
                filed("pi.2", 20),
                done("pi.3", NOW - 60, "pi.1"),
            ],
            &reads(
                vec![op("pi.1", Some("left"), 50), op("pi.2", Some("right"), 60)],
                vec![branch("left", false, false), branch("right", false, false)],
                None,
            ),
            Verdicts {
                pairs: vec![collide("left", "right", &["shared.txt"])],
            },
        );
        assert!(board.closed[0].collides.is_empty());
        assert!(
            board.triage[0].collides.is_empty(),
            "a closed flight is not a live partner either"
        );
    }

    #[test]
    fn collide_entries_follow_filed_order() {
        let board = probed(
            &[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)],
            &reads(
                vec![
                    op("pi.1", Some("a"), 50),
                    op("pi.2", Some("b"), 60),
                    op("pi.3", Some("c"), 70),
                ],
                vec![
                    branch("a", false, false),
                    branch("b", false, false),
                    branch("c", false, false),
                ],
                None,
            ),
            // Rows deliberately out of filed order; the view's entries
            // follow the assignment list, not the verdict list.
            Verdicts {
                pairs: vec![collide("c", "b", &["y.txt"]), collide("a", "b", &["x.txt"])],
            },
        );
        let two = board.triage.iter().find(|v| v.id == "pi.2").unwrap();
        let withs: Vec<&str> = two.collides.iter().map(|c| c.with.as_str()).collect();
        assert_eq!(withs, ["pi.1", "pi.3"]);
    }
}

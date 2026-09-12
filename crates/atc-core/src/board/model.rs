//! The board: the fold's flights, flattened to rows and grouped by the
//! status each record derives.
//!
//! `enrich` is pure — it runs over the fold, plus the one scalar it
//! refuses to read for itself: `now`, which the closed window's span
//! reads. A flight sits in the group its derived status names — the
//! projection the fold already made over the stored facts, the open
//! question and the edges — and `enrich` moves it nowhere further.
//!
//! Above the groups sits the inbox — questions and yours — a view of the
//! same rows rather than a seventh group. Having a parent is not a
//! grouping fact either: a sub-flight is a flight, and it files beside
//! every other row.

use std::collections::HashMap;

use serde::Serialize;

use crate::log::Event;

use super::flight::{Flight, Fold};

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
/// caller asked for — one window's selection, which the render deals
/// into two sections — and the log keeps the rest.
#[derive(Debug, Serialize)]
pub struct Board {
    pub waiting_on_you: WaitingOnYou,
    pub backlog: Vec<FlightView>,
    pub waiting: Vec<FlightView>,
    pub ready: Vec<FlightView>,
    pub in_progress: Vec<FlightView>,
    pub held: Vec<FlightView>,
    /// Done and canceled, newest first, cut to the [`ClosedWindow`]: one
    /// window's selection, the JSON key `board --json` consumers read.
    /// The render deals it into a done section and a canceled one.
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
    /// Ready in the `me` lane or the viewer's own callsign's — the todo
    /// list. Narrower than `Picks::yours`, which counts every Ready
    /// flight outside the pool: an unassigned flight is nobody's claim,
    /// and it still stands in the `ready` group.
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
    /// a match rule chose at file time.
    pub procedure: Option<String>,
    pub subject: String,
    /// The filing's prose, verbatim. Carried on the row because the
    /// query filters over it — `body=contains:…` cannot be answered
    /// from a row that has no body — and never rendered as a column.
    pub body: String,
    pub filed_by: String,
    /// The filer's session, when the filing carried one.
    pub filed_session: Option<String>,
    /// The filer's callsign, when the filing carried one.
    pub filed_callsign: Option<String>,
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
    /// The mover's session, when that gesture carried one.
    pub status_session: Option<String>,
    /// The mover's callsign, when that gesture carried one — the pilot,
    /// what the In Progress note names first.
    pub status_callsign: Option<String>,
    pub status_at: Option<i64>,
    /// Why the mark is someone else's gesture: "dependency <id> done"
    /// or "… canceled" when a dependency's closing made the flight
    /// Ready. `null` when the mark is the flight's own.
    pub status_reason: Option<String>,
    pub assignee: Option<String>,
    /// Whether the lane is the viewer's: `me`, or the viewer's own
    /// callsign. Derived against the viewer at fold time, so the web's
    /// `for=me` and the CLI's inbox read one flag.
    pub mine: bool,
    pub priority: String,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    /// Closed children over total, whenever this flight has children at
    /// all — what a render prints as `(2/6)`.
    pub progress: Option<(usize, usize)>,
    /// The open question tower's own `hold` attached.
    pub question: Option<String>,
    pub asked_at: Option<i64>,
    /// A close's `-m` — a cancel's reason, most often — standing where
    /// the question stood. `null` while the flight is open or when the
    /// close said nothing.
    pub closed_reason: Option<String>,
}

/// The fold's flights as flat rows — everything [`enrich`] does before
/// it sections anything.
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

/// Flatten every folded flight into a row, routing nothing.
///
/// Every flight is flattened the same way: the stored fields, the derived
/// status and its mark, the progress mark, and the open question.
/// `viewer` is the reader's callsign, the one thing a row carries that
/// is relative to who is looking: `mine`.
pub fn rows(fold: Fold, viewer: Option<&str>) -> Rows {
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
        let mut view = view(flight, reasons.remove(&id), viewer);
        view.progress = marks.get(&id).copied();
        flights.push(view);
    }

    Rows {
        flights,
        unrouted: fold.unrouted,
        retired: fold.retired,
    }
}

/// Group the fold's flights by their derived status.
///
/// `now` and `closed` are arguments so the module stays pure and reads
/// no clock and no command line: a board is a function of its inputs.
///
/// [`rows`] does the flattening; what happens here is the routing. A row
/// lands in the group its `status` field names and a status string this
/// binary has never heard of routes nowhere rather than being invented
/// into a group. The inbox is a second view over the same rows: an open
/// question puts a flight in `questions`, Ready in the viewer's lane —
/// `me`, or their callsign — puts it in `yours`, and both keep their
/// place in the status group below.
///
/// A sub-flight is a flight: it lands in its own status group beside
/// every other row, and nothing about having a parent moves or hides it.
/// What says a row is a family is the parent's progress mark, closed
/// children over total, which every parent carries. The family itself is
/// the projects view's shape, not this list's.
pub fn enrich(fold: Fold, now: i64, closed: ClosedWindow, viewer: Option<&str>) -> Board {
    let rows = rows(fold, viewer);

    let mut inbox = WaitingOnYou {
        questions: Vec::new(),
        yours: Vec::new(),
    };
    let mut backlog = Vec::new();
    let mut waiting = Vec::new();
    let mut ready = Vec::new();
    let mut in_progress = Vec::new();
    let mut held = Vec::new();
    let mut group = Vec::new();
    for view in rows.flights {
        // The inbox: an open question, or Ready in the viewer's lane on
        // a live row — a closed flight needs nobody, and the fold already
        // took a close's question off the record.
        let live = !closed_row(&view);
        let questioned = view.question.is_some();
        let mine = live && view.status == "ready" && view.mine;
        if questioned {
            inbox.questions.push(view.clone());
        } else if mine {
            inbox.yours.push(view.clone());
        }

        match view.status.as_str() {
            "backlog" => backlog.push(view),
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
        &mut backlog,
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
        backlog,
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

fn view(flight: Flight, status_reason: Option<String>, viewer: Option<&str>) -> FlightView {
    let mine = flight.mine(viewer);
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
        filed_session: flight.filed_session,
        filed_callsign: flight.filed_callsign,
        filed_at: flight.filed_at,
        comments: flight.comments.len(),
        depends_on: flight.depends_on.iter().map(ToString::to_string).collect(),
        blocks: flight.blocks.iter().map(ToString::to_string).collect(),
        status: flight.status,
        status_by: flight.status_mark.as_ref().map(|mark| mark.by.clone()),
        status_session: flight
            .status_mark
            .as_ref()
            .and_then(|mark| mark.session.clone()),
        status_callsign: flight
            .status_mark
            .as_ref()
            .and_then(|mark| mark.callsign.clone()),
        status_at: flight.status_mark.as_ref().map(|mark| mark.at),
        status_reason,
        assignee: flight.assignee,
        mine,
        priority: flight.priority,
        labels: flight.labels,
        skill: flight.skill,
        progress: None,
        question,
        asked_at,
        closed_reason: flight.closed_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::log::{Event, EventId, Kind};

    /// The tests' clock: far enough past every fixture time that an age
    /// is whatever the fixture says it is.
    const NOW: i64 = 1_000_000;

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
            session: None,
            callsign: None,
            id,
            kind,
        }
    }

    fn filed(id: &str, time: i64) -> Event {
        event(
            id,
            time,
            filing("backlog", "none", None, &format!("subject of {time}")),
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
        event(id, time, filing("backlog", "none", None, subject))
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

    fn canceled(id: &str, time: i64, flight: &str, reason: &str) -> Event {
        lifecycle(
            id,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: "canceled".to_string(),
                reason: Some(reason.to_string()),
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

    /// The common shape: the default closed window.
    fn board(events: &[Event]) -> Board {
        enrich(fold(events), NOW, ClosedWindow::default(), None)
    }

    /// The same board with the closed window named, where a test is
    /// about the window itself.
    fn windowed(events: &[Event], closed: ClosedWindow) -> Board {
        enrich(fold(events), NOW, closed, None)
    }

    fn ids(views: &[FlightView]) -> Vec<&str> {
        views.iter().map(|view| view.id.as_str()).collect()
    }

    #[test]
    fn every_status_routes_to_its_own_group() {
        // Waiting and Held are never words a filing sets: the edge and
        // the question are what put a row in those groups.
        let board = board(&[
            filed_as("pi.1", 10, "backlog", "none", None),
            filed_as("pi.2", 20, "ready", "none", None),
            filed_as("pi.3", 30, "ready", "none", None),
            filed_as("pi.4", 40, "in_progress", "none", None),
            filed_as("pi.5", 50, "ready", "none", None),
            linked("pi.6", 60, "pi.2", "pi.1"),
            held("pi.7", 70, "pi.5", "which?"),
        ]);
        assert_eq!(ids(&board.backlog), ["pi.1"]);
        assert_eq!(ids(&board.waiting), ["pi.2"]);
        assert_eq!(ids(&board.ready), ["pi.3"]);
        assert_eq!(ids(&board.in_progress), ["pi.4"]);
        assert_eq!(ids(&board.held), ["pi.5"]);
        assert!(board.closed.is_empty());
    }

    #[test]
    fn the_old_word_triage_lands_in_the_backlog_group() {
        let board = board(&[
            filed_as("pi.1", 10, "triage", "none", None),
            filed_as("pi.2", 20, "ready", "none", None),
            moved("pi.3", 30, "pi.2", "triage"),
        ]);
        assert_eq!(ids(&board.backlog), ["pi.1", "pi.2"]);
        assert!(
            board.ready.is_empty(),
            "the old word on a move parks the flight"
        );
    }

    #[test]
    fn an_unknown_status_routes_nowhere() {
        let board = board(&[filed("pi.1", 10), moved("pi.2", 20, "pi.1", "parked")]);
        assert!(board.backlog.is_empty());
        assert!(board.waiting.is_empty());
        assert!(board.ready.is_empty());
        assert!(board.in_progress.is_empty());
        assert!(board.held.is_empty());
        assert!(board.closed.is_empty());
    }

    #[test]
    fn a_group_sorts_by_priority_then_oldest_first() {
        let board = board(&[
            filed_as("pi.1", 10, "backlog", "low", None),
            filed_as("pi.2", 20, "backlog", "urgent", None),
            filed_as("pi.3", 30, "backlog", "none", None),
            filed_as("pi.4", 40, "backlog", "high", None),
            filed_as("pi.5", 50, "backlog", "urgent", None),
            filed_as("pi.6", 60, "backlog", "medium", None),
        ]);
        assert_eq!(
            ids(&board.backlog),
            ["pi.2", "pi.5", "pi.4", "pi.6", "pi.1", "pi.3"],
            "urgent oldest-first, then high, medium, low, none"
        );
    }

    #[test]
    fn an_unknown_priority_sorts_after_none() {
        let board = board(&[
            filed_as("pi.1", 10, "backlog", "blocker", None),
            filed_as("pi.2", 20, "backlog", "none", None),
        ]);
        assert_eq!(ids(&board.backlog), ["pi.2", "pi.1"]);
    }

    #[test]
    fn the_closed_group_carries_the_three_newest_by_default() {
        let board = board(&[
            filed("pi.1", 10),
            filed("pi.2", 20),
            filed("pi.3", 30),
            filed("pi.4", 40),
            done("pi.5", NOW - 3_600, "pi.1"),
            moved("pi.6", NOW - 60, "pi.2", "canceled"),
            done("pi.7", NOW - 600, "pi.3"),
            done("pi.8", NOW - 10, "pi.4"),
        ]);
        assert_eq!(
            ids(&board.closed),
            ["pi.4", "pi.2", "pi.3"],
            "the three newest, newest first — the fourth is dropped by the count, \
             minutes old though it is"
        );
        assert_eq!(board.closed[1].status, "canceled");
        assert!(board.backlog.is_empty(), "a closed flight leaves its group");
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

        let board = windowed(&events, ClosedWindow::All);
        assert_eq!(ids(&board.closed), ["pi.2", "pi.1"]);

        let board = windowed(&events, ClosedWindow::None);
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

        let board = windowed(&events, ClosedWindow::Count(2));
        assert_eq!(ids(&board.closed), ["pi.3", "pi.2"]);

        let board = windowed(&events, ClosedWindow::Count(0));
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
        let board = board(&[
            filed("pi.1", 10),
            filed("pi.2", 20),
            filed_as("pi.3", 30, "ready", "none", Some("me")),
            filed_as("pi.4", 40, "ready", "none", Some("agent")),
            filed_as("pi.5", 50, "ready", "none", None),
            held("pi.6", 70, "pi.2", "later"),
            held("pi.7", 60, "pi.1", "sooner"),
        ]);
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
        let board = board(&[
            filed("pi.1", 10),
            held("pi.2", 20, "pi.1", "which?"),
            done("pi.3", NOW - 60, "pi.1"),
        ]);
        assert!(board.waiting_on_you.questions.is_empty());
        assert_eq!(ids(&board.closed), ["pi.1"]);
    }

    #[test]
    fn a_canceled_rows_question_is_gone_and_its_reason_stands_in_its_place() {
        let board = board(&[
            filed("pi.1", 10),
            held("pi.2", 20, "pi.1", "which?"),
            canceled("pi.3", NOW - 60, "pi.1", "superseded"),
        ]);
        assert!(board.waiting_on_you.questions.is_empty());
        let row = &board.closed[0];
        assert_eq!(row.id, "pi.1");
        assert!(row.question.is_none());
        assert!(row.asked_at.is_none());
        assert_eq!(row.closed_reason.as_deref(), Some("superseded"));
    }

    #[test]
    fn a_canceled_child_lifts_the_parent_to_ready_with_the_reason() {
        let board = board(&[
            filed_as("pi.1", 10, "ready", "none", None),
            filed("pi.2", 20),
            linked("pi.3", 30, "pi.1", "pi.2"),
            moved("pi.4", 40, "pi.2", "canceled"),
        ]);
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
        let board = board(&[
            subjected("pi.1", 10, "top"),
            event(
                "pi.2",
                20,
                filing("ready", "none", Some("agent"), "top · middle"),
            ),
            subjected("pi.3", 30, "leaf"),
            linked("pi.4", 40, "pi.1", "pi.2"),
            linked("pi.5", 50, "pi.2", "pi.3"),
        ]);
        assert_eq!(
            ids(&board.backlog),
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
        let board = board(&[
            subjected("pi.1", 10, "a broad task"),
            subjected("pi.2", 20, "part one"),
            subjected("pi.3", 30, "part two"),
            linked("pi.4", 40, "pi.1", "pi.2"),
            linked("pi.5", 50, "pi.1", "pi.3"),
            done("pi.6", NOW - 60, "pi.2"),
        ]);
        assert_eq!(ids(&board.backlog), ["pi.1", "pi.3"]);
        assert_eq!(board.backlog[0].progress, Some((1, 2)));
        assert!(
            board.backlog[1].progress.is_none(),
            "a child with no children of its own carries no mark"
        );
        assert_eq!(ids(&board.closed), ["pi.2"]);
    }

    /// The inbox is still a second view over the same rows, and a
    /// sub-flight reaches it on the same terms as any flight.
    #[test]
    fn a_questioned_sub_flight_reaches_the_inbox_and_its_group() {
        let board = board(&[
            subjected("pi.1", 10, "check the PR"),
            subjected("pi.2", 20, "check the PR · verdict"),
            linked("pi.3", 30, "pi.1", "pi.2"),
            held("pi.4", 40, "pi.2", "which flow wins?"),
        ]);
        assert_eq!(ids(&board.waiting_on_you.questions), ["pi.2"]);
        assert_eq!(ids(&board.held), ["pi.2"]);
        assert_eq!(ids(&board.backlog), ["pi.1"]);
    }

    #[test]
    fn a_flight_with_no_children_carries_no_progress_mark() {
        let board = board(&[filed("pi.1", 10)]);
        assert!(board.backlog[0].progress.is_none());
    }

    #[test]
    fn an_untouched_flight_carries_its_stored_fields_and_nothing_else() {
        let board = board(&[filed("pi.1", 10)]);
        let view = &board.backlog[0];
        assert_eq!(view.id, "pi.1");
        assert_eq!(view.number, 1);
        assert_eq!(view.status, "backlog");
        assert!(view.status_by.is_none() && view.status_at.is_none());
        assert!(view.assignee.is_none());
        assert_eq!(view.priority, "none");
        assert!(view.labels.is_empty());
        assert!(view.skill.is_none());
        assert!(view.procedure.is_none());
    }

    #[test]
    fn the_inbox_pins_the_viewers_own_callsign_beside_me() {
        let events = [
            filed_as("pi.1", 10, "ready", "none", Some("me")),
            filed_as("pi.2", 20, "ready", "none", Some("qwen-review")),
            filed_as("pi.3", 30, "ready", "none", Some("claude")),
            filed_as("pi.4", 40, "ready", "none", Some("agent")),
            filed_as("pi.5", 50, "backlog", "none", Some("qwen-review")),
        ];
        let qwen = enrich(
            fold(&events),
            NOW,
            ClosedWindow::default(),
            Some("qwen-review"),
        );
        assert_eq!(
            ids(&qwen.waiting_on_you.yours),
            ["pi.1", "pi.2"],
            "`me` and the viewer's own, Ready only"
        );
        let mine: Vec<bool> = qwen.backlog.iter().map(|view| view.mine).collect();
        assert_eq!(mine, [true], "the flag is the lane's, whatever the status");
        let nobody = enrich(fold(&events), NOW, ClosedWindow::default(), None);
        assert_eq!(ids(&nobody.waiting_on_you.yours), ["pi.1"]);
        assert!(
            !nobody
                .ready
                .iter()
                .any(|view| view.id == "pi.2" && view.mine)
        );
    }

    #[test]
    fn the_row_carries_the_filers_and_the_movers_callsign() {
        let mut filing = filed_as("pi.1", 10, "ready", "none", None);
        filing.callsign = Some("tyler".to_string());
        let mut pull = moved("pi.2", 20, "pi.1", "in_progress");
        pull.callsign = Some("claude".to_string());
        let board = board(&[filing, pull]);
        let row = &board.in_progress[0];
        assert_eq!(row.filed_callsign.as_deref(), Some("tyler"));
        assert_eq!(row.status_callsign.as_deref(), Some("claude"));
        assert_eq!(row.status_by.as_deref(), Some("a@b.c"));
    }
}

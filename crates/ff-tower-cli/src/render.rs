//! The human render, in fufu's list grammar: a head line per flight, then
//! an indented dim note joining phrases with ` · ` in urgency order —
//! the open question first, then held/resolving, then a `collides` warn
//! per conflicting neighbor and a `no verdict` dim per unanswered one,
//! then the two audit lines, then the pilot phrase — `in progress —
//! <by>` — then `on <branch>`, then the comment count, then age. No
//! affirmative "lands clean" phrase: absence of a warn is the verdict,
//! and board noise is the enemy.
//!
//! The board is the inbox pinned above the status groups, and the groups
//! are the stored model in lifecycle order. Glyphs carry the meaning
//! independent of color: `?` a question stopped on you, `!` yours, `·`
//! triage, `⋯` waiting, `○` ready, `▸` in progress, `‖` held, `▪` closed.
//! A local vocabulary, not fufu's — `@ ● ✓ ✕` name git objects, not
//! flight states.
//!
//! Ids render in DESIGN's display form: the flight's dense number,
//! `#<n>` when the board's filed flights span one writer, `<writer>#<n>`
//! otherwise — the wire's dotted form never renders here.

use std::collections::HashMap;

use anstyle::{AnsiColor, Color, Style};
use ff_tower_core::board::{Board, FlightView};

const ID: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
const WARN: Style = Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)));
const DIM: Style = Style::new().dimmed();

/// Whether stdout gets escape codes. `AutoStream::choice` already honors
/// `NO_COLOR` and piped output, so piped test output is byte-deterministic.
pub fn colored() -> bool {
    !matches!(
        anstream::AutoStream::choice(&std::io::stdout()),
        anstream::ColorChoice::Never
    )
}

fn paint(style: Style, text: &str, colored: bool) -> String {
    if colored {
        format!("{style}{text}{style:#}")
    } else {
        text.to_string()
    }
}

pub fn paint_id(text: &str, colored: bool) -> String {
    paint(ID, text, colored)
}

pub fn paint_warn(text: &str, colored: bool) -> String {
    paint(WARN, text, colored)
}

pub fn paint_dim(text: &str, colored: bool) -> String {
    paint(DIM, text, colored)
}

/// Whether the given full ids span at most one writer, so `#<n>` alone
/// names a flight unambiguously.
pub fn short_ids<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut writers = ids.map(writer_of);
    match writers.next() {
        None => true,
        Some(first) => writers.all(|writer| writer == first),
    }
}

/// The writer half of a wire id — everything before the last `.`, safe to
/// split on because a sanitized writer contains no dots.
fn writer_of(id: &str) -> &str {
    id.rsplit_once('.').map_or(id, |(writer, _)| writer)
}

/// The display form of a flight's number: `#3` when short, `pi-8c2e#3`
/// otherwise. The long form takes no leading `#` — the interior `#` is
/// the marker, and it binds a writer to a flight number the way `.` binds
/// one to an event seq, so the two names cannot be confused.
pub fn flight_ref(writer: &str, number: u64, short: bool) -> String {
    if short {
        format!("#{number}")
    } else {
        format!("{writer}#{number}")
    }
}

/// The collide path phrase: the one path, or a count — `next`'s grammar,
/// shared with the brief's beat rows and the board's warn.
pub fn paths_phrase(paths: &[String]) -> String {
    match paths {
        [path] => path.clone(),
        paths => format!("{} paths", paths.len()),
    }
}

/// `4m`, `2h`, `2d` — a duration in seconds, s/m/h/d/w, with no trailing
/// "ago". The threshold phrases print a span; the row's own age prints
/// the same span with the word.
pub fn span(seconds: i64) -> String {
    let delta = seconds.max(0);
    match delta {
        0..60 => format!("{delta}s"),
        60..3_600 => format!("{}m", delta / 60),
        3_600..86_400 => format!("{}h", delta / 3_600),
        86_400..604_800 => format!("{}d", delta / 86_400),
        _ => format!("{}w", delta / 604_800),
    }
}

/// `4m ago`, `2d ago`. `now` is an argument so a render is a pure
/// function of its inputs.
pub fn age(now: i64, then: i64) -> String {
    format!("{} ago", span(now - then))
}

/// The tip column: the branch tip short, `—` for a flight with no tip, and
/// the literal `(detached)` for `@detached` — printing the sentinel as a
/// branch name would read as a real branch.
fn tip_column(view: &FlightView) -> String {
    if view.branch.as_deref() == Some("@detached") {
        return "(detached)".to_string();
    }
    match &view.tip {
        Some(tip) => tip.chars().take(8).collect(),
        None => "—".to_string(),
    }
}

/// The subject column: the subject, then the progress mark for a flight
/// that has children. The mark is where "waiting on 2 flights" used to
/// be — one fact, one place — and on a flat board it is the whole of
/// what says a row is a family.
fn subject_column(view: &FlightView) -> String {
    let mut text = view.subject.clone();
    if let Some((closed, total)) = view.progress {
        text.push_str(&format!(" ({closed}/{total})"));
    }
    text
}

/// The display form for a flight the board is showing, or the wire id for
/// one it is not: a collide partner can be a flight outside the closed
/// window, and a note must still name it.
fn reference(refs: &HashMap<&str, String>, id: &str) -> String {
    refs.get(id).cloned().unwrap_or_else(|| id.to_string())
}

fn note(
    view: &FlightView,
    refs: &HashMap<&str, String>,
    now: i64,
    stale_after: i64,
    colored: bool,
) -> String {
    let mut phrases = Vec::new();
    if let Some(question) = view.question.as_deref() {
        phrases.push(paint_warn(question, colored));
    }
    if view.held {
        phrases.push(paint_warn("held", colored));
    }
    if view.resolving {
        phrases.push(paint_warn("resolving", colored));
    }
    for collide in &view.collides {
        let with = reference(refs, &collide.with);
        let on = paths_phrase(&collide.paths);
        phrases.push(paint_warn(&format!("collides {with} on {on}"), colored));
    }
    for with in &view.unanswered {
        phrases.push(paint_dim(
            &format!("no verdict vs {}", reference(refs, with)),
            colored,
        ));
    }
    // The two audits, each its own phrase and neither under a shared
    // word: one says the branch has forgotten a flight that claims to be
    // flying, the other says a branch moved under one that claims not to
    // be. The stale phrase names the threshold, which is what the row's
    // own age cannot say.
    if view.stale {
        phrases.push(paint_warn(
            &format!("no changes on the branch for {}", span(stale_after)),
            colored,
        ));
    }
    if view.changed_since_ready {
        phrases.push(paint_warn(
            "changes on the branch since it was set ready",
            colored,
        ));
    }
    // The pilot, ahead of the branch: the stored In Progress and who set
    // it — the byline is the pilot, the field is the chip.
    if view.status == "in_progress" {
        phrases.push(paint_dim(
            &match view.status_by.as_deref() {
                Some(by) => format!("in progress — {by}"),
                None => "in progress".to_string(),
            },
            colored,
        ));
    }
    if let Some(branch) = view.branch.as_deref()
        && branch != "@detached"
    {
        phrases.push(paint_dim(&format!("on {branch}"), colored));
    }
    if view.comments > 0 {
        let noun = if view.comments == 1 {
            "comment"
        } else {
            "comments"
        };
        phrases.push(paint_dim(&format!("{} {noun}", view.comments), colored));
    }
    match (view.asked_at, view.last_change) {
        (Some(asked), _) => phrases.push(paint_dim(&format!("asked {}", age(now, asked)), colored)),
        (None, Some(change)) => {
            phrases.push(paint_dim(&format!("changed {}", age(now, change)), colored))
        }
        (None, None) => phrases.push(paint_dim(
            &format!("filed {}", age(now, view.filed_at)),
            colored,
        )),
    }
    phrases.join(&paint_dim(" · ", colored))
}

/// The whole board, as one string ending in a newline.
///
/// The inbox first — a view of the same rows, so a flight in it prints
/// twice — then the status groups in lifecycle order, empty ones skipped.
/// The columns are measured across every group at once, so the board
/// aligns down its whole height rather than per section.
pub fn board(board: &Board, now: i64, stale_after: i64, colored: bool) -> String {
    let sections: [(&str, char, &[FlightView]); 8] = [
        ("questions", '?', &board.waiting_on_you.questions),
        ("yours", '!', &board.waiting_on_you.yours),
        ("triage", '·', &board.triage),
        ("waiting", '⋯', &board.waiting),
        ("ready", '○', &board.ready),
        ("in progress", '▸', &board.in_progress),
        ("held", '‖', &board.held),
        ("closed", '▪', &board.closed),
    ];
    // The footer counts live work: the inbox is a second view of rows the
    // groups already carry, and a closed flight is on the record rather
    // than on the board. Every other live flight is a row in exactly one
    // of the five, sub-flights included, so the sum is the whole board.
    let flights = sections[2..7]
        .iter()
        .map(|(_, _, views)| views.len())
        .sum::<usize>();
    let short = short_ids(
        sections
            .iter()
            .flat_map(|(_, _, views)| views.iter())
            .map(|view| view.id.as_str()),
    );
    // Wire id to display form, over every section at once.
    let refs: HashMap<&str, String> = sections
        .iter()
        .flat_map(|(_, _, views)| views.iter())
        .map(|view| {
            (
                view.id.as_str(),
                flight_ref(writer_of(&view.id), view.number, short),
            )
        })
        .collect();
    let id_width = refs
        .values()
        .map(|reference| reference.chars().count())
        .max()
        .unwrap_or(0);
    let subject_width = sections
        .iter()
        .flat_map(|(_, _, views)| views.iter())
        .map(|view| subject_column(view).chars().count())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for (title, glyph, views) in sections {
        if views.is_empty() {
            continue;
        }
        out.push_str(title);
        out.push('\n');
        for view in views {
            let id = format!("{:<id_width$}", refs[view.id.as_str()]);
            let subject = format!("{:<subject_width$}", subject_column(view));
            out.push_str(&format!(
                "{glyph} {}  {}  {}\n",
                paint_id(&id, colored),
                subject,
                paint_dim(&tip_column(view), colored),
            ));
            out.push_str(&format!(
                "    {}\n",
                note(view, &refs, now, stale_after, colored)
            ));
        }
        out.push('\n');
    }

    if !board.unrouted.is_empty() {
        let (noun, verb) = if board.unrouted.len() == 1 {
            ("event", "is")
        } else {
            ("events", "are")
        };
        out.push_str(&paint_warn(
            &format!(
                "{} {noun} in the log {verb} not on the board — `ff tower doctor` says which and why",
                board.unrouted.len()
            ),
            colored,
        ));
        out.push('\n');
    }

    let footer = if flights == 0 {
        "nothing on the board · ff tower file to add one".to_string()
    } else {
        let noun = if flights == 1 { "flight" } else { "flights" };
        format!("{flights} {noun} · ff tower file to add one")
    };
    out.push_str(&paint_dim(&footer, colored));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff_tower_core::board::WaitingOnYou;

    const NOW: i64 = 1_000_000;
    const TWO_DAYS: i64 = 2 * 24 * 60 * 60;

    fn view(id: &str, number: u64, status: &str, subject: &str) -> FlightView {
        FlightView {
            id: id.to_string(),
            number,
            procedure: None,
            subject: subject.to_string(),
            body: String::new(),
            filed_by: "a@b.c".to_string(),
            filed_at: NOW - 60,
            comments: 0,
            depends_on: Vec::new(),
            blocks: Vec::new(),
            status: status.to_string(),
            status_by: None,
            status_at: None,
            assignee: None,
            priority: "none".to_string(),
            labels: Vec::new(),
            skill: None,
            bay: None,
            branch: None,
            tip: None,
            last_change: None,
            stale: false,
            changed_since_ready: false,
            progress: None,
            held: false,
            resolving: false,
            current: false,
            question: None,
            asked_at: None,
            collides: Vec::new(),
            unanswered: Vec::new(),
        }
    }

    fn empty() -> Board {
        Board {
            waiting_on_you: WaitingOnYou {
                questions: Vec::new(),
                yours: Vec::new(),
            },
            triage: Vec::new(),
            waiting: Vec::new(),
            ready: Vec::new(),
            in_progress: Vec::new(),
            held: Vec::new(),
            closed: Vec::new(),
            unrouted: Vec::new(),
            retired: Vec::new(),
        }
    }

    #[test]
    fn a_stale_row_names_the_threshold_it_passed() {
        let mut flight = view("pi.1", 1, "in_progress", "the stalled work");
        flight.stale = true;
        flight.last_change = Some(NOW - 300_000);
        let mut rendered = empty();
        rendered.in_progress.push(flight);

        let out = board(&rendered, NOW, TWO_DAYS, false);
        assert!(out.contains("no changes on the branch for 2d"), "{out}");
    }

    #[test]
    fn a_ready_row_the_branch_moved_under_says_so() {
        let mut flight = view("pi.1", 1, "ready", "cleared, and moving");
        flight.changed_since_ready = true;
        flight.last_change = Some(NOW - 30);
        let mut rendered = empty();
        rendered.ready.push(flight);

        let out = board(&rendered, NOW, TWO_DAYS, false);
        assert!(
            out.contains("changes on the branch since it was set ready"),
            "{out}"
        );
        assert!(out.contains("changed 30s ago"), "{out}");
        assert!(!out.contains("no changes on the branch"), "{out}");
    }

    #[test]
    fn a_parent_prints_its_mark_and_a_sub_flight_its_own_subject() {
        let mut parent = view("pi.1", 1, "waiting", "check the PR");
        parent.progress = Some((2, 6));
        let child = view("pi.2", 2, "ready", "check the PR · verdict");
        let mut rendered = empty();
        rendered.waiting.push(parent);
        rendered.ready.push(child);

        let out = board(&rendered, NOW, TWO_DAYS, false);
        assert!(out.contains("check the PR (2/6)"), "{out}");
        assert!(out.contains("check the PR · verdict"), "{out}");
        assert!(
            out.contains("2 flights · ff tower file to add one"),
            "{out}"
        );
    }
}

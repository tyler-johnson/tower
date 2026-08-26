//! The human render, in fufu's list grammar: a head line per flight, then
//! an indented dim note joining phrases with ` · ` in urgency order —
//! held/resolving first, then `on <branch>`, then the comment count,
//! then age.
//!
//! Glyphs carry the meaning independent of color: `▸` in the air, `‖`
//! holding, `·` open. A local vocabulary, not fufu's — `@ ● ✓ ✕` name git
//! objects, not flight states.
//!
//! Ids render in DESIGN's display form: `#`-prefixed, the seq alone when
//! the board's filed flights span one writer, `#<writer>.<seq>` otherwise.

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

fn paint_warn(text: &str, colored: bool) -> String {
    paint(WARN, text, colored)
}

pub fn paint_dim(text: &str, colored: bool) -> String {
    paint(DIM, text, colored)
}

/// Whether the given full ids span at most one writer, so `#<seq>` alone
/// names a flight unambiguously. The writer is everything before the last
/// `.` — safe to split on because a sanitized writer contains no dots.
pub fn short_ids<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut writers = ids.map(|id| id.rsplit_once('.').map_or(id, |(writer, _)| writer));
    match writers.next() {
        None => true,
        Some(first) => writers.all(|writer| writer == first),
    }
}

/// The display form of a full id: `#3` when short, `#pi-8c2e.3` otherwise.
pub fn flight_ref(id: &str, short: bool) -> String {
    if short {
        format!("#{}", id.rsplit_once('.').map_or(id, |(_, seq)| seq))
    } else {
        format!("#{id}")
    }
}

/// `4m ago`, `2d ago` — s/m/h/d/w. `now` is an argument so a render is a
/// pure function of its inputs.
fn age(now: i64, then: i64) -> String {
    let delta = (now - then).max(0);
    match delta {
        0..60 => format!("{delta}s ago"),
        60..3_600 => format!("{}m ago", delta / 60),
        3_600..86_400 => format!("{}h ago", delta / 3_600),
        86_400..604_800 => format!("{}d ago", delta / 86_400),
        _ => format!("{}w ago", delta / 604_800),
    }
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

fn note(view: &FlightView, now: i64, colored: bool) -> String {
    let mut phrases = Vec::new();
    if view.held {
        phrases.push(paint_warn("held", colored));
    }
    if view.resolving {
        phrases.push(paint_warn("resolving", colored));
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
    match view.last_motion {
        Some(motion) => phrases.push(paint_dim(&format!("moved {}", age(now, motion)), colored)),
        None => phrases.push(paint_dim(
            &format!("filed {}", age(now, view.filed_at)),
            colored,
        )),
    }
    phrases.join(&paint_dim(" · ", colored))
}

/// The whole board, as one string ending in a newline.
pub fn board(board: &Board, now: i64, colored: bool) -> String {
    let sections: [(&str, char, &[FlightView]); 3] = [
        ("in the air", '▸', &board.in_the_air),
        ("holding", '‖', &board.holding),
        ("open", '·', &board.open),
    ];
    let flights = board.waiting_on_you.len()
        + sections
            .iter()
            .map(|(_, _, views)| views.len())
            .sum::<usize>();
    let short = short_ids(
        sections
            .iter()
            .flat_map(|(_, _, views)| views.iter())
            .map(|view| view.id.as_str()),
    );
    let id_width = sections
        .iter()
        .flat_map(|(_, _, views)| views.iter())
        .map(|view| flight_ref(&view.id, short).chars().count())
        .max()
        .unwrap_or(0);
    let subject_width = sections
        .iter()
        .flat_map(|(_, _, views)| views.iter())
        .map(|view| view.subject.chars().count())
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
            let id = format!("{:<id_width$}", flight_ref(&view.id, short));
            let subject = format!("{:<subject_width$}", view.subject);
            out.push_str(&format!(
                "{glyph} {}  {}  {}\n",
                paint_id(&id, colored),
                subject,
                paint_dim(&tip_column(view), colored),
            ));
            out.push_str(&format!("    {}\n", note(view, now, colored)));
        }
        out.push('\n');
    }

    if !board.unrouted.is_empty() {
        let noun = if board.unrouted.len() == 1 {
            "event"
        } else {
            "events"
        };
        out.push_str(&paint_warn(
            &format!(
                "{} {noun} unrouted — a merge ahead of a filing, or a future tower",
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

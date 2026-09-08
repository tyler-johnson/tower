//! `ff tower briefing` — one line for fufu's session briefing.
//!
//! fufu runs it in the event's cwd with `FF_REPO`, `FF_CONTRACT`, and
//! `FF_SESSION` set, under a one-second box with stderr discarded, and
//! takes stdout verbatim: trimmed, one line, at most 240 characters, or
//! dropped whole. So the render is the line and nothing else — no color,
//! no `board: ff tower` tail — and the line is one line by construction.
//! A failure costs nothing: fufu discards it, and the ordinary `report()`
//! path says why to anyone running the verb by hand.
//!
//! The pipeline is `bay list`'s — fold, gather, the bay fold — because
//! the question is the bay's: when this worktree is flying a flight, the
//! line names it; otherwise it counts what is ready.

use crate::error::CliError;
use crate::machine;
use ff_tower_core::board;
use ff_tower_core::log::Store;

/// fufu drops a longer line whole rather than clipping it, so the
/// clipping happens here, on the subject, with the rest of the line
/// intact.
pub const LINE_CAP: usize = 240;

pub fn run(json: bool) -> Result<(), CliError> {
    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let fold = board::fold(&store.read_all()?);
    let reads = board::gather(&ff)?;
    let views = board::bays(&fold, &reads);

    let line = match views
        .iter()
        .find(|view| view.current)
        .and_then(|view| Some((view, view.flight.as_deref()?)))
    {
        Some((view, flight)) => {
            let id = flight.parse().expect("the fold's ids parse");
            let display = board::display(&fold, &id);
            let subject = one_line(&board::flight(&fold, &id).subject);
            flying(&display, &subject, &view.id)
        }
        None => {
            let ready = fold
                .flights
                .iter()
                .filter(|flight| flight.status == "ready")
                .count();
            match ready {
                0 => "tower: nothing ready".to_string(),
                1 => "tower: 1 flight ready — ff tower".to_string(),
                n => format!("tower: {n} flights ready — ff tower"),
            }
        }
    };

    if json {
        println!(
            "{}",
            machine::emit("briefing", &serde_json::json!({ "line": line }))
        );
    } else {
        println!("{line}");
    }
    Ok(())
}

/// The flying line, with the subject elided so the whole stays under
/// the cap.
fn flying(display: &str, subject: &str, bay: &str) -> String {
    let render = |subject: &str| {
        format!("tower: flying {display} {subject} in {bay} — ff tower brief {display}")
    };
    let line = render(subject);
    if line.chars().count() <= LINE_CAP {
        return line;
    }
    let overhead = render("").chars().count();
    let room = LINE_CAP.saturating_sub(overhead + 1);
    let clipped: String = subject.chars().take(room).collect();
    render(&format!("{}…", clipped.trim_end()))
}

/// A subject's first line with its control characters dropped: one
/// line, whatever was filed.
fn one_line(subject: &str) -> String {
    subject
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_subject_is_elided_under_the_cap() {
        let subject = "x".repeat(300);
        let line = flying("#1", &subject, "main");
        assert!(line.chars().count() <= LINE_CAP, "{}", line.chars().count());
        assert!(line.contains('…'));
        assert!(line.ends_with("in main — ff tower brief #1"), "{line}");
        assert_eq!(line.lines().count(), 1);
    }

    #[test]
    fn a_short_subject_is_whole() {
        let line = flying("pi#3", "fix the login redirect", "bay-2");
        assert_eq!(
            line,
            "tower: flying pi#3 fix the login redirect in bay-2 — ff tower brief pi#3"
        );
    }

    #[test]
    fn a_subject_is_one_line() {
        assert_eq!(one_line("first\nsecond"), "first");
        assert_eq!(one_line("tab\there "), "tabhere");
    }
}

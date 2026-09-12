//! References: a flight named in prose, stored as its wire id.
//!
//! Pure like `pick.rs` — no store, no spawns. A body that says `see #3`
//! names a flight by its dense per-writer number, which a renumbering
//! would silently stale; so every verb that takes prose runs [`rewrite`]
//! over it at write time and stores `#<wire id>` in its place, and every
//! surface that prints prose runs [`project`] to put the current display
//! form back. [`named`] is the fold's half: the ids a text names, so the
//! brief can say who names whom.
//!
//! The scan is a run tokenizer, not a grammar. A token char is ASCII
//! alphanumeric or one of `# ~ . _ -`; each maximal run splits into a
//! head — the run with its trailing non-alphanumeric chars removed — and
//! that tail, the sentence's own `.` or `-`. A run is a candidate when
//! its head contains `#` or starts with `~`, and every candidate goes to
//! `parse_ref`, so the reference grammar stays in one place and a form
//! it gains later is scanned for free. Everything else, tails included,
//! is copied verbatim.

use super::flight::Fold;
use super::resolve::{ResolveError, parse_ref, resolve};
use crate::log::EventId;

/// One piece of a text: a run of token chars, split into head and tail,
/// or the text between runs.
enum Piece<'a> {
    Plain(&'a str),
    Run { head: &'a str, tail: &'a str },
}

fn is_token(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '#' | '~' | '.' | '_' | '-')
}

/// The text as pieces, in order; concatenated they are the text.
fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let run_end = rest.find(|c| !is_token(c)).unwrap_or(rest.len());
        if run_end == 0 {
            let plain_end = rest.find(is_token).unwrap_or(rest.len());
            pieces.push(Piece::Plain(&rest[..plain_end]));
            rest = &rest[plain_end..];
        } else {
            let run = &rest[..run_end];
            let head_end = run
                .trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
                .len();
            pieces.push(Piece::Run {
                head: &run[..head_end],
                tail: &run[head_end..],
            });
            rest = &rest[run_end..];
        }
    }
    pieces
}

/// Whether a head is worth handing to the reference grammar.
fn candidate(head: &str) -> bool {
    head.contains('#') || head.starts_with('~')
}

/// The wire id a head spells, when it is the `#<writer>.<seq>` shape.
fn wire(head: &str) -> Option<EventId> {
    head.strip_prefix('#')?.parse().ok()
}

/// The text rebuilt with `each` applied to every run's head: `Some` is
/// the head's replacement, `None` keeps it, and an error stops the pass.
fn splice<E>(
    text: &str,
    mut each: impl FnMut(&str) -> Result<Option<String>, E>,
) -> Result<String, E> {
    let mut out = String::with_capacity(text.len());
    for piece in pieces(text) {
        match piece {
            Piece::Plain(plain) => out.push_str(plain),
            Piece::Run { head, tail } => {
                match each(head)? {
                    Some(replacement) => out.push_str(&replacement),
                    None => out.push_str(head),
                }
                out.push_str(tail);
            }
        }
    }
    Ok(out)
}

/// Write-time: every reference `fold` resolves to one flight becomes
/// `#<wire id>`; a bare number two writers hold refuses; the rest stays.
///
/// Text that is no reference — `#ff0000`, `C#`, a heading's `#` — and a
/// reference matching nothing filed both stay as typed: a GitHub number,
/// a `#1 priority`, or a flight not yet filed is never guessed at. A
/// `#<wire id>` of a filed flight resolves to itself, so the pass is
/// idempotent and an `edit -m` over stored text changes no reference.
pub fn rewrite(fold: &Fold, text: &str) -> Result<String, ResolveError> {
    splice(text, |head| {
        if !candidate(head) || parse_ref(head).is_err() {
            return Ok(None);
        }
        match resolve(fold, head) {
            Ok(id) => Ok(Some(format!("#{id}"))),
            Err(ResolveError::NotFound { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    })
}

/// Render-time: every `#<wire id>` `name` knows becomes what `name`
/// says. Only the `#<writer>.<seq>` shape is touched — a `#3` left in
/// stored text was unresolved at write, and stays what it was.
pub fn project(text: &str, name: impl Fn(&EventId) -> Option<String>) -> String {
    let projected: Result<String, ()> =
        splice(text, |head| Ok(wire(head).and_then(|id| name(&id))));
    projected.expect("projection never fails")
}

/// The flights a text names, in order, once each — every `#<wire id>`
/// token, whether or not anything filed carries it.
pub fn named(text: &str) -> Vec<EventId> {
    let mut ids: Vec<EventId> = Vec::new();
    for piece in pieces(text) {
        if let Piece::Run { head, .. } = piece
            && let Some(id) = wire(head)
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::fold;
    use crate::log::{Event, Kind};

    fn filed(id: &str, time: i64, subject: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            session: None,
            callsign: None,
            id,
            kind: Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: String::new(),
                status: "backlog".to_string(),
                assignee: None,
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        }
    }

    /// Three flights on one writer: seqs 1, 3, 5 are numbers 1, 2, 3.
    fn one_writer() -> Fold {
        fold(&[
            filed("pi.1", 10, "one"),
            filed("pi.3", 30, "two"),
            filed("pi.5", 50, "three"),
        ])
    }

    fn id(text: &str) -> EventId {
        text.parse().expect("id")
    }

    #[test]
    fn every_reference_form_becomes_the_wire_id() {
        let fold = one_writer();
        assert_eq!(rewrite(&fold, "see #3").expect("ok"), "see #pi.5");
        assert_eq!(rewrite(&fold, "see pi#2").expect("ok"), "see #pi.3");
        assert_eq!(rewrite(&fold, "see #pi#2").expect("ok"), "see #pi.3");
        assert_eq!(rewrite(&fold, "see #pi.3").expect("ok"), "see #pi.3");
        assert_eq!(
            rewrite(&fold, "#1, #2 and #3").expect("ok"),
            "#pi.1, #pi.3 and #pi.5"
        );
    }

    #[test]
    fn the_sentences_punctuation_stays_outside_the_reference() {
        let fold = one_writer();
        assert_eq!(
            rewrite(&fold, "blocked on #3.").expect("ok"),
            "blocked on #pi.5."
        );
        assert_eq!(rewrite(&fold, "#3's test").expect("ok"), "#pi.5's test");
        assert_eq!(rewrite(&fold, "(#3)").expect("ok"), "(#pi.5)");
        assert_eq!(rewrite(&fold, "#3-").expect("ok"), "#pi.5-");
        assert_eq!(rewrite(&fold, "see [#3](x)").expect("ok"), "see [#pi.5](x)");
    }

    #[test]
    fn text_that_is_no_reference_stays_as_typed() {
        let fold = one_writer();
        for text in [
            "# Heading",
            "## Heading",
            "written in C#",
            "color #ff0000",
            "#L12",
            "3#issuecomment-12",
            "#",
            "a #one priority",
        ] {
            assert_eq!(rewrite(&fold, text).expect("ok"), text, "{text:?}");
        }
    }

    #[test]
    fn a_reference_naming_nothing_filed_stays_as_typed() {
        let fold = one_writer();
        for text in ["see #999", "see qi#1", "see #pi.9", "foo#pi.3", "issues#3"] {
            assert_eq!(rewrite(&fold, text).expect("ok"), text, "{text:?}");
        }
    }

    #[test]
    fn a_tilde_is_not_a_reference_today() {
        let fold = one_writer();
        assert_eq!(rewrite(&fold, "see ~3").expect("ok"), "see ~3");
    }

    #[test]
    fn a_bare_number_two_writers_hold_refuses_with_both() {
        let fold = fold(&[filed("pi.1", 10, "from pi"), filed("qi.1", 20, "from qi")]);
        let err = rewrite(&fold, "see #1 and pi#1").expect_err("ambiguous");
        assert_eq!(err.id(), "flight/ambiguous");
        assert_eq!(err.to_string(), "`#1` names two flights: `pi#1`, `qi#1`");
        // The exact forms still pass.
        assert_eq!(
            rewrite(&fold, "see pi#1 and qi#1").expect("ok"),
            "see #pi.1 and #qi.1"
        );
    }

    #[test]
    fn the_rewrite_is_idempotent() {
        let fold = one_writer();
        let once = rewrite(&fold, "see #3, #pi#2, and #999.").expect("ok");
        assert_eq!(rewrite(&fold, &once).expect("ok"), once);
    }

    #[test]
    fn the_empty_text_and_text_with_no_runs_pass_through() {
        let fold = one_writer();
        assert_eq!(rewrite(&fold, "").expect("ok"), "");
        assert_eq!(rewrite(&fold, " — ").expect("ok"), " — ");
        assert_eq!(
            rewrite(&fold, "plain words\nover lines").expect("ok"),
            "plain words\nover lines"
        );
    }

    #[test]
    fn project_puts_the_display_form_back_and_leaves_the_unknown_alone() {
        let five = id("pi.5");
        let name = |id: &EventId| (id == &five).then(|| "#3".to_string());
        assert_eq!(project("see #pi.5.", name), "see #3.");
        assert_eq!(project("see #pi.9 and #3", name), "see #pi.9 and #3");
        assert_eq!(project("#pi.5's test (#pi.5)", name), "#3's test (#3)");
        assert_eq!(project("no refs", name), "no refs");
    }

    #[test]
    fn named_lists_every_wire_id_once_in_order() {
        assert_eq!(
            named("#pi.5 then #qi.2, then #pi.5 again; #3 and #pi.9."),
            [id("pi.5"), id("qi.2"), id("pi.9")]
        );
        assert!(named("nothing #3 here").is_empty());
    }
}

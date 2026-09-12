//! `atc brief <flight>` — everything known about one flight, for
//! whoever picks it up: the full record and where it stands.
//!
//! The read half of the handoff: `next` hands out a flight id and a
//! subject, and the brief is what an agent reads next. A fold, nothing
//! more. Not `ensure_active`: a closed flight briefs, the log
//! keeps the record, and the render carries the closing move alongside
//! everything else.

use crate::error::CliError;
use crate::{machine, render};
use atc_core::board::{self, Brief, Detail, Fold, Moment, Standing};

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let events = store.read_all()?;
    let fold = board::fold(&events);
    let id = super::resolve(&fold, flight)?;

    let now = board::now();
    let brief =
        board::brief(&fold, &events, &id, store.callsign()).expect("resolved to a filed flight");

    if json {
        println!("{}", machine::emit("brief", &brief));
    } else {
        print!("{}", page(&fold, &brief, now, render::colored()));
    }
    Ok(())
}

/// The detail page: head and note in the board's grammar, then the body
/// verbatim, the family, the comments in reading order, and the history
/// last — the record before the log of how it got that way.
fn page(fold: &Fold, brief: &Brief, now: i64, colored: bool) -> String {
    let mut out = String::new();
    let mut subject = brief.subject.clone();
    if let Some((closed, total)) = brief.progress {
        subject.push_str(&format!(" ({closed}/{total})"));
    }
    out.push_str(&format!(
        "{}  {subject}\n",
        render::paint_id(&show(fold, &brief.id), colored),
    ));
    out.push_str(&format!("    {}\n", note(brief, now, colored)));
    out.push_str(&format!("    {}\n", fields_line(brief, colored)));
    // The last edit, comment rewords included — the record has been
    // touched, and the mark says by whom.
    if let (Some(by), Some(at)) = (brief.edited_by.as_deref(), brief.edited_at) {
        out.push_str(&format!(
            "    {}\n",
            render::paint_dim(
                &format!("edited · by {by} · {}", render::age(now, at)),
                colored
            )
        ));
    }
    if !brief.body.is_empty() {
        out.push('\n');
        out.push_str(&brief.body);
        out.push('\n');
    }

    // The family, parents up and children down: every depends-on edge is
    // a parent edge, so `blocks` is this flight's parents and
    // `depends_on` is its children — the same two lists the fold already
    // keeps, named for what they mean rather than for the edge direction.
    for (title, links) in [("parents", &brief.blocks), ("children", &brief.depends_on)] {
        if links.is_empty() {
            continue;
        }
        out.push('\n');
        out.push_str(title);
        out.push('\n');
        for link in links {
            out.push_str(&format!(
                "· {}  {}",
                render::paint_id(&show(fold, &link.flight), colored),
                link.subject
            ));
            if link.closed {
                out.push_str(&format!(
                    "  {}",
                    render::paint_dim(&link.status.replace('_', " "), colored)
                ));
            }
            out.push('\n');
        }
    }

    if !brief.comments.is_empty() {
        out.push('\n');
        out.push_str("comments\n");
        for comment in &brief.comments {
            // The wire id leads the header: it is a comment's only name,
            // and what `edit` takes — what tower prints, tower accepts.
            out.push_str(&format!(
                "  {}\n",
                render::paint_dim(
                    &format!(
                        "{} · {} · {}",
                        comment.id,
                        comment.author,
                        render::age(now, comment.at)
                    ),
                    colored
                )
            ));
            for line in comment.text.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }

    // What happened, in the comments' grammar and their reading order.
    // One dim line per moment: the verb and the words it took — the
    // status word, the lane, the fields, the other end of the edge —
    // then, for a reason or a routing's because, the text indented under
    // it. The words a gesture only points at — the question, the
    // comment's text — are already printed above, and repeating them
    // here would make the section a second, staler copy of the page.
    // When the byline is a callsign and the event carried a session
    // too, the session follows on its own line: the pilot is the
    // byline, and the run stays provenance underneath.
    if !brief.history.is_empty() {
        out.push('\n');
        out.push_str("history\n");
        for moment in &brief.history {
            let (words, follow) = phrase(fold, &brief.id, moment);
            out.push_str(&format!(
                "  {}\n",
                render::paint_dim(
                    &format!(
                        "{} · {}{} · {} · {}",
                        moment.id,
                        moment.what,
                        words,
                        render::byline(
                            moment.callsign.as_deref(),
                            moment.session.as_deref(),
                            &moment.by
                        ),
                        render::age(now, moment.at)
                    ),
                    colored
                )
            ));
            if let (Some(_), Some(session)) = (&moment.callsign, moment.session.as_deref()) {
                out.push_str(&format!(
                    "    {}\n",
                    render::paint_dim(
                        &format!(
                            "session {}",
                            render::byline(None, Some(session), &moment.by)
                        ),
                        colored
                    )
                ));
            }
            for line in follow.into_iter().flat_map(str::lines) {
                out.push_str(&format!("    {}\n", render::paint_dim(line, colored)));
            }
        }
    }

    out.push('\n');
    out.push_str(&super::tail(colored));
    out.push('\n');
    out
}

/// The stored fields, one line: lane, priority, labels, skill, and the
/// procedure the filing was minted under. Its own line rather than
/// phrases in the note: the note is urgency ordered, and a field is not
/// urgency — it is what a reader needs to know before picking the
/// flight up, which is what a brief is for.
fn fields_line(brief: &Brief, colored: bool) -> String {
    let mut phrases = vec![match brief.assignee.as_deref() {
        Some(lane) => format!("assignee {lane}"),
        None => "unassigned".to_string(),
    }];
    if brief.priority != "none" {
        phrases.push(format!("priority {}", brief.priority));
    }
    if !brief.labels.is_empty() {
        phrases.push(brief.labels.join(", "));
    }
    if let Some(skill) = brief.skill.as_deref() {
        phrases.push(format!("skill {skill}"));
    }
    if let Some(procedure) = brief.procedure.as_deref() {
        phrases.push(format!("under {procedure}"));
    }
    render::paint_dim(&phrases.join(" · "), colored)
}

/// The note line, in the board's phrase order with the status ahead of
/// everything — a reader must know first where the flight stands, and
/// who put it there when someone did; when that someone closed a
/// dependency rather than moving this flight, the since line says so
/// right after. The standing joins as one phrase before the age:
/// precedence makes it exclusive with the mark phrases — a walk standing
/// only exists with no closing move, question, or pull — so the line
/// never says a thing twice.
fn note(brief: &Brief, now: i64, colored: bool) -> String {
    let mut phrases = Vec::new();
    let status = brief.status.replace('_', " ");
    phrases.push(render::paint_dim(
        &match (brief.status_by.as_deref(), brief.status_at) {
            (Some(by), Some(at)) => format!(
                "{status} — {} {}",
                render::byline(
                    brief.status_callsign.as_deref(),
                    brief.status_session.as_deref(),
                    by
                ),
                render::age(now, at)
            ),
            _ => status,
        },
        colored,
    ));
    if let Some(reason) = brief.status_reason.as_deref() {
        phrases.push(render::paint_dim(reason, colored));
    }
    if let Some(question) = brief.question.as_deref() {
        phrases.push(render::paint_warn(question, colored));
    } else if let Some(reason) = brief.closed_reason.as_deref() {
        // The same slot, dim: a close's reason needs nobody.
        phrases.push(render::paint_dim(reason, colored));
    }
    match &brief.standing {
        // Said above, from the brief's own flat facts.
        Standing::Done | Standing::Question | Standing::InProgress => {}
        Standing::Yours => phrases.push(render::paint_dim(
            &match brief.assignee.as_deref() {
                Some(lane) => format!("yours — assigned {lane}"),
                None => "yours — unassigned".to_string(),
            },
            colored,
        )),
        Standing::Ready => phrases.push(render::paint_dim("ready", colored)),
    }
    match brief.asked_at {
        Some(asked) => phrases.push(render::paint_dim(
            &format!("asked {}", render::age(now, asked)),
            colored,
        )),
        None => phrases.push(render::paint_dim(
            &format!("filed {}", render::age(now, brief.filed_at)),
            colored,
        )),
    }
    phrases.join(&render::paint_dim(" · ", colored))
}

/// The words a moment's verb took, as a phrase after the verb — a leading
/// space and the words, or nothing when the kind carries none — and the
/// free text that follows on its own line: a move's reason, a routing's
/// because.
fn phrase<'a>(fold: &Fold, brief_id: &str, moment: &'a Moment) -> (String, Option<&'a str>) {
    match &moment.detail {
        Some(Detail::Status { status, reason }) => (format!(" {status}"), reason.as_deref()),
        Some(Detail::Assigned { assignee }) => {
            (format!(" {}", assignee.as_deref().unwrap_or("none")), None)
        }
        Some(Detail::Edited { fields, comment }) => match comment {
            Some(comment) => (format!(" comment {comment}"), None),
            None => (format!(" {}", fields.join(", ")), None),
        },
        Some(Detail::Edge { from, to }) => {
            // `from` depends on `to`: seen from `from` the edge is a
            // dependency, seen from `to` it is what this flight blocks.
            let words = if from == brief_id {
                format!(" depends on {}", endpoint(fold, to))
            } else {
                format!(" blocks {}", endpoint(fold, from))
            };
            (words, None)
        }
        // The note above already prints an open question, and the CLI
        // keeps comments and history as two sections rather than one
        // stream — so these words ride the wire for the web's blended
        // stream and this render does not move.
        Some(Detail::Held { .. }) => (String::new(), None),
        Some(Detail::Answered { .. }) => (String::new(), None),
        Some(Detail::Routed {
            procedure, because, ..
        }) => (
            format!(" {procedure}"),
            (!because.is_empty()).then_some(because.as_str()),
        ),
        None => (String::new(), None),
    }
}

/// An edge's other end in the board's display form when the fold knows
/// it, and the wire id when it does not — an unlinked flight from another
/// writer's log may never have been filed here.
fn endpoint(fold: &Fold, id: &str) -> String {
    match id.parse() {
        Ok(parsed) if fold.flights.iter().any(|flight| flight.id == parsed) => {
            super::display(fold, &parsed)
        }
        _ => id.to_string(),
    }
}

/// A wire id from the brief, in the board's display form. Infallible —
/// the ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

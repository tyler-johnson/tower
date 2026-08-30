//! `ff tower brief <flight>` — everything known about one flight, for
//! whoever picks it up: the full record, its standing on `next`'s walk,
//! and what it beat.
//!
//! The read half of the handoff: `next` hands out a flight id and a
//! subject, and the brief is what an agent reads next. Fold plus gather,
//! and the collide probes only where `wants_verdicts` says they can
//! change the answer — the laziness lives here because the fold stays
//! spawn-free, and a closed or branchless flight briefs with zero
//! probes, byte-identical to the probed run. Not `ensure_active`: a
//! closed flight briefs, the log keeps the record, and the render
//! carries the closing move alongside everything else.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board::{self, Brief, Fold, Skip, Standing, Verdicts};

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let events = store.read_all()?;
    let fold = board::fold(&events);
    let id = super::resolve(&fold, flight)?;

    let ff = super::ff()?;
    let reads = board::gather(&ff)?;
    let verdicts = if board::wants_verdicts(&fold, &reads, &id) {
        board::probe(&ff, &fold, &reads)?
    } else {
        Verdicts::default()
    };
    let brief =
        board::brief(&fold, &events, &reads, &verdicts, &id).expect("resolved to a filed flight");

    if json {
        println!("{}", machine::emit("brief", &brief));
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        print!("{}", page(&fold, &brief, now, render::colored()));
    }
    Ok(())
}

/// The detail page: head and note in the board's grammar, then the body
/// verbatim, the link sections, the comments in reading order, and the
/// history last — the record before the log of how it got that way. The
/// beat rows land right after the routing line — one dim line per row.
fn page(fold: &Fold, brief: &Brief, now: i64, colored: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}  {}\n",
        render::paint_id(&show(fold, &brief.id), colored),
        brief.subject
    ));
    out.push_str(&format!("    {}\n", note(fold, brief, now, colored)));
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
    for beaten in &brief.beat {
        let reason = match &beaten.reason {
            Skip::Waiting { .. } => unreachable!("waiting rows never enter beat"),
            Skip::Collides { paths, .. } => {
                format!("collides on {}", render::paths_phrase(paths))
            }
            Skip::NoVerdict { .. } => "no verdict".to_string(),
        };
        out.push_str(&format!(
            "    {}\n",
            render::paint_dim(
                &format!("beat {} · {reason}", show(fold, &beaten.flight)),
                colored
            )
        ));
    }

    if !brief.body.is_empty() {
        out.push('\n');
        out.push_str(&brief.body);
        out.push('\n');
    }

    for (title, links) in [("depends on", &brief.depends_on), ("blocks", &brief.blocks)] {
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
    // One dim line per moment and nothing more: the words behind a
    // gesture — the question, the comment's text, the route's `because`
    // — are already printed above, and repeating them here would make
    // the section a second, staler copy of the page.
    if !brief.history.is_empty() {
        out.push('\n');
        out.push_str("history\n");
        for moment in &brief.history {
            out.push_str(&format!(
                "  {}\n",
                render::paint_dim(
                    &format!(
                        "{} · {} · {} · {}",
                        moment.id,
                        moment.what,
                        moment.by,
                        render::age(now, moment.at)
                    ),
                    colored
                )
            ));
        }
    }

    out.push('\n');
    out.push_str(&super::tail(colored));
    out.push('\n');
    out
}

/// The stored fields, one line: lane, priority, labels, skill, bay, and
/// the procedure the filing was minted under. Its own line rather than
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
    if let Some(bay) = brief.bay.as_deref() {
        phrases.push(format!("bay {bay}"));
    }
    if let Some(procedure) = brief.procedure.as_deref() {
        phrases.push(format!("under {procedure}"));
    }
    render::paint_dim(&phrases.join(" · "), colored)
}

/// The note line, in the board's phrase order with the status ahead of
/// everything — a reader must know first where the flight stands, and
/// who put it there when someone did. The standing joins as one phrase
/// before the branch: precedence makes it exclusive with the mark
/// phrases — a walk standing only exists with no closing move, question,
/// hold, or pull — so the line never says a thing twice.
fn note(fold: &Fold, brief: &Brief, now: i64, colored: bool) -> String {
    let mut phrases = Vec::new();
    let status = brief.status.replace('_', " ");
    phrases.push(render::paint_dim(
        &match (brief.status_by.as_deref(), brief.status_at) {
            (Some(by), Some(at)) => format!("{status} — {by} {}", render::age(now, at)),
            _ => status,
        },
        colored,
    ));
    if let Some(question) = brief.question.as_deref() {
        phrases.push(render::paint_warn(question, colored));
    }
    if brief.held {
        phrases.push(render::paint_warn("held", colored));
    }
    if brief.resolving {
        phrases.push(render::paint_warn("resolving", colored));
    }
    match &brief.standing {
        // Said above, from the brief's own flat facts.
        Standing::Done | Standing::Question | Standing::Held | Standing::InProgress => {}
        Standing::Yours => phrases.push(render::paint_dim(
            &match brief.assignee.as_deref() {
                Some(lane) => format!("yours — assigned {lane}"),
                None => "yours — unassigned".to_string(),
            },
            colored,
        )),
        Standing::Ready => phrases.push(render::paint_dim("ready", colored)),
        Standing::Waiting { on } => phrases.push(render::paint_dim(
            &format!(
                "waiting on {}",
                on.iter()
                    .map(|dep| show(fold, dep))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            colored,
        )),
        Standing::Collides { with, paths } => phrases.push(render::paint_warn(
            &format!(
                "collides with {} on {}",
                show(fold, with),
                render::paths_phrase(paths)
            ),
            colored,
        )),
        Standing::NoVerdict { with } => phrases.push(render::paint_warn(
            &format!("no verdict vs {}", show(fold, with)),
            colored,
        )),
    }
    match brief.branch.as_deref() {
        Some("@detached") => phrases.push(render::paint_dim("(detached)", colored)),
        Some(branch) => {
            let mut phrase = format!("on {branch}");
            if let Some(tip) = brief.tip.as_deref() {
                phrase.push(' ');
                phrase.extend(tip.chars().take(8));
            }
            phrases.push(render::paint_dim(&phrase, colored));
        }
        None => {}
    }
    match (brief.asked_at, brief.last_motion) {
        (Some(asked), _) => phrases.push(render::paint_dim(
            &format!("asked {}", render::age(now, asked)),
            colored,
        )),
        (None, Some(motion)) => phrases.push(render::paint_dim(
            &format!("moved {}", render::age(now, motion)),
            colored,
        )),
        (None, None) => phrases.push(render::paint_dim(
            &format!("filed {}", render::age(now, brief.filed_at)),
            colored,
        )),
    }
    phrases.join(&render::paint_dim(" · ", colored))
}

/// A wire id from the brief, in the board's display form. Infallible —
/// the ids came out of this fold's filed flights.
fn show(fold: &Fold, id: &str) -> String {
    super::display(fold, &id.parse().expect("the fold's ids parse"))
}

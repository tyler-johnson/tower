//! The brief: one flight's full record over the same fold as the board,
//! plus where it stands.
//!
//! Pure like `flight.rs` and `pick.rs` — no `crate::ff` spawns, no
//! `std::process`; the brief runs over a [`Fold`] and the events it was
//! folded from. The events ride along for the history alone: every other
//! field is the fold's, and both call sites hold the slice already. A
//! closed flight briefs like any other — the log keeps the record, and
//! reading it is never a lifecycle move.
//!
//! Waiting is a status the fold derives, not a standing of its own: a
//! flight with a live dependency is not pullable, so it stands as
//! `yours` with the status line saying Waiting.
//!
//! Standing precedence is `enrich`'s partition, not pick's one boolean:
//! closed, then the open question, then In Progress, then the lane and
//! the edges — `!pullable()` is *yours* — and a pool candidate is
//! *ready*. A brief that said "in progress" where the board shows
//! *holding* would fail the one-glance test.

use serde::Serialize;

use crate::log::{Event, EventId};

use super::flight::{Flight, Fold};
use super::history::{Moment, history};

/// One flight, in full: the fold's record, flat in wire form like
/// `FlightView`. Absent facts are `None`/empty, never
/// missing keys.
#[derive(Debug, Serialize)]
pub struct Brief {
    pub id: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    /// Provenance only: the procedure the filing was minted under, or
    /// a match rule chose at file time.
    pub procedure: Option<String>,
    pub subject: String,
    pub body: String,
    pub filed_by: String,
    /// The filer's session, when the filing carried one.
    pub filed_session: Option<String>,
    /// The filer's callsign, when the filing carried one.
    pub filed_callsign: Option<String>,
    pub filed_at: i64,
    /// The derived status — the brief is the read surface for one
    /// flight, so this is where the fields are meant to be read.
    pub status: String,
    /// Who made the gesture the status rests on, and when — the mover,
    /// the asker, the answerer, or the closer of the dependency that
    /// released it. `None` while the flight still stands where it was
    /// filed.
    pub status_by: Option<String>,
    /// The mover's session, when that gesture carried one.
    pub status_session: Option<String>,
    /// The mover's callsign, when that gesture carried one — the pilot.
    pub status_callsign: Option<String>,
    pub status_at: Option<i64>,
    /// Why the mark is someone else's gesture: "dependency <id> done"
    /// or "… canceled" when a dependency's closing is what made the
    /// flight Ready. `None` when the mark is the flight's own.
    pub status_reason: Option<String>,
    pub assignee: Option<String>,
    pub priority: String,
    pub labels: Vec<String>,
    pub skill: Option<String>,
    /// The last edit touching the record — the flight's own fields or a
    /// comment's text — flat like the status mark.
    pub edited_by: Option<String>,
    pub edited_at: Option<i64>,
    pub question: Option<String>,
    pub asked_by: Option<String>,
    pub asked_at: Option<i64>,
    /// A close's `-m` — a cancel's reason, most often — standing where
    /// the question stood. `None` while the flight is open or when the
    /// close said nothing.
    pub closed_reason: Option<String>,
    /// Closed children over total, whenever this flight has children at
    /// all — the family's progress, rendered `(2/6)`.
    pub progress: Option<(usize, usize)>,
    pub depends_on: Vec<LinkView>,
    pub blocks: Vec<LinkView>,
    /// Reading order, the fold's order.
    pub comments: Vec<CommentView>,
    /// What happened to this flight, oldest first — the log's own
    /// gestures, which the last-wins fold cannot reconstruct.
    pub history: Vec<Moment>,
    /// Where the flight stands, flat on the envelope beside the raw facts
    /// it arbitrates — the reader gets `"standing": "ready"`, no inner
    /// nesting.
    #[serde(flatten)]
    pub standing: Standing,
}

/// Where one flight stands, in `enrich`'s precedence, flattened onto the
/// brief. Every variant is a unit — its facts (the status fields, the
/// question fields, `assignee`) already sit flat on [`Brief`], and a
/// payload here would emit the same keys twice.
#[derive(Debug, Serialize)]
#[serde(tag = "standing", rename_all = "kebab-case")]
pub enum Standing {
    /// Off the board — done or canceled; the log keeps the record.
    Done,
    /// Held on tower's own question — waiting on you.
    Question,
    /// In Progress — someone already flies it; the status mark beside
    /// it says who.
    InProgress,
    /// Not in the pool by the status and the lane alone: not Ready —
    /// Backlog, or Waiting on a live dependency — or in neither the
    /// agent lane nor the reader's own callsign's. Unknown never rounds
    /// down.
    Yours,
    /// In the pool: `next` will hand it out in filed order.
    Ready,
}

/// One linked flight, carrying enough that a reader judges readiness
/// without a second call.
#[derive(Debug, Serialize)]
pub struct LinkView {
    pub flight: String,
    /// The dense per-writer flight number — the human name's numeric
    /// half, beside the wire id.
    pub number: u64,
    pub subject: String,
    pub status: String,
    pub closed: bool,
}

/// A note on the record, as the brief carries it.
#[derive(Debug, Serialize)]
pub struct CommentView {
    /// The wire id — a comment's only name, and what `edit` takes.
    pub id: String,
    pub author: String,
    pub session: Option<String>,
    pub callsign: Option<String>,
    pub at: i64,
    pub text: String,
}

/// The brief for one flight, or `None` when no such flight is filed.
///
/// `events` is the slice `fold` was built from — the history's only
/// source, since the fold keeps marks rather than gestures.
///
/// Enrichment is `enrich`'s per-flight derivation, reused: the status
/// mark and its reason, the progress mark, and the open question.
/// `viewer` is the reader's callsign, which the standing reads: a
/// flight laned to it is in the reader's pool.
pub fn brief(fold: &Fold, events: &[Event], id: &EventId, viewer: Option<&str>) -> Option<Brief> {
    let flight = fold.flights.iter().find(|flight| &flight.id == id)?;
    let standing = standing(flight, viewer);

    Some(Brief {
        id: flight.id.to_string(),
        number: flight.number,
        procedure: flight.procedure.clone(),
        subject: flight.subject.clone(),
        body: flight.body.clone(),
        filed_by: flight.filed_by.clone(),
        filed_session: flight.filed_session.clone(),
        filed_callsign: flight.filed_callsign.clone(),
        filed_at: flight.filed_at,
        status: flight.status.clone(),
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
        status_reason: super::model::status_reason(fold, flight),
        assignee: flight.assignee.clone(),
        priority: flight.priority.clone(),
        labels: flight.labels.clone(),
        skill: flight.skill.clone(),
        edited_by: flight.edited.as_ref().map(|mark| mark.by.clone()),
        edited_at: flight.edited.as_ref().map(|mark| mark.at),
        question: flight.question.as_ref().map(|q| q.text.clone()),
        asked_by: flight.question.as_ref().map(|q| q.by.clone()),
        asked_at: flight.question.as_ref().map(|q| q.at),
        closed_reason: flight.closed_reason.clone(),
        progress: super::model::progress(fold, flight),
        depends_on: links(fold, &flight.depends_on),
        blocks: links(fold, &flight.blocks),
        comments: flight
            .comments
            .iter()
            .map(|comment| CommentView {
                id: comment.id.to_string(),
                author: comment.author.clone(),
                session: comment.session.clone(),
                callsign: comment.callsign.clone(),
                at: comment.at,
                text: comment.text.clone(),
            })
            .collect(),
        history: history(events, id),
        standing,
    })
}

/// Where the flight stands, in enrich's precedence, over the same gate
/// as pick's. A pool candidate is always ready — there is no gate for it
/// to lose.
fn standing(flight: &Flight, viewer: Option<&str>) -> Standing {
    if flight.closed() {
        Standing::Done
    } else if flight.question.is_some() {
        Standing::Question
    } else if flight.status == "in_progress" {
        Standing::InProgress
    } else if flight.pullable(viewer) {
        Standing::Ready
    } else {
        Standing::Yours
    }
}

/// Link rows, resolved inside the fold. Infallible — the fold routes a
/// link with a missing endpoint to `unrouted`, so every carried id names a
/// filed flight.
fn links(fold: &Fold, ids: &[EventId]) -> Vec<LinkView> {
    ids.iter()
        .map(|id| {
            let other = fold
                .flights
                .iter()
                .find(|flight| &flight.id == id)
                .expect("the fold's links resolve");
            LinkView {
                flight: other.id.to_string(),
                number: other.number,
                subject: other.subject.clone(),
                status: other.status.clone(),
                closed: other.closed(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::log::{Event, EventId, Kind};

    /// A filing with the given status and lane stored — the pool gate's
    /// two fields, everything else defaulted.
    fn stored(id: &str, time: i64, status: &str, assignee: Option<&str>) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "filer@b.c".to_string(),
            time,
            session: None,
            callsign: None,
            id,
            kind: Kind::Filed {
                procedure: Some("review".to_string()),
                subject: format!("subject of {time}"),
                body: String::new(),
                status: status.to_string(),
                assignee: assignee.map(str::to_string),
                priority: "none".to_string(),
                labels: Vec::new(),
                skill: None,
                bay: None,
                done: "asserted".to_string(),
                branch: None,
            },
        }
    }

    /// A bare filing with subject and body.
    fn filed(id: &str, time: i64, subject: &str, body: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "filer@b.c".to_string(),
            time,
            session: None,
            callsign: None,
            id,
            kind: Kind::Filed {
                procedure: None,
                subject: subject.to_string(),
                body: body.to_string(),
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

    /// The pool's norm: Ready, agent lane.
    fn agent(id: &str, time: i64) -> Event {
        stored(id, time, "ready", Some("agent"))
    }

    fn lifecycle(id: &str, author: &str, time: i64, kind: Kind) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: author.to_string(),
            time,
            session: None,
            callsign: None,
            id,
            kind,
        }
    }

    fn commented(id: &str, author: &str, time: i64, flight: &str, text: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Commented {
                flight: flight.parse().expect("id"),
                text: text.to_string(),
            },
        )
    }

    fn edited(
        id: &str,
        author: &str,
        time: i64,
        target: &str,
        subject: Option<&str>,
        body: Option<&str>,
    ) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Edited {
                target: target.parse().expect("id"),
                subject: subject.map(str::to_string),
                body: body.map(str::to_string),
                priority: None,
                labels: None,
                skill: None,
                bay: None,
            },
        )
    }

    fn linked(id: &str, time: i64, from: &str, to: &str) -> Event {
        lifecycle(
            id,
            "a@b.c",
            time,
            Kind::Linked {
                from: from.parse().expect("id"),
                to: to.parse().expect("id"),
            },
        )
    }

    fn moved(id: &str, author: &str, time: i64, flight: &str, to: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: to.to_string(),
                reason: None,
            },
        )
    }

    fn held(id: &str, author: &str, time: i64, flight: &str, question: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Held {
                flight: flight.parse().expect("id"),
                question: question.to_string(),
            },
        )
    }

    fn done(id: &str, author: &str, time: i64, flight: &str) -> Event {
        moved(id, author, time, flight, "done")
    }

    fn canceled(id: &str, author: &str, time: i64, flight: &str, reason: &str) -> Event {
        lifecycle(
            id,
            author,
            time,
            Kind::Status {
                flight: flight.parse().expect("id"),
                status: "canceled".to_string(),
                reason: Some(reason.to_string()),
            },
        )
    }

    fn id(text: &str) -> EventId {
        text.parse().expect("id")
    }

    /// `brief` over one slice of events — the fold and the history from
    /// the same log, which is the only honest way to pair them.
    fn brief_of(events: &[Event], id: &EventId) -> Option<Brief> {
        brief(&fold(events), events, id, None)
    }

    #[test]
    fn the_filing_is_carried_whole() {
        let brief = brief_of(
            &[filed("pi.1", 10, "the subject", "the body\ntwo lines")],
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.id, "pi.1");
        assert_eq!(brief.number, 1);
        assert!(brief.procedure.is_none(), "a bare filing has no procedure");
        assert_eq!(brief.subject, "the subject");
        assert_eq!(brief.body, "the body\ntwo lines");
        assert_eq!(brief.filed_by, "filer@b.c");
        assert_eq!(brief.filed_at, 10);
        assert_eq!(brief.status, "backlog");
        assert!(brief.status_by.is_none() && brief.status_at.is_none());
        assert!(brief.assignee.is_none());
        assert_eq!(brief.priority, "none");
        assert!(brief.labels.is_empty());
        assert!(brief.skill.is_none());
    }

    #[test]
    fn the_stored_fields_reach_the_brief() {
        // The brief is the read surface for one flight, so this is where
        // the fields are meant to be read.
        let mut event = filed("pi.1", 10, "the retry test · pass", "");
        let Kind::Filed {
            status,
            assignee,
            priority,
            labels,
            skill,
            ..
        } = &mut event.kind
        else {
            unreachable!("filed");
        };
        *status = "ready".to_string();
        *assignee = Some("agent".to_string());
        *priority = "high".to_string();
        *labels = vec!["chore".to_string()];
        *skill = Some("review".to_string());

        let brief = brief_of(&[event], &id("pi.1")).expect("filed");
        assert_eq!(brief.status, "ready");
        assert_eq!(brief.assignee.as_deref(), Some("agent"));
        assert_eq!(brief.priority, "high");
        assert_eq!(brief.labels, ["chore"]);
        assert_eq!(brief.skill.as_deref(), Some("review"));
    }

    #[test]
    fn the_session_rides_the_byline_to_the_brief() {
        // The author stays the email; the session says which session
        // typed it. A filing's lands beside `filed_by`, a move's beside
        // `status_by`, a comment's on its view, and every gesture's on
        // its history row — and an old chain with none reads as none.
        let session = |mut event: Event, tag: Option<&str>| {
            event.session = tag.map(str::to_string);
            event
        };
        let uuid = "95b36d9d-efdc-4564-9b06-91842f51ef6b";
        let events = [
            session(filed("pi.1", 10, "s", ""), Some(uuid)),
            session(
                lifecycle(
                    "pi.2",
                    "mover@b.c",
                    20,
                    Kind::Status {
                        flight: id("pi.1"),
                        status: "in_progress".to_string(),
                        reason: None,
                    },
                ),
                Some("tyler"),
            ),
            session(commented("pi.3", "one@b.c", 30, "pi.1", "note"), None),
        ];
        let brief = brief_of(&events, &id("pi.1")).expect("filed");
        assert_eq!(brief.filed_by, "filer@b.c");
        assert_eq!(brief.filed_session.as_deref(), Some(uuid));
        assert_eq!(brief.status_by.as_deref(), Some("mover@b.c"));
        assert_eq!(brief.status_session.as_deref(), Some("tyler"));
        assert_eq!(brief.comments[0].author, "one@b.c");
        assert!(brief.comments[0].session.is_none());
        let sessions: Vec<Option<&str>> = brief
            .history
            .iter()
            .map(|moment| moment.session.as_deref())
            .collect();
        assert_eq!(sessions, [Some(uuid), Some("tyler"), None]);

        let untagged: Vec<Event> = events
            .iter()
            .cloned()
            .map(|event| session(event, None))
            .collect();
        let brief = brief_of(&untagged, &id("pi.1")).expect("filed");
        assert!(brief.filed_session.is_none());
        assert!(brief.status_session.is_none());
        assert!(brief.history.iter().all(|moment| moment.session.is_none()));
    }

    #[test]
    fn the_callsign_rides_the_marks_the_comments_and_the_moments() {
        let flown = |mut event: Event, callsign: Option<&str>| {
            event.callsign = callsign.map(str::to_string);
            event
        };
        let events = [
            flown(filed("pi.1", 10, "s", ""), Some("tyler")),
            flown(
                moved("pi.2", "one@b.c", 20, "pi.1", "in_progress"),
                Some("claude"),
            ),
            flown(commented("pi.3", "one@b.c", 30, "pi.1", "note"), None),
        ];
        let brief = brief_of(&events, &id("pi.1")).expect("filed");
        assert_eq!(brief.filed_callsign.as_deref(), Some("tyler"));
        assert_eq!(brief.status_callsign.as_deref(), Some("claude"));
        assert_eq!(brief.status_by.as_deref(), Some("one@b.c"));
        assert!(brief.comments[0].callsign.is_none());
        let callsigns: Vec<Option<&str>> = brief
            .history
            .iter()
            .map(|moment| moment.callsign.as_deref())
            .collect();
        assert_eq!(callsigns, [Some("tyler"), Some("claude"), None]);
    }

    #[test]
    fn a_flight_laned_to_the_viewer_briefs_ready_and_yours_to_anyone_else() {
        let events = [stored("pi.1", 10, "ready", Some("qwen-review"))];
        let folded = fold(&events);
        let own = brief(&folded, &events, &id("pi.1"), Some("qwen-review")).expect("filed");
        assert!(matches!(own.standing, Standing::Ready));
        let other = brief(&folded, &events, &id("pi.1"), Some("claude")).expect("filed");
        assert!(matches!(other.standing, Standing::Yours));
        let nobody = brief(&folded, &events, &id("pi.1"), None).expect("filed");
        assert!(matches!(nobody.standing, Standing::Yours));
    }

    #[test]
    fn comments_arrive_in_reading_order_with_author_and_time() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                commented("pi.2", "one@b.c", 20, "pi.1", "first"),
                commented("pi.3", "two@b.c", 30, "pi.1", "second"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.comments.len(), 2);
        assert_eq!(brief.comments[0].id, "pi.2");
        assert_eq!(brief.comments[0].author, "one@b.c");
        assert_eq!(brief.comments[0].at, 20);
        assert_eq!(brief.comments[0].text, "first");
        assert_eq!(brief.comments[1].id, "pi.3");
        assert_eq!(brief.comments[1].text, "second");
    }

    #[test]
    fn the_edited_mark_lands_flat_and_counts_as_motion() {
        let plain = brief_of(&[filed("pi.1", 10, "s", "")], &id("pi.1")).expect("filed");
        assert!(plain.edited_by.is_none());
        assert!(plain.edited_at.is_none());

        let reworded = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                edited("pi.2", "editor@b.c", 30, "pi.1", Some("reworded"), None),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(reworded.subject, "reworded");
        assert_eq!(reworded.edited_by.as_deref(), Some("editor@b.c"));
        assert_eq!(reworded.edited_at, Some(30));
    }

    #[test]
    fn links_carry_both_directions_with_subjects_and_statuses() {
        let events = [
            filed("pi.1", 10, "the dependent", ""),
            filed("pi.2", 20, "the dependency", ""),
            linked("pi.3", 30, "pi.1", "pi.2"),
            done("pi.4", "a@b.c", 40, "pi.2"),
        ];

        let one = brief_of(&events, &id("pi.1")).expect("filed");
        assert_eq!(one.depends_on.len(), 1);
        assert_eq!(one.depends_on[0].flight, "pi.2");
        assert_eq!(one.depends_on[0].number, 2);
        assert_eq!(one.depends_on[0].subject, "the dependency");
        assert_eq!(one.depends_on[0].status, "done");
        assert!(one.depends_on[0].closed);
        assert!(one.blocks.is_empty());

        let two = brief_of(&events, &id("pi.2")).expect("filed");
        assert_eq!(two.blocks.len(), 1);
        assert_eq!(two.blocks[0].flight, "pi.1");
        assert_eq!(two.blocks[0].number, 1);
        assert_eq!(two.blocks[0].subject, "the dependent");
        assert_eq!(two.blocks[0].status, "backlog");
        assert!(!two.blocks[0].closed);
        assert!(two.depends_on.is_empty());
    }

    #[test]
    fn the_open_question_carries_who_and_when() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                held("pi.2", "asker@b.c", 60, "pi.1", "which?"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.question.as_deref(), Some("which?"));
        assert_eq!(brief.asked_by.as_deref(), Some("asker@b.c"));
        assert_eq!(brief.asked_at, Some(60));
        assert_eq!(brief.status, "held");
        assert!(matches!(brief.standing, Standing::Question));
    }

    #[test]
    fn a_cancel_over_a_question_briefs_the_reason_and_history_keeps_the_hold() {
        let events = [
            filed("pi.1", 10, "s", ""),
            held("pi.2", "asker@b.c", 60, "pi.1", "which?"),
            canceled("pi.3", "a@b.c", 70, "pi.1", "superseded"),
        ];
        let brief = brief_of(&events, &id("pi.1")).expect("filed");
        assert!(brief.question.is_none(), "the close took the question");
        assert!(brief.asked_by.is_none());
        assert!(brief.asked_at.is_none());
        assert_eq!(brief.closed_reason.as_deref(), Some("superseded"));
        assert_eq!(brief.status, "canceled");
        assert!(matches!(brief.standing, Standing::Done));
        assert!(
            brief.history.iter().any(|moment| matches!(
                &moment.detail,
                Some(super::super::history::Detail::Held { question }) if question == "which?"
            )),
            "the hold stays a moment of the record: {:?}",
            brief.history
        );
    }

    #[test]
    fn a_bare_done_over_a_question_briefs_no_question_and_no_reason() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                held("pi.2", "asker@b.c", 60, "pi.1", "which?"),
                done("pi.3", "a@b.c", 70, "pi.1"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert!(brief.question.is_none());
        assert!(brief.asked_by.is_none());
        assert!(brief.asked_at.is_none());
        assert!(brief.closed_reason.is_none());
    }

    #[test]
    fn a_status_move_carries_who_and_when() {
        let brief = brief_of(
            &[
                agent("pi.1", 10),
                moved("pi.2", "crew@b.c", 40, "pi.1", "in_progress"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.status, "in_progress");
        assert_eq!(brief.status_by.as_deref(), Some("crew@b.c"));
        assert_eq!(brief.status_at, Some(40));
        assert!(matches!(brief.standing, Standing::InProgress));
    }

    #[test]
    fn a_closed_flight_briefs_with_its_mark() {
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", "the body"),
                done("pi.2", "closer@b.c", 90, "pi.1"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert_eq!(brief.status, "done");
        assert_eq!(brief.status_by.as_deref(), Some("closer@b.c"));
        assert_eq!(brief.status_at, Some(90));
        assert_eq!(brief.body, "the body");
        assert!(matches!(brief.standing, Standing::Done));
    }

    #[test]
    fn the_briefs_progress_mark_matches_the_boards() {
        let childless = [filed("pi.1", 10, "s", "")];
        let alone = brief_of(&childless, &id("pi.1")).expect("filed");
        assert!(alone.progress.is_none());

        let family = [
            filed("pi.1", 10, "a broad task", ""),
            filed("pi.2", 20, "part one", ""),
            filed("pi.3", 30, "part two", ""),
            linked("pi.4", 40, "pi.1", "pi.2"),
            linked("pi.5", 50, "pi.1", "pi.3"),
            done("pi.6", "a@b.c", 60, "pi.2"),
        ];
        let parent = brief_of(&family, &id("pi.1")).expect("filed");
        assert_eq!(parent.progress, Some((1, 2)));
    }

    #[test]
    fn an_unfiled_id_is_none() {
        assert!(brief_of(&[filed("pi.1", 10, "s", "")], &id("pi.99"),).is_none());
    }

    #[test]
    fn question_outranks_in_progress() {
        // One flight carrying both: the question wins.
        let both = brief_of(
            &[
                agent("pi.1", 10),
                moved("pi.2", "a@b.c", 20, "pi.1", "in_progress"),
                held("pi.3", "a@b.c", 30, "pi.1", "which?"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(both.standing, Standing::Question));
        assert_eq!(both.status, "held", "the flat fact carries the detail");
    }

    #[test]
    fn a_me_laned_flight_is_yours_and_reads_waiting() {
        // Waiting is the status, yours is the standing: the flight is
        // not pullable, and the lane reads off the flat field.
        let brief = brief_of(
            &[
                stored("pi.1", 10, "ready", Some("me")),
                agent("pi.2", 20),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::Yours));
        assert_eq!(brief.status, "waiting");
        assert_eq!(
            brief.assignee.as_deref(),
            Some("me"),
            "the lane reads off the flat field, not the standing"
        );
    }

    #[test]
    fn a_backlog_flight_is_yours_with_no_lane() {
        let brief = brief_of(&[filed("pi.1", 10, "s", "")], &id("pi.1")).expect("filed");
        assert!(matches!(brief.standing, Standing::Yours));
        assert!(brief.assignee.is_none());
    }

    #[test]
    fn a_late_agent_flight_briefs_ready() {
        // `next`'s default want is 1; the standing does not care where in
        // filed order the flight sits.
        let brief = brief_of(
            &[agent("pi.1", 10), agent("pi.2", 20), agent("pi.3", 30)],
            &id("pi.3"),
        )
        .expect("filed");
        assert!(matches!(brief.standing, Standing::Ready));
    }

    #[test]
    fn a_waiting_dependent_is_yours_and_its_release_names_the_closer() {
        let events = [
            agent("pi.1", 10),
            agent("pi.2", 20),
            linked("pi.3", 30, "pi.1", "pi.2"),
        ];

        // Derived Waiting: not a pool candidate.
        let dependent = brief_of(&events, &id("pi.1")).expect("filed");
        assert_eq!(dependent.status, "waiting");
        assert!(matches!(dependent.standing, Standing::Yours));
        assert!(dependent.status_reason.is_none());

        let dependency = brief_of(&events, &id("pi.2")).expect("filed");
        assert!(matches!(dependency.standing, Standing::Ready));

        // The closing releases the dependent, and the brief says whose
        // gesture the Ready is.
        let mut released = events.to_vec();
        released.push(moved("pi.4", "closer@b.c", 40, "pi.2", "canceled"));
        let dependent = brief_of(&released, &id("pi.1")).expect("filed");
        assert_eq!(dependent.status, "ready");
        assert!(matches!(dependent.standing, Standing::Ready));
        assert_eq!(dependent.status_by.as_deref(), Some("closer@b.c"));
        assert_eq!(dependent.status_at, Some(40));
        assert_eq!(
            dependent.status_reason.as_deref(),
            Some("dependency pi.2 canceled")
        );
    }

    #[test]
    fn the_slimmed_standing_carries_no_duplicate_payload() {
        // The mark variants flatten to the tag alone: `status_by` appears
        // once, from the brief's own field, never a second time from the
        // standing.
        let brief = brief_of(
            &[
                filed("pi.1", 10, "s", ""),
                done("pi.2", "closer@b.c", 90, "pi.1"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        let json = serde_json::to_value(&brief).expect("serializes");
        assert_eq!(json["standing"], serde_json::json!("done"));
        assert_eq!(json["status_by"], serde_json::json!("closer@b.c"));
        let text = serde_json::to_string(&brief).expect("serializes");
        assert_eq!(text.matches("\"status_by\"").count(), 1);
    }

    #[test]
    fn a_link_row_carries_its_number_on_the_wire() {
        // The web renders a link from the brief alone — a linked flight
        // may have aged past the board's closed window — so the number
        // rides beside the wire id.
        let brief = brief_of(
            &[
                filed("pi.1", 10, "the dependent", ""),
                filed("pi.2", 20, "the dependency", ""),
                linked("pi.3", 30, "pi.1", "pi.2"),
            ],
            &id("pi.1"),
        )
        .expect("filed");
        let json = serde_json::to_value(&brief).expect("serializes");
        assert_eq!(json["depends_on"][0]["flight"], serde_json::json!("pi.2"));
        assert_eq!(json["depends_on"][0]["number"], serde_json::json!(2));
    }
}

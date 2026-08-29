//! The API: the CLI's `--json` surface, re-exposed over HTTP.
//!
//! Two halves, one contract. The read half is four resources over five
//! GET routes, every one answering the exact envelope the matching verb
//! emits under `--json` — same fold, same serializer, same bytes,
//! trailing newline included. The write half is the verb API: nine POST
//! routes — file, claim, take, requeue, hold, answer, done, comment,
//! triage — each taking the verb's arguments as a small JSON body,
//! appending to the log, and answering the verb's own data envelope. The
//! arguments ride the body rather than the path on purpose: a flight
//! reference can carry `#`, and `#` is a URL fragment. `cmd` on each
//! envelope keeps the CLI's wire name (`/api/bays` answers `bay list`),
//! because the envelope is the contract and the route is only the door.
//! A refusal is the same one-line error envelope, with the id tables and
//! Display strings read from core rather than re-implemented here; only
//! the status varies by surface, the way only the exit code does on the
//! CLI.
//!
//! Every handler runs its whole pipeline inside `spawn_blocking` — the
//! folds spawn ff processes and `Store` is not `Sync` — opening a fresh
//! `Ff` and `Store` per request, exactly what [`run`](crate::run)'s doc
//! promised. The state is the repository path and the feed's receiving
//! end, and only `/api/feed` reads the channel: every GET stays a fresh
//! fold, so serving stale data on the pull side is impossible by
//! construction. The feed itself is one SSE stream pushing the full
//! board envelope — the bytes `/api/board` answers, minus the trailing
//! newline — whenever the repository moves, and a POST appends and
//! answers without publishing anything: the board it changed arrives on
//! the feed through the watcher, the way every writer's does.
//!
//! # Non-goals
//!
//! No caching: every GET is a fresh fold at request time. No exit-code channel:
//! `hold`'s success is a 200 with the full data envelope, and only the
//! CLI says 3. No auth: that posture is deliberately unfiled, and the
//! verb's wide-bind warning says so — the loopback default is the
//! protection this slice has. No `now` on any payload: the reads are
//! time-free, and ages are the human render's alone. No CORS: the GUI is
//! same-origin once built and its dev server proxies, so a CORS layer
//! now would presume the auth posture not yet decided. The per-request
//! cost is accepted — a board or a brief is a handful of local ff
//! spawns, and "render never blocks on the network" holds untouched.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path as RoutePath, State};
use axum::http::{HeaderName, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use ff_tower_core::board::{self, ResolveError, Verdicts};
use ff_tower_core::ff::{self, Ff};
use ff_tower_core::log::{self, Store};
use ff_tower_core::machine;
use ff_tower_core::{procedure, verb};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::{Stream, StreamExt};

use crate::feed::Latest;

/// What every route shares: the repository path the handlers fold from,
/// and the feed channel's receiving end — read by `/api/feed` alone, so
/// the state stays structurally incapable of serving a GET a cached
/// fold.
pub(crate) struct AppState {
    repo: PathBuf,
    feed: watch::Receiver<Latest>,
}

/// The routes, mounted under `/api`.
pub(crate) fn router(repo: &Path, feed: watch::Receiver<Latest>) -> Router {
    Router::new()
        .route("/api/board", get(board))
        .route("/api/feed", get(feed_route))
        .route("/api/brief/{flight}", get(brief))
        .route("/api/bays", get(bays))
        .route("/api/procedures", get(procedures))
        .route("/api/procedures/{name}", get(procedure))
        .route("/api/file", post(file))
        .route("/api/claim", post(claim))
        .route("/api/take", post(take))
        .route("/api/requeue", post(requeue))
        .route("/api/hold", post(hold))
        .route("/api/answer", post(answer))
        .route("/api/done", post(done))
        .route("/api/comment", post(comment))
        .route("/api/triage", post(triage))
        .with_state(Arc::new(AppState {
            repo: repo.to_path_buf(),
            feed,
        }))
}

/// What every route answers with: a status, the JSON content type, and
/// one envelope line.
type Reply = (StatusCode, [(HeaderName, &'static str); 1], String);

/// One request: the sync pipeline on a blocking thread, the envelope —
/// data or error — as the body.
async fn respond(
    cmd: &'static str,
    state: Arc<AppState>,
    work: impl FnOnce(&Path) -> Result<String, ApiError> + Send + 'static,
) -> Reply {
    match tokio::task::spawn_blocking(move || work(&state.repo)).await {
        Ok(Ok(envelope)) => reply(StatusCode::OK, envelope),
        Ok(Err(err)) => refusal(cmd, &err),
        // The pipeline panicked. The server survives it — that is what
        // the blocking thread buys — and the request reports it under
        // the id that already means "the server failed for its own
        // reason".
        Err(_panicked) => reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            machine::emit_error(
                cmd,
                "serve/failed",
                &format!("the fold behind `{cmd}` panicked"),
                &["ff tower explain serve/failed".to_string()],
            ),
        ),
    }
}

/// One write request: the body parsed as the verb's arguments, then the
/// same blocking pipeline every read runs. Not axum's `Json` extractor,
/// whose rejections are not envelope-shaped: a body that does not parse
/// — not JSON at all, a missing field, an unknown field — refuses here
/// as `usage/bad-body`, before any thread is spawned.
async fn act<Body: DeserializeOwned + Send + 'static>(
    cmd: &'static str,
    state: Arc<AppState>,
    body: Bytes,
    verb: impl FnOnce(&Path, Body) -> Result<String, ApiError> + Send + 'static,
) -> Reply {
    match serde_json::from_slice::<Body>(&body) {
        Ok(parsed) => respond(cmd, state, move |repo| verb(repo, parsed)).await,
        Err(err) => refusal(
            cmd,
            &ApiError::Body {
                detail: err.to_string(),
            },
        ),
    }
}

/// A refusal as a reply: the id's status, and the error envelope.
fn refusal(cmd: &str, err: &ApiError) -> Reply {
    reply(
        err.status(),
        machine::emit_error(cmd, err.id(), &err.to_string(), &err.exits()),
    )
}

/// The plumbing every reply shares: the JSON content type, and the
/// trailing newline that is the byte parity with the CLI's `println!`.
fn reply(status: StatusCode, line: String) -> Reply {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        format!("{line}\n"),
    )
}

/// The board's envelope, folded fresh. Shared verbatim between the
/// `/api/board` handler and the feed's refold loop, so a pushed board
/// and a pulled one can only ever be the same bytes.
pub(crate) fn board_envelope(repo: &Path) -> Result<String, ApiError> {
    let ff = Ff::at(repo).env_program();
    let store = Store::open(repo)?;
    let events = store.read_all()?;
    let board = board::assemble(&ff, &events)?;
    Ok(machine::emit("board", &board))
}

async fn board(State(state): State<Arc<AppState>>) -> Reply {
    respond("board", state, board_envelope).await
}

/// The feed: one SSE stream over the watch channel. `WatchStream` hands
/// a new subscriber the current value — the initial board on connect —
/// and laggards skip intermediates, which is right for a full-board
/// broadcast. `Pending` is skipped, `Closing` ends the stream so
/// graceful shutdown can drain, and the default `message` event type
/// stays: the envelope's `cmd` is the contract, the route is only the
/// door.
async fn feed_route(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = WatchStream::new(state.feed.clone())
        .map_while(|latest| match latest {
            Latest::Closing => None,
            live => Some(live),
        })
        .filter_map(|latest| match latest {
            Latest::Board(envelope) => Some(Ok(Event::default().data(envelope))),
            _ => None,
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn brief(State(state): State<Arc<AppState>>, RoutePath(flight): RoutePath<String>) -> Reply {
    respond("brief", state, move |repo| {
        // Syntax first, so a reference that could never name a flight
        // costs no I/O — the CLI verb's own order. `writer#n` arrives
        // percent-encoded (`pi%231`) and the extractor already decoded
        // it.
        board::parse_ref(&flight)?;
        let store = Store::open(repo)?;
        let events = store.read_all()?;
        let fold = board::fold(&events);
        let id = board::resolve(&fold, &flight)?;
        let ff = Ff::at(repo).env_program();
        let reads = board::gather(&ff)?;
        let verdicts = if board::wants_verdicts(&fold, &reads, &id) {
            board::probe(&ff, &fold, &reads)?
        } else {
            Verdicts::default()
        };
        let brief = board::brief(&fold, &events, &reads, &verdicts, &id)
            .expect("resolved to a filed flight");
        Ok(machine::emit("brief", &brief))
    })
    .await
}

async fn bays(State(state): State<Arc<AppState>>) -> Reply {
    respond("bay list", state, |repo| {
        let ff = Ff::at(repo).env_program();
        let store = Store::open(repo)?;
        let fold = board::fold(&store.read_all()?);
        let reads = board::gather(&ff)?;
        let bays = board::bays(&fold, &reads);
        Ok(machine::emit("bay list", &board::Pool { bays }))
    })
    .await
}

async fn procedures(State(state): State<Arc<AppState>>) -> Reply {
    respond("procedures", state, |repo| {
        let store = Store::open(repo)?;
        let root = store.main_worktree();
        let installed = procedure::registry(root.as_deref())?;
        Ok(machine::emit(
            "procedures",
            &procedure::Listing {
                procedures: installed.definitions().collect(),
            },
        ))
    })
    .await
}

async fn procedure(
    State(state): State<Arc<AppState>>,
    RoutePath(name): RoutePath<String>,
) -> Reply {
    respond("procedures", state, move |repo| {
        let store = Store::open(repo)?;
        let root = store.main_worktree();
        let installed = procedure::registry(root.as_deref())?;
        let definition = installed.require(&name)?;
        Ok(machine::emit(
            "procedures",
            &procedure::One {
                procedure: definition,
            },
        ))
    })
    .await
}

/// `file`'s arguments, as the POST body carries them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileBody {
    subject: String,
    message: Option<String>,
    procedure: Option<String>,
}

/// The single-argument verbs: claim, take, requeue, done. The flight is
/// required — for `done` deliberately so, since the server has no
/// invoking worktree to derive one from.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlightBody {
    flight: String,
}

/// The message verbs: hold, answer, comment. `message` stays optional
/// here so its absence reaches core's `usage/needs-message` — one
/// refusal vocabulary on both surfaces.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageBody {
    flight: String,
    message: Option<String>,
}

async fn file(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("file", state, body, |repo, body: FileBody| {
        let store = Store::open(repo)?;
        let outcome = verb::file(&store, &body.subject, body.message, body.procedure)?;
        Ok(machine::emit("file", &outcome.payload))
    })
    .await
}

async fn claim(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("claim", state, body, |repo, body: FlightBody| {
        let store = Store::open(repo)?;
        let outcome = verb::claim(&store, &body.flight)?;
        Ok(machine::emit("claim", &outcome.payload))
    })
    .await
}

async fn take(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("take", state, body, |repo, body: FlightBody| {
        let store = Store::open(repo)?;
        let outcome = verb::take(&store, &body.flight)?;
        Ok(machine::emit("take", &outcome.payload))
    })
    .await
}

async fn requeue(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("requeue", state, body, |repo, body: FlightBody| {
        let store = Store::open(repo)?;
        let outcome = verb::requeue(&store, &body.flight)?;
        Ok(machine::emit("requeue", &outcome.payload))
    })
    .await
}

/// A hold's success is a 200 like any verb's: the exit-code channel that
/// makes it a 3 on the CLI does not exist here, and the envelope's held
/// event already says the flight stopped with a question.
async fn hold(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("hold", state, body, |repo, body: MessageBody| {
        let store = Store::open(repo)?;
        let outcome = verb::hold(&store, &body.flight, body.message)?;
        Ok(machine::emit("hold", &outcome.payload))
    })
    .await
}

async fn answer(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("answer", state, body, |repo, body: MessageBody| {
        let store = Store::open(repo)?;
        let outcome = verb::answer(&store, &body.flight, body.message)?;
        Ok(machine::emit("answer", &outcome.payload))
    })
    .await
}

async fn done(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("done", state, body, |repo, body: FlightBody| {
        let store = Store::open(repo)?;
        let outcome = verb::done(&store, &body.flight)?;
        Ok(machine::emit("done", &outcome.payload))
    })
    .await
}

async fn comment(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("comment", state, body, |repo, body: MessageBody| {
        let store = Store::open(repo)?;
        let outcome = verb::comment(&store, &body.flight, body.message)?;
        Ok(machine::emit("comment", &outcome.payload))
    })
    .await
}

/// `triage`'s arguments. `message` is the `-m` explanation, stored
/// beside the route and never recomputed.
///
/// `procedure` is required where `-p` is optional on the CLI: the CLI's
/// flight-without-procedure branch is a clap-shaped `usage/no-procedure`
/// refusal that has no meaning over a body, where a missing field is
/// `usage/bad-body` — the same 400.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TriageBody {
    flight: String,
    procedure: String,
    message: Option<String>,
}

async fn triage(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("triage", state, body, |repo, body: TriageBody| {
        let store = Store::open(repo)?;
        let outcome = verb::route(&store, &body.flight, &body.procedure, body.message)?;
        Ok(machine::emit("triage", &outcome.payload))
    })
    .await
}

/// What a route can fail with — the serve twin of the CLI's `CliError`,
/// holding no table of its own beyond the one id only this surface can
/// raise: everything else delegates id, message, and exits to the
/// methods core keeps beside each error's variants.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ApiError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Log(#[from] log::Error),
    #[error(transparent)]
    Ff(#[from] ff::Error),
    #[error(transparent)]
    Procedure(#[from] procedure::Error),
    /// A write verb refusing its input — core's table, the CLI's words.
    #[error(transparent)]
    Verb(#[from] verb::Error),
    /// A POST body that does not parse as the verb's arguments — the one
    /// refusal only this surface can raise, since the CLI has clap where
    /// the server has JSON.
    #[error("the POST body is not this verb's JSON: {detail}")]
    Body { detail: String },
}

impl ApiError {
    fn id(&self) -> &str {
        match self {
            ApiError::Resolve(err) => err.id(),
            ApiError::Log(err) => err.id(),
            ApiError::Ff(err) => err.id(),
            ApiError::Procedure(err) => err.id(),
            ApiError::Verb(err) => err.id(),
            ApiError::Body { .. } => "usage/bad-body",
        }
    }

    /// The CLI's `exits_for` fallback, without its registry: the site's
    /// own exits when it has any; a refusal fufu shaped itself keeps
    /// fufu's verbatim, empty included, because its prose lives in
    /// fufu's registry; anything else points at the lookup that holds
    /// the prose.
    fn exits(&self) -> Vec<String> {
        if let ApiError::Ff(err @ ff::Error::Ff(_)) = self {
            return err.exits();
        }
        let own = match self {
            ApiError::Resolve(err) => err.exits(),
            ApiError::Log(err) => err.exits(),
            ApiError::Ff(err) => err.exits(),
            ApiError::Procedure(err) => err.exits(),
            ApiError::Verb(err) => err.exits(),
            ApiError::Body { .. } => Vec::new(),
        };
        if own.is_empty() {
            vec![format!("ff tower explain {}", self.id())]
        } else {
            own
        }
    }

    /// The status, from the id — one rule, stated once. The syntax
    /// failed: 400. The resolution failed: 404 — the reference is
    /// well-formed, it just names no one resource; the envelope's id
    /// carries the distinction the way the exit code does on the CLI. A
    /// guard refused the write because of the board's standing state:
    /// 409, the conflict being with what some earlier write already
    /// made true. The log's CAS lost thrice: 503, worth an immediate
    /// retry where a 500 is not. The pipeline itself failed: 500.
    fn status(&self) -> StatusCode {
        let id = self.id();
        if id.starts_with("usage/") {
            StatusCode::BAD_REQUEST
        } else if matches!(
            id,
            "flight/not-found" | "flight/ambiguous" | "procedure/not-found"
        ) {
            StatusCode::NOT_FOUND
        } else if matches!(
            id,
            "claim/taken"
                | "take/taken"
                | "requeue/unclaimed"
                | "hold/exists"
                | "answer/not-held"
                | "flight/done"
        ) {
            StatusCode::CONFLICT
        } else if id == "log/contended" {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The status table over the ids the HTTP suite cannot deterministically
    /// provoke: contention is a live race, so its 503 is pinned here, with a
    /// guard refusal's 409 and the bad body's 400 beside it for the table's
    /// other two rows.
    #[test]
    fn the_status_table_maps_contention_conflict_and_bad_body() {
        let contended = ApiError::Log(log::Error::Contended {
            writer: "pi".to_string(),
        });
        assert_eq!(contended.status(), StatusCode::SERVICE_UNAVAILABLE);

        let taken = ApiError::Verb(verb::Error::AlreadyClaimed {
            display: "#1".to_string(),
            by: "someone@tower.invalid".to_string(),
        });
        assert_eq!(taken.status(), StatusCode::CONFLICT);

        let garbled = ApiError::Body {
            detail: "expected value".to_string(),
        };
        assert_eq!(garbled.id(), "usage/bad-body");
        assert_eq!(garbled.status(), StatusCode::BAD_REQUEST);
    }
}

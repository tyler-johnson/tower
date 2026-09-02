//! The API: the CLI's `--json` surface, re-exposed over HTTP.
//!
//! Two halves, one contract. The read half is five resources over six
//! GET routes, every one answering the envelope the matching verb emits
//! under `--json` — same fold, same serializer, same bytes, trailing
//! newline included. The board route is the one that takes an argument:
//! `/api/board?<query>` folds the rows through the query string `ff
//! tower`'s views store and answers the groups and the two counts,
//! `Query::default()` when the string is empty — the CLI board's rows
//! under the CLI board's grouping. The write half is the verb API: twelve
//! POST routes — file, assign, status, hold, answer, done, cancel,
//! comment, decompose, and the three under `/api/views` — each taking
//! the verb's arguments as a small JSON body, appending to the log, and
//! answering the verb's own data envelope. The views ride their own GET
//! rather than the board envelope: a saved view is not a row.
//! The arguments ride the body rather than the path on purpose: a flight
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
//! construction. The feed is one SSE stream per query: `/api/feed?
//! <query>` folds its own frame against its own query on connect and
//! again whenever the channel stamps motion, and the frame is
//! `/api/board?<same query>`'s body minus the trailing newline. A
//! different query is a new subscription. A POST appends and answers
//! without publishing anything: the board it changed arrives on the
//! feed through the watcher, the way every writer's does.
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
use axum::extract::{Path as RoutePath, RawQuery, State};
use axum::http::{HeaderName, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use ff_tower_core::board::{self, Query, ResolveError, Verdicts};
use ff_tower_core::config;
use ff_tower_core::ff::{self, Ff};
use ff_tower_core::log::{self, Store};
use ff_tower_core::machine;
use ff_tower_core::{procedure, verb};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, watch};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use crate::feed::Latest;

/// What every route shares: the repository path the handlers fold from,
/// and the feed channel's receiving end — the motion stamp, read by
/// `/api/feed` alone and carrying no board, so the state stays
/// structurally incapable of serving a GET a cached fold.
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
        .route("/api/assign", post(assign))
        .route("/api/status", post(status))
        .route("/api/hold", post(hold))
        .route("/api/answer", post(answer))
        .route("/api/done", post(done))
        .route("/api/cancel", post(cancel))
        .route("/api/comment", post(comment))
        .route("/api/decompose", post(decompose))
        .route("/api/views", get(views))
        .route("/api/views/save", post(view_save))
        .route("/api/views/edit", post(view_edit))
        .route("/api/views/delete", post(view_delete))
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

/// The board audit's threshold for this repository — the CLI's read,
/// made once per request so a served board says the same thing a typed
/// one does. A repository whose config will not open gets the compiled
/// default rather than a failed request.
fn stale_after(repo: &Path) -> i64 {
    config::Config::open(repo)
        .as_ref()
        .map(config::stale_flight_threshold)
        .unwrap_or(config::DEFAULT_STALE_FLIGHT)
}

/// The lazy pass, run ahead of a read so it serves a settled board. The
/// `/api/board` handler runs it before its fold, and the feed loop runs
/// it once per motion ahead of every subscriber's fold — so the pass's
/// own append re-trips the watcher, the next pass concludes nothing,
/// and the loop stamps once more and stops. Best-effort: a pass that
/// cannot run is one stderr line, and the board still folds; a lost
/// race (`log/contended`) is another pass having concluded the same
/// things. The error is the store not opening, which is the fold's
/// failure too.
pub(crate) fn pass(repo: &Path) -> Result<(), ApiError> {
    let store = Store::open(repo)?;
    match procedure::registry(store.main_worktree().as_deref()) {
        Ok(installed) => match verb::pass(&store, &installed) {
            Ok(_) | Err(log::Error::Contended { .. }) => {}
            Err(err) => eprintln!("ff-tower-serve: the pass did not run: {err}"),
        },
        Err(err) => eprintln!("ff-tower-serve: the pass did not run: {err}"),
    }
    Ok(())
}

/// The board's envelope for one query, folded fresh. Shared verbatim
/// between the `/api/board` handler and every feed subscriber, so a
/// pushed frame and a pulled body under one query can only ever be the
/// same bytes.
pub(crate) fn board_envelope(repo: &Path, query: &Query) -> Result<String, ApiError> {
    let ff = Ff::at(repo).env_program();
    let store = Store::open(repo)?;
    let events = store.read_all()?;
    let folded = board::answer(&ff, &events, board::now(), stale_after(repo), query)?;
    Ok(machine::emit("board", &folded))
}

/// The query string, parsed before any thread is spawned — the way
/// `act` parses a body first. `Query::parse` takes the empty string as
/// the default query, so a bare `/api/board` is the CLI board's rows.
fn parse_query(raw: Option<String>) -> Result<Query, ApiError> {
    Ok(Query::parse(raw.as_deref().unwrap_or_default())?)
}

async fn board(State(state): State<Arc<AppState>>, RawQuery(raw): RawQuery) -> Reply {
    match parse_query(raw) {
        Ok(query) => {
            respond("board", state, move |repo| {
                pass(repo)?;
                board_envelope(repo, &query)
            })
            .await
        }
        Err(err) => refusal("board", &err),
    }
}

/// The feed: one SSE stream per query. The query parses first, and a
/// failure is the same 400 the board route answers. Otherwise one task
/// per subscriber holds the watch receiver and the query, folds a frame
/// on every stamp, and sends it down a channel of one: a slow client
/// backpressures its own task, and the watch coalesces the stamps it
/// missed — the laggard behavior a broadcast has, with the fold per
/// subscriber. `Pending` waits, `Closing` ends the stream so graceful
/// shutdown can drain, and the default `message` event type stays: the
/// envelope's `cmd` is the contract, the route is only the door.
async fn feed_route(State(state): State<Arc<AppState>>, RawQuery(raw): RawQuery) -> Response {
    let query = match parse_query(raw) {
        Ok(query) => query,
        Err(err) => return refusal("board", &err).into_response(),
    };
    let (frames, rx) = mpsc::channel::<String>(1);
    tokio::spawn(subscribe(
        state.feed.clone(),
        state.repo.clone(),
        query,
        frames,
    ));
    let stream =
        ReceiverStream::new(rx).map(|data| Ok::<_, Infallible>(Event::default().data(data)));
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// One subscriber's loop: fold on the current stamp, wait for the next.
/// A frame that will not send is the client gone, and the task ends
/// with it; a fold that fails, panic included, says one stderr line
/// and sends nothing, so the last frame stands.
async fn subscribe(
    mut rx: watch::Receiver<Latest>,
    repo: PathBuf,
    query: Query,
    frames: mpsc::Sender<String>,
) {
    loop {
        let latest = rx.borrow_and_update().clone();
        match latest {
            Latest::Pending => {}
            Latest::Moved => {
                let fold = {
                    let repo = repo.clone();
                    let query = query.clone();
                    tokio::task::spawn_blocking(move || board_envelope(&repo, &query)).await
                };
                match fold {
                    Ok(Ok(frame)) => {
                        if frames.send(frame).await.is_err() {
                            return;
                        }
                    }
                    Ok(Err(err)) => {
                        eprintln!("a feed subscriber's fold failed; its last frame stands: {err}");
                    }
                    Err(_panicked) => {
                        eprintln!(
                            "a feed subscriber's fold failed; its last frame stands: the fold panicked"
                        );
                    }
                }
            }
            Latest::Closing => return,
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
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
        let brief = board::brief(
            &fold,
            &events,
            &reads,
            &verdicts,
            &id,
            board::now(),
            stale_after(repo),
        )
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

/// `file`'s arguments, as the POST body carries them — every stored
/// field the CLI's flags set.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileBody {
    subject: String,
    message: Option<String>,
    procedure: Option<String>,
    priority: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    skill: Option<String>,
    assignee: Option<String>,
    bay: Option<String>,
}

/// `assign`'s arguments: the lane is required — `none` clears it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignBody {
    flight: String,
    assignee: String,
}

/// `status`'s arguments: the move is required, the reason optional.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusBody {
    flight: String,
    status: String,
    message: Option<String>,
}

/// The single-argument verb: done. The flight is required, deliberately
/// so, since the server has no invoking worktree to derive one from.
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

/// `view save`'s arguments: the name and the query are required, and
/// `shared` defaults to personal.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewSaveBody {
    name: String,
    query: String,
    #[serde(default)]
    shared: bool,
}

/// `view edit`'s arguments: the view is required, the three fields
/// optional so an all-absent body reaches core's `usage/needs-edit`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewEditBody {
    view: String,
    name: Option<String>,
    query: Option<String>,
    shared: Option<bool>,
}

/// `view delete`'s one argument.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewBody {
    view: String,
}

/// `decompose`'s arguments: the parts are required, so an omitted list
/// is `usage/bad-body` and an empty one reaches core's `usage/no-parts`
/// — the refusal a caller who meant to split into nothing deserves.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecomposeBody {
    flight: String,
    parts: Vec<String>,
}

async fn file(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("file", state, body, |repo, body: FileBody| {
        let store = Store::open(repo)?;
        let outcome = verb::file(
            &store,
            &body.subject,
            verb::Fields {
                message: body.message,
                priority: body.priority,
                labels: body.labels,
                skill: body.skill,
                assignee: body.assignee,
                bay: body.bay,
            },
            body.procedure.as_deref(),
        )?;
        Ok(machine::emit("file", &outcome.payload))
    })
    .await
}

async fn assign(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("assign", state, body, |repo, body: AssignBody| {
        let store = Store::open(repo)?;
        let outcome = verb::assign(&store, &body.flight, &body.assignee)?;
        Ok(machine::emit("assign", &outcome.payload))
    })
    .await
}

async fn status(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("status", state, body, |repo, body: StatusBody| {
        let store = Store::open(repo)?;
        let outcome = verb::status(&store, &body.flight, &body.status, body.message)?;
        Ok(machine::emit("status", &outcome.payload))
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

async fn cancel(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("cancel", state, body, |repo, body: MessageBody| {
        let store = Store::open(repo)?;
        let outcome = verb::cancel(&store, &body.flight, body.message)?;
        Ok(machine::emit("cancel", &outcome.payload))
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

async fn decompose(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("decompose", state, body, |repo, body: DecomposeBody| {
        let store = Store::open(repo)?;
        let outcome = verb::decompose(&store, &body.flight, &body.parts)?;
        Ok(machine::emit("decompose", &outcome.payload))
    })
    .await
}

/// The views this process's author sees — the store's git identity is
/// the viewer, since no request carries one.
async fn views(State(state): State<Arc<AppState>>) -> Reply {
    respond("view list", state, |repo| {
        let store = Store::open(repo)?;
        Ok(machine::emit("view list", &verb::views(&store)?))
    })
    .await
}

async fn view_save(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("view save", state, body, |repo, body: ViewSaveBody| {
        let store = Store::open(repo)?;
        let outcome = verb::save(&store, &body.name, &body.query, body.shared)?;
        Ok(machine::emit("view save", &outcome.payload))
    })
    .await
}

async fn view_edit(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("view edit", state, body, |repo, body: ViewEditBody| {
        let store = Store::open(repo)?;
        let outcome = verb::edit(&store, &body.view, body.name, body.query, body.shared)?;
        Ok(machine::emit("view edit", &outcome.payload))
    })
    .await
}

async fn view_delete(State(state): State<Arc<AppState>>, body: Bytes) -> Reply {
    act("view delete", state, body, |repo, body: ViewBody| {
        let store = Store::open(repo)?;
        let outcome = verb::delete(&store, &body.view)?;
        Ok(machine::emit("view delete", &outcome.payload))
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
    /// A query string the codec declined — the board and feed routes'
    /// argument, under the ids the CLI's views verbs raise for the same
    /// text.
    #[error(transparent)]
    Query(#[from] board::QueryError),
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
            ApiError::Query(err) => err.id(),
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
            ApiError::Query(err) => err.exits(),
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
            "flight/not-found" | "flight/ambiguous" | "procedure/not-found" | "view/not-found"
        ) {
            StatusCode::NOT_FOUND
        } else if matches!(
            id,
            "flight/done" | "hold/exists" | "answer/not-held" | "status/held"
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

        let held = ApiError::Verb(verb::Error::StatusHeld {
            display: "#1".to_string(),
            question: "which way?".to_string(),
        });
        assert_eq!(held.status(), StatusCode::CONFLICT);

        let garbled = ApiError::Body {
            detail: "expected value".to_string(),
        };
        assert_eq!(garbled.id(), "usage/bad-body");
        assert_eq!(garbled.status(), StatusCode::BAD_REQUEST);

        // A view reference naming nothing visible is a 404 like a
        // flight's, and a query the codec declined arrives through the
        // verb under its own `usage/*` id.
        let unseen = ApiError::Verb(verb::Error::ViewNotFound {
            text: "pi.9".to_string(),
        });
        assert_eq!(unseen.status(), StatusCode::NOT_FOUND);
        let misspelled = ApiError::Verb(verb::Error::Query(board::QueryError::UnknownField {
            text: "bogus".to_string(),
        }));
        assert_eq!(misspelled.id(), "usage/unknown-field");
        assert_eq!(misspelled.status(), StatusCode::BAD_REQUEST);

        // The same text on a board or feed URL arrives as the direct
        // variant: the same id, the same 400, and the query's own exit.
        let on_the_url = ApiError::Query(board::QueryError::UnknownField {
            text: "bogus".to_string(),
        });
        assert_eq!(on_the_url.id(), "usage/unknown-field");
        assert_eq!(on_the_url.status(), StatusCode::BAD_REQUEST);
        assert_eq!(on_the_url.exits(), vec!["ff tower".to_string()]);
    }
}

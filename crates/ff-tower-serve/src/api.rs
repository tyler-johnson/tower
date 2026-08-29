//! The read API: the CLI's `--json` folds, re-exposed over HTTP.
//!
//! Four resources, five routes, every one a GET answering the exact
//! envelope the matching verb emits under `--json` — same fold, same
//! serializer, same bytes, trailing newline included. `cmd` on each
//! envelope keeps the CLI's wire name (`/api/bays` answers `bay list`),
//! because the envelope is the contract and the route is only the door.
//! A refusal is the same one-line error envelope, with the id tables and
//! Display strings read from core rather than re-implemented here.
//!
//! Every handler runs its whole pipeline inside `spawn_blocking` — the
//! folds spawn ff processes and `Store` is not `Sync` — opening a fresh
//! `Ff` and `Store` per request, exactly what [`run`](crate::run)'s doc
//! promised. The only state is the repository path, so serving stale
//! data is impossible by construction.
//!
//! # Non-goals
//!
//! No caching: every response is a fresh fold at request time. No write
//! endpoints, and no change feed — SSE is its own flight. No `now` on
//! any payload: all four are time-free, and ages are the human render's
//! alone. No CORS: the GUI is same-origin once built and its dev server
//! proxies, so a CORS layer now would presume an auth posture not yet
//! decided. The per-request cost is accepted — a board or a brief is a
//! handful of local ff spawns, and "render never blocks on the network"
//! holds untouched.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path as RoutePath, State};
use axum::http::{HeaderName, StatusCode, header};
use axum::routing::get;
use ff_tower_core::board::{self, ResolveError, Verdicts};
use ff_tower_core::ff::{self, Ff};
use ff_tower_core::log::{self, Store};
use ff_tower_core::machine;
use ff_tower_core::procedure;

/// The routes, mounted under `/api`. State is the repository path and
/// nothing else — plain data, `Sync`, and structurally incapable of
/// caching a fold.
pub(crate) fn router(repo: &Path) -> Router {
    Router::new()
        .route("/api/board", get(board))
        .route("/api/brief/{flight}", get(brief))
        .route("/api/bays", get(bays))
        .route("/api/procedures", get(procedures))
        .route("/api/procedures/{name}", get(procedure))
        .with_state(Arc::new(repo.to_path_buf()))
}

/// What every route answers with: a status, the JSON content type, and
/// one envelope line.
type Reply = (StatusCode, [(HeaderName, &'static str); 1], String);

/// One request: the sync pipeline on a blocking thread, the envelope —
/// data or error — as the body. The trailing newline is the byte parity
/// with the CLI's `println!`.
async fn answer(
    cmd: &'static str,
    repo: Arc<PathBuf>,
    fold: impl FnOnce(&Path) -> Result<String, ApiError> + Send + 'static,
) -> Reply {
    let (status, line) = match tokio::task::spawn_blocking(move || fold(&repo)).await {
        Ok(Ok(envelope)) => (StatusCode::OK, envelope),
        Ok(Err(err)) => (
            err.status(),
            machine::emit_error(cmd, err.id(), &err.to_string(), &err.exits()),
        ),
        // The fold panicked. The server survives it — that is what the
        // blocking thread buys — and the request reports it under the id
        // that already means "the server failed for its own reason".
        Err(_panicked) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            machine::emit_error(
                cmd,
                "serve/failed",
                &format!("the fold behind `{cmd}` panicked"),
                &["ff tower explain serve/failed".to_string()],
            ),
        ),
    };
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        format!("{line}\n"),
    )
}

async fn board(State(repo): State<Arc<PathBuf>>) -> Reply {
    answer("board", repo, |repo| {
        let ff = Ff::at(repo).env_program();
        let store = Store::open(repo)?;
        let events = store.read_all()?;
        let board = board::assemble(&ff, &events)?;
        Ok(machine::emit("board", &board))
    })
    .await
}

async fn brief(State(repo): State<Arc<PathBuf>>, RoutePath(flight): RoutePath<String>) -> Reply {
    answer("brief", repo, move |repo| {
        // Syntax first, so a reference that could never name a flight
        // costs no I/O — the CLI verb's own order. `writer#n` arrives
        // percent-encoded (`pi%231`) and the extractor already decoded
        // it.
        board::parse_ref(&flight)?;
        let store = Store::open(repo)?;
        let fold = board::fold(&store.read_all()?);
        let id = board::resolve(&fold, &flight)?;
        let ff = Ff::at(repo).env_program();
        let reads = board::gather(&ff)?;
        let verdicts = if board::wants_verdicts(&fold, &reads, &id) {
            board::probe(&ff, &fold, &reads)?
        } else {
            Verdicts::default()
        };
        let brief =
            board::brief(&fold, &reads, &verdicts, &id).expect("resolved to a filed flight");
        Ok(machine::emit("brief", &brief))
    })
    .await
}

async fn bays(State(repo): State<Arc<PathBuf>>) -> Reply {
    answer("bay list", repo, |repo| {
        let ff = Ff::at(repo).env_program();
        let store = Store::open(repo)?;
        let fold = board::fold(&store.read_all()?);
        let reads = board::gather(&ff)?;
        let bays = board::bays(&fold, &reads);
        Ok(machine::emit("bay list", &board::Pool { bays }))
    })
    .await
}

async fn procedures(State(repo): State<Arc<PathBuf>>) -> Reply {
    answer("procedures", repo, |repo| {
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

async fn procedure(State(repo): State<Arc<PathBuf>>, RoutePath(name): RoutePath<String>) -> Reply {
    answer("procedures", repo, move |repo| {
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

/// What a route can fail with — the serve twin of the CLI's `CliError`,
/// holding no table of its own: id, message, and exits all delegate to
/// the methods core keeps beside each error's variants.
#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Log(#[from] log::Error),
    #[error(transparent)]
    Ff(#[from] ff::Error),
    #[error(transparent)]
    Procedure(#[from] procedure::Error),
}

impl ApiError {
    fn id(&self) -> &str {
        match self {
            ApiError::Resolve(err) => err.id(),
            ApiError::Log(err) => err.id(),
            ApiError::Ff(err) => err.id(),
            ApiError::Procedure(err) => err.id(),
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
    /// carries the distinction the way the exit code does on the CLI.
    /// The fold failed: 500.
    fn status(&self) -> StatusCode {
        let id = self.id();
        if id.starts_with("usage/") {
            StatusCode::BAD_REQUEST
        } else if matches!(
            id,
            "flight/not-found" | "flight/ambiguous" | "procedure/not-found"
        ) {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

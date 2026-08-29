//! The standing process behind `ff tower serve`.
//!
//! DESIGN.md's line on it is the whole of the constraint: a clock and a
//! subscriber rather than an actor. It holds no state the log does not,
//! decides nothing, and dispatches nothing; every other interface keeps
//! working with it down, just staler. A person starts it, and tower never
//! starts it for them.
//!
//! Today it is the placeholder page and the API under `/api`: every GET
//! a fresh fold answering the CLI's own `--json` envelope, the verb
//! API — eight POSTs appending to the log and answering the same way —
//! and the change feed: `GET /api/feed`, one SSE stream pushing the full
//! board envelope whenever the repository moves, whoever moved it.
//!
//! # Shape
//!
//! [`run`] is synchronous, and the runtime is built inside it, so tokio
//! stays behind this crate's boundary — the CLI links a server, not an
//! async world. It returns once the socket is bound, before a byte is
//! served, because the caller has an address to announce and `--json`
//! promises that envelope at startup rather than at shutdown. Serving is
//! [`Bound::serve`], and it does not return until Ctrl-C.
//!
//! # No lock
//!
//! A second server on one repository is another writer, indistinguishable
//! from the CLI or an agent in a bay — the log already serializes writers
//! and names contention, so two of these cost duplicated work and never
//! correctness. What is worth refusing is two processes wanting one port,
//! and the socket says that itself: `EADDRINUSE` at bind, rendered as a
//! tower refusal. A lock file would go stale on every Ctrl-C, since
//! `Drop` does not run on a default SIGINT.

mod api;
mod error;
mod feed;

pub use error::{Error, Result};

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use ff_tower_core::log::{Store, WatchPaths};
use tokio::net::TcpListener;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::watch;

use feed::Latest;

/// The page `GET /` answers with until there is a real build output to
/// serve. Embedded next to the Rust, `integ`'s convention: the constant
/// is the staleness fingerprint, so the file carries no version or hash
/// of its own.
const PLACEHOLDER: &str = include_str!("placeholder.html");

/// A bound server: the socket is open and the address is settled, and
/// nothing is being answered yet.
pub struct Bound {
    addr: SocketAddr,
    listener: TcpListener,
    runtime: Runtime,
    router: Router,
    repo: PathBuf,
    paths: WatchPaths,
    feed: Arc<watch::Sender<Latest>>,
}

impl Bound {
    /// Where it landed. Not always the port that was asked for — port 0
    /// is a real request for whatever is free — so this is what the
    /// caller announces.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Start the change feed, then answer requests until Ctrl-C.
    ///
    /// The feed comes first, and its watcher failing to install is a
    /// hard startup failure under the id that already means "the server
    /// failed for its own reason": a feed that silently missed tower's
    /// own ref writes is the one outcome the design forbids.
    ///
    /// Graceful shutdown sends [`Latest::Closing`] before the drain, and
    /// that send is load-bearing: hyper waits for in-flight responses,
    /// an SSE stream never finishes on its own, and without the close a
    /// Ctrl-C with a browser attached would hang forever. The watch
    /// child is `kill_on_drop`, so the runtime going down reaps it.
    pub fn serve(self) -> Result<()> {
        let Bound {
            addr,
            listener,
            runtime,
            router,
            repo,
            paths,
            feed,
        } = self;
        runtime.block_on(async {
            feed::start(repo, paths, Arc::clone(&feed)).map_err(|source| Error::Failed {
                addr,
                source: std::io::Error::other(source),
            })?;
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    let _ = feed.send(Latest::Closing);
                })
                .await
                .map_err(|source| Error::Failed { addr, source })
        })
    }
}

/// Validate the repository, build the runtime, and bind the socket.
///
/// The repository comes first and on purpose: `Ff::here()` answers with
/// the current directory outside a worktree, and `Store::open` resolves
/// the author and refuses without `user.email`. Both are startup facts,
/// and a person who typed the verb in the wrong directory should hear it
/// now rather than from a blank page. The store is opened and dropped —
/// it never writes, it holds a `gix::Repository` and is not `Sync`, and a
/// route that needs the log opens its own, the way every CLI verb calls
/// `cmd::store()` fresh.
///
/// The default is the loopback, and the caller may say otherwise: the
/// address arrives already parsed and this crate does not judge it. What
/// a wider bind means for a board with no authentication in front of it
/// is the verb's sentence to say, once, where a person can read it.
pub fn run(repo: &Path, host: IpAddr, port: u16) -> Result<Bound> {
    // The store is the validation, and the watch paths ride out before
    // it drops: they are plain paths off the common dir, the one thing
    // the feed needs from gix.
    let paths = Store::open(repo)?.watch_paths();

    let wanted = SocketAddr::from((host, port));

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|source| Error::Failed {
            addr: wanted,
            source,
        })?;

    let listener = runtime
        .block_on(TcpListener::bind(wanted))
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::AddrInUse => Error::AddressInUse { addr: wanted },
            std::io::ErrorKind::AddrNotAvailable => Error::NoSuchAddress { addr: wanted },
            _ => Error::Failed {
                addr: wanted,
                source,
            },
        })?;
    let addr = listener.local_addr().map_err(|source| Error::Failed {
        addr: wanted,
        source,
    })?;

    // Seeded `Pending`, which no stream forwards: the first thing a
    // subscriber can see is the first fold.
    let (tx, rx) = watch::channel(Latest::Pending);

    Ok(Bound {
        addr,
        listener,
        runtime,
        router: router(repo, rx),
        repo: repo.to_path_buf(),
        paths,
        feed: Arc::new(tx),
    })
}

/// The placeholder, the read API, and a fallback. The fallback is the
/// point of the fallback: an unmounted path is visibly not there yet
/// rather than mysteriously blank — and it stays plain text even under
/// `/api/`, because an envelope needs a `cmd` and an unmounted path has
/// no verb.
fn router(repo: &Path, feed: watch::Receiver<Latest>) -> Router {
    Router::new()
        .route("/", get(placeholder))
        .merge(api::router(repo, feed))
        .fallback(missing)
}

async fn placeholder() -> Html<&'static str> {
    Html(PLACEHOLDER)
}

async fn missing() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "nothing is mounted here yet\n")
}

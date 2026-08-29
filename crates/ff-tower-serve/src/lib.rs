//! The standing process behind `ff tower serve`.
//!
//! DESIGN.md's line on it is the whole of the constraint: a clock and a
//! subscriber rather than an actor. It holds no state the log does not,
//! decides nothing, and dispatches nothing; every other interface keeps
//! working with it down, just staler. A person starts it, and tower never
//! starts it for them.
//!
//! Today it is the board and the API under `/api`. The board is the
//! SvelteKit build, embedded at compile time and answered by the
//! fallback (see `assets`): every non-`/api` path is a file out of the
//! build or `index.html`, so the binary serves the real app with no
//! Node at runtime. The API is every GET a fresh fold answering the
//! CLI's own `--json` envelope, the verb API — POSTs appending to the
//! log and answering the same way — and the change feed: `GET
//! /api/feed`, one SSE stream pushing the full board envelope whenever
//! the repository moves, whoever moved it.
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
mod assets;
mod error;
mod feed;

pub use error::{Error, Result};

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use ff_tower_core::log::{Store, WatchPaths};
use tokio::net::TcpListener;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::watch;

use feed::Latest;

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

/// The API and the app. `api::router` owns everything under `/api`, and
/// the fallback is the embedded web build — the file the path names, or
/// `index.html` for everything the build does not, `/` included. The
/// unmounted answers survive in the fallback too: a non-GET, or an
/// `/api` path nothing is mounted on, is visibly not there yet rather
/// than mysteriously blank — and stays plain text even under `/api/`,
/// because an envelope needs a `cmd` and an unmounted path has no verb.
fn router(repo: &Path, feed: watch::Receiver<Latest>) -> Router {
    api::router(repo, feed).fallback(assets::spa)
}

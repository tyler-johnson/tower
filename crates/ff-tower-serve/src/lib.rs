//! The standing process behind `ff tower serve`.
//!
//! DESIGN.md's line on it is the whole of the constraint: a clock and a
//! subscriber rather than an actor. It holds no state the log does not,
//! decides nothing, and dispatches nothing; every other interface keeps
//! working with it down, just staler. A person starts it, and tower never
//! starts it for them.
//!
//! Today it is a bound socket and a placeholder page. The read API, the
//! verb API and the change feed mount onto this; nothing of them is here.
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

mod error;

pub use error::{Error, Result};

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use ff_tower_core::log::Store;
use tokio::net::TcpListener;
use tokio::runtime::{Builder, Runtime};

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
}

impl Bound {
    /// Where it landed. Not always the port that was asked for — port 0
    /// is a real request for whatever is free — so this is what the
    /// caller announces.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Answer requests until Ctrl-C. Graceful shutdown is not
    /// load-bearing without a lock to release, but it is free and it is
    /// what a person pressing Ctrl-C means.
    pub fn serve(self) -> Result<()> {
        let Bound {
            addr,
            listener,
            runtime,
        } = self;
        runtime
            .block_on(async {
                axum::serve(listener, router())
                    .with_graceful_shutdown(async {
                        let _ = tokio::signal::ctrl_c().await;
                    })
                    .await
            })
            .map_err(|source| Error::Failed {
                port: addr.port(),
                source,
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
/// The socket is `127.0.0.1` and never `0.0.0.0`. This is a process a
/// person started for themselves.
pub fn run(repo: &Path, port: u16) -> Result<Bound> {
    drop(Store::open(repo)?);

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|source| Error::Failed { port, source })?;

    let listener = runtime
        .block_on(TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))))
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::AddrInUse => Error::AddressInUse { port },
            _ => Error::Failed { port, source },
        })?;
    let addr = listener
        .local_addr()
        .map_err(|source| Error::Failed { port, source })?;

    Ok(Bound {
        addr,
        listener,
        runtime,
    })
}

/// One route and a fallback. The fallback is the point of the fallback:
/// an unmounted path is visibly not there yet rather than mysteriously
/// blank.
fn router() -> Router {
    Router::new().route("/", get(placeholder)).fallback(missing)
}

async fn placeholder() -> Html<&'static str> {
    Html(PLACEHOLDER)
}

async fn missing() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "nothing is mounted here yet\n")
}

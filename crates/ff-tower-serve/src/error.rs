//! What the server can fail with before it is anyone's problem but the
//! person who typed the verb.
//!
//! Three ids, all raised at startup. A repository the store refuses is
//! none of them: that error is core's, and the CLI flattens it back
//! into its own `Log` arm so `identity/missing` keeps the id it has
//! everywhere else.
//!
//! Every one of them carries the whole `SocketAddr` rather than a bare
//! port, because the address is a lane now and a message that spelled
//! `127.0.0.1` itself would be wrong for every other bind.

use std::net::SocketAddr;

use ff_tower_core::log;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Something already holds the port — most often another
    /// `ff tower serve`. Caught at bind and named, because a lock file
    /// would go stale on every Ctrl-C and the socket cannot.
    #[error("{addr} is already in use — something is serving there")]
    AddressInUse { addr: SocketAddr },

    /// No interface on this machine has the address asked for — the most
    /// likely way `--host` fails, and a different answer from the port
    /// being taken.
    #[error("cannot bind {addr} — no interface on this machine has that address")]
    NoSuchAddress { addr: SocketAddr },

    /// The runtime would not build, the socket would not bind for any
    /// other reason, or the accept loop stopped with one. The OS's own
    /// words ride along, because they are the whole of what is known.
    #[error("the server on {addr} failed: {source}")]
    Failed {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    /// The repository refused before anything bound — no worktree, or no
    /// identity to file events under.
    #[error(transparent)]
    Repo(#[from] log::Error),
}

impl Error {
    /// The stable id. `Repo` has none of its own: it carries core's error
    /// whole, and the caller maps it through the table that already names
    /// every log failure.
    pub fn id(&self) -> Option<&'static str> {
        match self {
            Error::AddressInUse { .. } => Some("serve/address-in-use"),
            Error::NoSuchAddress { .. } => Some("serve/no-such-address"),
            Error::Failed { .. } => Some("serve/failed"),
            Error::Repo(_) => None,
        }
    }
}

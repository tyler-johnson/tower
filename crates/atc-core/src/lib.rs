//! tower's engine: the flight log, the fold that becomes a board, and the
//! procedures that shape work as it arrives.
//!
//! None of it is built. What is settled is the seam, and it is worth
//! writing down here before the first line of code lands on either side of
//! it.
//!
//! **tower spawns fufu; it does not link it.** Everything tower asks of
//! fufu arrives as `ff <verb> --json` over the machine contract — the
//! envelope's version checked against `FF_CONTRACT`, the payload parsed
//! as data — and the board asks nothing: it is a fold of tower's own log.
//! Two calls remain. `ff version` is the doctor's seam check, and
//! `ff watch` is serve's change feed. tower holds no fufu type, and there
//! is no `ff-core` in this dependency tree.
//!
//! The weaker reason is the obvious one. `ff-core` is `publish = false` and
//! deliberately unfrozen — fufu's DESIGN says publishing it waits until
//! what it would freeze has stopped moving — so a git dependency would pin
//! a commit somebody has to bump by hand, and break on refactors upstream
//! that were nobody's mistake.
//!
//! The stronger reason is that the CLI seam is the one every other
//! extension has to use. An extension that reached past it would be the
//! consumer proving the contract while exempt from it, and whatever the
//! seam was missing would stay missing.
//!
//! What tower does need a git library for is that own store.
//! `refs/tower/log/<author>/<writer>` is an orphan commit chain with its
//! own tree, CAS-appended, never touching the working tree and never
//! touching `refs/fufu/*`. That is `gix` directly, and it is why the
//! dependency is declared here rather than inherited from anything.

pub mod board;
pub mod config;
pub mod ff;
pub mod log;
pub mod machine;
pub mod model;
pub mod procedure;
pub mod skill;
pub mod verb;

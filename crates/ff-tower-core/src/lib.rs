//! tower's engine: the flight log, the fold that becomes a board, and the
//! procedures that shape work as it arrives.
//!
//! None of it is built. What is settled is the seam, and it is worth
//! writing down here before the first line of code lands on either side of
//! it.
//!
//! **tower spawns fufu; it does not link it.** Everything tower knows about
//! a repository arrives as `ff <verb> --json` over the machine contract —
//! the envelope's version checked against `FF_CONTRACT`, the payload parsed
//! as data. tower holds no fufu type, and there is no `ff-core` in this
//! dependency tree.
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
//! One property of that seam worth knowing before it surprises someone: a
//! fufu read verb is not side-effect-free. `ff status --json` takes a
//! capture first, like every fufu verb, so tower rendering a board against
//! a dirty tree appends an operation to that worktree's chain. It is a
//! no-op when nothing changed, and when something did change the snapshot
//! is fufu's floor doing exactly its job — but "tower only reads" is true
//! of tower's own store and not of the repository underneath it.
//!
//! What tower does need a git library for is that own store.
//! `refs/tower/log/<author>/<writer>` is an orphan commit chain with its
//! own tree, CAS-appended, never touching the working tree and never
//! touching `refs/fufu/*`. That is `gix` directly, and it is why the
//! dependency is declared here rather than inherited from anything.

pub mod ff;
pub mod log;

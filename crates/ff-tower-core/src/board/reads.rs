//! The module's only I/O: three fufu spawns, one struct of answers.
//!
//! Exactly three, constant in flight count. Every fufu read takes a
//! capture first, so spawn count is log noise as well as latency: `op log`
//! over `session(glob:*)` answers for every tagged operation at once,
//! `branch list` carries fufu's holds per branch, and `status` says which
//! branch this render is sitting on. Everything downstream of here is pure
//! over what these returned.

use crate::ff::{self, BranchList, Ff, OpEntry};

/// Everything `enrich` needs from the repository, already fetched.
#[derive(Debug)]
pub struct Reads {
    /// Every operation row carrying a session tag — the flight-to-branch
    /// derivation.
    pub ops: Vec<OpEntry>,
    pub branches: BranchList,
    /// The branch this render's worktree sits on; `None` when HEAD is
    /// detached or unborn.
    pub current_branch: Option<String>,
}

/// The three spawns. The revset positional is one argv token handed to the
/// process with no shell, so `(`, `)`, `*` need no quoting; `glob:` goes
/// through git's own wildmatch.
pub fn gather(ff: &Ff) -> ff::Result<Reads> {
    let ops = ff.op_log("session(glob:*)")?;
    let branches = ff.branch_list()?;
    let status = ff.status()?;
    Ok(Reads {
        ops,
        branches,
        current_branch: status.head.branch().map(str::to_string),
    })
}

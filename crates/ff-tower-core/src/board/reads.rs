//! The module's I/O: three fufu spawns and one struct of answers.
//!
//! Three spawns, constant in flight count. Every fufu read takes a capture
//! first, so spawn count is log noise as well as latency — a read-lane call
//! in an unchanged tree is a `NoOp` capture that appends nothing. `op log`
//! over `session(glob:*)` answers for every tagged operation on the
//! invoking worktree's chain at once, `branch list` carries fufu's holds
//! per branch, and `status` says which branch this render is sitting on.
//! Everything downstream of here is pure over what these returned.

use std::collections::HashMap;

use crate::ff::{self, BranchInfo, BranchList, Ff, OpEntry};

/// Everything `enrich` needs from the repository, already fetched.
#[derive(Debug)]
pub struct Reads {
    /// Every operation row carrying a session tag on the invoking
    /// worktree's chain — the flight-to-branch derivation.
    pub ops: Vec<OpEntry>,
    pub branches: BranchList,
    /// The branch this render's worktree sits on; `None` when HEAD is
    /// detached or unborn.
    pub current_branch: Option<String>,
}

impl Reads {
    /// The freshest op row per session tag — which branch each flight is
    /// on, by its latest motion.
    pub fn freshest(&self) -> HashMap<&str, &OpEntry> {
        let mut freshest: HashMap<&str, &OpEntry> = HashMap::new();
        for op in &self.ops {
            let Some(session) = op.session.as_deref() else {
                continue;
            };
            match freshest.get(session) {
                Some(seen) if seen.time >= op.time => {}
                _ => {
                    freshest.insert(session, op);
                }
            }
        }
        freshest
    }

    /// Every branch fufu listed, named and anonymous, keyed by name.
    pub fn branch_index(&self) -> HashMap<&str, &BranchInfo> {
        self.branches
            .named
            .iter()
            .chain(self.branches.anonymous.iter())
            .map(|branch| (branch.name.as_str(), branch))
            .collect()
    }
}

/// The three spawns. The revset positional is one argv token handed to
/// the process with no shell, so `(`, `)`, `*` need no quoting; `glob:`
/// goes through git's own wildmatch.
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

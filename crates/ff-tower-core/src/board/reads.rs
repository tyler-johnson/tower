//! The module's I/O: three fufu spawns and the collide probes, two
//! structs of answers.
//!
//! Three spawns constant in flight count, plus one `collide` per distinct
//! pair of in-flight branches — zero extra in the solo norm, where fewer
//! than two distinct branches are flying. Every fufu read takes a capture
//! first, so spawn count is log noise as well as latency: `op log` over
//! `session(glob:*)` answers for every tagged operation at once, `branch
//! list` carries fufu's holds per branch, and `status` says which branch
//! this render is sitting on. Everything downstream of here is pure over
//! what these returned.

use std::collections::HashMap;

use crate::ff::{self, BranchInfo, BranchList, Ff, OpEntry, Pairing, UnknownReason};

use super::flight::Fold;

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

/// The verdicts a render's probes came back with.
///
/// Derived facts, never stored — probed per render, so there is nothing to
/// invalidate and nothing to go stale.
#[derive(Debug, Default)]
pub struct Verdicts {
    pub pairs: Vec<BranchPairing>,
}

/// One pair of branches, judged.
#[derive(Debug)]
pub struct BranchPairing {
    pub a: String,
    pub b: String,
    pub pairing: Pairing,
}

impl Verdicts {
    /// The verdict for a pair, whichever way around it was asked. `None`
    /// means the pair was never probed — distinct from `Unknown`, which is
    /// fufu answering "no base."
    pub fn between(&self, x: &str, y: &str) -> Option<&Pairing> {
        self.pairs
            .iter()
            .find(|pair| (pair.a == x && pair.b == y) || (pair.a == y && pair.b == x))
            .map(|pair| &pair.pairing)
    }
}

/// The distinct in-flight branch pairs worth probing: live flights only,
/// flight-to-branch via the freshest op row, and the branch must resolve
/// in the index — `@detached` and a deleted or landed name are the
/// existing cannot-be-held idiom, and a probe against them would only
/// refuse. Unordered pairs, deduped, each asked once; two flights on one
/// branch are one tree and make no pair.
pub fn branch_pairs(fold: &Fold, reads: &Reads) -> Vec<(String, String)> {
    let freshest = reads.freshest();
    let index = reads.branch_index();

    let mut branches: Vec<&str> = Vec::new();
    for flight in &fold.flights {
        if flight.done.is_some() {
            continue;
        }
        let Some(op) = freshest.get(flight.id.to_string().as_str()) else {
            continue;
        };
        let Some(name) = op.branch.as_deref() else {
            continue;
        };
        if name == "@detached" || !index.contains_key(name) {
            continue;
        }
        if !branches.contains(&name) {
            branches.push(name);
        }
    }

    let mut pairs = Vec::new();
    for (i, a) in branches.iter().enumerate() {
        for b in &branches[i + 1..] {
            pairs.push((a.to_string(), b.to_string()));
        }
    }
    pairs
}

/// One `collide` per pair. A refusal folds to unanswered rather than
/// killing the render — a branch can vanish between `branch list` and the
/// spawn, and a board that dies on that race is worse than a row saying
/// "no verdict." The other error kinds are the whole seam broken, not one
/// pair unanswered, so they propagate.
pub fn probe(ff: &Ff, fold: &Fold, reads: &Reads) -> ff::Result<Verdicts> {
    let mut pairs = Vec::new();
    for (a, b) in branch_pairs(fold, reads) {
        let pairing = match ff.collide(&a, &b) {
            Ok(collision) => collision.pairing,
            Err(ff::Error::Ff(_)) => Pairing::Unknown {
                reason: UnknownReason::Other,
            },
            Err(err) => return Err(err),
        };
        pairs.push(BranchPairing { a, b, pairing });
    }
    Ok(Verdicts { pairs })
}

#[cfg(test)]
mod tests {
    use super::super::flight::fold;
    use super::*;
    use crate::log::{Event, EventId, Kind};

    fn filed(id: &str, time: i64) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Filed {
                procedure: "open".to_string(),
                subject: format!("subject of {time}"),
                body: String::new(),
            },
        }
    }

    fn done(id: &str, time: i64, flight: &str) -> Event {
        let id: EventId = id.parse().expect("id");
        Event {
            writer: id.writer.clone(),
            author: "a@b.c".to_string(),
            time,
            id,
            kind: Kind::Done {
                flight: flight.parse().expect("id"),
            },
        }
    }

    fn op(session: &str, branch: Option<&str>, time: i64) -> OpEntry {
        OpEntry {
            branch: branch.map(str::to_string),
            session: Some(session.to_string()),
            time,
        }
    }

    fn branch(name: &str) -> BranchInfo {
        BranchInfo {
            name: name.to_string(),
            tip: Some("3c8f91686a9e35a10ae8ebb6f0d6f9bbbfdd6940".to_string()),
            held: false,
            resolving: false,
        }
    }

    fn reads(ops: Vec<OpEntry>, named: Vec<BranchInfo>) -> Reads {
        Reads {
            ops,
            branches: BranchList {
                named,
                anonymous: Vec::new(),
            },
            current_branch: None,
        }
    }

    #[test]
    fn two_distinct_branches_make_one_pair() {
        let pairs = branch_pairs(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20)]),
            &reads(
                vec![op("pi.1", Some("left"), 50), op("pi.2", Some("right"), 60)],
                vec![branch("left"), branch("right")],
            ),
        );
        assert_eq!(pairs, [("left".to_string(), "right".to_string())]);
    }

    #[test]
    fn two_flights_on_one_branch_make_no_pair() {
        let pairs = branch_pairs(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20)]),
            &reads(
                vec![op("pi.1", Some("work"), 50), op("pi.2", Some("work"), 60)],
                vec![branch("work")],
            ),
        );
        assert!(pairs.is_empty());
    }

    #[test]
    fn a_done_flight_contributes_no_branch() {
        let pairs = branch_pairs(
            &fold(&[
                filed("pi.1", 10),
                filed("pi.2", 20),
                done("pi.3", 70, "pi.2"),
            ]),
            &reads(
                vec![op("pi.1", Some("left"), 50), op("pi.2", Some("right"), 60)],
                vec![branch("left"), branch("right")],
            ),
        );
        assert!(pairs.is_empty(), "one live branch pairs with nothing");
    }

    #[test]
    fn detached_and_index_absent_branches_are_excluded() {
        let pairs = branch_pairs(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            &reads(
                vec![
                    op("pi.1", Some("left"), 50),
                    op("pi.2", Some("@detached"), 60),
                    op("pi.3", Some("landed"), 70),
                ],
                // `@detached` in the index must not resolve the sentinel,
                // and `landed` is absent — deleted or already landed.
                vec![branch("left"), branch("@detached")],
            ),
        );
        assert!(pairs.is_empty());
    }

    #[test]
    fn three_branches_make_three_pairs_each_once() {
        let pairs = branch_pairs(
            &fold(&[filed("pi.1", 10), filed("pi.2", 20), filed("pi.3", 30)]),
            &reads(
                vec![
                    op("pi.1", Some("a"), 50),
                    op("pi.2", Some("b"), 60),
                    op("pi.3", Some("c"), 70),
                ],
                vec![branch("a"), branch("b"), branch("c")],
            ),
        );
        assert_eq!(
            pairs,
            [
                ("a".to_string(), "b".to_string()),
                ("a".to_string(), "c".to_string()),
                ("b".to_string(), "c".to_string()),
            ]
        );
    }

    #[test]
    fn an_empty_fold_makes_no_pairs() {
        let pairs = branch_pairs(&fold(&[]), &reads(Vec::new(), vec![branch("left")]));
        assert!(pairs.is_empty());
    }

    #[test]
    fn between_answers_either_way_around_and_none_means_unprobed() {
        let verdicts = Verdicts {
            pairs: vec![BranchPairing {
                a: "left".to_string(),
                b: "right".to_string(),
                pairing: Pairing::Clear,
            }],
        };
        assert!(verdicts.between("left", "right").is_some());
        assert!(verdicts.between("right", "left").is_some());
        assert!(verdicts.between("left", "other").is_none());
    }
}

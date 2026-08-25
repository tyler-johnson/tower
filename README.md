<div align="center">

# tower

**it doesn't fly anything — it keeps things from colliding**

*Project management for people and agents, built on [fufu](https://github.com/tyler-johnson/fufu).<br>
State is derived from the repository, never entered. Only intent is stored.*

</div>

---

> **Status: design only.** This repository is a scaffold. Nothing described below is built, though nearly everything it stands on is. [DESIGN.md](DESIGN.md) is the founding sketch and the thing to read.

Every tracker asks a human to say what is happening, and then drifts from the repository the moment attention lapses. fufu already knows: capture runs before every action, futures are computed for free, branches and sessions are observable. So tower stores what a person authored — title, body, links, priority, dependencies — and derives the rest. A flight is `active` because a branch exists with snapshots on it, not because anyone clicked.

The consequence worth the whole design: tower sees work start before the first commit exists, because the capture floor does. An agent that edits for twenty minutes and commits nothing is visible.

## The seam

tower is a separate program with its own authority, its own store, and its own cadence. It reads fufu over the CLI and the machine contract, and it never links `ff-core`:

```
reads     ff status --json · ff log --json · ff collide --json · ff watch --all
calls     ff start · ff switch · ff worktree add|remove, tagged --session <flight>
stores    refs/tower/log/<author>
derives   state · progress · conflicts · land order
writes    nothing under refs/fufu/*, ever
```

That last line is fufu's extension rule unmodified: extensions read fufu state and call fufu verbs; only fufu writes fufu state.

## Passive by construction

**tower is a thing agents call. It never calls agents.** Every verb is a read plus a local write. No daemon, no cron, no dispatch, no iteration verb. If work should loop, the agent harness loops and calls `ff tower next` again — the harness is the scheduler, tower is only the queue.

## Layout

| crate | what it is |
|---|---|
| `ff-tower-core` | the flight log, the fold that becomes a board, procedures, intake, land order |
| `ff-tower-cli` | the one binary, `ff-tower`, which fufu's dispatch finds for `ff tower` |
| `ff-tower-testsupport` | shared fixtures |

Forge adapters are separate binaries discovered on PATH — `tower-github`, `tower-linear` — on git's extension model, so a third party can write one without touching this repository.

## Building

```console
$ cargo build
$ cargo test --workspace
```

fufu is a runtime dependency rather than a build one: tower spawns `ff`, so a `cargo build` needs nothing installed and running tower needs `ff` on PATH.

## License

MIT

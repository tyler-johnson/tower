<div align="center">

# tower

**it doesn't fly anything — it keeps things from colliding**

*Project management for people and agents, built on [fufu](https://github.com/tyler-johnson/fufu).<br>
Intent is stored; the repository audits it.*

</div>

---

> [DESIGN.md](DESIGN.md) is the design and the thing to read; the flights on the board are what stands built so far.

The model is the one every tracker uses, on purpose: a flight is an issue with a status, an assignee, a priority, labels, and links, recognizable in one glance to anyone who has used Linear. The engine underneath is what no other tracker has. fufu's capture floor runs before every action, so tower checks the claims its own board makes — a flight marked In Progress with no motion says so on its row, work is visible before the first commit exists, and two agents editing the same hunk on different branches is a discovered conflict, not a surprise at merge. Drift is flagged, never corrected.

**tower is a thing agents call. It never calls agents.** There is no dispatch and no iteration verb: the harness loops and calls `ff tower next`, which hands back the next conflict-free set of ready work — the harness is the scheduler, tower is only the queue. And the engine ships empty: no built-in procedures, no built-in skills, no default opinions about how work should flow. Structure and judgment are files their owner authors; the documentation teaches by example, and [`docs/`](docs/) carries the worked ones to copy in.

## The seam

tower is a separate program with its own authority, its own store, and its own cadence. It reads fufu over the CLI and the machine contract, and it never links `ff-core`:

```
reads     ff status --json · ff log --json · ff collide --json · ff watch --all
calls     ff start · ff switch · ff worktree add|remove, tagged --session <flight>
stores    refs/tower/log/<author>/<writer>
derives   motion · conflicts · drift · land order
writes    nothing under refs/fufu/*, ever
```

That last line is fufu's extension rule unmodified: extensions read fufu state and call fufu verbs; only fufu writes fufu state.

## Layout

| crate | what it is |
|---|---|
| `ff-tower-core` | the flight log, the fold that becomes a board, procedures, intake, land order |
| `ff-tower-cli` | the one binary, `ff-tower`, which fufu's dispatch finds for `ff tower` |
| `ff-tower-serve` | the standing server: the embedded web board, its API, and the change feed |
| `ff-tower-testsupport` | shared fixtures |

Forge adapters are separate binaries discovered on PATH — `tower-github`, `tower-linear` — on git's extension model, so a third party can write one without touching this repository.

## Install

tower rides fufu, so install [fufu](https://github.com/tyler-johnson/fufu) first — the verb is reached as `ff tower`.

Linux/macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/tyler-johnson/tower/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/tyler-johnson/tower/main/install.ps1 | iex
```

Homebrew:

```sh
brew install tyler-johnson/tap/ff-tower
```

Installed binaries keep themselves fresh: a passive lane checks for releases about daily and auto-installs (`ff tower config updateCheck false` turns it off, `autoUpdate false` downgrades it to a one-line notice), and `ff tower update` moves the binary by hand.

## Building

```console
$ make              # fast dogfood build -- release semantics, no LTO link cost
$ make install      # link ~/.cargo/bin/ff-tower at it; `ff tower` is live
$ make test         # the suite
$ make release      # the honest fat-LTO build
```

`make install` is the whole install: fufu's `ff-<name>` dispatch searches PATH, so a symlink is all `ff tower` needs. It is idempotent, and the binary is live the moment a build links — no reinstall step between editing and running.

Building needs Node and pnpm: cargo's build script runs the web build itself and embeds the output, so `cargo build` alone yields the full binary with the board inside. They are build dependencies only — fufu stays a runtime dependency rather than a build one: tower spawns `ff`, so running tower needs `ff` on PATH and nothing else.

## License

MIT

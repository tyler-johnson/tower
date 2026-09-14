<div align="center">

# tower

**the board in your repository**

*Project management for people and agents, stored in git.<br>
Intent is stored; the board is derived.*

</div>

---

> [DESIGN.md](DESIGN.md) is the design and the thing to read; the flights on the board are what stands built so far.

The model is the one every tracker uses, on purpose: a flight is an issue with a status, an assignee, a priority, labels, and links. Every mutation appends an event to ordinary Git refs in the repository, and the board folds those local logs. Optional shared-board synchronization runs synchronously under a deadline; the fold itself performs no networking.

## Sharing a board

Set `atc config remote origin` in each clone to share through an existing Git remote. Tower fetches writer chains and publishes its own chain on ordinary board touches and the server's cadence. Leave `tower.remote` unset for local-only operation. Each clone mints its own writer ID; do not copy `tower.writer` between machines.

Claimed flights have the same `#n` on every synchronized board. Offline filings show `~n`, or `writer~n` when guesses overlap, and a later touch claims them. Wire IDs and `writer#n` ordinal aliases remain stable. Joining an established remote reports how many local flights must move; confirm with `atc config remote origin --renumber`. References stored in prose keep naming the same flights.

Ordinary sync defaults to a 30-second cadence and a 3-second total deadline. Every filing and decomposition uses a 10-second allocation budget, independent of cadence, and returns confirmed numbers when allocation succeeds. Configure these with `syncInterval`, `syncTimeout`, and `numberTimeout`. No detached board sync continues after return. `atc doctor` reports sync and enrollment problems; `git log refs/tower/seq` reads the counter.

**tower is a thing agents call. It never calls agents.** There is no dispatch and no iteration verb: the harness loops and calls `atc next`, which hands back the next ready work — the harness is the scheduler, tower is only the queue. tower ships the manual and the mechanism and no workflow: one skill, `tower`, installed by `atc hook`; a `skill` field on a flight, a shelf it names into, and `atc skills <name>` to print one raw. No built-in procedures, no built-in loop, no default opinions about how work should flow. Structure and judgment are files their owner authors; the documentation teaches by example, and [`docs/`](docs/) carries the worked ones to copy in.

## The program

tower is a standalone program with its own store and release cadence. The CLI and embedded web board read the same repository-backed log:

```
reached   atc, in the current directory
envelope  {"atc": 1, "cmd": "<verb>", data | error} — bare ids
stores    refs/tower/log/<author>/<writer>
```

## Layout

| crate | what it is |
|---|---|
| `atc-core` | the flight log, the fold that becomes a board and the query over it, procedures, intake |
| `atc-cli` | the standalone binary, `atc` |
| `atc-serve` | the standing server: the embedded web board, its API, and the change feed |
| `atc-testsupport` | shared fixtures |

Forge adapters are separate binaries discovered on PATH — `atc-github`, `atc-linear` — dispatched through `atc <name>`, so a third party can write one without touching this repository. `atc adapter <name>` declares one on this machine; bare `atc adapter` lists declarations, and `atc adapter -d <name>` removes one.

## Install

### 1. Install the binary

The binary gives you every CLI verb and the embedded web board. It is complete on its own; agent integration is the second step.

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
brew install tyler-johnson/tap/atc
```

### 2. Connect your agent clients

```sh
atc hook
```

The same command works on Windows. It reports the clients and shells it found, then asks which to wire. tower supports Claude Code, Codex, Qwen Code, OpenCode, Copilot CLI, and Cursor CLI. The wiring delivers the board's notice to agent sessions and installs the `tower` manual in clients that load skills; Qwen receives the notice alone. Codex requires you to review the installed hook through `/hooks`. See [Reaching an agent](docs/README.md#reaching-an-agent) for client details and shell integration.

Both install scripts already run `atc hook -u`: it refreshes existing wiring and skills without adding unwired clients. On a fresh machine it wires nothing, so run `atc hook` to choose your clients. `atc unhook` removes tower-owned wiring and skills while preserving other entries in shared files.

`atc update` names the command that owns the binary; the install scripts refresh existing hooks and skills with `atc hook -u`.

## Web board

Run `atc serve` inside a repository and open [http://127.0.0.1:7420](http://127.0.0.1:7420). The binary includes the web app, its API, and a live change feed over the same board the CLI reads and edits.

A planned Changes tab will use [fufu](https://github.com/tyler-johnson/fufu), when installed, to put repository history and changes on a repository's page. That tab is later work; the board and agent integration work on their own.

## Skills

`atc hook` ships one manual, `tower`, reached as `/tower:tower` in Claude Code and `$tower` in Codex and OpenCode. It teaches the model, command contract, and how to find each verb's help.

Workflow instructions are yours to write on the shelf: `~/.config/tower/skills/<name>.md` for yourself or `.tower/skills/<name>.md` in the main worktree for the team. A repository skill replaces a user skill of the same name. A flight's `skill` field names one; `atc next` hands that name to the agent, which reads it with `atc skills <name>` and follows it for the flight. The manual is installed separately from this shelf.

The [plan](docs/skills/plan.md), [work](docs/skills/work.md), and [review](docs/skills/review.md) skills are worked examples to copy and fork. See [the shelf and copying instructions](docs/README.md#the-two-layers) for the full layout.

## Building

```console
$ make              # fast dogfood build -- release semantics, no LTO link cost
$ make test         # the suite
$ make release      # the honest fat-LTO build
```

The suite includes a live test per wired agent client (`crates/atc-cli/tests/live_<client>.rs`) that runs the real client binary against a scripted mock model when it is on PATH and skips otherwise; `ATC_LIVE=1` makes a skip a failure, which is how CI runs them.

With cargo's target dir shared machine-wide and its `dogfood/` directory on PATH, `make` makes `atc` available the moment a build links — no reinstall step between editing and running. Run `atc hook -u` after building to refresh existing client wiring and skills.

Building needs the Rust toolchain, Node, and pnpm: cargo's build script runs the web build itself and embeds the output, so `cargo build` alone yields the full binary with the board inside. Node and pnpm are build dependencies only.

## License

MIT

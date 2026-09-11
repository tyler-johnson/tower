<div align="center">

# tower

**the board over fufu**

*Project management for people and agents, built on [fufu](https://github.com/tyler-johnson/fufu).<br>
Intent is stored; the board is derived.*

</div>

---

> [DESIGN.md](DESIGN.md) is the design and the thing to read; the flights on the board are what stands built so far.

The model is the one every tracker uses, on purpose: a flight is an issue with a status, an assignee, a priority, labels, and links, recognizable in one glance to anyone who has used Linear. What is different is where it lives. Every verb appends an event to a log kept as ordinary git refs in the repository, the board is a fold over that log, and sync is one refspec — nothing tower stores needs tower to read back, and a render never blocks on the network. The word a row shows is the word someone set, attributed, and tower checks it against nothing.

**tower is a thing agents call. It never calls agents.** There is no dispatch and no iteration verb: the harness loops and calls `atc next`, which hands back the next ready work — the harness is the scheduler, tower is only the queue. tower ships the manual and the mechanism and no workflow: one skill, `tower`, installed by `ff hook`; a `skill` field on a flight, a shelf it names into, and `atc skills <name>` to print one raw. No built-in procedures, no built-in loop, no default opinions about how work should flow. Structure and judgment are files their owner authors; the documentation teaches by example, and [`docs/`](docs/) carries the worked ones to copy in.

## The seam

tower is a separate program with its own authority, its own store, and its own cadence. It is a declared extension of fufu's, speaks the machine contract, and never links `ff-core`:

```
reached   atc, in the current directory
envelope  {"atc": 1, "cmd": "<verb>", data | error} — bare ids, fufu's exit codes
stores    refs/tower/log/<author>/<writer>
spawns    ff --version in doctor · ff watch --all in serve
writes    nothing under refs/fufu/*, ever
```

That last line is fufu's extension rule unmodified: extensions read fufu state and call fufu verbs; only fufu writes fufu state. Outside `doctor` and `serve`, tower spawns fufu for nothing, and no verb's answer depends on a fufu read.

## Layout

| crate | what it is |
|---|---|
| `atc-core` | the flight log, the fold that becomes a board and the query over it, procedures, intake |
| `atc-cli` | the one binary, `atc`, which fufu's dispatch finds for `atc` |
| `atc-serve` | the standing server: the embedded web board, its API, and the change feed |
| `atc-testsupport` | shared fixtures |

Forge adapters are separate binaries discovered on PATH — `tower-github`, `tower-linear` — on git's extension model, so a third party can write one without touching this repository.

## Install

tower rides fufu, so install [fufu](https://github.com/tyler-johnson/fufu) first — the verb is reached as `atc`.

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

Installed binaries keep themselves fresh: a passive lane checks for releases about daily and auto-installs (`atc config updateCheck false` turns it off, `autoUpdate false` downgrades it to a one-line notice), and `atc update` moves the binary by hand.

## Building

```console
$ make              # fast dogfood build -- release semantics, no LTO link cost
$ make test         # the suite
$ make release      # the honest fat-LTO build
```

`make` is the whole install: fufu's `ff-<name>` dispatch searches PATH, and with cargo's target dir shared machine-wide and its dogfood/ on PATH, `atc` is live the moment a build links — no reinstall step between editing and running.

Building needs Node and pnpm: cargo's build script runs the web build itself and embeds the output, so `cargo build` alone yields the full binary with the board inside. They are build dependencies only — fufu stays a runtime dependency rather than a build one: tower spawns `ff`, so running tower needs `ff` on PATH and nothing else.

## License

MIT

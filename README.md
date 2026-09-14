<div align="center">

# tower

**the board in your repository**

*Project management for people and agents, stored in git.<br>
Intent is stored; the board is derived.*

</div>

---

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

## Layout

| crate | what it is |
|---|---|
| `atc-core` | the flight log, the fold that becomes a board and the query over it, procedures, intake |
| `atc-cli` | the standalone binary, `atc` |
| `atc-serve` | the standing server: the embedded web board, its API, and the change feed |
| `atc-testsupport` | shared fixtures |

Forge adapters are separate binaries discovered on PATH — `atc-github`, `atc-linear` — dispatched through `atc <name>`, so a third party can write one without touching this repository. `atc adapter <name>` declares one on this machine; bare `atc adapter` lists declarations, and `atc adapter -d <name>` removes one.

## Build

```console
$ make              # fast dogfood build -- release semantics, no LTO link cost
$ make test         # the suite
$ make release      # the honest fat-LTO build
```

Building needs the Rust toolchain, Node, and pnpm: cargo's build script runs the web build itself and embeds the output, so `cargo build` alone yields the full binary with the board inside. Node and pnpm are build dependencies only.

The suite includes a live test per wired agent client (`crates/atc-cli/tests/live_<client>.rs`) that runs the real client binary against a scripted mock model when it is on PATH and skips otherwise; `ATC_LIVE=1` makes a skip a failure, which is how CI runs them.

With cargo's target dir shared machine-wide and its `dogfood/` directory on PATH, `make` makes `atc` available the moment a build links — no reinstall step between editing and running. Run `atc hook -u` after building to refresh existing client wiring and skills.

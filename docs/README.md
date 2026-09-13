# Worked examples

The engine ships empty. There are no built-in procedures and no built-in workflow skills — every opinion about how work flows is a file its owner authored, and this directory is where the examples live so you can copy one in and fork it.

The three skills here are one workflow among many. tower ships the manual and the mechanism, and the shape of the loop is yours to fork.

A **procedure** is the named recipe for one shape of work, as data: a name, optional match rules, and the flights it stamps out, each with the same fields any flight carries, pre-filled. Filing under one mints the whole family in a single append, and the definition is read once — editing it afterwards never disturbs a flight already in the air.

A **skill** is the prose an agent-assigned flight is flown with: instructions a harness executes, never a process tower spawns. A procedure's flight names a skill by string, and that seam is what keeps structure in data and judgment in markdown.

## The two layers

Both kinds layer the same way, keyed by name, the more specific replacing the less wholesale:

| layer | procedures | skills |
|---|---|---|
| user | `~/.config/tower/procedures/<name>.toml` | `~/.config/tower/skills/<name>.md` |
| repository | `<main worktree>/.tower/procedures/<name>.toml` | `<main worktree>/.tower/skills/<name>.md` |

The user layer roams with your config; the repository layer is the team's, and it is anchored to the main worktree so every worktree sees the same set. `$XDG_CONFIG_HOME` replaces `~/.config` when it is set. A missing directory is an empty layer; a file that does not parse is a refusal naming the path.

`atc procedures` and `atc skills` list what is installed and name the layer each came from.

## Copy one in

For the team, from the root of a clone of this repository:

```sh
mkdir -p .tower/procedures .tower/skills
cp docs/procedures/ticket.toml .tower/procedures/     # one flight, yours
cp docs/procedures/review.toml .tower/procedures/     # pass and smoke, then your verdict
cp docs/skills/plan.md .tower/skills/                 # decompose a goal into linked flights
cp docs/skills/review.md .tower/skills/               # first-pass a branch
cp docs/skills/work.md .tower/skills/                 # claim, do, hold or commit, repeat
```

For yourself, `~/.config/tower/procedures/` and `~/.config/tower/skills/` take the same files.

The name a procedure is filed under is the `name =` line inside the file, not the file name — rename `ticket.toml` freely, and rename what is inside it to change the word `atc file` takes.

## Reaching an agent

The manual is the binary's to install: `atc hook` writes the `tower` skill where each client reads skills, and `atc unhook` takes it back. tower wires Claude Code through its own plugin, Codex through a plugin at `~/.agents/plugins/tower` reached by the personal marketplace beside it, Copilot CLI through a separate Agent Plugins 1.0 plugin at `~/.agents/plugins/copilot/tower` registered as `tower@tower-atc` through the marketplace beside it, Cursor CLI through an automatically discovered native plugin at `~/.cursor/plugins/local/tower`, Qwen Code through its settings file, and OpenCode through a plugin module at `~/.config/opencode/plugins/tower.js`. The five plugins carry the manual and declared adapters' skills; Qwen receives the notice alone because it reads no skills directory. Claude's `--settings` mode also installs hooks alone. Nothing here is the manual, and the manual never appears on the shelf. Each wired client has a conformance test against the installed client, and CI runs them at pinned versions. Copilot's test checks live plugin registration and removal; Cursor's checks discovery, hooks, skills, and removal through the installed client's loader. The other clients also run sessions against a mock model to verify what reaches the prompt. Cursor's authenticated session captures are under `crates/atc-cli/tests/fixtures/cursor/`.

A shelf skill reaches an agent by name. An agent-assigned flight names the skill it is flown with, `atc next` hands that name out on the picked row, and the agent prints it with `atc skills <name>` and follows it for that flight.

The trigger delivers the notice at a context boundary, renews the session's lease silently on activity, and releases it at the session's end, using the events each client supports. OpenCode's notice is standing in the system prompt on every model call. `atc hook -l` reports wiring; `atc hook -u` refreshes adapter declarations and repairs existing installs without adding unwired clients. `atc unhook` removes tower-owned paths and tower's entries in shared files exactly, preserving other entries and manually authored trigger lines. The [design's Hook section](../DESIGN.md#hook) lists each client's installation and removal paths.

An agent reaches the board under a callsign: `ATC_CALLSIGN` when the launcher set one, else the word it gave `atc callsign` this session, else the client it runs under, else the login name at a terminal. An agent runs `atc callsign <name>` first thing when its brief names it and otherwise flies as its client. The word is held on the session's lease — one file per session under the machine's state directory, renewed by every `atc` call and every trigger event, and gone when the session ends or a day passes without one — and no two live sessions on a machine hold one word; when a session's word changes, the open flights it laned under the old word follow it. Subagents share the session and its callsign, and separately tracked work is a separate session — `claude -p` under `ATC_SESSION` and `ATC_CALLSIGN`, the way the handoff skill runs one; a launcher that runs a pool sets `ATC_SESSION` per worker.

A terminal is a session too. `atc hook bash` (or zsh, fish, powershell) writes marked lines into the shell's rc file that mint a session id once per interactive shell into `ATC_SHELL_SESSION`, renew its lease before every prompt, and release it when the shell exits, so a person at a prompt has what an agent has: `atc callsign <name>`, `atc next`, a pilot with a session and a lease on the board. `atc whoami` says which session this is and where it came from; `atc session` says who else is on the machine, and where. An agent launched from a wired terminal is still its own session, because the client's variable ranks ahead of the terminal's, and a launcher that wants a worker tracked sets `ATC_SESSION` — never `ATC_SHELL_SESSION`, which is the terminal's own.

## A procedure should end with you

The boundary where work becomes visible to the team is a human gesture. `atc procedures` and `atc doctor` warn — by name and by flight — when every terminal flight of a definition is agent-assigned. It is a warning and not a refusal, because the file is yours and the boundary that actually holds is `never auto-outward`: whatever an agent finishes, nothing leaves the machine without a person's verb.

## Extensions

Adapters are separate executables that extend tower through its CLI. No upstream adapter ships yet; the interface below is implemented and is the contract an adapter author builds against.

### Undeclared adapters

`atc <name>` runs `atc-<name>` from PATH when no built-in verb matches. The name starts with an ASCII letter or digit and otherwise contains only ASCII letters, digits, `-`, or `_`. Arguments, stdin, stdout, stderr, and the exit status pass through. Built-ins always win, and dispatch needs no registry entry.

The child receives `ATC_CONTRACT=1`, `ATC_REPO` as the canonical worktree root with forward slashes when invoked in a repository, and `ATC_SESSION` when tower resolves a session. Outside a repository, tower unsets `ATC_REPO` even if the parent environment carried one. An undeclared adapter contributes no help delegation, explanation routing, notice, skills, update recipes, or adapter diagnostics.

### Declared adapters

`atc adapter <name>` resolves the executable, runs `atc-<name> --atc-manifest`, validates its reply, and records the manifest, resolved path, and declaration time. Bare `atc adapter` lists declarations; `atc adapter -d <name>` forgets one without uninstalling its executable. Future execution resolves PATH again; the recorded path is evidence for diagnostics.

Declarations live in `tower/adapters.json` under the user's platform config directory: `$XDG_CONFIG_HOME` or `~/.config` on Linux, `~/Library/Application Support` on macOS, and `%APPDATA%` on Windows. They are machine-local. Unknown manifest fields survive recording. A declaration with a different contract is retained as stale and excluded from integrations until refreshed. `atc hook -u` re-asks declarations, including stale ones, even when no client is wired. `atc doctor` reports unreadable registries, missing executables, handshake failures, and drift in manifests or PATH locations.

The manifest handshake exits 0 and writes one envelope on one stdout line. For a hypothetical `atc-example`:

```json
{"atc":1,"cmd":"manifest","data":{"name":"example","version":"1.0.0","contract":1,"verbs":[{"name":"list","read_only":true,"summary":"List upstream work"}],"undoable":false}}
```

Required fields are `name`, matching the executable's suffix; a nonempty `version`; `contract`, currently `1`; a nonempty `verbs` list whose entries have a one-word `name` and boolean `read_only`; and boolean `undoable`. A verb's `summary` is optional. `undoable` declares metadata; tower has no adapter undo dispatcher. Handshakes run with stdin closed and `ATC_NONINTERACTIVE=1`, without a timeout. Banners, progress output, pretty-printed envelopes, error envelopes, and nonzero exits fail the handshake.

Optional capabilities:

| field | contribution |
|---|---|
| `briefing` | a literal line, or `true` to ask the executable's `briefing` verb; absent or `false` contributes nothing |
| `skills` | names fetched with `--atc-skill <name>` and installed beside tower's manual by hook |
| `update` | install-channel recipes: `brew`, `install`, optional `bin` for that installer, and `releases` |
| `build` | `official` or `source`; defaults to `official` |
| `tools` | boolean metadata, default `false`; tower currently serves no MCP tools and has no tool-serving handshake |

Declaration also enables `atc help <name>` and `atc explain <name>/<id>`. Tower delegates these to the adapter's `help` and `explain <id>` verbs, stripping the adapter prefix for the latter. Built-in explanations win. An adapter's error IDs should use its name as their outer namespace so tower can route them back.

### What an adapter served through tower owes

**Keep the machine surface machine-readable.** Under `--json`, answer in a one-line `atc` envelope with `cmd` and either `data` or `error`; errors carry `id`, `message`, and `exits`. Use wire IDs in data and the shared exit meanings: 0 success, 1 refusal or a negative outcome, 2 usage, 3 held, 4 contended. Human passthrough output is the adapter's own. Only tower writes `refs/tower/*`; an adapter changes the board by calling tower's verbs. Upstream writes remain a deliberate user gesture.

**Help and explanations are bounded reads.** Delegation closes stdin, suppresses stderr, and allows one second for a successful reply; stdout is forwarded. A failed or late reply becomes `adapter/delegate-failed`. Honor the arguments passed through, including `--json` when present, and require no prompt or network round trip to explain a verb or refusal.

**A notice contribution is one short line.** With `briefing: true`, tower calls `atc-<name> briefing` in the caller's directory with the adapter context variables, under a one-second budget. Return plain text, not an envelope: after trimming, it must be nonempty, contain no newline, and fit within 240 characters. Literal lines follow the same size rule. Failure, lateness, or an invalid line drops the whole contribution; `ATC_DEBUG` enables diagnostics for failed asks. Contributions appear in declaration order and never claim work.

**Skills are named file bundles.** Each name is either the adapter name itself or starts with `<adapter>-`, using the same character rules as adapter names. The `--atc-skill <name>` handshake returns `data.files`, an array of objects with `path` and UTF-8 `content`. For example:

```json
{"atc":1,"cmd":"skill","data":{"files":[{"path":"SKILL.md","content":"---\nname: example\ndescription: Read upstream work.\n---\n\nUse the adapter's help for its commands.\n"}]}}
```

The bundle must contain a root `SKILL.md`. Paths must be relative, unique, and contain no `.` or `..` components; total content is capped at 8 MiB. Hook installs successful replies in each client's plugin or skills root, not the workflow shelf. A failed reply is reported and preserves an older installed copy. Repair removes retired skills, and unhook removes the adapter skills tower installed along with its own. Qwen and Claude's settings-only mode receive no skill bundles.

**Update recipes describe the install channel.** An `update` block must have at least one of `brew`, `install`, or `releases`; supplied values must be nonempty, and `bin` requires `install`. `brew` names the formula, `install` the installer URL, `bin` its destination directory, and `releases` the releases URL. `atc update <name>` uses the declaration's recipe for the detected channel: Script recipes can run after consent, while other channels print the command or location, or explain why no recipe applies. A `source` build opts out of passive release checks; official adapters enable them with a GitHub URL shaped as `https://github.com/<owner>/<repo>/releases/latest`. Adapter declarations describe capabilities; they do not authorize unattended upstream writes or agent execution.

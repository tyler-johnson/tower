# Worked examples

The engine ships empty. There are no built-in procedures and no built-in skills — every opinion about how work flows is a file its owner authored, and this directory is where the examples live so you can copy one in and fork it.

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

The manual is the binary's to install: `atc hook` writes the `tower` skill where each client reads skills, and `atc unhook` takes it back. tower wires Claude Code through its own plugin, Codex through a plugin at `~/.agents/plugins/tower` reached by the personal marketplace beside it, Copilot CLI through a separate Agent Plugins 1.0 plugin at `~/.agents/plugins/copilot/tower` registered as `tower@tower-atc` through the marketplace beside it, Qwen Code through its settings file, and OpenCode through a plugin module at `~/.config/opencode/plugins/tower.js`; the notice lands where the client runs hooks, and the manual lands with it. Nothing here is the manual, and the manual never appears on the shelf. Each wired client has a conformance test against the real binary, and CI runs them at pinned client versions. Copilot's test checks live plugin registration and removal; the other clients also run sessions against a mock model to verify what reaches the prompt.

A shelf skill reaches an agent by name. An agent-assigned flight names the skill it is flown with, `atc next` hands that name out on the picked row, and the agent prints it with `atc skills <name>` and follows it for that flight. tower writes no other program's config, so there is no redirect to set up: the flight carries the name, and the pick delivers it.

An agent reaches the board under a callsign: `ATC_CALLSIGN` when the launcher set one, else the word it gave `atc callsign` this session, else the client it runs under, else the login name at a terminal. An agent runs `atc callsign <name>` first thing when its brief names it and otherwise flies as its client. The word is held on the session's lease — one file per session under the machine's state directory, renewed by every `atc` call and every trigger event, and gone when the session ends or a day passes without one — and no two live sessions on a machine hold one word; when a session's word changes, the open flights it laned under the old word follow it. Subagents share the session and its callsign, and separately tracked work is a separate session — `claude -p` under `ATC_SESSION` and `ATC_CALLSIGN`, the way the handoff skill runs one; a launcher that runs a pool sets `ATC_SESSION` per worker.

A terminal is a session too. `atc hook bash` (or zsh, fish, powershell) writes marked lines into the shell's rc file that mint a session id once per interactive shell into `ATC_SHELL_SESSION`, renew its lease before every prompt, and release it when the shell exits, so a person at a prompt has what an agent has: `atc callsign <name>`, `atc next`, a pilot with a session and a lease on the board. `atc whoami` says which session this is and where it came from; `atc session` says who else is on the machine, and where. An agent launched from a wired terminal is still its own session, because the client's variable ranks ahead of the terminal's, and a launcher that wants a worker tracked sets `ATC_SESSION` — never `ATC_SHELL_SESSION`, which is the terminal's own.

## A procedure should end with you

The boundary where work becomes visible to the team is a human gesture. `atc procedures` and `atc doctor` warn — by name and by flight — when every terminal flight of a definition is agent-assigned. It is a warning and not a refusal, because the file is yours and the boundary that actually holds is `never auto-outward`: whatever an agent finishes, nothing leaves the machine without a person's verb.

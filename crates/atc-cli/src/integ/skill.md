---
name: tower
description: Advanced use of tower (atc), a repository-local board for people and agents. Use when driving tower from a script or a loop, reading its JSON envelope and exit codes, naming a flight by number or wire id, holding a flight on a question or answering one, claiming work with next, filing under a procedure or decomposing a flight, or whenever the board says something a verb refuses to change.
---

# tower

`atc` is a repository-local board for people and agents. Work is filed as flights, mutations append events to a log kept as ordinary git refs in the repository, and every render folds that log fresh. Nothing is entered twice, and nothing tower stores needs tower to read back: the refs are plain git objects beside history. The binary runs directly; fufu is optional.

tower is called and never calls agents. It runs no agent dispatch and no loop; the harness a session runs in is the scheduler, and tower is the queue and the record. The wired notice introduces bare `atc` and the loop's four gestures. This manual supplies the model and machine contract.

## Hook and unhook

`atc hook` reports detected clients and shells, then asks which to wire. Name them to wire exactly those, as in `atc hook claude codex`; `atc hook --all` wires everything detected without asking, and `atc hook -l` only reports. The client names are `claude`, `codex`, `qwen`, `opencode`, `copilot`, and `cursor`; the shell names are `bash`, `zsh`, `fish`, and `powershell`.

| client | wiring |
| --- | --- |
| Claude Code | its own plugin at `~/.claude/skills/tower`; `atc hook claude --settings` instead merges hooks into `~/.claude/settings.json` and installs no skills |
| Codex | a plugin at `~/.agents/plugins/tower`, reached through the personal marketplace beside it; hook runs client registration when Codex is available and otherwise names the command; review the hook through `/hooks` in Codex before it can run |
| Qwen Code | entries merged into `~/.qwen/settings.json`; the notice alone, because Qwen reads no skills directory |
| OpenCode | `~/.config/opencode/plugins/tower.js` and the manual at `~/.config/opencode/skills/tower`; the module sets `OPENCODE_SESSION_ID` on shell commands |
| Copilot CLI | an Agent Plugins 1.0 plugin at `~/.agents/plugins/copilot/tower`, a dedicated marketplace beside it, and `tower@tower-atc` registration in `~/.copilot/settings.json` |
| Cursor CLI | a native plugin at `~/.cursor/plugins/local/tower`, discovered on a new session |

The five plugins carry this manual and skills from declared adapters. The trigger has three event classes: a context boundary prints the notice and renews the session's lease, activity renews it silently, and the session's end releases it. Each client wires the events it supports; ending a turn is activity. OpenCode's notice is standing in the system prompt on every model call, so it survives compaction. Unknown events and trigger failures exit 0 without output. Shell wiring installs marked rc lines for a session heartbeat at each prompt and a release at exit, with no notice.

`atc hook -u` re-asks every declared adapter's manifest, even when no client is wired, then repairs existing client and shell installs without adding unwired ones. It refreshes the binary path, hook entries, manual, and adapter skills. Failed skill replies preserve older installed copies; retired skills are removed on repair.

`atc unhook claude` takes back exactly tower's wiring for that client; other names and `atc unhook --all` work the same way. Tower-owned plugin directories and OpenCode's module are removed whole, together with the skills tower installed. Shared settings and marketplaces lose only tower's entries, and shell rc files lose only tower's marked lines; other entries and manually authored trigger lines remain. Copilot's dedicated marketplace and registration are removed with its plugin. Codex's plugin and marketplace entry are removed, and the report names a separate command to clear its client cache. Board records and workflow shelves are independent of wiring.

## The model

**A flight is a record, and the board is derived from it.** The stored fields are subject, body, status word, assignee lane, priority (a free string, `none` when unsaid), labels, skill, the edges it depends on, its comments, and a history of every gesture with the byline and session that made it. A sub-flight is a flight: it files into its own status group, and what says a row is a family is the parent's progress mark, `(1/3)`, closed children over total.

**Intent is stored; the board is derived.** The word a row shows is folded from the record at every render — in backlog, started, closed, the open question, the edges — and nothing on it was read from a tree or asked of fufu. In Progress is a word someone set, with their byline; Done is asserted, never derived. tower checks neither against the repository, and it stores nothing it did not receive as a verb.

**Nothing tower writes is undoable by `ff undo`.** The manifest says so (`undoable: false`): the log is append-only, and every verb is a new event. Disagreeing with the record is another event — `atc edit <target>`, `atc unlink <a> <b>`, `atc cancel <flight> -m "<why>"` — and the brief's history keeps both.

## Naming a flight

A flight has two names. The board prints `#3`; when two writers share a board and the numbers clash it prints `pi-8c2e#3`; the wire carries `pi-8c2e.140`, the id of the event that filed it. Every verb that takes a flight accepts all three: `<n>`, `<writer>#<n>`, or `<writer>.<seq>`, with one leading `#` stripped so what tower prints pastes back in. A bare number must match exactly one filed flight, or the verb refuses with `flight/ambiguous` and lists the full forms. A flight named inside `-m` prose — `see #3` in a body, a comment, a question, an answer, or a cancel's reason — is stored as its wire id by the same resolution and printed by its current number; a match on nothing stays as typed. Event seqs are shared by every event kind on a writer's chain, so wire ids are sparse: `pi-8c2e.140` and `pi-8c2e.146` can be neighbors.

## Reading

Every read folds the log fresh and never blocks on the network.

- `atc`, and `atc board` — what needs a person pinned on top: `questions` (held flights, oldest ask first) and `yours` (Ready in the `me` lane); under it backlog, waiting, ready, in progress, held, and the three newest closed. `atc --closed 7d` widens that last group to a span; a count, `all`, and `none` work too.
- `atc brief <flight>` (alias `show`) — the whole record: every field, the newest handoff pinned above the comments, the comments with any question and answer among them, the links with each linked flight's subject and status, each parent as a link row, with `-x`/`--expand` printing its body under it, one level up and no further, the flights whose prose names this one under `referenced by`, the history with the byline and the words each verb took, and the standing. Nothing from the repository. A closed flight briefs like any other.
- `atc procedures` and `atc skills` — the store's two shelves, what is installed on this machine and in this repository. Neither is the binary's; see Landmines.
- `atc explain <id>` — the prose behind a refusal; `atc explain --list` is the whole catalog. A pure lookup, no repository needed.
- `atc config` — every setting with its value and default; `atc version`; `atc doctor`, which exits 1 on findings so a script can gate on it; `atc trigger`, the notice delivered at a client's context boundary or standing in OpenCode's prompt.
- `atc whoami` — who you are: the session and where it came from, the callsign and its source, the lease, the pid, the writer and the author; bare `atc callsign` prints the same. `atc session` — every session on this machine with a lease, yours marked, the repositories each was seen in, dead leases swept first. A terminal wired by `atc hook bash` (or zsh, fish, powershell) is a session of its own, minted once per interactive shell into `ATC_SHELL_SESSION`; an agent launched from it is still its own session, because the client's variable ranks ahead of the terminal's. A launcher that wants a worker tracked sets `ATC_SESSION`, never `ATC_SHELL_SESSION`; `atc session --mint` prints an id for it.

## Filing and shaping

`atc file <subject>` files one flight; `atc file <procedure> <subject>` files under an installed procedure. The procedure is the first positional, and `-p` is priority: `atc file "upgrade axum" -p high` sets a priority and names no procedure. The other flags are `-m` for the body, `--label` (repeat it for more than one), `--skill`, `--assignee`, and `--status`, which takes only `backlog`, `ready`, or `in_progress`. A filing that says nothing lands on `tower.defaultFileStatus`, `ready` by default.

A procedure is those same fields saved across a graph of flights. A one-flight procedure collapses onto the filing, your flags winning; two or more file a parent and its parts in one append, so no flight is ever live, unlinked, and pullable. A bare filing whose fields a match rule covers files under that rule's procedure once, at file time, and a `routed` event on the record names the rule. The definition is copied into the log at filing, so editing it afterward disturbs nothing in the air.

- `atc decompose <flight> <part>` with one subject per argument splits a flight by hand; exactly one argument naming an installed procedure mints its flights instead. Parts are born Ready whatever the default says. Every part closed makes the parent Ready, not done: finishing the whole is a judgment.
- `atc link <a> <b>` declares that `a` depends on `b`; `atc unlink <a> <b>` takes it back, and is the only way to disagree with a derived Waiting.
- `atc comment <flight> -m "<note>"` goes on the record and nowhere else. `atc comment <flight> --handoff -m "<state of play>"` flags the note as the state of play: the brief pins the newest handoff above the stream, every prior one stays in it, and the flight's status does not move.
- `atc edit <target>` rewords a flight (`-s`, `-m`, `-p`, `--label`, `--skill`) or, given a comment's event id, the comment. An overlay: the fold reads the newest value per field and the log keeps every prior one. `--label` replaces the set wholesale and cannot clear it.

Procedures and skills live in two layers keyed by name, `~/.config/tower/procedures/<name>.toml` and `<main worktree>/.tower/procedures/<name>.toml` (skills under `skills/<name>.md` beside them), and a repository entry replaces the user's wholesale. tower ships none.

## Status

Seven words, and you can type five of them: `atc status <flight> backlog`, `ready`, `in_progress`, `done`, or `canceled`. `waiting` comes from links and `held` from a question, and typing either is refused with the verb that gets you there. The record derives the word a flight shows, first rule winning: a foreign word stands verbatim; closed is closed whatever the edges say; an open question is Held; backlog; started is In Progress, and a pull beats an open dependency; any dependency not closed is Waiting; else Ready. The echo says where the word landed and how many dependencies it waits on, so `atc status <flight> ready` on a gated flight answers `waiting`.

A closed flight refuses every move; the log keeps its record, and comments and edits still land. An open question refuses every move except `done` and `canceled`. `atc done <flight>` finishes. `atc cancel <flight> -m "<why>"` closes without the finish, and the reason is stored on the move.

## Holds and answers

`atc hold <flight> -m "<question>"` stops a flight with the question on its record. The exit is 3, an outcome and not an error: the envelope is a full success envelope carrying the held event, and only the code says the flight stopped with a question. One question per flight; a second hold refuses with `hold/exists`. Holding clears started, so the flight is no longer In Progress, and nothing is torn down.

`atc answer <flight> -m "<answer>"` clears the question. The answer counts as the flight's freshest motion and the record derives Ready, or Waiting when a dependency is still live, never straight back to In Progress; the next pull is a fresh claim, and the answer is on the brief for whoever makes it. `next` never picks a held flight, and `elsewhere` never counts one.

Hold is the fallback for an unattended run. With a person in the conversation, ask there, and put the decision on the record as a comment or an edit.

## Claiming

`atc next [<lane>...]` pulls from the lanes named, walked in the order given and nothing appended: `me`, your own queue, the literal `me` lane and your callsign's; `agent`, the shared pool alone; `none`, the unassigned lane; or a callsign, that pilot's queue. Nothing named is your own default — under a client, `me`, your client's lane, `agent`, `none`, with the client's lane left out when your callsign already is the client word; at a shell with no client, `me` then `none`. Each lane walks by priority, then filed order. A lane named twice walks once. `-n 3` admits up to three the same way; `--assignee <lane>` says where each pick lands, with `file`'s words — `me` when unsaid, storing your callsign, so `--assignee agent` is how a pick stays in the pool — and the re-lane is written only when the lane changes; `--peek` runs the same computation with nothing written, and the envelope's `pulled` says which happened. Three outcomes: `work` (exit 0, something picked), `drained` (exit 1, the board has nothing left), and `elsewhere` (exit 1, Ready work exists in a lane the walk never entered — the count rides as `elsewhere`). Both empties are full data envelopes: `--json` exits 0 on a pick and 1 on an empty one, a JSON reader branches on `outcome`, and a shell loop stops on the code. `-n 0` refuses.

The pick is the claim and nothing else: each picked flight is set In Progress with your callsign as the pilot and moves into your queue unless `--assignee` says otherwise, the re-lane riding in the same append, and `next` hands out no branch and no tree. Each `picked` row carries `flight`, `number`, `subject`, and `skill` (absent when the flight names none); a `skill` is an installed skill's name, and `atc skills <name>` prints it raw. A flight with a live dependency is Waiting, not in the pool, and never reaches the walk.

## Lanes

`atc assign <flight> me`, `agent`, `none`, or a callsign; `none` is absence, and an unassigned Ready flight stands in the `ready` group as nobody's claim. The lane is the whole routing decision: `agent` is the open pool, a callsign is one pilot's own queue, and `me` stores your callsign — so nothing unshaped is handed out, and `assign` is the gate. Which pilot flew a flight is the callsign on the event: your callsign is `ATC_CALLSIGN` when the launcher set one, else the word you gave `atc callsign` this session, else the client you run under, else the login name at a terminal, else none, and every event carries it beside the session and the author, so the brief's history shows the pilot. A callsign needs nothing to be a lane. Run `atc callsign <name>` first thing when your brief names you, and otherwise fly as your client: the word is held on your session's lease, no two live sessions on a machine hold one word, and when your word changes the open flights you laned under the old one follow it. Subagents share the session and its callsign; work to be tracked as a separate pilot is a separate session — `claude -p` under `ATC_SESSION` and `ATC_CALLSIGN` — and a launcher that runs a pool sets `ATC_SESSION` per worker. Bare `atc callsign` says who you are.

## Machine surface

`--json` on every verb, success and failure alike, emits one line on stdout:

```
{"atc":1,"cmd":"next","data":{…}}
{"atc":1,"cmd":"status","error":{"id":"status/held","message":"…","exits":["…"]}}
```

`data` and `error` never appear together. A refusal fufu shaped itself is forwarded verbatim under fufu's id, and `ff explain <id>` holds its prose; `atc explain <id>` holds tower's.

| Exit | Meaning |
| --- | --- |
| 0 | success, an empty board included |
| 1 | a refusal; also `next`'s empty pick and `doctor`'s findings, each with a data envelope |
| 2 | `usage/*`, and clap's own refusal of a command line |
| 3 | `hold` succeeded; the flight stopped with a question |
| 4 | `ref/contended`; another writer had the lock, run it again |

The log is `refs/tower/log/<author>/<writer>`, one orphan chain per writer; `tower.writer` is minted at the first append and is not a setting to copy between machines. Sync is a `git push` or `git fetch` of that refspec; there is no verb, and a chain this repository has not fetched shows in `atc doctor` as events off the board. `atc serve` answers the same envelopes at `/api/…` and streams changes at `/api/feed`; a person starts it, and every other interface works with it down.

## Landmines

- **There is no requeue.** Handing a flight back is `atc status <flight> ready`; the record decides between Ready and Waiting.
- **`-p` is priority.** A procedure is the first positional of `atc file`, never a flag.
- **`waiting` and `held` are never typed.** They are derived from links and questions; `atc link <a> <b>` and `atc hold <flight> -m "<question>"` are the way in, `atc unlink <a> <b>` and `atc answer <flight> -m "<answer>"` the way out.
- **`done` on a held flight abandons the question.** Deliberate when the flight itself is over, and the only move a question allows besides `canceled`.
- **Labels cannot be cleared through `edit`.** `--label` replaces the set; with no `--label` the set stands.
- **`atc skills` is the store's shelf.** It lists what is installed under `.tower/skills/` and `~/.config/tower/skills/`, the policy a flight carries. This manual is the binary's and never appears there.
- **Only tower writes tower state.** Never hand-edit `refs/tower/*`, and never `refs/fufu/*`; `atc doctor` names what a hand-edited log produces.

## The authority

Every verb's own `--help` is the last word on its flags and its behavior, and it is long-form and worth reading: `atc help next`, or `atc next --help`. `atc explain --list` is every refusal tower can make. This page is routing and the model; the binary is the specification.

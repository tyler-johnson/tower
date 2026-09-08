---
name: tower
description: Advanced use of tower (ff tower), the board over fufu. Use when driving tower from a script or a loop, reading its JSON envelope and exit codes, naming a flight by number or wire id, holding a flight on a question or answering one, claiming work with next into a bay, filing under a procedure or decomposing a flight, or whenever the board says something a verb refuses to change.
---

# tower

`ff tower` is the board over fufu. Work is filed as flights, every verb appends one event to a log kept as ordinary git refs in the repository, and every render folds that log fresh. Nothing is entered twice, and nothing tower stores needs tower to read back: the refs are plain git objects beside history.

tower is called and never calls. It runs no dispatch and no loop; the harness a session runs in is the scheduler, and tower is the queue and the record. The once-per-session briefing already gave the agent bare `ff tower` and the loop's four gestures. This is the rest.

## The tools

The MCP server `ff mcp` serves four tower tools beside fufu's own: `tower__next`, `tower__brief`, `tower__hold`, and `tower__done`, the loop's four gestures. Each takes the verb's own flags as fields — `{"count": 3, "peek": true}` on `next` is `ff tower next -n 3 --peek` — plus a `cwd`, and returns the envelope as structured content. Everything else on this page is the shell.

## The model

**A flight is a record, and the board is derived from it.** The stored fields are subject, body, status word, assignee lane, priority (a free string, `none` when unsaid), labels, skill, bay ask, the edges it depends on, its comments, and a history of every gesture with the byline and session that made it. A sub-flight is a flight: it files into its own status group, and what says a row is a family is the parent's progress mark, `(1/3)`, closed children over total.

**Intent is stored; the repository audits it.** Two lines appear on a row when the branch disagrees with the word: `no changes on the branch for 2d` under In Progress, after `tower.staleFlightThreshold` (`false` turns it off), and `changes on the branch since it was set ready` under Ready, which has no threshold and is never off. Both are flagged, never corrected. Done is asserted, never derived.

**Nothing tower writes is undoable by `ff undo`.** The manifest says so (`undoable: false`): the log is append-only, and every verb is a new event. Disagreeing with the record is another event — `ff tower edit <target>`, `ff tower unlink <a> <b>`, `ff tower cancel <flight> -m "<why>"` — and the brief's history keeps both.

## Naming a flight

A flight has two names. The board prints `#3`; when two writers share a board and the numbers clash it prints `pi-8c2e#3`; the wire carries `pi-8c2e.140`, the id of the event that filed it, and that is the string on every `--session` tag fufu records for it. Every verb that takes a flight accepts all three: `<n>`, `<writer>#<n>`, or `<writer>.<seq>`, with one leading `#` stripped so what tower prints pastes back in. A bare number must match exactly one filed flight, or the verb refuses with `tower/flight/ambiguous` and lists the full forms. Event seqs are shared by every event kind on a writer's chain, so wire ids are sparse: `pi-8c2e.140` and `pi-8c2e.146` can be neighbors.

## Reading

Every read folds the log fresh and never blocks on the network.

- `ff tower`, and `ff tower board` — what needs a person pinned on top: `questions` (held flights, oldest ask first) and `yours` (Ready in the `me` lane); under it backlog, waiting, ready, in progress, held, and the three newest closed. `ff tower --closed 7d` widens that last group to a span; a count, `all`, and `none` work too.
- `ff tower brief <flight>` (alias `show`) — the whole record plus the reads: branch, tip, last change, the two audit bits, `current` (the invoking worktree is its bay), `held` (a fufu hold on the branch), `resolving`, and the standing on `next`'s walk. It runs no collide probe unless a verdict could change the answer, so a brief is instant. A closed flight briefs like any other.
- `ff tower procedures` and `ff tower skills` — the store's two shelves, what is installed on this machine and in this repository. Neither is the binary's; see Landmines.
- `ff tower explain <id>` — the prose behind a refusal; `ff tower explain --list` is the whole catalog. A pure lookup, no repository needed.
- `ff tower config` — every setting with its value and default; `ff tower version`; `ff tower doctor`, which exits 1 on findings so a script can gate on it; `ff tower briefing`, the line fufu shows a new session.

## Filing and shaping

`ff tower file <subject>` files one flight; `ff tower file <procedure> <subject>` files under an installed procedure. The procedure is the first positional, and `-p` is priority: `ff tower file "upgrade axum" -p high` sets a priority and names no procedure. The other flags are `-m` for the body, `--label` (repeat it for more than one), `--skill`, `--assignee`, `--bay`, and `--status`, which takes only `backlog`, `ready`, or `in_progress`. A filing that says nothing lands on `tower.defaultFileStatus`, `ready` by default.

A procedure is those same fields saved across a graph of flights. A one-flight procedure collapses onto the filing, your flags winning; two or more file a parent and its parts in one append, so no flight is ever live, unlinked, and pullable. A bare filing whose fields a match rule covers files under that rule's procedure once, at file time, and a `routed` event on the record names the rule. The definition is copied into the log at filing, so editing it afterward disturbs nothing in the air.

- `ff tower decompose <flight> <part>` with one subject per argument splits a flight by hand; exactly one argument naming an installed procedure mints its flights instead. Parts are born Ready whatever the default says. Every part closed makes the parent Ready, not done: finishing the whole is a judgment.
- `ff tower link <a> <b>` declares that `a` depends on `b`; `ff tower unlink <a> <b>` takes it back, and is the only way to disagree with a derived Waiting.
- `ff tower comment <flight> -m "<note>"` goes on the record and nowhere else.
- `ff tower edit <target>` rewords a flight (`-s`, `-m`, `-p`, `--label`, `--skill`, `--bay`) or, given a comment's event id, the comment. An overlay: the fold reads the newest value per field and the log keeps every prior one. `--label` replaces the set wholesale and cannot clear it.

Procedures and skills live in two layers keyed by name, `~/.config/tower/procedures/<name>.toml` and `<main worktree>/.tower/procedures/<name>.toml` (skills under `skills/<name>.md` beside them), and a repository entry replaces the user's wholesale. tower ships none.

## Status

Seven words, and you can type five of them: `ff tower status <flight> backlog`, `ready`, `in_progress`, `done`, or `canceled`. `waiting` comes from links and `held` from a question, and typing either is refused with the verb that gets you there. The record derives the word a flight shows, first rule winning: a foreign word stands verbatim; closed is closed whatever the edges say; an open question is Held; backlog; started is In Progress, and a pull beats an open dependency; any dependency not closed is Waiting; else Ready. The echo says where the word landed and how many dependencies it waits on, so `ff tower status <flight> ready` on a gated flight answers `waiting`.

A closed flight refuses every move; the log keeps its record, and comments and edits still land. An open question refuses every move except `done` and `canceled`. `ff tower done <flight>` finishes; bare `ff tower done` finishes the invoking worktree's flight, derived from its newest session-tagged operation. `ff tower cancel <flight> -m "<why>"` closes without the finish, and the reason is stored on the move.

## Holds and answers

`ff tower hold <flight> -m "<question>"` stops a flight with the question on its record. The exit is 3, an outcome and not an error: the envelope is a full success envelope carrying the held event, and only the code says the flight stopped with a question. One question per flight; a second hold refuses with `tower/hold/exists`. Holding clears started, so the flight is no longer In Progress, and nothing is torn down: the bay stays warm, and the branch and its tip stay on the row.

`ff tower answer <flight> -m "<answer>"` clears the question. The answer counts as the flight's freshest motion and the record derives Ready, or Waiting when a dependency is still live, never straight back to In Progress; the next pull is a fresh claim, and the answer is on the brief for whoever makes it. `next` never picks a held flight, and `yours` never counts one.

Hold is the fallback for an unattended run. With a person in the conversation, ask there, and put the decision on the record as a comment or an edit.

## Claiming and bays

`ff tower next` pulls from the pool: every Ready flight in the `agent` lane, walked in filed order. `-n 3` admits up to three that collide with neither each other nor anything flying; `--peek` runs the same computation with nothing written, and the envelope's `pulled` says which happened. Three outcomes: `work` (exit 0, something picked), `drained` (exit 1, the board has nothing left), and `yours` (exit 1, Ready work exists that the lane alone kept out of the pool). Both empties are full data envelopes; a JSON reader branches on `outcome`, and a shell loop stops on the code. `-n 0` refuses.

Each `picked` row carries `flight`, `number`, `subject`, `branch`, `bay`, `bay_id`, `warmed`, `skill` (absent when the flight names none), and `refused`. `passed` rows say why a checked candidate lost — `collides`, with the flight and the paths, or `no-verdict` when fufu could not judge the pair — and stop where the walk stopped, so the output is bounded by the ask. The collision rule: only a candidate that already has a branch is checked, against every flying tree and every candidate already admitted, and an unknown verdict excludes. A fresh flight is admitted unchecked, because one tree per flight is the deconfliction. A flight with a live dependency is Waiting, not in the pool, and never reaches the walk.

**Bays are worktrees, never registered.** The pool is what `ff worktree list` reports, and occupancy is derived per render: the live flight whose freshest session-tagged operation sits on a bay's branch is the occupant, and a closed flight frees its bay on the next render. `next` seats each pick in a free bay, or warms one under `tower.bays` when none is free and the flight's `--bay warm` ask says to; otherwise `bay` is null and the pick stands with no tree. The seat is bound with `ff switch` when the flight has a branch and `ff start` when it does not, minting `flight/<wire id>`, both under `--session <wire id>`: that tag is what every later derivation reads back. A bind fufu refuses lands in the row's `refused`, the pull stands, and `ff tower status <flight> ready` hands the flight back.

- `ff tower bay` lists the pool with occupants; `ff tower bay warm` mints the next slot under `tower.bays` (refused until it is set; a path argument puts one exactly there); `ff tower bay release <bay>` tears one down, refused with `tower/bay/occupied` while a live flight sits in it.
- Discipline for an agent: `cd` into the picked bay, work only there on the picked branch, commit with `ff commit`, and finish with bare `ff tower done` from inside it. From outside, `ff -C <bay> tower done` says the same thing.

## Lanes

`ff tower assign <flight> me`, `agent`, or `none`; `none` is absence, and an unassigned Ready flight stands in the `ready` group as nobody's claim. The lane is the whole routing decision: the queue draws only from the agent lane, so nothing unshaped is handed out, and `assign` is the gate. Which agent flew a flight is not a field; every event carries the byline and the session of whoever wrote it, and the brief's history shows the pilot.

## Machine surface

`--json` on every verb, success and failure alike, emits one line on stdout:

```
{"ff":1,"cmd":"tower next","data":{…}}
{"ff":1,"cmd":"tower status","error":{"id":"tower/status/held","message":"…","exits":["…"]}}
```

`data` and `error` never appear together. A refusal fufu shaped itself is forwarded verbatim under fufu's bare id, and `ff explain <id>` holds its prose; everything of tower's is under `tower/`, and `ff tower explain <id>` takes it with the namespace or without.

| Exit | Meaning |
| --- | --- |
| 0 | success, an empty board included |
| 1 | a refusal; also `next`'s empty pick and `doctor`'s findings, each with a data envelope |
| 2 | `tower/usage/*`, and clap's own refusal of a command line |
| 3 | `hold` succeeded; the flight stopped with a question |
| 4 | `tower/ref/contended`; another writer had the lock, run it again |

The log is `refs/tower/log/<author>/<writer>`, one orphan chain per writer; `tower.writer` is minted at the first append and is not a setting to copy between machines. Sync is a `git push` or `git fetch` of that refspec; there is no verb, and a chain this repository has not fetched shows in `ff tower doctor` as events off the board. `ff tower serve` answers the same envelopes at `/api/…` and streams changes at `/api/feed`; a person starts it, and every other interface works with it down.

Every fufu read tower makes captures first, like every fufu verb, so folding a board against a dirty tree appends a snapshot to that worktree's fufu chain. Under `ff tower <verb>` the child inherits `FF_REPO`, `FF_CONTRACT`, and `FF_SESSION` from fufu's dispatch, and bare `ff tower done` and bay occupancy both read the session.

## Landmines

- **There is no requeue.** Handing a flight back is `ff tower status <flight> ready`; the record decides between Ready and Waiting.
- **`-p` is priority.** A procedure is the first positional of `ff tower file`, never a flag.
- **`waiting` and `held` are never typed.** They are derived from links and questions; `ff tower link <a> <b>` and `ff tower hold <flight> -m "<question>"` are the way in, `ff tower unlink <a> <b>` and `ff tower answer <flight> -m "<answer>"` the way out.
- **`done` on a held flight abandons the question.** Deliberate when the flight itself is over, and the only move a question allows besides `canceled`.
- **Labels cannot be cleared through `edit`.** `--label` replaces the set; with no `--label` the set stands.
- **`ff tower skills` is the store's shelf.** It lists what is installed under `.tower/skills/` and `~/.config/tower/skills/`, the policy a flight carries. This manual is the binary's and never appears there.
- **Only tower writes tower state.** Never hand-edit `refs/tower/*`, and never `refs/fufu/*`; `ff tower doctor` names what a hand-edited log produces.
- **A handshake flag is argv[1] or nothing.** `--ff-manifest` behind a verb is clap's unknown argument, not a handshake.

## The authority

Every verb's own `--help` is the last word on its flags and its behavior, and it is long-form and worth reading: `ff tower help next`, or `ff tower next --help`. `ff tower explain --list` is every refusal tower can make. This page is routing and the model; the binary is the specification.

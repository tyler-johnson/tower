# tower — design

*The design of the tower that shipped, September 2026. It began as a sketch in August, and #123 untangled it from fufu; this is what stands.*

**tower** is project management for people and agents, built on fufu. It lives in its own repository and installs one binary, `atc`, which is what fufu's `ff-<name>` dispatch finds for `atc`. There is no bare `tower` command: the dependency on fufu is real rather than decorative, so the verb is reached through fufu or not at all.

The name comes from fufu's own metaphor. fufu is the pilot — it flies the repository. tower flies nothing; it assigns work and keeps the record.

## The premise

Every tracker uses the same model, because the model works: a ticket with a status, an assignee, a priority, labels, and links. tower keeps that model on purpose. A flight is instantly recognizable to anyone who has used Linear, Jira, or GitHub Projects, because nothing about agents changes what work *is* — what agents change is the granularity of it and who is asking for the next piece.

What is different is where the model lives. tower is linear-lite over fufu, with the repository as the database: every verb appends an event to a log kept as ordinary git refs in the repository, the board is a fold over that log, sync is one refspec, and nothing tower stores needs tower to read back — the refs are plain git objects beside history, legible to `git log` on a machine that has never heard of tower.

> **Intent is stored; the board is derived.**

Status is a projection the verbs move by setting facts — in backlog, started, closed, a question, the edges — and the words are the ones every tracker uses. Nothing is entered twice: the word a row shows is folded from the facts at every render, and a render never blocks on the network, because the log is local. tower reads nothing from the working tree and asks fufu for nothing at render. The board is a pure function of the log.

So the shape of the product is a boring model with an interesting engine. The model — flights, statuses, assignees, priorities, labels, sub-flights — is the one everyone already knows, and you could describe it to a Linear user in one breath. The engine underneath is the log, the fold, and the seam: an append-only record on refs, one derivation over it that every surface renders, and a contract with fufu narrow enough to print on a card. Delete the engine and a small ordinary tracker remains; that property is deliberate and load-bearing.

## The seam

tower is a separate program in a separate repository. Issue tracking is not a version control operation, and fufu's principle 10 — verbs must earn their existence — kills it as a native verb on its own merits. It is discovered as `atc` through fufu's extension dispatch, and has its own release, its own authority, its own store, and its own cadence. It links no fufu crate: the contract below is the entire surface between the two, and it is the same surface every other declared extension gets.

The contract:

```
reached   atc, in the current directory
envelope  {"atc": 1, "cmd": "<verb>", data | error} — bare ids, fufu's exit codes
stores    refs/tower/log/<author>/<writer>
spawns    ff --version in doctor · ff watch --all in serve
writes    nothing under refs/fufu/*, ever
```

That last line is fufu's extension rule, unmodified: extensions read fufu state and call fufu verbs; only fufu writes fufu state. tower stays well inside it. Outside `doctor` and `serve` it spawns fufu for nothing at all, and no verb's answer depends on a fufu read.

## Passive by construction

**tower is a thing agents call. It never calls agents.**

Passive is a statement about initiative, not about process count. tower starts no agent work and owns no agent lifecycle; it does not claim that no tower process runs. Every verb is a read plus a local write, and there is no dispatch and no iteration verb. If work should loop, the agent harness loops and calls `atc next` again — the harness is the scheduler, tower is only the queue.

`atc serve` is a standing process, a daemon by any honest reading, and it stays inside the line because it is a clock and a subscriber rather than an actor. It refolds when the repository moves, pulls upstream on a cadence, and everything it learns lands in the log as the same events a lazy pull would have written. It holds no state the log does not, decides nothing, and dispatches nothing. Every interface works without it, just staler — an accelerant, never a dependency. A person starts it; tower never starts it for them.

The reasons to stay passive compound. Initiating means owning agent lifecycle: keys, model selection, retries, context limits, per-vendor quirks — a second product, and a moving one. Staying passive makes tower vendor-neutral by construction, because it never learns who is calling beyond the byline on an event. And a queue that dispatches on its own is a background process making outward-facing decisions nobody asked for that minute, which fufu's principle 9 already forbids in its own domain.

Assignment is routing, not dispatch. Assigning a flight to the agent lane puts it in a queue an agent has to come ask about; it calls no one, and tower still never learns who will answer.

Sync follows the same discipline. Upstream is pulled lazily at invocation, gated by a cadence stamp, the way fufu's auto-trim and update check already work. With the standing process up, that same pull runs on its cadence and appends the same events — the cadence is config it reads, never authority it holds. The board is fresh because you just asked for it, or because a subscriber refolded it, and the fold is identical either way. Anything that needs to reach you unasked belongs in fufu's ambient shell channel — a heartbeat the user started — or in a process the user started, never in one tower spawned behind them.

The passive update lane remains the one process tower starts for itself, fufu's carve-out carried over verbatim: official binaries (never dev, dogfood, or test builds; never under CI) spawn a detached `atc update --check` at most once per `tower.updateCheck` (default daily) — the one sanctioned self-spawn. It refreshes a small cache file under the user cache dir and exits; foreground commands read the cache, and with `tower.autoUpdate` on (the default) a newer release installs itself silently in the background, or with it off a one-line notice lands on stderr instead. Three throttles keep it polite: the cadence gates the checks, auto-install probes retry at most daily, and a release is announced at most once, ever. `tower.updateCheck false` kills the whole lane. The trust root is deliberately plain — HTTPS to GitHub plus the release's sha256, the same root the install scripts rely on.

tower enforces nothing. It stores the word and shows it: a flight set In Progress is In Progress because someone said so, and the record says who and when. tower hooks nothing, vetoes nothing, and never reaches into the tree to check. That is fufu's regime boundary, inherited.

## Storage and sync

Not files in the working tree. `.tower/flights/*.md` is the obvious move and the trap every git-native tracker falls into: the board becomes branch-dependent, ticket edits pollute code diffs, and closing something on an unmerged branch means the board lies until merge.

**An orphan ref, shaped like fufu's journal.** `refs/tower/log/<author>/<writer>` — a commit chain with its own tree, no relation to code history, never touching the working tree, CAS-appended, reachability as the gc pin. Sync is one explicit refspec. The writer component is the machine's, minted once into local config: one ref per author alone breaks the moment two machines append under one email — both chains diverge and a push is rejected with no merge available, because a commit chain has no union — while a ref per writer makes every push a fast-forward, and the fold unions `refs/tower/log/**` either way.

**Every event carries four stamps.** The writer, which chain; the author, whose email; the session, which run — fufu's tag when a client handed one down, the login name at a terminal; and the callsign, which pilot. The first three are provenance, and none of them says who is flying a flight: an email is a person's every machine, a session is one run. The callsign is a chosen readable name for a pilot, person or agent, shared across machines on purpose — the same role on two machines is one pilot — and it comes from `ATC_CALLSIGN` when the launcher sets one, else the client the process runs under, detected from the mark every agent client leaves on its shell — `CLAUDECODE` is claude, `GEMINI_CLI` is gemini, `CURSOR_AGENT` is cursor, `CODEX_SANDBOX` is codex — else the login name at a terminal, else none. Under Claude Code `me` stores `claude` with nothing configured, and the variable is the override for a launcher that names its own pilot, the way a headless `claude -p` running a review exports `qwen-review` over the mark beneath it.

The conflict problem dissolves because of what is stored. Stored intent is an append-only event log partitioned per writer, so merging divergent logs is a **union, not a merge** — conflict-free by construction. The board is a fold over the union. The only genuine collision is two people editing one field in the same window; last-writer-wins with a stable tiebreak, and both events survive in the log regardless.

**Sync is three tiers, and only one of them needs anything built.** *Machine-local* — caches and the writer id — never syncs and mostly rebuilds. *Mine across machines* — solo flights, notes, decompositions — is single-author and append-only, so backup or roaming is one plain `git push refs/tower/log/<me>/*` with no protocol at all — and no verb: tower builds nothing here. tower is a local tool that interfaces with remote data, and the designed way anything leaves the machine is promotion, the publish boundary named under *Upstream is a foreign writer*. *Shared with others* is the only hard tier, and tower does not have it: in team mode upstream already holds it, and in solo mode it does not exist.

Multi-writer works anyway — fetch `refs/tower/log/*`, fold the union — and it stays documented and unsupported. Every git-native tracker that tried to be the shared board was technically fine and socially dead: shared work needs a place people look, and a ref in a repository is not one. Making it one means notifications, identity, and permissions, which is a different product wearing this one as a hat. **tower never becomes the shared board; sharing is promotion.**

The deeper reason is that tower has no mechanism for agreement. Its facts need no consensus — the log says who set what, and every event carries its byline — which is why tower can assert them unilaterally and be believed. Upstream state is negotiated: priority, ownership, what ships this cycle. A shared tower board would manufacture consensus data with nothing underneath it, and two people would confidently read different boards.

One honest consequence: this is the first fufu-adjacent state that is not a cache. fufu's principle 3 says state is rebuildable and the repository wins; authored text is derivable from nothing. It holds anyway — the store *is* ordinary git objects in the repository, so the repository still wins literally — but authored flights are losable in a way no fufu state is. That is accepted rather than papered over: the store is ordinary git refs, and whether they leave the machine is git's business, not tower's — tower carries no backup surface at all, no verb, no warning, no doctor row.

## The model

A flight is an issue, and it carries what every issue carries:

| field | values |
|---|---|
| subject, body | authored text |
| status | Backlog · Waiting · Ready · In Progress · Held · Done · Canceled |
| assignee | me · agent · a callsign |
| priority | urgent · high · medium · low · none |
| labels | freeform strings, rendered as chips, filterable everywhere |
| skill | what an agent reads to fly it — the one field other trackers do not have |
| edges | depends-on and blocks, declared with `link`, taken back with `unlink`; a parent depends on its sub-flights |
| comments, history | the record, append-only |

Every field is settable directly at filing — `atc file "subject" -p high --label chore --skill review --assignee agent` — and editable after. A procedure (below) is nothing more than those same fields saved across a graph of flights.

**Assignee is a lane: `me`, `agent`, `none`, or a callsign.** The lane is the routing decision — whose queue is this in. `agent` is the shared pool, which `next agent` draws from; a callsign is one pilot's own queue; `me` stores your own callsign when you have one and the literal word when you do not, and either way the flight is yours on the board. Your own queue — the `me` lane and your callsign's — is what bare `next` walks, and the unassigned lane is everyone's overflow: every walk falls through to it once its own lane is drained. A callsign needs nothing to be a lane — values are open everywhere in tower, and there is no roster to be on. Which pilot flew a flight is the callsign on the event — the board shows the chip and the history shows the pilot, "In Progress — claude, 4m ago" — and the session stays provenance underneath it. Describe the work with `skill` and labels first, and let workers pull what they know how to fly; a callsign lane is for the case where one pilot is the right one.

**Sub-flights are just flights.** Decomposing a flight — by hand or under a procedure — mints real flights under a parent, joined by the same edges `link` writes. A sub-flight has every field a flight has, appears in queues on its own merits, and is distinguishable from hand-filed work by nothing except its parent edge. One level of flights and edges is the whole model; a tree of flights is what other trackers call a project, and it needs no second entity.

## Status

Status is derived from the record, moved through the same verbs and words, and every gesture is attributed in the history. A status word — a verb, a drag on the board, an agent's gesture mid-loop — assigns three stored facts, in backlog, started, closed; the open question and the dependency edges are facts of their own; and every fold projects the seven words back out of them. The names are chosen so a Linear user reads the board cold; Waiting and Held are the two Linear does not have, and each earns its sentence:

- **Backlog** — not yet cleared for work, deliberately. The parking place for work nobody has decided about: where a filing lands when it says `--status backlog`, when `tower.defaultFileStatus` is set to it, or when a procedure flight declares it. Nothing leaves Backlog except by a person's gesture; that deliberateness is the definition.
- **Waiting** — cleared, but gated by the graph: something this flight depends on is still live. Never written: a flight is born here by its edges and computed here at every fold, so linking a dependency re-gates a Ready flight, and a dependency closing, done or canceled, releases it with no event appended — the closer's gesture is the mark. Unlinking the dependency releases it the same way: the edge leaves, and the next fold derives Ready. The row says what it waits on.
- **Ready** — cleared and unblocked. The agent queue draws from Ready flights assigned to the agent lane; your own Ready flights are the list you pick from.
- **In Progress** — someone is flying it. The pick sets it for agents, with the pilot's byline; you set it by hand.
- **Held** — stopped on a blocking question. Holding clears started: the answer returns the flight to Ready or Waiting by the graph, never straight back In Progress. The question piece is its own section below.
- **Done / Canceled** — closed, finished or abandoned, with the reason on the record. Closed flights stay visible: the board carries two closed groups, done and canceled, holding the three newest across both — a constant rather than a config key because the window is a render's memory of the week and not a preference — because a board that forgets the week is amnesiac, and the log was always the full record regardless. The CLI's `--closed` takes more or less of that group for one render: a count, a span like `7d`, `all`, or `none`. Closed is closed: a dependency closing releases the flights that waited on it whether it finished or was abandoned, and a canceled part still shows on the parent's brief, where the person reconsidering the parent will see it.

Two of the words are said rather than observed. In Progress is a word someone set — the pick, or a hand — and Done is asserted by whoever finishes; tower stores the byline and the moment and checks neither against a tree. What the record holds is what was said and by whom, and what the repository did about it is fufu's to show. A tracker that silently rewrites your fields is guessing, so tower does not.

Only one stamp happens without a hand on it, and it is deterministic, attributed, and explained in the history: a match rule choosing a bare filing's procedure at file time. The filing machine matches the fields once, against the procedures it has, and the routing event lands in the filing's own batch under the filer's byline, naming the rule. There is no later pass and no standing process: nothing walks the board later, and nothing appended by one machine restamps what another filed. There is no Waiting → Ready advance either, because Waiting and Ready were never separate facts: the fold derives both from the edges, and a closing releases its dependents the moment anyone folds. Everything else conditional is judgment, and judgment lives in a skill.

## Held — the question piece

An agent mid-flight that hits something it genuinely cannot decide — an ambiguous requirement, a design fork, anything where guessing is worse than stopping — holds the flight: `atc hold <flight> -m "<the question>"`, exit code 3. The flight's status becomes Held with the question attached, and nothing is torn down: the record keeps the question, the tree is the harness's and sits wherever the harness left it, and nothing was guessed. Holding is stopping, not abandoning.

A question is a blocking comment — that is the whole object. It lands in the comment stream flagged as holding the flight, the answer is the reply that releases it, and both survive on the record permanently, which is what makes the resume work: `atc answer <flight> -m "<the answer>"` clears the question and the record derives the flight Ready, or Waiting if a dependency is still live, and whichever agent pulls it next reads the brief — which now carries the question and the answer — and continues. The original asker may be long gone, context wiped, session over. That is fine and expected: tower is the durable half, the agent is disposable, the flight is not.

One open question per flight. A hold means "I cannot proceed," and an agent with four blocking questions on one flight has a decomposition problem, not a Q&A problem. Questions that do not block are comments.

Exit 3 is an outcome, not an error — fufu's precedent. The envelope is a full success envelope with the held event in `data`; only the exit code says the flight stopped with a question. A machine caller branches on the code, a human reads the echo, and neither has to parse an error to learn that holding is what happened. In a loop, 3 is the signal that work exists but needs you — the harness stops cleanly or moves to other flights instead of spinning.

Held inherits fufu's principle 8 whole: announced at creation, pinned in **waiting on you** until answered, loud the entire time. An agent question that goes quiet is how the whole system rots, and the board cannot stop showing one until someone answers.

Hold is the durable fallback, not the preferred channel. An agent in a live session with a person asks in the conversation — better latency, better bandwidth, no ceremony — and holds only when nobody is on the other end: unattended loops, fan-outs, walk-away work. The skills that drive agents say this ordering explicitly, so holds do not get cargo-culted into interactive sessions.

## Upstream is a foreign writer

At work the team already has a tracker. tower does not replace it and is never authoritative over it. This is fufu's principle 2 one layer up: Linear and GitHub are first-class foreign writers, observed and absorbed, never owned. None of this is built — no adapter exists, and solo mode is the only mode that runs today — but the ownership table is the design's, and the local layer already keeps to its row.

Field ownership is enforced hard, or sync becomes a merge problem it does not need to be:

| owner | fields | status |
|---|---|---|
| upstream tracker | exists, title, body, its assignee, its priority, its status, cycle | upstream truth |
| forge | PR, review state, CI, merge | upstream truth |
| tower | status, assignee, priority, labels, skill, edges, queue, briefs, notes | local truth |

The rows do not compete, because tower's fields are the local layer and upstream's are upstream's. The same issue can be In Progress in Linear and Waiting here, and both boards are telling the truth about their own scope — Linear says where the team thinks it is, tower says where this machine's work on it actually is. tower never writes status upstream, never derives its status from upstream's, and shows upstream's fields — when an adapter supplies them — as labeled foreign facts on the brief, nothing more.

Upstream changes arrive as `foreign` events in the local log — labeled, undoable, loud — and upstream wins every field it owns. tower holds a pointer and a local layer beside it; it never merges into someone else's model.

**Never auto-outward.** Automation moves local state freely: assign, decompose, route. Anything the team sees — opening a PR, posting a comment, moving an upstream status — is a deliberate gesture. An agent commenting at machine rate is a social failure with no technical apology.

**Local steps are anonymous branches.** A team ticket decomposes into steps that are real, tracked, briefed, and assignable — and invisible upstream. They are fufu's anonymous branches: genuine from birth, merely not yet named to anyone outside. Promotion is the same gesture at the same boundary: a step that turns out to need a teammate or a PR of its own gets promoted, which mints a real upstream ticket, links it, and keeps the local history — exactly `ff branch <name>` claiming a placeholder at the publish boundary. The team's board stays as coarse as the team wants, the local board is as fine as the work actually is, and neither has to negotiate with the other. Promotion is not built; it waits on the adapter it would mint through.

Adapters are the same fractal as tower itself: `tower-<adapter>` on PATH, reached through tower the way tower is reached through fufu. Solo mode is the case where none are installed, and nothing else changes.

## Surfaces

One model, every renderer — fufu's principle 14, so every surface is a thin shell over the same contract the CLI renders, never a second implementation.

```
caller          surface        what it does
────────────────────────────────────────────────────────────────
a person        CLI            decide, answer, route, publish
an agent        CLI            pull, read a brief, hold, finish
the clock       serve          refold on motion; no verb a caller
                               did not ask for
nothing         —              one sanctioned self-spawn, the
                               detached update check
```

The CLI pins **waiting on you** above a list grouped by status: the inbox, everything that needs a human right now, in two groups that fall out of stored fields with no judgment and no model call — **questions**, the Held flights an agent is stopped on you for, oldest ask first; and **yours**, the Ready flights in the `me` lane, which is the todo list. The web pins nothing, because the built-in **For Me** view is the inbox, `for=me`, an open question in any lane or the `me` lane at any status, beside **All Flights** and the saved views in the chip row under the header. Below the inbox, the list is the one every tracker renders — status groups, then priority, then age within them — with the done and canceled groups collapsed at the bottom. A flight in the inbox still has a status; the inbox is a view of the same rows, and it is the feature the borrowed layout does not come with.

The row is the recognizable anatomy: priority glyph, flight ref, status dot, subject, label chips, assignee, age right-aligned. Filters compose over the stored fields, encode into the URL so a filtered board is a shareable link, and fold server-side: the query is one type in core, parsed from that URL and answered against the same rows the board is built from. The web app adds the views the model earns: a kanban board whose columns are the statuses, where a drag is a verb or it is not offered — to In Progress is pull, to Done is done, to Canceled is cancel, and a drop with no verb behind it does not land; a command palette over verbs, flights, and navigation; single-key movement and verbs on the selected row; projects, the family as an indented tree over the same rows; and search over subjects and bodies, nothing semantic. The CLI renders the same model with the same vocabulary and the same two closed groups. Filters are a system rather than a flag — composable predicates, saved defaults, saved views — so both surfaces wait on it together rather than half of one landing early in one of them.

tower answers no handshakes and serves no tools: an agent reaches every verb from the shell, with the same contract a typed tool would carry, and fufu's own `ff mcp` went for the same reason.

The whole design is aimed at one reflex: bare `atc`, often, because it is the fastest way to learn what to do next. Two things have to hold or the reflex never forms. It has to be honest, which is what deriving the board from the record is for: the word a row shows is a word someone set, folded, never a guess. And **render must never block on the network** — fold the local log, draw, note the age, refresh on the cadence stamp. A board that is fresh and slow loses to one that is instant and honest about how stale it is.

### Projects

The board lists every flight in its status group, one row each. A sub-flight is a flight, and having a parent moves it nowhere: it sorts by priority and age beside everything else, it is counted, and it is filtered. What still says a row is a family is the parent's progress mark, `(1/3)` — closed children over total, one fact in one place. Counts count flights.

The family itself is a view rather than a rule over the list, and that view is **projects**: an indented, folder-like tree read straight off the `depends_on` edges, parents up and children down. A parent's sub-flights render there as a checks list, the way a forge renders CI: `pass ✓ · smoke ✓ · verdict ● yours`. People already read that shape instantly, and it happens to be the truth — a procedure is a pipeline whose stages are flights. The flight detail is the one-level version: a flat parents list and a folder-style children list, off the brief's own links.

The projects view is not built.

### Flight ids

A flight has two names, and the split is human against wire. The wire name is the id of the `filed` event that minted it: `<writer>.<seq>`, unique across machines because the writer component is. JSON envelopes carry it raw, always. The event sequence is shared by every kind of event on a writer's chain — comments, assignments, links, status moves all consume one — so wire ids are sparse by construction and count nothing a person cares about.

Humans get a dense number instead. A flight's number is its position among its writer's `filed` events — derived from the fold, never stored, so there is no second counter to mint, CAS, or sync, and the append-only log makes the numbering stable forever: a closed flight keeps its number, and no filing can renumber an earlier one. Human output prints `#3`, and a board folded from a single writer — the normal case, since tower is local-first and log sync is the exception — needs nothing more. When a second writer's flights are on the board, the writer rides along as `pi-8c2e#3`: `#` binds a writer to a flight number the way `.` binds one to an event seq, so the two forms can never be confused.

On input, any verb taking a flight accepts either name. A bare number resolves as a flight number against the board's filed flights — a unique match wins, an ambiguous one refuses and lists the full forms — `writer#n` names another writer's flight exactly, and the dotted form is always the filing event's id, accepted everywhere a number is. A leading `#` on a bare number is accepted and stripped for paste tolerance; the documented spelling is unprefixed, because an unquoted `#` starts a shell comment.

### Next

`atc next [<lane>]` is a queue's Ready check and the claim in one command. The lane is the argument — `me`, `agent`, `none`, or a callsign — and the caller's own queue, `me`, when unsaid: the literal `me` lane and the caller's callsign, or the literal alone for a caller with none. `agent` is the shared pool alone, `none` the unassigned lane, a callsign that pilot's queue. The walk is the named lane in filed order, then the unassigned lane in filed order — everyone falls through to `none` once their own lane is drained, and naming `none` walks it once — and `-n <k>` admits up to `k` of the walk. Each picked flight is set In Progress in one append, and the callsign on the event is the pilot: the pick is the claim, and nothing else. `--assignee <lane>` re-lanes each pick in that same append, with `file`'s words — `me` stores the caller's callsign — so a worker can claim from the pool and take the flight into its own queue in one gesture; unsaid, the lane stays. `next` hands out no branch and no tree — where the work happens is the harness's to arrange, one tree per flight when it fans out — and a flight with a live dependency is Waiting, in no walk. `--peek` is the same computation with nothing written, and the envelope's `pulled` says which happened. A bad lane word on either side is `usage/bad-assignee`, the refusal `assign` raises.

Three outcomes, and the envelope names each on `outcome`. `work`, exit 0, is a pick: the `picked` rows carry `flight`, `number`, `subject`, and the `skill` the flight names when it names one. `drained`, exit 1, is a board with nothing Ready anywhere. `elsewhere`, exit 1, is Ready work in a lane the walk never entered — the count rides beside it as `elsewhere`. Both empties are full data envelopes, fufu's "no": a shell loop stops on the code, and a JSON reader branches on the word.

### Brief

`atc brief` is the read half of the handoff: `next` hands an agent a flight id and a subject, and the brief is what it reads next — everything the log knows about one flight, in one read over the fold. It is the record and nothing but: subject, body, every field, the comments with any question and answer among them, links carrying each linked flight's subject and status, the history with the byline and the words each verb took, and the standing. No repository facts and no probes, so the brief is instant and reads the same from any tree. A closed flight briefs like any other, because the log keeps the record and reading it is never a lifecycle move. `show` is accepted as a second spelling of `brief`, fufu's own word for reading one thing; the envelope says `brief` either way.

The skill named on the flight is what the agent flies it with; the brief is what the agent flies it *from*, and it is why the asker of a held question does not need to be its resumer. The body and the comments are authored prose, and the reader decides how it reads: the web board renders them as markdown, the terminal prints the source text.

### Decompose

`atc decompose <flight> [<procedure> | <part>…]` makes a flight a parent: under a procedure, the definition's flights are minted beneath it; by hand, each argument files as one sub-flight. Either way the children are `linked` edges and nothing else — no container kind, no parent type — so a sub-flight is indistinguishable from a hand-declared dependency, and that is the point: Waiting derivations, `depends_on`/`blocks`, and the brief's link sections all work on it unchanged. The filings and their edges land in one append, because two would leave a window where the parent is live, unlinked, and pullable — exactly the state the Waiting gate exists to prevent.

A parent's Done stays asserted. Every sub-flight closing makes the parent Ready, not finished — whether the broad task is over is a judgment, and `atc done` is where it gets made.

### The verbs

fufu's rule that every verb must earn its existence carries over, and the one it kills first is `run`. Tower cannot run anything — a verb that implies dispatch would be the first crack in principle 2, and that line is too load-bearing to contradict casually.

| verb | what it does | caller |
|---|---|---|
| `atc` (alias `board`) | the board: what needs you, then the list by status; `--closed` widens or narrows the closed group for one render | you |
| `atc next [<lane>] [--assignee <lane>] [-n <k>] [--peek]` | claim the next Ready flight from a lane — your own queue when unsaid, then the unassigned overflow — or `k` of them in filed order; the pick sets In Progress under your callsign, and `--assignee` re-lanes it in the same append; `--peek` is the same computation with nothing written | an agent |
| `atc brief <flight>` (alias `show`) | everything the record knows about one flight, for whoever picks it up | either |
| `atc file [<procedure>] <subject>` | put work on the board — bare, or under a procedure; every field a procedure sets is a flag here (`-m`, `-p`, `--label`, `--skill`, `--assignee`, `--status`) | either |
| `atc status <flight> <status>` | move a flight; the lifecycle verbs below are this verb carrying a payload | either |
| `atc assign <flight> <me\|agent\|none\|callsign>` | route the flight's queue; `me` stores your callsign | either |
| `atc hold <flight> -m <question>` | stop with a blocking question — exit 3 | an agent |
| `atc answer <flight> -m <answer>` | answer the question and release the flight | you |
| `atc done <flight>` | finish it — off the board, on the record | either |
| `atc cancel <flight> [-m <why>]` | close it unfinished, reason on the record | you |
| `atc link <a> <b>` | declare that one flight depends on another | either |
| `atc unlink <a> <b>` | take back a declared dependency — the only way to disagree with a derived Waiting | either |
| `atc comment <flight> -m <note>` | a note on the record, local; saying it to the team is a separate, deliberate gesture | either |
| `atc edit <target> [-s <subject>] [-m <msg>] [-p <priority>] [--label <label>] [--skill <name>]` | reword a flight's fields, or a comment's text by its event id — an overlay event, the log keeps every prior word | either |
| `atc decompose <flight> [<procedure> \| <part>…]` | make a flight a parent — a procedure's flights, or parts by hand | either |
| `atc explain <error-id>` | look up an error id and see what it means — the prose behind every coded refusal; `--list` is the whole catalog | either |
| `atc procedures [<name>]` | what is installed, what each matches, and where it came from | you |
| `atc skills [<name>]` | what is installed; a name prints one raw, byte for byte, to fork or pipe | either |
| `atc config` | settings, on fufu's typed-registry model | you |
| `atc version` | which tower this is: the release, the commit it was built from, and — read from the update lane's cache, without touching the network — whether it is still the current one. `--json` reports the three as fields | either |
| `atc update` | move this binary to the latest release: verified download, atomic swap; a passive lane checks ~daily and auto-installs, or prints a one-line notice | you |
| `atc doctor` | the seam, the log, and the registries: fufu's version first, then every event the fold could not place, then the installed procedures and skills and the update lane's cache; observes and complains, never enforces, and exits 1 on findings | you |
| `atc serve` | run the standing process: a server the browser board and its API mount into, in the foreground until Ctrl-C; `--host`, then `ATC_HOST`, then `tower.serveHost`, then 127.0.0.1, and `--port`, then `ATC_PORT`, then `tower.servePort`, then 7420. The default is the loopback; a wider bind works and says once that the board has no authentication in front of it. Mounted: the board itself — the web app embedded in the binary at build time, every path outside `/api` answering a build file or the app shell, the client router taking it from there; the read API — GET /api/board, /api/brief/<flight>, /api/procedures, /api/views — each a fresh fold answering the same envelope the verb emits under `--json`, `{"atc": 1, "cmd": "<verb>", …}`, and `/api/board?<query>` taking the query string the views store, answering the groups plus the hidden and filtered counts, and refusing one it cannot parse as a 400; the verb API — POST /api/file, /api/status, /api/assign, /api/hold, /api/answer, /api/done, /api/cancel, /api/comment, /api/decompose, /api/edit, /api/link, /api/unlink, /api/views/save, /api/views/edit, /api/views/delete — each taking the verb's arguments as a JSON body, appending to the log, and answering the verb's own envelope; and the change feed — GET /api/feed, one SSE stream per query, taking the same query on its URL and pushing each subscriber's own fold whenever the repository moves, whoever moved it, every frame being `/api/board?<query>`'s body minus the newline: the server stamps motion when its watcher sees the log refs or `ff watch --all` sees the repository, each subscriber folds against its own query, and a POST publishes nothing directly, so every writer's board arrives the same way | you |
| `atc briefing` | one line for a session-start hook — the flights In Progress under your callsign, pointing at `atc brief`, else the Ready count, pointing at `atc` when there is something to pull; the hook runs it under a short box and drops anything longer than one line | hook |

Every one of them is a read plus a local write — `serve` excepted, which is a process rather than an answer, and still decides nothing. Nothing in the column on the right is a dispatch target.

Two verbs the sketch named are not in the table because they are not built. Promotion — minting the upstream ticket for a local step, linking it, keeping the local history — is the publish boundary described under *Upstream is a foreign writer*. Adapter passthrough — `tower-<adapter>` on PATH, reached through tower — is the seam an adapter would arrive by. Both wait on an adapter existing, and neither is spelled as a command line until it parses.

## Intake

**Every signal comes through one front door.** A GitHub review request, a Linear assignment, and a hallway conversation are one event with different provenance, and `atc file` is the same intake path an adapter takes. If the human-originated signal is second class, a large fraction of most people's week is invisible and the board lies about the day.

Intake is a read, not a subscription — upstream is pulled lazily at invocation, as everything else here is. So it does not matter where work was born: file the ticket by hand in Linear, and the next call picks it up with no webhook and nothing running in between.

A flight that arrives without a procedure lands where `tower.defaultFileStatus` says: Ready unless the setting is changed, because most filings are work already decided on, and a second move typed to say so is a step for nothing. `--status` overrides the setting for one filing, and a procedure flight that declares its own status keeps it. What happens next is deterministic or it is yours — there is no third tier. Match rules live in procedure definitions (below): each rule matches facts a signal carries at intake — an adapter's provenance (`source = "github"`, `event = "review_requested"`), an upstream field, a label given at filing, the status the filing lands with — flag or setting, so a `backlog` rule covers what is parked either way — and a match applies the procedure. The match runs once, at file time, on the filing machine against its own procedures: the filing is minted exactly as naming the procedure would mint it, first match wins, and the routing event stores which rule fired, so every stamp stays explained and overridable — *filed under review because rule github-reviews matched event review_requested* — and a silent stamp is a black box you stop trusting on the second bad call. Because the match happens where the filing does, a pushed log never gets a teammate's flight restamped under rules only your machine holds. A filing no rule covers lands as a bare flight where the setting says, and a rule's own flights land where a named filing's would: the rule decides the shape, and the setting decides the clearance. Backlog stays the deliberate bucket for what a person has not decided about, and in solo mode — where flights are filed by you and your agents directly, with their fields already on them — it is simply empty.

Routing is stored, never recomputed; principle 11 governs it exactly as it governs any judgment. Editing your rules never restamps a flight already filed, for the same reason editing a procedure never disturbs a flight in the air; a flight that should have a shape gets one by being filed under the procedure by name, or decomposed under it.

**A flight's subject resolves late.** File a review against a bare branch with no PR, or a ticket that exists nowhere — tower holds a local subject and stays silent about fields it cannot see. When the PR opens or the ticket is minted, the adapter links it and upstream truth flows into the fields upstream owns. Which forces one piece of exactness: a signal arriving for a subject you already filed merges into that flight as a `foreign` event rather than filing a second one. This is identity equality on a resolved reference, cheap and exact, and deliberately not semantic deduplication.

Three things tower should not build: **estimates** (measurable for started work, fiction for unstarted — report what is known and invent nothing), **learned ranking** (no data on day one and not enough for a long time; weights live in config and are tuned by hand), and **automatic deduplication** (semantic, rarely urgent, expensive when wrong). And one rule that keeps the board believable wherever an agent's judgment does enter — a hand routing out of Backlog, a sorting note, a verdict:

> **An agent's judgment is stored as intent, never recomputed as state.**

A model call at render time makes the board flicker: same data, different call, different answer. Judgments are frozen into the log, attributed to the agent that made them, overridable, and never re-run behind your back, so the board stays a pure function of the log.

## Procedures

Work does not arrive in one shape. A ticket assigned to you, a review requested of you, a thing your manager asked for in a meeting — each has a different decomposition, a different split between what a machine can carry and what only you can, and a different meaning of done. A **procedure** is a named recipe for one shape of work: the instruction set for how a flight proceeds. The word is the metaphor's: a published procedure is a standard sequence for a recurring situation, and the tower clears you for one by name. A plan and a permission, never a hand on the yoke.

A procedure is a graph of flights, saved. Its definition lists the flights it stamps out — each with the same fields any flight carries, pre-filled — the edges between them, and the match rules that apply it to arriving signals:

```toml
name = "review"

[[match]]                     # adapter-keyed, so inert until an adapter can fire it
name   = "github-reviews"
source = "github"
event  = "review_requested"

[[flight]]
id       = "pass"
assignee = "agent"
skill    = "review"

[[flight]]
id       = "smoke"
assignee = "me"

[[flight]]
id    = "verdict"
assignee = "me"
after = ["pass", "smoke"]
```

Filing under it mints the parent plus every flight in the graph, in one atomic append — the whole family exists from the first second, and "where we are" is only ever which of those flights are closed. Order is a DAG through `after` — the same edges `atc link` writes — so concurrency is the absence of a declaration rather than a keyword: `pass` and `smoke` fly together because neither names the other. Every flight in the graph is born cleared, and the edges make Waiting: a flight with an `after` waits on it, and the parent waits on them all, at every fold rather than at the mint. A single-flight procedure collapses onto the flight itself — filing under it mints one flight carrying those fields, never a parent and a lone child, because `atc file "fix the typo"` must not cost two flights to say one thing.

**A procedure is not required.** A bare flight defaults to the minimal shape — assigned to me, no skill, done when I say so — and every verb works on it: file it, work it, finish it, and no procedure is ever involved. The procedure is for work worth decomposing, and the default assignee being me is what keeps agents out of shapeless work: the queue draws only from the agent lane, so nothing unshaped is ever handed out.

**Procedures are personal, and tower ships none.** No built-in procedures, no built-in workflow skills: the binary is pure engine — flights, statuses, edges, queues, the board — and every opinion about how work should flow lives in files their owner authored. Definitions layer in two, keyed by the name inside the file, the more specific replacing the less wholesale: **user**, `$XDG_CONFIG_HOME/tower/procedures/*.toml` — `~/.config/tower/procedures` when that variable is unset — which roams with your config; and **repository**, `<main worktree>/.tower/procedures/*.toml`, which is the team's. The documentation carries worked examples — a ticket shape, a review shape — that a person copies in and forks, and a builder UI can assemble them eventually; either way the file is the owner's, visibly. The main-worktree anchor is so every worktree sees one set: a path resolved against the invoking worktree would hand each checkout its own procedure set. A missing directory is an empty layer; a file that does not parse is a refusal naming the path, because a definition you cannot see is worse than one that refuses.

The repository layer is in the tree, and that is not the working-tree trap from *Storage and sync*: what must never live there is mutable board state, and a procedure definition is config that changes monthly. It also has to be in the tree to be the team's at all — a definition on an orphan ref is a definition nobody clones.

**The definition is read once, at file time, and its fields are copied onto the minted flights.** Readiness and order stay the engine's as ever. Editing a procedure therefore never disturbs a flight already in the air — a board that re-read config at render time would flicker for exactly the reason principle 11 forbids re-running judgment, and forking a procedure mid-week has to be safe or nobody will.

**Procedures declare structure; skills hold judgment.** A procedure is data — a name, match rules, flights, edges — and it cannot express control flow. No conditions, no loops. Everything conditional lives in the skill an agent-assigned flight points at, in markdown, which is where this document already puts judgment. The moment a procedure needs an `if`, it is a skill. That rule is the only thing between this feature and Jira's workflow editor, which is where configurable trackers go to die: the config language grows into a bad programming language.

**A procedure should end with you.** Principle 3 at the flight level: the boundary where the team sees the work is always a human gesture, so the last flight in a shape of work is normally assigned to me. The loader warns — by name and by flight — when a definition's terminal flights are all agent-assigned, and it warns rather than refuses because the file is personal and the boundary that actually holds is `never auto-outward`: whatever an agent finishes, nothing leaves the machine without a person's verb.

**`done` is a closed enum** on a flight: `asserted` (its owner says so), `committed`, `promoted`, `landed`. Four values cannot grow into an expression language, which is the whole point. *Done when CI is green and two people approved* is a me-assigned flight you assert, and what convinces you belongs to the skill. A flight that does not say is `asserted`, and `asserted` is the only one anything reads today — the other three parse, validate, and store against the verbs that will be able to see them. The enum is closed in the loader and open in the log: a flight's fields copied into a `filed` event carry `done` as a free string, because a newer tower's completion word must not take an older tower's whole board down rather than one flight. The refusal belongs where a person is editing a file.

## Skills

A skill is the agent's flight manual: instructions a harness executes, never a process tower spawns. That is not a contradiction of principle 2 — tower ships the seam, the harness runs the judgment, and uninstalling the harness leaves tower working. tower never grows a process supervisor.

It is also the right home for judgment. tower reports facts and what is Ready; a skill decides what to do when a flight holds, when a review comment needs a person, when to stop, when to ask in conversation instead of holding. Policy in markdown the user can fork beats policy compiled into Rust.

tower ships one skill and one mechanism, and no workflow. The skill is the manual, `tower` — the model, the envelope, the exit codes, the landmines, and where every verb's own `--help` is the last word. The mechanism is the `skill` field on a flight, the shelf that field names into — user, then repository, the same name replacing wholesale, like procedures — and `atc skills <name>`, which prints one raw, byte for byte. Nothing on the shelf is the binary's, and the manual never appears there. The documentation's examples cover the recurring three: **plan** (decompose a goal into a tree of flights — solo mode's entry point), **work** (claim, fly, hold or finish, repeat — the loop that pairs with `next`), and **review** (first-pass someone else's branch: commit the mechanical fixes, write the pass as a comment, hold the judgment for a person's verdict). They are one workflow among many, and the shape of the loop is the owner's to fork.

The bridge to an agent is the pick, not a redirect. Each agent-assigned flight names the skill it is flown with, `next` hands out that name on the picked row, and the agent prints it with `atc skills <name>` and follows it for that flight. The user never typed the name; the flight carried it, which is the seam that keeps structure in data and judgment in prose.

Loop control: 0 is work and 1 stops the loop. `outcome` on the envelope — `work`, `drained`, `elsewhere` — is what the loop reports, and the CLI's exit is that field's rendering. No timeout, no sentinel.

Fan-out is `atc next agent -n 3` handing out three flights. One tree per flight is the harness's to arrange; tower says nothing about where the work happens, and the pick is the same claim whether one worker takes it or three.

An example skill stops short of the push boundary — committed on a branch, PR unopened — because principle 3 is easy to state and easy for an unattended loop to violate fourteen times before anyone looks. Where a person's fork draws that line is the person's call, and visibly theirs.

## The three modes

**Solo** — no adapters. Planning with an agent produces a tree of flights — the agent files each step and links the order, and tower stores a DAG it did not author. Then context can be wiped safely, because tower is the durable half: the plan, each brief, and every question and answer live outside the agent. The agent is disposable; the flight is not. A tree of flights is what other trackers would call a project, and it needs no second entity to be one.

**Team** — adapters installed. Upstream owns its fields, tower owns the local layer, and the local layer is where the actual day happens.

**In between** — one upstream ticket, many local sub-flights, one promotion when a step outgrows the local board.

Three layers of memory stay apart: a **skill** knows how to drive tower, the **agent's own memory** knows house style and conventions, and a **brief** knows this flight — the record, the family, the facts. tower owns only the third. A skill that starts accumulating project conventions has taken the agent's job, and tower trying to own house style would do it badly when the agent already has a system for it.

## The later layer

The sketch designed tower around what fufu could tell it: which branch a flight stood on, whether that branch had moved, whether two flights would land on each other, and a pool of warm trees to hand out with the pick. #123 untangled all of it, because every one of those was a fufu read at render time, and the premise is that render reads the log alone. What came out is not dead. It is the layer above the record, and each piece is its own flight when the time comes, in roughly this order:

- **Events stamp the branch and the head commit.** The store's own HEAD read at append time, a fact the writer had in hand, stored on the event and never read back from a tree at render, beside the four stamps every event already carries — writer, author, session, callsign. The pick, the hold, and the finish each say where they happened.
- **Audits as tip comparisons over refs.** With a branch and a tip on the event, "no changes on the branch for 2d" under In Progress and "changes on the branch since it was set ready" under Ready are one ref read each, no fufu spawn. Flagged, never corrected; the threshold was a setting, `2d` by default, and the second line had none. Two unrelated checks, one staleness and one its opposite, so neither hides under a shared word.
- **tower's own undo and redo**, by appending the inverse event. The manifest says `undoable: false` because `ff undo` cannot reach an orphan ref; tower's own gesture can, and the log keeps both the event and its inverse.
- **fufu-only facts, last.** Snapshot ids on an event; held and resolving, read from fufu's state of a branch; collide verdicts between in-flight branches, and the land order folded over them; and bays, a pool of warm worktrees handed out with the pick. Each of these is a read tower would ask fufu for, which is exactly what the premise keeps off the render path today — they come back behind a subscription that keeps render instant, and not before.

## Principles

1. **Intent is stored; the board is derived.** Fields are set the way every tracker sets them; the board is a fold over the record, and nothing on it was guessed.
2. **tower is called; it never calls.** No dispatch, no agent loop. A standing process may refold and subscribe; it decides nothing. The harness schedules; tower queues.
3. **Never auto-outward.** Local state moves freely; anything the team sees is a deliberate gesture.
4. **Upstream owns its fields.** tower is never authoritative over someone else's tracker, and never merges into their model. tower's status, assignee, and priority are the local layer, never synced with upstream's.
5. **Store the word, show the word, enforce nothing.** A status is what someone said, attributed; tower does not hook, veto, or check it against a tree.
6. **Conflict-free by construction.** Union-merged event logs, not a synced database.
7. **Local work stays local until promoted.** Sub-flights are anonymous branches; promotion is the publish boundary.
8. **Deferred requires loud.** Inherited whole from fufu: a held flight is announced, pinned, and blocks its exits.
9. **One model, every surface.** CLI, the web, and anything later consume one contract.
10. **Facts, not consensus.** tower is authoritative over what you alone authored, on your own writer's chain. It holds no negotiated state, because it has no way to negotiate.
11. **Judgment is stored, never recomputed.** A model's verdict is written to the log as authored intent, attributed and overridable. The board is a pure function of the log, or it flickers and is not believed.
12. **The engine ships empty.** No built-in procedures, no built-in workflow, no default opinions about how work flows. Structure and judgment are the owner's files, and the documentation teaches by example.
13. **Procedures declare structure; skills hold judgment.** Procedures are data and carry no control flow. Every conditional lives in markdown a person can fork.
14. **A sub-flight is a flight.** It appears where every flight appears, counted and sorted with the rest; the family is a view over the same rows, never a filter on the list.

## What it stands on

The seam, and every piece of it exists.

- **The briefing.** A session-start hook runs `atc briefing` when a session starts: one line, a short box, dropped whole past either.
- **`ff watch --all`** for serve's feed: one stream over every chain in the repository, so the server stamps motion when the repository moves, whoever moved it, and refolds each subscriber's query. It is the one fufu read a standing tower makes, and the render never waits on it.

## What it waits on

Load-bearing and absent:

- **Forge reads.** A review shape stands almost entirely on state the repository cannot see, so the adapter that supplies it is a dependency of the documented examples rather than a nicety. This one is tower's own to build, and promotion and passthrough wait on it with the review shape.

Everything the record needs exists today: the store, the fold, the seam, the pick, the brief, the hold. What is missing is the layer above the record, and *The later layer* is its order.

## Open questions

- **~~The closed window.~~** *Answered.* The three newest, newest first, compiled in. A count and not a span: three rows hold their size whatever the week did, where a span shows nothing on a quiet Monday and a wall of rows after a Friday sweep. Still not a config key — the window is a render's memory of the week, and the log was always the full record regardless. `--closed` overrides it for one render, taking a count, a span like `7d`, `all`, or `none`; it is the CLI's alone, and `serve` and the web app keep the default rather than wait on the filter system below.
- **~~Filters and saved views.~~** *Answered.* Parsed once in core and shared. One `Query` — the filters, the grouping, the ordering, the closed window, and the display properties a fold ignores — folds server-side into groups and two disjoint counts, and every surface reads it rather than reinventing it. A query is a string on every wire and a struct only in memory: `parse` and `render` are its whole contract, so one text is what a route takes, what a browser URL holds, and what a saved view stores, and there is exactly one place a query can be spelled wrong. The codec is hand-rolled with its own percent escape, like every other grammar in the engine — the workspace carries no URL crate and `deny.toml` gates additions. Field names, operators and axes are closed and refuse; values are never checked against a vocabulary, because a view saved by a newer tower must not become unparseable by an older one. `for` is the one derived predicate, over two facts — an open question in any lane, or the `me` lane — and it is a filter alone: no column, no grouping, no ordering. `--closed` stays what it was, and the CLI grows no filter flags: principle 9 is honored by distilling them from the web later rather than growing a parallel set now.
- **~~Saved views.~~** *Answered.* A view is a `view_saved` event on the log holding a name, the rendered query, and whether it is personal or shared, so it is persisted, carried by the repository, and readable without tower. A save naming an earlier view replaces its three fields wholesale, last-wins in log order — a view is three fields, and a per-field overlay would buy nothing — and `view_deleted` is final: a later save naming a deleted view folds as nothing. Personal is a rendering rule and not a privacy one. The log is shared by construction, so the event is readable by anyone with the repository; it renders for its author and not for others, and that is the whole of what the word promises. The viewer is the process's git identity, the same email the store stamps as every event's author, and a reference to a view the viewer cannot see is `view/not-found` rather than a permission refusal.
- **Does the flight own the branch, or the branch own the flight?** If `ff branch <name>` claims a placeholder, does claiming mint a flight? The everything-is-a-flight version is seductive and probably wrong, and it is the first question the later layer's branch stamp reopens.
- **How much forge state to absorb.** Not whether — the review shape settles that — but where it stops. Every field pulled in punctures the ownership table a little further, and that table is the only thing keeping this from becoming a second tracker.
- **Whether the `done` enum stays at four.** It is closed on purpose, and the first genuinely missing value is the moment to check whether the answer is a fifth constant or a flight nobody wanted to own.
- **What a flight means after a rewrite** folds its snapshots into a commit — fufu's open session-boundary question, made urgent rather than theoretical the day an event carries a snapshot id.
- **How much orchestration belongs in a documented example skill** before it is a scheduler with extra steps and principle 2 has been defeated by paperwork.
- **Naming.** `atc` against crates.io, npm, and Homebrew. Almost certainly taken; the metaphor is what matters, not the word.

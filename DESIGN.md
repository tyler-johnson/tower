# tower — design sketch

*Founding sketch, August 2026. Speculative: none of tower is built, though nearly everything it stands on is.*

**tower** is project management for people and agents, built on fufu. It lives in its own repository and installs one binary, `ff-tower`, which is what fufu's `ff-<name>` dispatch finds for `ff tower`. There is no bare `tower` command: the dependency on fufu is real rather than decorative, so the verb is reached through fufu or not at all.

The name comes from fufu's own metaphor. fufu is the pilot — it flies the repository. tower is the tower: it doesn't fly anything, it assigns work, sequences landings, and keeps traffic from colliding. Deconfliction is literally the job, and it is also the one thing a version-control-native tracker can do that no other tracker can.

## Thesis

Every tracker uses the same model, because the model works: a ticket with a status, an assignee, a priority, labels, and links. tower keeps that model on purpose. A flight is instantly recognizable to anyone who has used Linear, Jira, or GitHub Projects, because nothing about agents changes what work *is* — what agents change is the granularity of it and what a tracker can know about it.

What a tracker can know is where fufu comes in. Capture runs before every action, futures are computed for free, branches and sessions are observable — so tower is the first tracker that can check the claims its own board makes:

> **Intent is stored; the repository audits it.**

Status is a field someone set, the way it is everywhere else. The difference is that tower never stops watching. A flight marked In Progress with no motion for two days says so on its row; a branch moving under a flight still marked Ready says so too. Drift is flagged, never corrected — a tracker that silently rewrites your fields is guessing, and a tracker that stays silent is lying, so tower does the third thing and complains.

The consequence worth the whole design: tower sees work start before the first commit exists, because the capture floor does. An agent that edits for twenty minutes and commits nothing is visible. No other tracker can audit that, because no other tracker has a record of it.

So the shape of the product is a boring model with an interesting engine. The model — flights, statuses, assignees, priorities, labels, sub-flights — is the one everyone already knows, and you could describe it to a Linear user in one breath. The engine underneath — the capture floor, discovered conflicts, the land order, conflict-free assignment — is available to no one else, and it never leaks upward into the model where users live. Delete the engine and a small ordinary tracker remains; that property is deliberate and load-bearing.

## The seam

tower is a separate program in a separate repository. Issue tracking is not a version control operation, and fufu's principle 10 — verbs must earn their existence — kills it as a native verb on its own merits. It is discovered as `ff tower` through fufu's extension dispatch, and has its own release, its own authority, its own store, and its own cadence. It links no fufu crate: the contract below is the entire surface between the two, and it is the same surface every other extension gets.

The contract:

```
reads     ff status --json · ff log --json · ff collide --json · ff worktree list --json · ff watch
calls     ff start · ff switch · ff worktree add|remove, every call tagged --session <flight>
stores    refs/tower/log/<author>/<writer>
derives   motion · conflicts · drift · land order
writes    nothing under refs/fufu/*, ever
```

That last line is fufu's extension rule, unmodified: extensions read fufu state and call fufu verbs; only fufu writes fufu state.

## Passive by construction

**tower is a thing agents call. It never calls agents.**

Passive is a statement about initiative, not about process count. tower starts no agent work and owns no agent lifecycle; it does not claim that no tower process runs. Every verb is a read plus a local write, and there is no dispatch and no iteration verb. If work should loop, the agent harness loops and calls `ff tower next` again — the harness is the scheduler, tower is only the queue.

`ff tower serve` is a standing process, a daemon by any honest reading, and it stays inside the line because it is a clock and a subscriber rather than an actor. It refolds when the repository moves, pulls upstream on a cadence, and everything it learns lands in the log as the same events a lazy pull would have written. It holds no state the log does not, decides nothing, and dispatches nothing. Every interface works without it, just staler — an accelerant, never a dependency. A person starts it; tower never starts it for them.

The reasons to stay passive compound. Initiating means owning agent lifecycle: keys, model selection, retries, context limits, per-vendor quirks — a second product, and a moving one. Staying passive makes tower vendor-neutral by construction, because it never learns who is calling beyond the byline on an event. And a queue that dispatches on its own is a background process making outward-facing decisions nobody asked for that minute, which fufu's principle 9 already forbids in its own domain.

Assignment is routing, not dispatch. Assigning a flight to the agent lane puts it in a queue an agent has to come ask about; it calls no one, and tower still never learns who will answer.

Sync follows the same discipline. Upstream is pulled lazily at invocation, gated by a cadence stamp, the way fufu's auto-trim and update check already work. With the standing process up, that same pull runs on its cadence and appends the same events — the cadence is config it reads, never authority it holds. The board is fresh because you just asked for it, or because a subscriber refolded it, and the fold is identical either way. Anything that needs to reach you unasked belongs in fufu's ambient shell channel — a heartbeat the user started — or in a process the user started, never in one tower spawned behind them.

The passive update lane remains the one process tower starts for itself, fufu's carve-out carried over verbatim: official binaries (never dev, dogfood, or test builds; never under CI) spawn a detached `ff tower update --check` at most once per `tower.updateCheck` (default daily) — the one sanctioned self-spawn. It refreshes a small cache file under the user cache dir and exits; foreground commands read the cache, and with `tower.autoUpdate` on (the default) a newer release installs itself silently in the background, or with it off a one-line notice lands on stderr instead. Three throttles keep it polite: the cadence gates the checks, auto-install probes retry at most daily, and a release is announced at most once, ever. `tower.updateCheck false` kills the whole lane. The trust root is deliberately plain — HTTPS to GitHub plus the release's sha256, the same root the install scripts rely on.

Tower cannot enforce, only observe and complain. `ff tower next` prints the bay path; it cannot relocate a running agent and does not try. Work landing on the wrong branch is reported loudly at the next render rather than prevented by a hook. That is fufu's regime boundary, inherited.

## Deconfliction — the earned existence

Merge simulation is free and side-effect-less — the whole replay runs inside one object-memory clone and writes nothing — so tower can ask "would these two land on each other?" continuously, about work that has not been committed yet. `ff collide` is that question already spelled: two branches in, one verdict out, with the paths where they touch the same thing. It reads each side's tree from the operation log rather than from a worktree, so a branch an agent is editing right now in another bay, with nothing committed, still answers. Tower does not need a probe of its own; it needs the verb's JSON and a reason to ask.

Two kinds of blocking, and they differ in kind:

- **declared** — a human said this depends on that. Stored intent. Every tracker has it.
- **discovered** — a merge probe found two branches inside the same hunk. Nobody typed it, it appeared the moment the second edit happened, and it disappears on its own when one lands.

From discovered conflicts comes a **land order**: topologically sort in-flight work by pairwise conflict, and say which sequence costs nothing. The verdicts are fufu's and the fold over them is tower's, which is the right seam — a verdict is a fact about two trees, a set is a fact about a queue. Tower caches what it has asked, invalidates on the branch motion `ff watch --all` reports, and admits a candidate when it is clear against every flight already in: greedy rather than maximum, since the maximum conflict-free set is NP-hard and a scheduler that stalls on one has stopped scheduling. That fold pointed at assignment rather than at landing is what `ff tower next -n <k>` hands out. And because the board knows what is in flight, the check runs at pull time — tower holds back a flight that would collide with one already flying instead of filing an incident after the fact. Sequencing on approach, not collision reporting.

This is fufu's principle 7 raised one level: if an outcome can be known in memory for free, the board should already know it.

## Upstream is a foreign writer

At work the team already has a tracker. tower does not replace it and is never authoritative over it. This is fufu's principle 2 one layer up: Linear and GitHub are first-class foreign writers, observed and absorbed, never owned.

Field ownership is enforced hard, or sync becomes a merge problem it does not need to be:

| owner | fields | status |
|---|---|---|
| upstream tracker | exists, title, body, its assignee, its priority, its status, cycle | upstream truth |
| forge | PR, review state, CI, merge | upstream truth |
| the repository | branch, snapshots, session, motion, conflicts | derived by fufu |
| tower | status, assignee, priority, labels, skill, edges, queue, bays, briefs, notes | local truth |

The rows do not compete, because tower's fields are the local layer and upstream's are upstream's. The same issue can be In Progress in Linear and Waiting here, and both boards are telling the truth about their own scope — Linear says where the team thinks it is, tower says where this machine's work on it actually is. tower never writes status upstream, never derives its status from upstream's, and shows upstream's fields — when an adapter supplies them — as labeled foreign facts on the brief, nothing more.

Upstream changes arrive as `foreign` events in the local log — labeled, undoable, loud — and upstream wins every field it owns. tower holds a pointer and a local layer beside it; it never merges into someone else's model.

**Never auto-outward.** Automation moves local state freely: assign, decompose, route, advance. Anything the team sees — opening a PR, posting a comment, moving an upstream status — is a deliberate gesture. An agent commenting at machine rate is a social failure with no technical apology.

Adapters are the same fractal: `ff tower linear` runs `tower-linear` from PATH. Solo mode is the case where none are installed, and nothing else changes.

## Local steps are anonymous branches

A team ticket decomposes into steps that are real, tracked, briefed, and assignable — and invisible upstream. They are fufu's anonymous branches: genuine from birth, merely not yet named to anyone outside.

Promotion is the same gesture at the same boundary. A step that turns out to need a teammate or a PR of its own gets `ff tower promote`, which mints a real upstream ticket, links it, and keeps the local history — exactly `ff branch <name>` claiming a placeholder at the publish boundary.

The team's board stays as coarse as the team wants. The local board is as fine as the work actually is. Neither has to negotiate with the other.

## The model

A flight is an issue, and it carries what every issue carries:

| field | values |
|---|---|
| subject, body | authored text |
| status | Triage · Waiting · Ready · In Progress · Held · Done · Canceled |
| assignee | me · agent |
| priority | urgent · high · medium · low · none |
| labels | freeform strings, rendered as chips, filterable everywhere |
| skill | what an agent reads to fly it — the one field other trackers do not have |
| edges | depends-on and blocks, declared with `link`; a parent depends on its sub-flights |
| comments, history | the record, append-only |

Every field is settable directly at filing — `ff tower file "subject" -p high --label chore --skill review --assignee agent` — and editable after. A procedure (below) is nothing more than those same fields saved across a graph of flights.

**Assignee is deliberately coarse.** Two values, me or agent, because that is the routing decision — whose queue is this in — and the routing decision is all the field has to carry. The finer fact, *which* agent is actually flying it, needs no field at all: every event in the log carries the byline of the identity that wrote it, so the board shows the chip and the history shows the pilot — "In Progress — claude, 4m ago." Addressing work to a named agent is the wrong primitive anyway: describe the work with `skill` and labels, and let workers pull what they know how to fly. tower stays vendor-neutral because it never stores who will answer, only who did.

**Sub-flights are just flights.** Decomposing a flight — by hand or under a procedure — mints real flights under a parent, joined by the same edges `link` writes. A sub-flight has every field a flight has, appears in queues on its own merits, and is distinguishable from hand-filed work by nothing except its parent edge. One level of flights and edges is the whole model; a tree of flights is what other trackers call a project, and it needs no second entity.

## Status

Status is a stored field with seven values, moved directly — a verb, a drag on the board, an agent's gesture mid-loop — and every change is attributed in the history. The names are chosen so a Linear user reads the board cold, and the two nonstandard ones earn their sentence:

- **Triage** — not yet cleared for work, deliberately. The default for anything that arrives unmatched, and the parking place for work nobody has decided about. Nothing leaves Triage except by a person's gesture or a person's match rules; that deliberateness is the definition.
- **Waiting** — cleared, but gated by the graph: something this flight depends on is still live. Flights are not put here so much as born here by their edges, and the engine's one automation moves a flight Waiting → Ready the moment its last live dependency closes. The row says what it waits on.
- **Ready** — cleared and unblocked. The agent queue draws from Ready flights assigned to the agent lane; your own Ready flights are the list you pick from.
- **In Progress** — someone is flying it. The pull sets it for agents; you set it, or just start and let the drift flag remind you.
- **Held** — stopped on a blocking question. The question piece is its own section below.
- **Done / Canceled** — closed, finished or abandoned, with the reason on the record. Closed flights stay visible: the board carries a closed group — newest first, windowed by config so the fold stays bounded, collapsed in the web UI and behind a flag in the CLI — because a board that forgets the week is amnesiac, and the log was always the full record regardless. A canceled dependency does not satisfy the flights that waited on it; it surfaces on them as a fact needing a look, because an abandoned part is a reason to reconsider the parent, not a green light.

The repository audits all of it. Motion — session-tagged captures on a flight's branch — is the fact tower reads from the floor, and the board annotates disagreement between motion and status without ever resolving it: "In Progress, no motion for 2d." "Motion on the branch, but status is Ready." Drift lines are the thesis made visible, and they are the entire enforcement mechanism — observe and complain, never correct, fufu's regime boundary again.

Only two transitions happen without a hand on them, and both are deterministic, attributed, and explained in the history: a match rule routing an arrival out of Triage, and the Waiting → Ready advance when dependencies close. Both run on the lazy pass — any invocation, and each of serve's refolds, examines what the rules cover and appends what they conclude — so the board catches up whenever anyone asks for anything, and no standing process is required for the queue to work. Everything else conditional is judgment, and judgment lives in a skill.

## Held — the question piece

An agent mid-flight that hits something it genuinely cannot decide — an ambiguous requirement, a design fork, anything where guessing is worse than stopping — holds the flight: `ff tower hold <flight> -m "<the question>"`, exit code 3. The flight's status becomes Held with the question attached, and nothing is torn down: the bay stays warm, the branch and its session-tagged captures sit exactly where they were, and nothing was guessed. Holding is stopping, not abandoning.

A question is a blocking comment — that is the whole object. It lands in the comment stream flagged as holding the flight, the answer is the reply that releases it, and both survive on the record permanently, which is what makes the resume work: `ff tower answer <flight> -m "<the answer>"` clears the question and sets the flight Ready, and whichever agent pulls it next reads the brief — which now carries the question and the answer — and continues in the warm bay. The original asker may be long gone, context wiped, session over. That is fine and expected: tower is the durable half, the agent is disposable, the flight is not.

One open question per flight. A hold means "I cannot proceed," and an agent with four blocking questions on one flight has a decomposition problem, not a Q&A problem. Questions that do not block are comments.

Exit 3 is an outcome, not an error — fufu's precedent. The envelope is a full success envelope with the held event in `data`; only the exit code says the flight stopped with a question. A machine caller branches on the code, a human reads the echo, and neither has to parse an error to learn that holding is what happened. In a loop, 3 is the signal that work exists but needs you — the harness stops cleanly or moves to other flights instead of spinning.

Held inherits fufu's principle 8 whole: announced at creation, pinned in **waiting on you** until answered, loud the entire time. An agent question that goes quiet is how the whole system rots. And the state has a property no ordinary tracker can offer: it is a question with a warm machine attached — the stopped work is physically parked, resumable mid-keystroke, and the board cannot stop showing it until someone answers.

Hold is the durable fallback, not the preferred channel. An agent in a live session with a person asks in the conversation — better latency, better bandwidth, no ceremony — and holds only when nobody is on the other end: unattended loops, fan-outs, walk-away work. The skills that drive agents say this ordering explicitly, so holds do not get cargo-culted into interactive sessions.

## Bays

Parallel agents need parallel working trees, and git worktrees are the idiom (principle 4). fufu's per-branch state lands on them almost by accident: snapshot chains, branch metadata, and futures caches are keyed by branch under the common dir, and a worktree is one branch, so that half collides over nothing.

**The operation log used to be the half that collided, and fufu has fixed it.** It was one ref for the whole repository — a single chain across every branch, one lock, and `ff undo` a pointer move on that one chain — so three agents in three bays shared one undo pointer and one queue. Sessions softened it and could not solve it: undo steps over a *run* of adjacent captures carrying the same tag, but adjacency is a fact about the log rather than about the bay, and interleaved agents do not produce adjacent runs. The chain is now keyed by worktree at `refs/fufu/wt/<id>/ops`, with its own undo pointer and its own lock, and a bay's ref table holds only the refs that bay owns. A bay's undo walks back its own steps and no one else's.

The cost that bites is bootstrap, not disk — everything gitignored (`target/`, `node_modules`, venvs, `.env`) does not come along, so per-flight creation means a cold build per flight. Hence a **pool of warm bays**, bootstrapped once and recycled, rather than create-and-destroy. A shared `CARGO_TARGET_DIR` is the tempting shortcut and a trap: cargo's file lock serializes the builds you bought concurrency to parallelize. The pool calls `ff worktree add` and `ff worktree remove` rather than shelling out to git: the chain floor then exists before the agent's first command, and a recycled bay's work is captured before the bay is torn down.

The surface is `ff tower bay`: bare or `list` renders the pool, `warm [<path>] [<branch>]` adds a bay through `ff worktree add` — bare `warm` mints the next slot itself — and `release <bay>` removes one through `ff worktree remove` — refused with `bay/occupied` while a live flight's branch is checked out there. There is no bay registry, and the config surface is one key: the pool is `ff worktree list`, occupancy is the board's own flight-to-branch derivation, and a closed flight frees its bay by derivation rather than bookkeeping. The key is `tower.bays`, a pool root — absolute, or relative to the main worktree, `git config tower.bays ../bays` putting the pool beside the repo — under which bare `warm` mints the smallest free `bay-<n>` slot, creating the root when it does not exist yet. Released numbers come back because `ff worktree remove` deletes the directory, a slot whose directory already exists is skipped rather than collided with, and the key says where new bays go and registers nothing. The list is also why the board's reads fan out — a session-tagged capture made inside a bay lands on that bay's chain and nowhere else, so the reads poll each bay's chain or the board is blind to every flight but the invoker's. And bays are what make bare `ff tower done` meaningful: the newest session-tagged operation on the invoking worktree's chain names the current flight, so an agent finishing in its bay types no id.

`bays: 1` must stay a supported configuration. Serialized agents in one tree lose throughput and keep every other feature, including deconfliction, and whether concurrency pays depends entirely on a project's cold-start cost.

Note that bays make fufu's tree memory moot for the agent lane — an agent owning a tree for the life of a flight never parks or switches. That is fine. Humans still switch, and Floor 2 still serves them.

Bays are not agent-only, though, and a review is the case that proves it: its human part is *check out someone else's branch and run it*, which is a bay with a foreign branch in it. Filing a review can warm one, so the tree is already built by the time you sit down. That is the moment the pool pays for itself for a person rather than a fleet.

## Storage and sync

Not files in the working tree. `.tower/flights/*.md` is the obvious move and the trap every git-native tracker falls into: the board becomes branch-dependent, ticket edits pollute code diffs, and closing something on an unmerged branch means the board lies until merge.

**An orphan ref, shaped like fufu's journal.** `refs/tower/log/<author>/<writer>` — a commit chain with its own tree, no relation to code history, never touching the working tree, CAS-appended, reachability as the gc pin. Sync is one explicit refspec. The writer component is the machine's, minted once into local config: one ref per author alone breaks the moment two machines append under one email — both chains diverge and a push is rejected with no merge available, because a commit chain has no union — while a ref per writer makes every push a fast-forward, and the fold unions `refs/tower/log/**` either way.

The conflict problem dissolves because of what is stored. The repository's facts — motion, branches, conflicts — are never stored at all, so they have zero merge surface and self-heal when someone works around tower. Stored intent is an append-only event log partitioned per author, so merging divergent logs is a **union, not a merge** — conflict-free by construction. The board is a fold over the union. The only genuine collision is two people editing one field in the same window; last-writer-wins with a stable tiebreak, and both events survive in the log regardless.

**Sync is three tiers, and only one of them needs anything built.** *Machine-local* — bays, pool state, caches — never syncs and mostly rebuilds. *Mine across machines* — solo flights, notes, decompositions — is single-author and append-only, so backup or roaming is one plain `git push refs/tower/log/<me>/*` with no protocol at all — and no verb: tower builds nothing here. tower is a local tool that interfaces with remote data, and the designed way anything leaves the machine is `ff tower promote`. *Shared with others* is the only hard tier, and tower does not have it: in team mode upstream already holds it, and in solo mode it does not exist.

Multi-writer works anyway — fetch `refs/tower/log/*`, fold the union — and it stays documented and unsupported. Every git-native tracker that tried to be the shared board was technically fine and socially dead: shared work needs a place people look, and a ref in a repository is not one. Making it one means notifications, identity, and permissions, which is a different product wearing this one as a hat. **tower never becomes the shared board; sharing is `ff tower promote`.**

The deeper reason is that tower has no mechanism for agreement. Its facts need no consensus — the branch exists, these hunks collide, the log says who set what — which is why tower can assert them unilaterally and be believed. Upstream state is negotiated: priority, ownership, what ships this cycle. A shared tower board would manufacture consensus data with nothing underneath it, and two people would confidently read different boards.

One honest consequence: this is the first fufu-adjacent state that is not a cache. fufu's principle 3 says state is rebuildable and the repository wins; authored text is derivable from nothing. It holds anyway — the store *is* ordinary git objects in the repository, so the repository still wins literally — but authored flights are losable in a way no fufu state is. That is accepted rather than papered over: the store is ordinary git refs, and whether they leave the machine is git's business, not tower's — tower carries no backup surface at all, no verb, no warning, no doctor row.

## Surfaces

One model, every renderer — fufu's principle 14, so the MCP server is a thin shell over the same contract the CLI renders, never a second implementation.

```
caller          surface        what it does
────────────────────────────────────────────────────────────────
a person        CLI            decide, answer, route, publish
an agent        MCP            pull, read a brief, report, hold
the clock       serve          refold on motion, pull on cadence;
                               no verb a caller did not ask for
nothing         —              one sanctioned self-spawn, the
                               detached update check
```

The board is **waiting on you** pinned above a list grouped by status. Waiting-on-you is the inbox — everything that needs a human right now — and inside it, items partition by what they cost you rather than by priority: an answer is thirty seconds, a review is twenty minutes, a decision is unbounded. Sorting them together means you cannot spend the five minutes you actually have. Cost is read off the shape of the thing, so it needs no judgment and no model call. Below the inbox, the list is the one every tracker renders — status groups, then priority, then age within them — with the closed group collapsed at the bottom. A flight in the inbox still has a status; the inbox is a view of the same rows, and it is the feature the borrowed layout does not come with.

The row is the recognizable anatomy: priority glyph, flight ref, status dot, subject, label chips, assignee, age right-aligned — plus the phrases only this tracker can print, drift and collisions, in the warn tone. Filters compose over status, priority, label, assignee, and procedure, encode into the URL so a filtered board is a shareable link, and run client-side over the board envelope the feed keeps live. The web app adds the views the model earns: a kanban board whose columns are the statuses, where a drag is a verb or it is not offered — to In Progress is pull, to Done is done, to Canceled is cancel, and a drop with no verb behind it does not land; a command palette over verbs, flights, and navigation; single-key movement and verbs on the selected row; and search over subjects and bodies, nothing semantic. The CLI renders the same model with the same vocabulary, filter flags mirroring the filter bar, and the closed group behind a flag.

The whole design is aimed at one reflex: bare `ff tower`, often, because it is the fastest way to learn what to do next. Two things have to hold or the reflex never forms. It has to be honest, which is what the audit is for. And **render must never block on the network** — fold the local log, draw, note the age, refresh on the cadence stamp. A board that is fresh and slow loses to one that is instant and honest about how stale it is.

### Altitude

Agent work is fine-grained — a review is four flights the moment it files — and a board that renders every sub-flight as a peer of its parent drowns in its own decompositions. The fix is one rule applied everywhere: **a sub-flight never competes with its parent for attention.**

People look at parents; actors act on sub-flights. The list shows the parent as the row — subject, progress mark (1/3), a disclosure that expands the family — so filing under a procedure visibly creates one thing, not four. Counts count parents. Filters apply to parents. The kanban shows parent cards. A sub-flight surfaces at top level in exactly one situation: it currently needs someone to act — it is in waiting-on-you, or it is what the agent queue would hand out — and there it carries a breadcrumbed subject ("check the PR › verdict") so it is legible alone. In a queue, fine granularity is the feature; everywhere else, the parent aggregates it.

Selecting a flight shows its tree in both interfaces — the family as an indented, folder-like view, parents up and children down, read straight off the edges. The detail page also renders a parent's sub-flights as a checks list, the way a forge renders CI: `pass ✓ · smoke ✓ · verdict ● yours`. People already read that shape instantly, and it happens to be the truth — a procedure is a pipeline whose stages are flights.

### Flight ids

A flight has two names, and the split is human against wire. The wire name is the id of the `filed` event that minted it: `<writer>.<seq>`, unique across machines because the writer component is. JSON envelopes and `--session` tags carry it raw, always. The event sequence is shared by every kind of event on a writer's chain — comments, assignments, links, status moves all consume one — so wire ids are sparse by construction and count nothing a person cares about.

Humans get a dense number instead. A flight's number is its position among its writer's `filed` events — derived from the fold, never stored, so there is no second counter to mint, CAS, or sync, and the append-only log makes the numbering stable forever: a closed flight keeps its number, and no filing can renumber an earlier one. Human output prints `#3`, and a board folded from a single writer — the normal case, since tower is local-first and log sync is the exception — needs nothing more. When a second writer's flights are on the board, the writer rides along as `pi-8c2e#3`: `#` binds a writer to a flight number the way `.` binds one to an event seq, so the two forms can never be confused.

On input, any verb taking a flight accepts either name. A bare number resolves as a flight number against the board's filed flights — a unique match wins, an ambiguous one refuses and lists the full forms — `writer#n` names another writer's flight exactly, and the dotted form is always the filing event's id, accepted everywhere a number is. A leading `#` on a bare number is accepted and stripped for paste tolerance; the documented spelling is unprefixed, because an unquoted `#` starts a shell comment.

### Verdicts

Conflict verdicts are facts read fresh, never stored: the board probes `ff collide` for every distinct pair of in-flight branches per render, which costs nothing in the solo norm — fewer than two distinct branches means zero probes — and O(pairs) beyond it. No cache until `ff watch` gives tower a subscription path to invalidate one; a cache without an invalidation signal would be the board lying about freshness.

Unknown never rounds down to clear. fufu answering "no base" and fufu refusing to judge one pair — a branch deleted mid-render, say — both land on the row as "no verdict," and a refusal on one pair is one unanswered row, never a dead board. Only the seam breaking wholesale (no `ff`, a contract tower does not read) fails the render.

The JSON carries the verdicts per flight — `collides`, each entry naming the other flight and the paths fufu reported, and `unanswered` for the pairs fufu could not judge — and no board-level pair list: the per-flight entries carry each pair once from each side, which is what a render and a machine caller both iterate anyway. The land order and the conflict-free set are `next`'s fold over these same pairs, deliberately not this surface's.

### Next

`ff tower next` is the agent queue's pull. The pool is every Ready flight assigned to the agent lane; admission is greedy, in filed order — a candidate joins the pick when its branch is clear against every flight already in the air and every candidate already admitted, since a maximum set would cost more than the answer is worth. Unknown excludes: a pairing fufu could not judge is a reason to leave a flight out of a fan-out set, never rounded down to clear.

The pull is one atomic append that sets the picked flights In Progress — the exclusivity is the append itself, one winner per flight, and the byline on the event is the pilot. `--peek` is the same computation with no write, and the envelope says which happened. An empty pick is exit 1 over a full data envelope — fufu's "no," on the hold precedent: an outcome rides the success path and only the code says it, so `while ff tower next` terminates on the code alone. The passed rows are the explained ranking — each flight the walk examined and why it lost (`waiting`, `collides`, `no-verdict`), and nothing past where the walk stopped, so the output stays bounded by the ask rather than the board.

### Brief

`ff tower brief` is the read half of the handoff: `next` hands an agent a flight id and a subject, and the brief is what it reads next — everything the log and the repository know about one flight, in one read over the fold and the reads. No probes: verdicts stay the board's and `next`'s surfaces, and the brief stays instant. A closed flight briefs like any other, because the log keeps the record and reading it is never a lifecycle move.

The brief is the record — subject, body, every field, the comments with any question and answer among them, the family tree, links carrying each linked flight's subject and status — plus the repository's facts: branch, tip, motion, drift, and whether the branch is the reader's own. The skill named on the flight is what the agent flies it with; the brief is what the agent flies it *from*, and it is why the asker of a held question does not need to be its resumer.

### Decompose

`ff tower decompose <flight> [<procedure> | <part>…]` makes a flight a parent: under a procedure, the definition's flights are minted beneath it; by hand, each argument files as one sub-flight. Either way the children are `linked` edges and nothing else — no container kind, no parent type — so a sub-flight is indistinguishable from a hand-declared dependency, and that is the point: Waiting derivations, `depends_on`/`blocks`, and the brief's link sections all work on it unchanged. The filings and their edges land in one append, because two would leave a window where the parent is live, unlinked, and pullable — exactly the state the Waiting gate exists to prevent.

A parent's Done stays asserted. Every sub-flight closing makes the parent Ready, not finished — whether the broad task is over is a judgment, and `ff tower done` is where it gets made.

### The verbs

fufu's rule that every verb must earn its existence carries over, and the one it kills first is `run`. Tower cannot run anything — a verb that implies dispatch would be the first crack in principle 2, and that line is too load-bearing to contradict casually.

| verb | what it does | caller |
|---|---|---|
| `ff tower` (alias `board`) | the board: what needs you, then the list by status | you |
| `ff tower next [-n <k>]` | pull the next Ready flight from the agent lane, or a set of `k` that collide with neither each other nor anything already flying; the pull sets In Progress; `--peek` reads without pulling | an agent |
| `ff tower file [<procedure>] [<subject>]` | put work on the board — bare, or under a procedure; every field a procedure sets is a flag here (`-m`, `-p`, `--label`, `--skill`, `--assignee`, `--bay`) | either |
| `ff tower status <flight> <status>` | move a flight; the lifecycle verbs below are this verb carrying a payload | either |
| `ff tower assign <flight> <me\|agent\|none>` | route the flight's queue | either |
| `ff tower hold <flight> -m <question>` | stop with a blocking question — bay warm, exit 3 | an agent |
| `ff tower answer <flight> -m <answer>` | answer the question and release the flight to Ready | you |
| `ff tower done [<flight>]` | finish it; bare in a bay, the session tag names the flight | either |
| `ff tower cancel <flight> [-m <why>]` | close it unfinished, reason on the record | you |
| `ff tower link <a> <b>` | declare that one flight depends on another — discovered conflicts need no verb | either |
| `ff tower comment <flight> -m <note>` | a note on the record, local; saying it to the team is a separate, deliberate gesture | either |
| `ff tower edit <target> [-s <subject>] [-m <msg>]` | reword a flight's subject/body, or a comment's text by its event id — an overlay event, the log keeps every prior word | either |
| `ff tower decompose <flight> [<procedure> \| <part>…]` | make a flight a parent — a procedure's flights, or parts by hand | either |
| `ff tower promote <flight>` | mint the upstream ticket, link it, keep local history — the publish boundary | you |
| `ff tower bay <list\|warm\|release>` | the pool: what is bootstrapped, what is occupied, what to build ahead of you; bare `warm` mints the next slot under `tower.bays` | either |
| `ff tower explain <error-id>` | look up an error id and see what it means — the prose behind every coded refusal; `--list` is the whole catalog | either |
| `ff tower procedures [<name>]` | what is installed, what each matches, and where it came from | you |
| `ff tower skills [<name>]` | what is installed; a name prints one raw, byte for byte, for the harness redirect | either |
| `ff tower config` | settings, on fufu's typed-registry model | you |
| `ff tower version` | which tower this is: the release, the commit it was built from, and — read from the update lane's cache, without touching the network — whether it is still the current one. `--json` reports the three as fields | either |
| `ff tower update` | move this binary to the latest release: verified download, atomic swap; a passive lane checks ~daily and auto-installs, or prints a one-line notice | you |
| `ff tower doctor` | stale adapters, bays that no longer resolve | you |
| `ff tower serve` | run the standing process: a server the browser board and its API mount into, in the foreground until Ctrl-C; `--host`, then `TOWER_HOST`, then `tower.serveHost`, then 127.0.0.1, and `--port`, then `TOWER_PORT`, then `tower.servePort`, then 7420. The default is the loopback; a wider bind works and says once that the board has no authentication in front of it. Mounted: the board itself — the web app embedded in the binary at build time, every path outside `/api` answering a build file or the app shell, the client router taking it from there; the read API — GET /api/board, /api/brief/<flight>, /api/bays, /api/procedures — each a fresh fold answering the same envelope the verb emits under `--json`; the verb API — POST /api/file, /api/status, /api/assign, /api/hold, /api/answer, /api/done, /api/cancel, /api/comment, /api/decompose — each taking the verb's arguments as a JSON body, appending to the log, and answering the verb's own envelope; and the change feed — GET /api/feed, one SSE stream pushing the full board envelope whenever the repository moves, whoever moved it: the server refolds when its watcher sees the log refs or `ff watch --all` sees the repository, and a POST publishes nothing directly, so every writer's board arrives the same way | you |
| `ff tower <adapter> <args>` | passthrough to `tower-<adapter>` on PATH: `ff tower linear`, `ff tower github` | either |

Every one of them is a read plus a local write — `serve` excepted, which is a process rather than an answer, and still decides nothing. Nothing in the column on the right is a dispatch target.

## Intake and triage

**Every signal comes through one front door.** A GitHub review request, a Linear assignment, and a hallway conversation are one event with different provenance, and `ff tower file` is the same intake path an adapter takes. If the human-originated signal is second class, a large fraction of most people's week is invisible and the board lies about the day.

Intake is a read, not a subscription — upstream is pulled lazily at invocation, as everything else here is. So it does not matter where work was born: file the ticket by hand in Linear, and the next call picks it up with no webhook and nothing running in between.

A flight that arrives without a procedure lands in Triage, and what happens next is deterministic or it is yours — there is no third tier. Match rules live in procedure definitions (below): each rule matches facts a signal carries — an adapter's provenance (`source = "github"`, `event = "review_requested"`), an upstream field, a label given at filing — and a match applies the procedure. Rules run on the lazy pass, first match wins, and the routing event stores which rule fired, so every stamp stays explained and overridable — *filed under review because rule github-reviews matched event review_requested* — and a silent stamp is a black box you stop trusting on the second bad call. What no rule matches sits in Triage until a person clears it, and that is the definition working as intended: Triage is the deliberate bucket, and in solo mode — where flights are filed by you and your agents directly, with their fields already on them — it is simply empty.

Routing is stored, never recomputed; principle 11 governs it exactly as it governs any judgment. Editing your rules never restamps a flight already routed, for the same reason editing a procedure never disturbs a flight in the air — but it does cover what still sits in Triage on the next pass, so a new rule drains the bucket the next time anything runs.

**A flight's subject resolves late.** File a review against a bare branch with no PR, or a ticket that exists nowhere — tower holds a local subject, reads what the repository shows, and stays silent about fields it cannot see. When the PR opens or the ticket is minted, the adapter links it and upstream truth flows into the fields upstream owns. Which forces one piece of exactness: a signal arriving for a subject you already filed merges into that flight as a `foreign` event rather than filing a second one. This is identity equality on a resolved reference, cheap and exact, and deliberately not semantic deduplication.

Three things tower should not build: **estimates** (measurable for started work, fiction for unstarted — report what is known and invent nothing), **learned ranking** (no data on day one and not enough for a long time; weights live in config and are tuned by hand), and **automatic deduplication** (semantic, rarely urgent, expensive when wrong). And one rule that keeps the board believable wherever an agent's judgment does enter — a hand routing out of Triage, a triage note, a verdict:

> **An agent's judgment is stored as intent, never recomputed as state.**

A model call at render time makes the board flicker: same data, different call, different answer. Judgments are frozen into the log, attributed to the agent that made them, overridable, and never re-run behind your back, so the board stays a pure function of the log and the repository's facts.

## Procedures

Work does not arrive in one shape. A ticket assigned to you, a review requested of you, a thing your manager asked for in a meeting — each has a different decomposition, a different split between what a machine can carry and what only you can, and a different meaning of done. A **procedure** is a named recipe for one shape of work: the instruction set for how a flight proceeds. The word is the metaphor's: a published procedure is a standard sequence for a recurring situation, and the tower clears you for one by name. A plan and a permission, never a hand on the yoke.

A procedure is a graph of flights, saved. Its definition lists the flights it stamps out — each with the same fields any flight carries, pre-filled — the edges between them, and the match rules that apply it to arriving signals:

```toml
name    = "review"
subject = "branch"            # may resolve to a PR later

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
bay      = "warm"             # build the tree ahead of me

[[flight]]
id    = "verdict"
assignee = "me"
after = ["pass", "smoke"]
```

Filing under it mints the parent plus every flight in the graph, in one atomic append — the whole family exists from the first second, and "where we are" is only ever which of those flights are closed. Order is a DAG through `after` — the same edges `ff tower link` writes — so concurrency is the absence of a declaration rather than a keyword: `pass` and `smoke` fly together because neither names the other. Statuses fall out of the edges at mint: a flight with no `after` is born Ready, a flight with dependencies is born Waiting, and the parent waits on them all. A single-flight procedure collapses onto the flight itself — filing under it mints one flight carrying those fields, never a parent and a lone child, because `ff tower file "fix the typo"` must not cost two flights to say one thing.

**A procedure is not required.** A bare flight defaults to the minimal shape — assigned to me, no skill, done when I say so — and every verb works on it: file it, work it, finish it, and no procedure is ever involved. The procedure is for work worth decomposing, and the default assignee being me is what keeps agents out of shapeless work: the queue draws only from the agent lane, so nothing unshaped is ever handed out.

**Procedures are personal, and tower ships none.** No built-in procedures, no built-in skills: the binary is pure engine — flights, statuses, edges, queues, deconfliction, the board — and every opinion about how work should flow lives in files their owner authored. Definitions layer in two, keyed by the name inside the file, the more specific replacing the less wholesale: **user**, `$XDG_CONFIG_HOME/tower/procedures/*.toml` — `~/.config/tower/procedures` when that variable is unset — which roams with your config; and **repository**, `<main worktree>/.tower/procedures/*.toml`, which is the team's. The documentation carries worked examples — a ticket shape, a review shape, a plan shape — that a person copies in and forks, and a builder UI can assemble them eventually; either way the file is the owner's, visibly. The main-worktree anchor is `tower.bays`'s, for `tower.bays`'s reason: every bay must see the same definitions, and a path resolved against the invoking worktree would hand each bay its own procedure set. A missing directory is an empty layer; a file that does not parse is a refusal naming the path, because a definition you cannot see is worse than one that refuses.

The repository layer is in the tree, and that is not the working-tree trap from *Storage and sync*: what must never live there is mutable board state, and a procedure definition is config that changes monthly. It also has to be in the tree to be the team's at all — a definition on an orphan ref is a definition nobody clones.

**The definition is read once, at file time, and its fields are copied onto the minted flights.** Readiness, conflicts, order, and drift stay the engine's as ever. Editing a procedure therefore never disturbs a flight already in the air — a board that re-read config at render time would flicker for exactly the reason principle 11 forbids re-running judgment, and forking a procedure mid-week has to be safe or nobody will.

**Procedures declare structure; skills hold judgment.** A procedure is data — a name, match rules, flights, edges — and it cannot express control flow. No conditions, no loops. Everything conditional lives in the skill an agent-assigned flight points at, in markdown, which is where this document already puts judgment. The moment a procedure needs an `if`, it is a skill. That rule is the only thing between this feature and Jira's workflow editor, which is where configurable trackers go to die: the config language grows into a bad programming language.

**A procedure should end with you.** Principle 3 at the flight level: the boundary where the team sees the work is always a human gesture, so the last flight in a shape of work is normally assigned to me. The loader warns — by name and by flight — when a definition's terminal flights are all agent-assigned, and it warns rather than refuses because the file is personal and the boundary that actually holds is `never auto-outward`: whatever an agent finishes, nothing leaves the machine without a person's verb.

**`done` is a closed enum** on a flight: `asserted` (its owner says so), `committed`, `promoted`, `landed`. Four values cannot grow into an expression language, which is the whole point. *Done when CI is green and two people approved* is a me-assigned flight you assert, and what convinces you belongs to the skill. A flight that does not say is `asserted`, and `asserted` is the only one anything reads today — the other three parse, validate, and store against the verbs that will be able to see them. The enum is closed in the loader and open in the log: a flight's fields copied into a `filed` event carry `done` as a free string, because a newer tower's completion word must not take an older tower's whole board down rather than one flight. The refusal belongs where a person is editing a file.

## Skills

A skill is the agent's flight manual: instructions a harness executes, never a process tower spawns. That is not a contradiction of principle 2 — tower ships the seam, the harness runs the judgment, and uninstalling the harness leaves tower working. tower never grows a process supervisor.

It is also the right home for judgment. tower reports facts and what is Ready; a skill decides what to do when a flight holds, when a review comment needs a person, when to stop, when to ask in conversation instead of holding. Policy in markdown the user can fork beats policy compiled into Rust.

Like procedures, skills are personal and tower ships none. They layer the same way — user, then repository, the same name replacing wholesale — and `ff tower skills <name>` prints one raw, byte for byte. The documentation's worked examples cover the recurring three: **plan** (decompose a goal into a tree of flights — solo mode's entry point), **work** (pull, fly, hold or finish, repeat — the loop that pairs with `next`), and **review** (first-pass someone else's branch: commit the mechanical fixes, write the pass as a comment, hold the judgment for a person's verdict). Each agent-assigned flight names the skill it is flown with, which is the seam that keeps structure in data and judgment in prose. The harness bridge is your own redirect, because tower never writes another program's config:

```sh
ff tower skills work > .claude/skills/tower-work/SKILL.md
```

Loop control is exit codes, fufu's own: **0** here is work, **1** nothing available, **3** work exists but it needs you. A loop runs until 1 or 3 and reports which. No timeout, no sentinel.

Fan-out needs a set, not an item, because conflict-freedom is a property of the set: `ff tower next -n 3` returns three flights that collide with neither each other nor anything already flying, and the caller spawns one agent per bay. That is deconfliction as an API rather than a report, and it is the sharpest reason the design is worth building. The verdicts underneath are `ff collide`'s, one pair at a time; the set is tower's fold over them, filtered to what is Ready, and the fold, the filter and the pull are all tower's contribution.

An example skill stops short of the push boundary — committed on a branch, PR unopened — because principle 3 is easy to state and easy for an unattended loop to violate fourteen times before anyone looks. Where a person's fork draws that line is the person's call, and visibly theirs.

## The three modes

**Solo** — no adapters. Planning with an agent produces a tree of flights — the agent files each step and links the order, and tower stores a DAG it did not author. Then context can be wiped safely, because tower is the durable half: the plan, each brief, every question and answer, and every capture chain live outside the agent. The agent is disposable; the flight is not. A tree of flights is what other trackers would call a project, and it needs no second entity to be one.

**Team** — adapters installed. Upstream owns its fields, tower owns the local layer, and the local layer is where the actual day happens.

**In between** — one upstream ticket, many local sub-flights, one promotion when a step outgrows the local board.

Three layers of memory stay apart: a **skill** knows how to drive tower, the **agent's own memory** knows house style and conventions, and a **brief** knows this flight — the record, the family, the facts. tower owns only the third. A skill that starts accumulating project conventions has taken the agent's job, and tower trying to own house style would do it badly when the agent already has a system for it.

## Principles

1. **Intent is stored; the repository audits it.** Fields are set the way every tracker sets them; the capture floor checks them. Drift is flagged, never corrected.
2. **tower is called; it never calls.** No dispatch, no agent loop. A standing process may refold and subscribe; it decides nothing. The harness schedules; tower queues.
3. **Never auto-outward.** Local state moves freely; anything the team sees is a deliberate gesture.
4. **Upstream owns its fields.** tower is never authoritative over someone else's tracker, and never merges into their model. tower's status, assignee, and priority are the local layer, never synced with upstream's.
5. **Observe and complain, never enforce.** tower prints the path, reports the drift, and does not hook or veto.
6. **Conflict-free by construction.** Union-merged event logs, not a synced database.
7. **Local work stays local until promoted.** Sub-flights are anonymous branches; promotion is the publish boundary.
8. **Deferred requires loud.** Inherited whole from fufu: a held flight is announced, pinned, and blocks its exits.
9. **One model, every surface.** CLI, MCP, and anything later consume one contract.
10. **Facts, not consensus.** tower is authoritative over what the repository shows and what you alone authored. It holds no negotiated state, because it has no way to negotiate.
11. **Judgment is stored, never recomputed.** A model's verdict is written to the log as authored intent, attributed and overridable. The board is a pure function of the log and the repository, or it flickers and is not believed.
12. **The engine ships empty.** No built-in procedures, no built-in skills, no default opinions about how work flows. Structure and judgment are the owner's files, and the documentation teaches by example.
13. **Procedures declare structure; skills hold judgment.** Procedures are data and carry no control flow. Every conditional lives in markdown a person can fork.
14. **A sub-flight never competes with its parent for attention.** Fine granularity is for queues; every other surface aggregates at the parent.

## What it stands on

Four fufu surfaces carry most of this, and all four exist.

- **`ff collide`** is the sideways axis. Base and remote were never the interesting pair for a tracker; every discovered conflict, land order, and pull-time holdback is sibling against sibling, and that is the axis this verb points. It answers one pair, which is the shape both questions tower asks actually take: whether a candidate hits anything already flying, and whether the next flight admitted to a set hits the ones already in it.
- **`ff watch`** streams the operation log as newline-delimited JSON, and `--session <name>` narrows it to one tag — so a flight's own motion is a subscription rather than a poll. It reports what the log *did* rather than what was appended: an undo that steps the pointer back, a fork after one, a trim that rewrites every id a subscriber holds. Tower must handle those the way any subscriber does, because the board's ids are the log's ids wherever a flight points at capture. `--all` is the fleet form: every chain in the repository on one stream, with a `worktree` field on every line — the field the board keys on, present in both modes — so `bays: N` is one process rather than N. Bays that appear mid-stream join it, retired bays keep their place through their last capture, and a trim in one bay ends that bay's addresses rather than the stream.
- **`ff publish`** is the outgoing half, and it is why review state and `landed` are readable at all: `ff sync` takes in, `ff publish` sends, and only the second one leaves the machine.
- **Sessions** are a tag on an operation and nothing more. `--session <name>` rides every fufu command, lands as a `fufu-session` trailer, and serves as the equality test that groups adjacent captures into one `ff undo` step. There is nothing to open or close: every fufu call tower makes carries `--session <flight>`, per-flight capture chains fall out of the tagging, and the extension seam hands `FF_SESSION` down to a child process, so an adapter's own `ff` calls inherit the tag without re-passing the flag.

## What it waits on

Load-bearing and absent:

- **~~One operation log across many bays.~~** *Answered.* fufu keys the chain by worktree, so each bay has its own log, its own undo pointer and its own lock, and records only the refs it owns. This was the largest thing tower waited on, and `bays: N` no longer waits on it. The reading half landed with it: `ff watch --all` is one stream over every chain in the repository, each line naming the worktree it came from, so a supervisor over a pool subscribes once instead of per bay. What remains is tower's own.
- **Forge reads.** A review shape stands almost entirely on state the repository cannot see, so the adapter that supplies it is a dependency of the documented examples rather than a nicety. This one is tower's own to build.
- **~~A handshake at the extension seam.~~** *Answered.* `ff <name>` hands a child `FF_REPO` — the worktree it was invoked against, absolute and resolved, unset outside one — alongside `FF_CONTRACT` and the session tag. A tower adapter reads which repository it is in rather than rediscovering it, and reads the envelope version before parsing an envelope. `ff -C <dir>` landed with it, so a bay is addressable without spawning from its directory: one process can ask every bay in the pool.

Most of what tower reads exists today: the event log store, per-flight session tags, flight-to-branch linkage, briefs, holds, and the pairwise verdicts underneath both the land order and the set `next -n <k>` hands out. The deconfliction that is the reason to build tower is available now. What is missing is the fold pointed at the queue, the concurrency to spend it on, and the forge state a review shape reads.

## Open questions

- **Drift thresholds.** How long is "no motion" before the row says so — a constant, a config key, or scaled to the flight's own cadence. Config with a plain default, probably, but the default is the product decision.
- **The closed window.** Days or count, and the default. Small enough to keep the fold bounded, large enough that Friday shows Monday.
- **Does the flight own the branch, or the branch own the flight?** If `ff branch <name>` claims a placeholder, does claiming mint a flight? The everything-is-a-flight version is seductive and probably wrong.
- **How much forge state to absorb.** Not whether — the review shape settles that — but where it stops. Every field pulled in punctures the ownership table a little further, and that table is the only thing keeping this from becoming a second tracker.
- **Whether the `done` enum stays at four.** It is closed on purpose, and the first genuinely missing value is the moment to check whether the answer is a fifth constant or a flight nobody wanted to own.
- **What a flight means after a rewrite** folds its snapshots into a commit — fufu's open session-boundary question, made urgent rather than theoretical.
- **Bay relocation.** tower prints a path and cannot make a running agent honor it. How loudly should misplaced work be reported, and is there a consented way to move an agent?
- **Sandboxing composes but is unaddressed.** A bay can be a worktree bind-mounted into a container without tower's model changing; whether that is tower's concern at all is open.
- **What loop control is on MCP.** The exit codes are fufu's and they are right for a shell loop, but MCP returns a result and has no exit code to carry 0/1/3. Either the three states become a field the tool returns and the exit codes are the CLI's rendering of it, or the agent lane loops through the CLI and MCP is for reading. Principle 9 says one model, so the answer is probably the first, and it is not decided.
- **How much orchestration belongs in a documented example skill** before it is a scheduler with extra steps and principle 2 has been defeated by paperwork.
- **Naming.** `ff tower` against crates.io, npm, and Homebrew. Almost certainly taken; the metaphor is what matters, not the word.

//! Every help page tower prints. The prose lives here rather than in
//! `cli.rs` doc comments for one mechanical reason: clap_derive joins a
//! doc comment's lines into a single paragraph and this build has no
//! `wrap_help`, so a doc comment prints as one very long line, while a
//! `&'static str` is emitted line for line. Hand-wrapped at 72 columns.
//!
//! Two consts per command: the long description clap prints above
//! `Usage:` (`long_about`), and the examples it prints below the options
//! (`after_long_help`). The one-line `about` stays in `cli.rs`, where it
//! is also the row in the parent's command list. `Board` shares `ROOT`'s
//! pair — bare `ff tower` is the board, so `ff tower help board` prints
//! the root page, fufu's bare-`ff`-and-`help map` precedent.

pub const ROOT: &str = "\
tower: the board over fufu

Work is filed as flights on a board, and the board is derived: every
verb appends an event to a log kept as ordinary git refs in the
repository, and every render folds that log fresh. Nothing is entered
twice, and a render never blocks on the network.

Bare `ff tower` is the board. What needs a person is pinned on top —
questions an agent stopped on, and your own Ready flights — and under
it the flights group by the status the record derives: triage,
waiting, ready, in progress, held, and the three newest closed.
--closed takes
more or less of that last group: a count, a span like 7d, `all`, or
`none`. The repository audits those fields as it goes: a flight In
Progress its branch has forgotten says so, and so does a Ready flight
the branch moved under. A sub-flight is a flight: it files into its
own status group like anything else, and what says a row is a family
is the parent's progress mark, (1/3), closed children over total.
Type it often; it is the fastest way to learn what to do next.
`board` is the same render made explicit, so this page is also
`ff tower help board`.

A flight has two names, human against wire. The board prints the dense
number, #3; the wire name is the id of the event that filed it,
<writer>.<seq>, and JSON carries it raw. Any verb taking a flight
accepts <n>, <writer>#<n>, or <writer>.<seq> — a bare number resolves
against the board's filed flights, and an ambiguous one refuses with
the full forms — and a leading # is stripped, so what tower prints
pastes back in.

--json swaps the human render for the machine envelope: one line of
JSON carrying the contract number, the verb, and either data or error,
never both — success and failure alike. A refusal's error id reads
back through `ff tower explain <id>`.

There is no bare tower binary. `ff tower` reaches ff-tower through
fufu's ff-<name> dispatch, git-style, so the verb is typed through
fufu — the dependency made honest: tower has nothing to say about a
machine with no fufu on it.";

pub const ROOT_EXAMPLES: &str = "\
Examples:
  ff tower                       the board: what needs you, what is moving
  ff tower next                  pull the next Ready flight
  ff tower brief 17              everything known about one flight
  ff tower file \"fix the login redirect\"   put work on the board
  ff tower hold 17 -m \"which flow wins?\"   stop with a question, bay warm
  ff tower done 17               off the board, on the record
  ff tower --closed 7d           a week of closed instead of three rows
  ff tower explain --list        every refusal tower can make

`ff tower help <command>` (or `ff tower <command> --help`) has the details.";

pub const NEXT: &str = "\
Pull the next Ready flight from the agent lane, or with -n <k> a set
of k that collide with neither each other nor anything already
flying. The pool is every Ready flight assigned to the agent lane;
admission is greedy, in filed order, and a pairing fufu could not
judge excludes — unknown never rounds down to clear.

The pull sets each picked flight In Progress in one atomic append, so
two agents pulling at once cannot take one flight — the append is the
exclusivity, and the event's byline is the pilot. --peek is the same
computation with nothing written, and the envelope says which
happened either way.

An empty pick is an outcome riding a full data envelope, and only the
exit code says which one: 3 when the lane emptied it — Ready work
exists and it needs you — and 1 when the board is truly drained,
fufu's \"no.\" A loop over `ff tower next` terminates on the code
alone.

The passed rows are the explained ranking: each flight the walk
examined and why it lost — collides, no-verdict — and nothing past
where the walk stopped, so the output stays bounded by the ask rather
than the board. A flight with a live dependency is Waiting, not in the
pool, and never reaches the walk.";

pub const NEXT_EXAMPLES: &str = "\
Examples:
  ff tower next                  pull the next Ready flight
  ff tower next -n 4             a set of four that cannot collide
  ff tower next --peek           the same computation, nothing written
  ff tower status 17 in_progress          take one by hand instead";

pub const BRIEF: &str = "\
Everything the log and the repository know about one flight, in one
read: subject and body, every stored field, the comments in reading
order, each link with the linked flight's subject and status, the
open question, and the reads' facts — branch, tip, holds, whether the
branch is yours.
The standing says where the flight sits on `ff tower next`'s walk and
what it beat. The history lists every gesture on the record in log
order, and each row carries the words the verb took: the status word,
the lane, the fields an edit touched, the other end of the edge.

Collide probes ride it only where a verdict can change the answer, so
a brief stays instant, and a done or branchless flight briefs with
zero probes. A done flight briefs like any other — the log keeps the
record, and reading is never a lifecycle move.";

pub const BRIEF_EXAMPLES: &str = "\
Examples:
  ff tower brief 17              one flight, in full
  ff tower brief pi-8c2e#3       another writer's, named exactly
  ff tower brief 17 --json       the record as fields
  ff tower next                  where the flight id came from";

pub const FILE: &str = "\
Put work on the board. One argument is a bare filing: the subject is
the flight's one line, and the flight lands in Triage — nothing
clears work to Ready but a procedure or your own `status` move. Two
arguments name a procedure first, then the subject; a name
that is not installed is refused, and one word is never guessed as a
procedure name.

Every stored field is a flag: -m the body, -p the priority, --label
(repeatable), --skill, --assignee (me or agent), --bay. A procedure
is nothing more than those same fields saved across a graph of
flights.

A procedure's definition is read at filing and never again — each
flight's fields are copied into the log, so editing a definition
afterwards cannot disturb a flight already in the air. Statuses fall
out of the edges at mint: no `after` is born Ready, dependencies are
born Waiting, and the parent waits on them all. One flight collapses
onto the filing — born Ready, your flags winning over the
definition's fields — because saying one thing must not cost two
flights. Two or more file a parent plus one flight each, on the same
edges `decompose` writes, all in one append, so no flight is ever
live, unlinked, and pullable.";

pub const FILE_EXAMPLES: &str = "\
Examples:
  ff tower file \"fix the login redirect\"        one line, into Triage
  ff tower file \"rotate the keys\" -m \"…\"        with a body
  ff tower file review feather                  under a procedure
  ff tower file \"upgrade axum\" -p high --label chore --assignee agent   fields at filing
  ff tower procedures                           what there is to file under";

pub const COMMENT: &str = "\
A note on a flight's record — on the log, in the brief from then on,
local. Saying it to a team is a separate, deliberate gesture; tower
forwards nothing.

-m is required, and the refusal is tower's rather than clap's: a
missing note is a coded refusal with an envelope under --json, never
usage text. A done flight still takes a comment, because the record
outlives the board.";

pub const COMMENT_EXAMPLES: &str = "\
Examples:
  ff tower comment 17 -m \"the flaky test is #12's\"   a note on the record
  ff tower brief 17              where comments read back
  ff tower edit <id> -m \"…\"      reword one, by its event id";

pub const EDIT: &str = "\
Reword a flight — its subject with -s, its body with -m — or reset
its fields: --priority, --label (repeatable, replacing the label set
wholesale), --skill, --bay. A comment's text rewords with -m, naming
the comment by its event id. An overlay, not a rewrite: the fold
applies the newest value per field, and the log keeps every prior
one.

An empty -m is a legitimate edit — clearing a body, or blanking a
comment's text. A closed flight's record edits like any other: a
wrong word in a closed record is the motivating case. Status and
assignee are not edits — `status` and `assign` are their own verbs,
attributed as moves.";

pub const EDIT_EXAMPLES: &str = "\
Examples:
  ff tower edit 17 -s \"the real subject\"   reword the subject
  ff tower edit 17 -p high --label chore    reset the fields
  ff tower edit pi-8c2e.41 -m \"…\"          a comment, by its event id
  ff tower brief 17              the record, overlay applied";

pub const LINK: &str = "\
Declare a dependency: `a` depends on `b`. One edge per event, stored
intent — the edge makes `a` Waiting until `b` closes, done or
canceled, at which point the record derives `a` Ready with the closer's
mark and no event of its own. The brief renders both directions as
depends on and blocks.

The identical edge declared twice is refused: the fold would render
it twice, and nothing in the log means it twice. Discovered conflicts
need no edge — verdicts are probed fresh per render, never stored.
`ff tower unlink <a> <b>` takes the edge back.";

pub const LINK_EXAMPLES: &str = "\
Examples:
  ff tower link 18 17            18 waits until 17 is done
  ff tower decompose 17 \"…\" \"…\"  parts ride these same edges
  ff tower brief 17              the edge, read from both sides";

pub const UNLINK: &str = "\
Take back a declared dependency: `a` no longer depends on `b`. One
`unlinked` event naming the edge, and the fold drops it from both
records. Waiting is derived from the edges, so the edge leaving is the
whole release: if `a` waited on `b` alone, the next render derives it
Ready with no event of its own. This is the only way to disagree with
a Waiting the record derives.

The edge must be on the record — there is nothing to take back
otherwise, and the refusal says so. The log keeps both events: the
brief's history shows the link and the unlink, and the edge can be
declared again.";

pub const UNLINK_EXAMPLES: &str = "\
Examples:
  ff tower unlink 18 17          18 no longer waits on 17
  ff tower brief 17              the record, the edge gone from both sides";

pub const DECOMPOSE: &str = "\
Make a flight a parent. Exactly one argument that names an installed
procedure mints the definition's flights beneath it. Anything else is
the by-hand form: each argument files as one sub-flight. Either way
the parts are born cleared, not in Triage — decomposing is your
gesture — and the record derives Waiting for any part whose edges say
so, the `after` of a procedure's flights included. A subject that
happens to collide with a procedure name is spelled around by giving
two subjects or renaming one.

Either way the children ride ordinary link edges —
`ff tower link <a> <b>` declares the same edge by hand, and every
reader works on both unchanged — and the filings and the edges land
in one append, so no sub-flight is ever live, unlinked, and pullable.
Every sub-flight closed, canceled included, makes the parent Ready,
not finished: whether the broad task is over is a judgment, and
`ff tower done` is where it gets made.";

pub const DECOMPOSE_EXAMPLES: &str = "\
Examples:
  ff tower decompose 17 \"the parser\" \"the render\"   two sub-flights, linked
  ff tower decompose 17 review   an installed procedure's flights, under it
  ff tower brief 17              the parent, its children under depends on
  ff tower next                  sub-flights are what it hands out first";

pub const PROCEDURES: &str = "\
What is installed: every procedure's name, the layer it came from,
and the flights it stamps out with their lanes. A name is the detail
page — the match rules by name with their predicates, which route
what sits in Triage the next time anything runs (adapter-keyed ones
stay inert until an adapter exists to fire them); every flight with
assignee, skill, after, and done; and the file it was read from.

Two layers, the most specific winning whole: user,
~/.config/tower/procedures/<name>.toml; repo,
.tower/procedures/<name>.toml under the main worktree. tower ships
none of its own — the documentation's docs/procedures/ carries worked
examples to copy in and fork.

Read-only, and it spawns no fufu. Filing under one is
`ff tower file <name> <subject>`, and the definition is copied into
the log at filing, so editing an installed procedure never disturbs a
flight already in the air. A definition whose terminal flights are
all agent-assigned carries a warning line: a procedure should end
with you.";

pub const PROCEDURES_EXAMPLES: &str = "\
Examples:
  ff tower procedures            every installed procedure
  ff tower procedures release    one in full: flights, rules, fork path
  ff tower file release \"…\"     file a flight under one
  ff tower decompose 17 release  mint one under a flight already filed";

pub const SKILLS: &str = "\
What skills are installed: the prose an agent-crewed part is flown
with — policy in markdown, forkable like a procedure. Bare lists
every name with its layer and one-line description; a name prints
the file raw, byte for byte, so redirecting it into a harness's
skill directory or a fork's starting point needs no flag.

Two layers, the most specific winning whole: user,
~/.config/tower/skills/<name>.md; repo, .tower/skills/<name>.md under
the main worktree. tower ships none of its own — the documentation's
docs/skills/ carries worked examples to copy in and fork. A flight
names the skill it is flown with, and `next` hands the name out on
the picked row.

Read-only, and it spawns no fufu. The documented examples stop at
committed on a branch — no push, no PR — and where your own copy
draws that line is your call, visibly yours: the listing names the
layer every skill came from.";

pub const SKILLS_EXAMPLES: &str = "\
Examples:
  ff tower skills          what is installed, and from where
  ff tower skills work     one, raw — redirect it where a harness reads
  ff tower procedures      the shapes whose agent parts name a skill";

pub const ASSIGN: &str = "\
Set a flight's lane: me, agent, or none to clear it. The lane is the
routing decision — whose queue this is in — and it is all the field
carries: the queue draws only from Ready flights in the agent lane,
so assigning is what opens or closes the gate. Which agent actually
flies it needs no field — every event carries the byline of whoever
wrote it, so the history shows the pilot.

A closed flight refuses; everything else re-lanes freely, and the
move is on the record with your name on it.";

pub const ASSIGN_EXAMPLES: &str = "\
Examples:
  ff tower assign 17 agent       into the agent queue
  ff tower assign 17 me          back to yours
  ff tower assign 17 none        no lane at all
  ff tower next --peek           what the agent lane would hand out";

pub const STATUS: &str = "\
Move a flight: triage, ready, in_progress, done, or canceled. One
event with your byline saying where you want it — the lifecycle verbs
are this verb carrying a payload — and the record derives where it
lands. Waiting and held are not words you can type: waiting comes
from links and held from a question, so `ff tower link <a> <b>` and
`ff tower hold <flight> -m <question>` are how a flight gets there.
`ready` clears the flight, and the record decides between ready and
waiting by its dependencies; the echo says which, and on how many.

A closed flight refuses every move — the log keeps its record. An
open question refuses any move except done and canceled: abandoning
the question is deliberate when the flight itself is over, and
everything short of that goes through
`ff tower answer <flight> -m <answer>`.";

pub const STATUS_EXAMPLES: &str = "\
Examples:
  ff tower status 17 ready       cleared for work
  ff tower status 17 in_progress          take it by hand
  ff tower done 17               the same append, its own verb
  ff tower cancel 17 -m \"…\"     off the board without the finish";

pub const CANCEL: &str = "\
Cancel a flight: off the board without the finish, on the record in
full. -m says why, stored on the move itself — a canceled flight
with no reason is a question your future self will ask.

Canceled and done are the two closed statuses, and they close alike:
comments and edits still land, the id still resolves, the flight
still briefs. Only the meaning differs, and the board drops both.";

pub const CANCEL_EXAMPLES: &str = "\
Examples:
  ff tower cancel 17 -m \"superseded by #21\"    closed, with the why
  ff tower brief 17              a canceled flight still briefs
  ff tower done 17               the other closed status";

pub const HOLD: &str = "\
Stop a flight with a question attached. The hold moves it to waiting
on you with its bay intact — branch and tip stay on the row, because
a warm bay is the point of holding rather than abandoning — and the
exit is 3: an outcome, not an error, fufu's precedent. The envelope
is a full success envelope with the held event in it; only the code
says the flight stopped with a question. Holding is stopping: the
flight is no longer in progress, and the answer returns it to ready
or waiting for whoever pulls it next.

-m carries the question, and a missing one is tower's coded refusal,
never clap usage. `ff tower answer <flight> -m <answer>` releases
the hold, and `ff tower done` finishes a waiting flight anyway —
abandoning the question is deliberate when the flight itself is
over.";

pub const HOLD_EXAMPLES: &str = "\
Examples:
  ff tower hold 17 -m \"which auth flow wins?\"   stop, and ask
  ff tower answer 17 -m \"…\"      the release
  ff tower                       the question, under waiting on you";

pub const ANSWER: &str = "\
Answer the open question and release the hold. The answer goes on the
log's record and counts as the flight's freshest motion — it does not
become a comment — and the flight returns to ready, or to waiting when
a dependency is still live: the record derives which from the graph,
and the answer is the mark.

A flight with no open question refuses: an answer to nothing would
append a gesture the board cannot show.";

pub const ANSWER_EXAMPLES: &str = "\
Examples:
  ff tower answer 17 -m \"the cookie flow; SSO is #21\"   release the hold
  ff tower brief 17              question and answer, on the record
  ff tower hold 17 -m \"…\"        the other half";

pub const DONE: &str = "\
Finish a flight: off the board, out of the count, out of the JSON —
and on the record, in full. Comments and links still land on a done
flight, its id still resolves, and it still briefs; the board shows
what is live and the log keeps everything else.

Bare `ff tower done` derives the flight from the invoking worktree:
the newest session-tagged operation on its own chain names it — the
one place this verb spawns fufu. Run it from the bay the work
happened in, or name the flight from anywhere.

Finishing a waiting flight is allowed: abandoning the question is
deliberate when the flight itself is over. Done is asserted, never
derived — a smoke test that went fine leaves no trace for tower to
read.";

pub const DONE_EXAMPLES: &str = "\
Examples:
  ff tower done                  the invoking worktree's flight
  ff tower done 17               by name, from anywhere
  ff tower brief 17              a done flight still briefs";

pub const EXPLAIN: &str = "\
Look up an error id — the prose behind every coded refusal: the
summary the refusal printed, the detail behind it, and the try: block
of exits. --list is the whole catalog, one line per id.

A pure registry lookup: no store, no repository, no fufu spawn — it
answers on a machine where nothing else does. Every refusal tower
prints carries an id shaped namespace/name, and the namespace picks
the exit code: usage/* exits 2, everything else 1. The 3s are not
among them — an outcome, not an error.";

pub const EXPLAIN_EXAMPLES: &str = "\
Examples:
  ff tower explain flight/not-found        one id, in full
  ff tower explain --list        every id tower knows
  ff tower explain usage/needs-message     why a bare comment refused";

pub const CONFIG: &str = "\
Settings, on fufu's typed-registry model. No subcommands — arity
decides: bare lists every setting with its value, its meaning, and a
(default) marker; a key alone gets it; key plus value sets it;
--unset returns it to the default; --global widens the set or unset
to every repo.

Storage is plain git config under tower.<key>, so `git config` and
tower can never disagree, and precedence is git's own. What the
registry adds is what git config cannot say: which settings exist,
what they default to, and whether a value will parse — validated
through the readers' own parsers before anything touches disk.
Spelling is forgiving: bays, tower.bays, and BAYS all name one
setting.

Five settings ship — bays, the pool root bare `ff tower bay warm`
mints slots under; serveHost and servePort, the address and the port
`ff tower serve` binds; updateCheck, how often the background release
check runs; autoUpdate, whether a new release installs itself
silently. This verb opens no store and spawns no fufu, so settings
stay reachable on a half-configured machine, before an identity
exists.";

pub const CONFIG_EXAMPLES: &str = "\
Examples:
  ff tower config                every setting, defaults marked
  ff tower config bays           what the pool root is
  ff tower config bays ../bays   set it, this repo
  ff tower config --global autoUpdate false   set it, every repo
  ff tower config --unset bays   back to the default";

pub const BAY: &str = "\
The pool of worktrees flights fly from, read off fufu's own survey —
`ff worktree list` — never entered and never registered, so there is
no bay state to drift. Occupancy is the same flight-to-branch
derivation the board runs: the live flight whose freshest work sits
on a bay's branch is the occupant.

Bare `ff tower bay` is the list. `warm` builds a slot ahead of the
work; `release` tears one down, and is refused while a live flight
sits in it.";

pub const BAY_EXAMPLES: &str = "\
Examples:
  ff tower bay                   the pool: occupied and free
  ff tower bay warm              build the next slot ahead of the work
  ff tower bay release bay-3     tear one down; fufu captures first
  ff tower config bays ../bays   where bare warm mints slots";

pub const VERSION: &str = "\
Which tower this is: the release, the commit and date it was built
from, and the project's home under it. ff-tower is dispatch plumbing
rather than a searchable string, so the name and the URL go where a
bug report gets pasted from.

The second half is whether it is the current one. The passive update
lane keeps the latest release in a cache on disk, and this reads the
cache rather than the network: nothing here reaches out, and nothing
waits. A line appears only when a newer release is cached; up to date
says nothing, because saying it every time teaches people to stop
reading.

--json splits the answer into fields — version, commit, date, and the
update status — so a caller never takes the display string apart.
`ff tower -v` is the verb spelled as a flag: same cache, same line,
same fields.";

pub const VERSION_EXAMPLES: &str = "\
Examples:
  ff tower version               the release, the build, the update lane
  ff tower -v                    the same, spelled as a flag
  ff tower version --json        the same, as fields";

pub const UPDATE: &str = "\
Move this binary to the latest release: pick this platform's asset,
verify it against the release's checksums, and atomically rename it
over the executable. Installs that are not tower's to touch are
pointed at their own updater instead — Homebrew at brew upgrade, a
source build at cargo install.

Official builds also keep themselves fresh without being asked: a
check runs at most once per tower.updateCheck (daily by default), and
a newer release either installs itself silently in the background
(tower.autoUpdate, on by default) or lands a one-line notice on
stderr instead. --check is that background lane by hand: refresh the
update cache, print nothing.";

pub const UPDATE_EXAMPLES: &str = "\
Examples:
  ff tower update                update now
  ff tower config autoUpdate false        keep checking, only notice
  ff tower config updateCheck false       turn the whole lane off
  ff tower version               is a newer release already cached?";

pub const DOCTOR: &str = "\
Stale bays, drift, and events off the board: doctor observes and
complains, never enforces — read-only, with no --fix and no writes.
The seam comes first: fufu's version runs before any bay-facing read,
because a drifted contract fails every spawn, and doctor is the verb
that reports the broken seam rather than dying of it.

On a healthy seam it reads what the board reads — the pool, with the
bays whose directories are gone from disk — and the update lane's
cache. It also names every event the fold could not place, which the
board can only count: a chain this repository has yet to fetch, a
kind a newer tower wrote, a kind tower has retired, and the two
shapes only a hand-edited log produces.

Rows come at three levels: ok counts nothing, info is news rather
than a problem, WARN is a finding. Findings drive the exit — 0
healthy, 1 findings — an outcome on the success path, so a script
gates on the code, and --json emits the same rows.";

pub const DOCTOR_EXAMPLES: &str = "\
Examples:
  ff tower doctor                read the pool, the seam, and the log
  ff tower doctor --json         the same rows, for machines
  ff tower bay                   the pool it is judging";

pub const SERVE: &str = "\
Run tower's standing process: the server behind the browser board.
It serves the read API, the verb API, the change feed, and the board
itself — the web app is embedded in the binary at build time, and
every path outside /api answers a build file or the app shell.

The read API is four GET routes — /api/board, /api/brief/<flight>,
/api/bays, and /api/procedures, bare or /<name> — each answering the
same envelope the matching verb emits under --json, folded fresh per
request; nothing is cached. The verb API is nine POST routes —
/api/file, /api/assign, /api/status, /api/hold, /api/answer,
/api/done, /api/cancel, /api/comment, /api/decompose — each taking
the verb's arguments as a small JSON body ({\"flight\": …} with an
optional \"message\", file's {\"subject\": …}, assign's
{\"assignee\": …}, status's {\"status\": …}, decompose's
{\"parts\": […]}), appending to the log, and answering the verb's own
data envelope; hold answers 200, its exit-3 outcome being the CLI's
channel, and done requires the flight named.
A refusal is the same one-line error envelope: 400 for a reference
or body that does not parse, 404 for a reference that names nothing,
409 when the board's standing state refuses the write, 503 when the
log is contended, 500 when the pipeline itself failed.

The change feed is GET /api/feed, one SSE stream: the current board
on connect, then an event whenever the repository moves, each
event's data being the board envelope — the same bytes /api/board
answers, minus the trailing newline. Updates arrive whoever wrote —
this server's own POSTs, the CLI, an agent in a bay, a push landing
— including writes that never touched this server.

It runs in the foreground the way `ff watch` does, and Ctrl-C ends
it. It holds no state the log does not, decides nothing, and
dispatches nothing, so every other interface keeps working with it
down — just staler. A person starts it; tower never starts it for
you.

Two lanes settle where it listens, each resolving through four
sources, highest first. The address: --host, then TOWER_HOST, then
tower.serveHost in git config, then 127.0.0.1. The port: --port, then
TOWER_PORT, then tower.servePort, then 7420. A value none of them can
parse is the same refusal wherever it came from, and the refusal
names the lane. An address is an IP literal and never a name — no DNS
in the startup path — so localhost is refused and 127.0.0.1 is how it
is spelled.

The default is the loopback, because this is a process a person
started for themselves. Binding wider works — 0.0.0.0 reaches the
tailnet, which is the case the flag exists for — and puts a board
with no authentication on every interface it reaches, so the verb
says so once on stderr and binds anyway.

Nothing is locked — a second server is another writer, which the log
already handles — so the one conflict worth naming is the port, and
the socket names it. The repository is checked before the socket is
bound, so a wrong directory or a missing git user.email is a refusal
at startup rather than a blank page later. --json emits one envelope
carrying the address it bound, then keeps serving.";

pub const SERVE_EXAMPLES: &str = "\
Examples:
  ff tower serve                 bind 127.0.0.1:7420 until Ctrl-C
  ff tower serve --port 9000     bind somewhere else, once
  ff tower serve --host 0.0.0.0  every interface — reachable, unguarded
  ff tower config servePort 9000          the same, remembered
  ff tower serve --json          the address as an envelope, then serve";

pub const BAY_LIST: &str = "\
Every bay, one row: the painted id, the branch it stands on, and the
occupant — the live flight whose freshest work sits on that branch —
or a dim free, with a dim here on the row you invoked from. Bare
`ff tower bay` is this list, the same optional-subcommand mechanism
as bare `ff tower` being the board.

Occupancy is derived per render, never registered, and the survey is
fufu's: a worktree `ff worktree list` does not show is not a bay.";

pub const BAY_LIST_EXAMPLES: &str = "\
Examples:
  ff tower bay list              the pool, spelled out
  ff tower bay                   the same, bare
  ff tower bay list --json       the rows, for a machine
  ff tower bay warm              add a slot to it";

pub const BAY_WARM: &str = "\
Build a bay ahead of the work — `ff worktree add`, so the chain floor
is laid before the first command runs in it and `ff undo` works there
from the start.

Bare warm mints the next slot under tower.bays: bay-<n>, smallest
free n, refused until the key is set. A path puts the bay exactly
there instead — a relative path resolves against the repository,
never the shell's directory — and the branch is a name you give, or
a new one named after the directory when unsaid.";

pub const BAY_WARM_EXAMPLES: &str = "\
Examples:
  ff tower bay warm              the next slot under tower.bays
  ff tower bay warm ../bays/api  exactly there
  ff tower bay warm ../bays/api feature-api   on a branch you name
  ff tower config bays ../bays   set the pool root once";

pub const BAY_RELEASE: &str = "\
Tear a bay down — `ff worktree remove` behind tower's one gate: a bay
a live flight sits in is refused, because releasing the ground under
a flight is a decision rather than housekeeping. Finish the flight
first, and the bay frees itself on the next render.

Everything else stays fufu's refusal, forwarded verbatim — a missing
worktree, the main worktree, the bay you are standing in. fufu
captures the tree before teardown either way, so uncommitted work in
the bay survives the release on the worktree's own chain.";

pub const BAY_RELEASE_EXAMPLES: &str = "\
Examples:
  ff tower bay release bay-3     by id
  ff tower bay release ../bays/api         by path
  ff tower done 17               what frees an occupied bay
  ff tower bay                   which bays are free";

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use crate::cli::Cli;

    struct Page {
        path: String,
        long_about: Option<String>,
        examples: Option<String>,
    }

    /// The clap tree with its built-ins materialized, so the `help`
    /// subcommand and the auto flags exist to be walked.
    fn tree() -> clap::Command {
        let mut root = Cli::command();
        root.build();
        root
    }

    /// Every visible command, with its resolved help texts — clap holds
    /// the final strings, so there is no const list to keep in step.
    fn walk(cmd: &clap::Command, path: &str, out: &mut Vec<Page>) {
        out.push(Page {
            path: path.to_string(),
            long_about: cmd.get_long_about().map(ToString::to_string),
            examples: cmd.get_after_long_help().map(ToString::to_string),
        });
        for sub in cmd.get_subcommands() {
            if sub.is_hide_set() || sub.get_name() == "help" {
                continue;
            }
            walk(sub, &format!("{path} {}", sub.get_name()), out);
        }
    }

    fn all_pages() -> Vec<Page> {
        let tree = tree();
        let mut out = Vec::new();
        walk(&tree, "ff tower", &mut out);
        out
    }

    /// `lanes()`'s exhaustive-table discipline, applied to prose: a verb
    /// added without a page fails here rather than shipping with clap's
    /// joined doc comment as its whole story.
    #[test]
    fn every_command_has_a_page() {
        let pages = all_pages();
        assert!(
            pages.len() >= 24,
            "only {} commands walked — the walk is broken, not the tree",
            pages.len()
        );
        for page in &pages {
            assert!(
                page.long_about.is_some(),
                "`{}` has no long_about — every command gets a page",
                page.path
            );
            let examples = page
                .examples
                .as_deref()
                .unwrap_or_else(|| panic!("`{}` has no after_long_help examples", page.path));
            assert!(
                examples.contains("Examples:"),
                "`{}`'s examples block is missing its `Examples:` opener",
                page.path
            );
        }
    }

    /// Every `ff tower …` span between backticks, as argv-shaped tokens.
    fn quoted(text: &str) -> Vec<Vec<String>> {
        text.split('`')
            // Odd fields are the ones between a pair of backticks.
            .skip(1)
            .step_by(2)
            .filter(|span| *span == "ff tower" || span.starts_with("ff tower "))
            .map(argv)
            .collect()
    }

    /// Example rows: the command column of every line spelling
    /// `ff tower …` — everything before the two-space gutter.
    fn example_rows(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("ff tower "))
            .map(|line| argv(line.split("  ").next().unwrap_or(line)))
            .collect()
    }

    /// An invocation as argv. Double-quoted spans collapse to one
    /// placeholder before the whitespace split — a subject is one value
    /// however many words it holds — and `<…>` tokens become one after;
    /// the grammar around a placeholder is what is under test.
    fn argv(text: &str) -> Vec<String> {
        let mut collapsed = String::new();
        let mut fields = text.split('"');
        collapsed.push_str(fields.next().unwrap_or(""));
        loop {
            if fields.next().is_none() {
                break;
            }
            collapsed.push('x');
            match fields.next() {
                Some(after) => collapsed.push_str(after),
                None => break,
            }
        }
        collapsed
            .split_whitespace()
            .map(|tok| {
                if tok.starts_with('<') {
                    "x".to_string()
                } else {
                    tok.to_string()
                }
            })
            .collect()
    }

    fn find_arg<'a>(cmd: &'a clap::Command, flag: &str) -> Option<&'a clap::Arg> {
        cmd.get_arguments().find(|arg| {
            if let Some(long) = flag.strip_prefix("--") {
                arg.get_long() == Some(long)
            } else {
                flag.strip_prefix('-')
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|short| arg.get_short() == Some(short))
            }
        })
    }

    /// One spelled invocation, held to the clap surface: the subcommand
    /// path must exist and be visible, every flag must exist and be
    /// visible — hidden is disqualifying, fufu's rule: retired surface
    /// stays declared so typing it reaches an answer, and prose must
    /// not teach it — and the whole line must parse. A placeholder
    /// standing where a verb goes checks the flags and skips the parse;
    /// `help <command>` resolves the path it names instead.
    fn check(root: &clap::Command, tokens: &[String], whose: &str) {
        let line = tokens.join(" ");
        let rest = &tokens[2..];
        if rest.first().map(String::as_str) == Some("help") {
            let mut cmd = root;
            for tok in &rest[1..] {
                if tok == "x" {
                    return;
                }
                cmd = cmd.find_subcommand(tok).unwrap_or_else(|| {
                    panic!("{whose}: `{line}` sends help to a command that does not exist")
                });
                assert!(!cmd.is_hide_set(), "{whose}: `{line}` names hidden surface");
            }
            return;
        }
        let mut cmd = root;
        let mut ahead = rest;
        while let Some(sub) = ahead.first().and_then(|tok| cmd.find_subcommand(tok)) {
            assert!(
                !sub.is_hide_set(),
                "{whose}: `{line}` names {:?}, which is hidden — retired or \
                 undocumented surface must not be taught",
                sub.get_name()
            );
            cmd = sub;
            ahead = &ahead[1..];
        }
        for flag in ahead.iter().filter(|tok| tok.starts_with('-')) {
            if flag.as_str() == "--help" || flag.as_str() == "-h" {
                continue;
            }
            let arg = find_arg(cmd, flag)
                .or_else(|| find_arg(root, flag))
                .unwrap_or_else(|| panic!("{whose}: `{line}` passes {flag}, which does not exist"));
            assert!(
                !arg.is_hide_set(),
                "{whose}: `{line}` passes {flag}, which is hidden — retired or \
                 undocumented surface must not be taught"
            );
        }
        if cmd.has_subcommands() && ahead.first().map(String::as_str) == Some("x") {
            return;
        }
        let mut parse = vec!["ff-tower".to_string()];
        parse.extend(rest.iter().cloned());
        if let Err(err) = <Cli as clap::Parser>::try_parse_from(&parse) {
            // Not every non-Ok is a failure: clap reports `--help` as an
            // error carrying the text it printed.
            use clap::error::ErrorKind::{DisplayHelp, DisplayVersion};
            assert!(
                matches!(err.kind(), DisplayHelp | DisplayVersion),
                "{whose}: `{line}` does not parse:\n{err}"
            );
        }
    }

    /// fufu's parse guard, improved: walked over every page clap holds
    /// rather than a hand-kept const list, so a new page joins the check
    /// by existing. Bare `ff …` spans are fufu's surface and fufu's
    /// guards hold them; git's is likewise not ours to check.
    #[test]
    fn every_command_the_prose_spells_parses() {
        let tree = tree();
        let mut found = 0usize;
        for page in all_pages() {
            for (label, text) in [
                ("long_about", page.long_about.as_deref()),
                ("examples", page.examples.as_deref()),
            ] {
                let Some(text) = text else { continue };
                let mut spans = quoted(text);
                spans.extend(example_rows(text));
                for tokens in &spans {
                    check(&tree, tokens, &format!("{} {label}", page.path));
                    found += 1;
                }
            }
        }
        // Same reason the exit walk proves it reads the tree: an
        // extractor that quietly matched nothing would pass while
        // checking nothing.
        assert!(
            found >= 40,
            "only {found} invocations extracted — the extractors are broken, not the prose"
        );
    }
}

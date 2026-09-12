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
//! pair — bare `atc` is the board, so `atc help board` prints
//! the root page, fufu's bare-`ff`-and-`help map` precedent.

pub const ROOT: &str = "\
tower: the board over fufu

Work is filed as flights on a board, and the board is derived: every
verb appends an event to a log kept as ordinary git refs in the
repository, and every render folds that log fresh. Nothing is entered
twice, and a render never blocks on the network.

Bare `atc` is the board. What needs a person is pinned on top —
questions an agent stopped on, and your own Ready flights — and under
it the flights group by the status the record derives: backlog,
waiting, ready, in progress, held, and the three newest closed.
--closed takes
more or less of that last group: a count, a span like 7d, `all`, or
`none`. A sub-flight is a flight: it files into its
own status group like anything else, and what says a row is a family
is the parent's progress mark, (1/3), closed children over total.
Type it often; it is the fastest way to learn what to do next.
`board` is the same render made explicit, so this page is also
`atc help board`.

A flight has two names, human against wire. The board prints the dense
number, #3; the wire name is the id of the event that filed it,
<writer>.<seq>, and JSON carries it raw. Any verb taking a flight
accepts <n>, <writer>#<n>, or <writer>.<seq> — a bare number resolves
against the board's filed flights, and an ambiguous one refuses with
the full forms — and a leading # is stripped, so what tower prints
pastes back in.

--json swaps the human render for the machine envelope: one line of
JSON keyed atc with the contract number, cmd naming the bare verb,
and either data or error, never both — success and failure alike. A
refusal's error id is bare, and `atc explain <id>` reads it back.

The repository is the current directory; there is no bare tower
binary. fufu on PATH is optional: the board is tower's own log, and
`atc doctor` reports whether fufu is there.";

pub const ROOT_EXAMPLES: &str = "\
Examples:
  atc                       the board: what needs you, what is moving
  atc next                  pull the next Ready flight
  atc brief 17              everything known about one flight
  atc file \"fix the login redirect\"   put work on the board
  atc hold 17 -m \"which flow wins?\"   stop with a question
  atc done 17               off the board, on the record
  atc --closed 7d           a week of closed instead of three rows
  atc explain --list        every refusal tower can make

`atc help <command>` (or `atc <command> --help`) has the details.";

pub const NEXT: &str = "\
Pull the next Ready flight from the lanes named, or with -n <k> the
next k. The lanes are the arguments, walked in the order given: me,
agent, none, or a callsign, and your own queue — me — when unsaid. me
is the literal me lane and your callsign's; agent is the shared pool
alone; none is the unassigned lane; a callsign is that pilot's queue.
Each lane walks by priority, then filed order — an urgent flight
filed late leads its lane, never an earlier one — and the unassigned
lane walks last unless you named it, in which case it walks where you
named it — so everyone falls through to none once their own lanes are
drained, and none walks once. A lane named twice walks once. The pull
is the Ready check and the move in one command: each picked flight is
set In Progress with your callsign as the pilot — the byline, and the
session underneath it — and lands in your own queue, the pull being
yours by default. --assignee <lane> says where it lands instead, with
file's words: agent keeps a pick in the pool, none clears its lane, a
callsign hands it to that pilot. The re-lane rides the same append,
and only when the lane changes — a pull from your own queue is one
moment on the record. --peek is the same computation with nothing
written, and the envelope says which happened either way. Your
callsign is ATC_CALLSIGN when set, else the client you are running
under, else the login name at a terminal, else none — and with none,
me is the literal lane alone.

An empty pick exits 1 with a full data envelope, and `outcome` on it
says which of `drained` and `elsewhere` it was: `drained` is a board
with nothing left, and `elsewhere` is Ready work in a lane the walk
never entered — the count rides beside it. A --json reader branches
on the word; a shell loop stops on the code.

A flight with a live dependency is Waiting, in no walk, and never
reaches the pick.";

pub const NEXT_EXAMPLES: &str = "\
Examples:
  atc next                  pull the next Ready flight from your own queue
  atc next agent            the shared pool, then the unassigned lane
  atc next me agent         your queue, then the pool, then the overflow
  atc next -n 4             the next four, by priority then filed order
  atc next --peek           the same computation, nothing written
  atc next agent --assignee agent    pull and leave it in the pool
  atc next agent -n 3       three from the pool, each moved to you
  atc status 17 in_progress          take one by hand instead";

pub const BRIEF: &str = "\
Everything the log and the repository know about one flight, in one
read: subject and body, every stored field, the newest handoff pinned
above the comments, the comments in reading order, each link with the
linked flight's subject and status, a parent's body under its row —
one level up, so a sub-flight reads whole — the flights whose prose
names this one under `referenced by`, and the open question.
The history lists every gesture on the record in log
order, and each row carries the words the verb took: the status word,
the lane, the fields an edit touched, the other end of the edge.
`show` is the same verb under fufu's spelling for reading one thing,
and the envelope says `brief` either way.

A done flight briefs like any other — the log keeps the record, and
reading is never a lifecycle move.";

pub const BRIEF_EXAMPLES: &str = "\
Examples:
  atc brief 17              one flight, in full
  atc show 17               the same, spelled the way fufu reads one thing
  atc brief pi-8c2e#3       another writer's, named exactly
  atc brief 17 --json       the record as fields
  atc next                  where the flight id came from";

pub const FILE: &str = "\
Put work on the board. One argument is a bare filing: the subject is
the flight's one line, and the flight lands Ready — cleared for work
at once, because most filings are work already decided on. Two
arguments name a procedure first, then the subject; a name
that is not installed is refused, and one word is never guessed as a
procedure name.

Every stored field is a flag: -m the body, -p the priority, --label
(repeatable), --skill, --assignee (me, agent, or a callsign; me
stores your callsign), --status (backlog, ready, or in_progress). A
procedure is nothing more than those same fields saved across a graph
of flights.

Where a filing lands is a setting: `atc config
defaultFileStatus backlog` parks every bare filing for a person
instead. --status beats the setting for one filing, and a procedure
flight that declares its own status keeps it. A bare filing whose
fields a match rule covers files under that rule's procedure as if
the name had been typed, and the record says which rule chose it.

A procedure's definition is read at filing and never again — each
flight's fields are copied into the log, so editing a definition
afterwards cannot disturb a flight already in the air. Each flight
is born with its own status or the setting's word, and the edges
have the last say: dependencies fold Waiting, and the parent waits
on them all. One flight collapses onto the filing — your flags,
--status included, winning over the definition's fields — because
saying one thing must not cost two flights. Two or more file a
parent plus one flight each, on the same edges `decompose` writes,
all in one append, so no flight is ever live, unlinked, and
pullable.";

pub const FILE_EXAMPLES: &str = "\
Examples:
  atc file \"fix the login redirect\"        one line, born Ready
  atc file \"rotate the keys\" -m \"…\"        with a body
  atc file \"decide later\" --status backlog  parked for a person
  atc file review feather                  under a procedure
  atc file \"upgrade axum\" --label chore     under whatever chore matches
  atc file \"upgrade axum\" -p high --label chore --assignee agent   fields at filing
  atc procedures                           what there is to file under";

pub const COMMENT: &str = "\
A note on a flight's record — on the log, in the brief from then on,
local. Saying it to a team is a separate, deliberate gesture; tower
forwards nothing.

-m is required, and the refusal is tower's rather than clap's: a
missing note is a coded refusal with an envelope under --json, never
usage text. A done flight still takes a comment, because the record
outlives the board.

A flight named in the note as `#3` or `writer#3` is stored by its wire
id and printed by its current number; a number two writers hold is
refused the same way the flight argument is, and a match on nothing
stays as typed.

--handoff flags the note as the state of play — done through where,
what is wrong, what is next. The brief pins the newest handoff above
the stream, and every prior one stays in it, flagged. A handoff never
holds the flight: a question only a person can answer is `hold`.";

pub const COMMENT_EXAMPLES: &str = "\
Examples:
  atc comment 17 -m \"the flaky test is #12's\"   a note on the record
  atc comment 17 --handoff -m \"done through step 3, next is the parser\"
  atc brief 17              where comments read back
  atc edit <id> -m \"…\"      reword one, by its event id";

pub const EDIT: &str = "\
Reword a flight — its subject with -s, its body with -m — or reset
its fields: --priority, --label (repeatable, replacing the label set
wholesale), --skill. A comment's text rewords with -m, naming
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
  atc edit 17 -s \"the real subject\"   reword the subject
  atc edit 17 -p high --label chore    reset the fields
  atc edit pi-8c2e.41 -m \"…\"          a comment, by its event id
  atc brief 17              the record, overlay applied";

pub const LINK: &str = "\
Declare a dependency: `a` depends on `b`. One edge per event, stored
intent — the edge makes `a` Waiting until `b` closes, done or
canceled, at which point the record derives `a` Ready with the closer's
mark and no event of its own. The brief renders both directions as
depends on and blocks.

The identical edge declared twice is refused: the fold would render
it twice, and nothing in the log means it twice.
`atc unlink <a> <b>` takes the edge back.";

pub const LINK_EXAMPLES: &str = "\
Examples:
  atc link 18 17            18 waits until 17 is done
  atc decompose 17 \"…\" \"…\"  parts ride these same edges
  atc brief 17              the edge, read from both sides";

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
  atc unlink 18 17          18 no longer waits on 17
  atc brief 17              the record, the edge gone from both sides";

pub const DECOMPOSE: &str = "\
Make a flight a parent. Exactly one argument that names an installed
procedure mints the definition's flights beneath it. Anything else is
the by-hand form: each argument files as one sub-flight. Either way
the parts are born Ready whatever defaultFileStatus says — decomposing
is itself the clearing gesture — and the record derives Waiting for
any part whose edges say so, the `after` of a procedure's flights
included. A subject that happens to collide with a procedure name is
spelled around by giving two subjects or renaming one.

Either way the children ride ordinary link edges —
`atc link <a> <b>` declares the same edge by hand, and every
reader works on both unchanged — and the filings and the edges land
in one append, so no sub-flight is ever live, unlinked, and pullable.
Every sub-flight closed, canceled included, makes the parent Ready,
not finished: whether the broad task is over is a judgment, and
`atc done <flight>` is where it gets made.";

pub const DECOMPOSE_EXAMPLES: &str = "\
Examples:
  atc decompose 17 \"the parser\" \"the render\"   two sub-flights, linked
  atc decompose 17 review   an installed procedure's flights, under it
  atc brief 17              the parent, its children under depends on
  atc next                  sub-flights are what it hands out first";

pub const PROCEDURES: &str = "\
What is installed: every procedure's name, the layer it came from,
and the flights it stamps out with their lanes. A name is the detail
page — the match rules by name with their predicates (label,
priority, skill, assignee, and status, which matches the word the
filing lands with, flag or setting), which a bare filing's fields
are matched against at file time, first match winning (adapter-keyed
ones stay inert until an adapter exists to fire them); every flight
with assignee, skill, status, after, and done; and the file it was
read from.

Two layers, the most specific winning whole: user,
~/.config/tower/procedures/<name>.toml; repo,
.tower/procedures/<name>.toml under the main worktree. tower ships
none of its own — the documentation's docs/procedures/ carries worked
examples to copy in and fork.

Read-only, and it spawns no fufu. Filing under one is
`atc file <name> <subject>`, and the definition is copied into
the log at filing, so editing an installed procedure never disturbs a
flight already in the air. A definition whose terminal flights are
all agent-assigned carries a warning line: a procedure should end
with you.";

pub const PROCEDURES_EXAMPLES: &str = "\
Examples:
  atc procedures            every installed procedure
  atc procedures release    one in full: flights, rules, fork path
  atc file release \"…\"     file a flight under one
  atc decompose 17 release  mint one under a flight already filed";

pub const SKILLS: &str = "\
What skills are installed: the prose an agent-crewed part is flown
with — policy in markdown, forkable like a procedure. Bare lists
every name with its layer and one-line description; a name prints
the file raw, byte for byte, so redirecting it into a harness's
skill directory or a fork's starting point needs no flag.

Two layers, the most specific winning whole: user,
~/.config/tower/skills/<name>.md; repo, .tower/skills/<name>.md under
the main worktree. tower ships none of its own: a skill is authored
under one of the two layers. A flight names the skill it is flown
with, and `next` hands the name out on the picked row.

Read-only, and it spawns no fufu. Where a copy draws the push line —
committed on a branch, or past it — is its owner's call, visibly so:
the listing names the layer every skill came from.";

pub const SKILLS_EXAMPLES: &str = "\
Examples:
  atc skills          what is installed, and from where
  atc skills work     one, raw — redirect it where a harness reads
  atc procedures      the shapes whose agent parts name a skill";

pub const ASSIGN: &str = "\
Set a flight's lane, in one of four shapes: me, agent, none to clear
it, or a callsign. The lane is the routing decision — whose queue
this is in. agent is the shared pool, which `atc next agent` draws
from; a callsign is one pilot's own queue, which that pilot's bare
`atc next` draws from, and where a pull lands the flight; none is the
unassigned lane, everyone's overflow; me stores your own callsign when you have one, and the
literal word when you do not, and either way the flight is yours on
the board. A callsign is one word, no spaces,
and needs nothing to be a lane. Which pilot flies a flight is the
callsign on the event: every event carries the callsign of whoever
wrote it, with the session and the author underneath, so the history
shows the pilot.

Your callsign is ATC_CALLSIGN when set, else the client you are
running under — claude, codex, cursor, gemini — else the login name
at a terminal, else none.

A closed flight refuses; everything else re-lanes freely, and the
move is on the record with your name on it.";

pub const ASSIGN_EXAMPLES: &str = "\
Examples:
  atc assign 17 agent       into the shared pool
  atc assign 17 me          back to yours, under your callsign
  atc assign 17 qwen-review  one pilot's own queue
  atc assign 17 none        no lane at all
  atc next --peek           what your queue would hand out";

pub const STATUS: &str = "\
Move a flight: backlog, ready, in_progress, done, or canceled. One
event with your byline saying where you want it — the lifecycle verbs
are this verb carrying a payload — and the record derives where it
lands. Waiting and held are not words you can type: waiting comes
from links and held from a question, so `atc link <a> <b>` and
`atc hold <flight> -m <question>` are how a flight gets there.
`ready` clears the flight, and the record decides between ready and
waiting by its dependencies; the echo says which, and on how many.

A closed flight refuses every move — the log keeps its record. An
open question refuses any move except done and canceled: abandoning
the question is deliberate when the flight itself is over, and
everything short of that goes through
`atc answer <flight> -m <answer>`.";

pub const STATUS_EXAMPLES: &str = "\
Examples:
  atc status 17 ready       cleared for work
  atc status 17 in_progress          take it by hand
  atc done 17               the same append, its own verb
  atc cancel 17 -m \"…\"     off the board without the finish";

pub const CANCEL: &str = "\
Cancel a flight: off the board without the finish, on the record in
full. -m says why, stored on the move itself — a canceled flight
with no reason is a question your future self will ask.

Canceled and done are the two closed statuses, and they close alike:
comments and edits still land, the id still resolves, the flight
still briefs. Only the meaning differs, and the board drops both.";

pub const CANCEL_EXAMPLES: &str = "\
Examples:
  atc cancel 17 -m \"superseded by #21\"    closed, with the why
  atc brief 17              a canceled flight still briefs
  atc done 17               the other closed status";

pub const HOLD: &str = "\
Stop a flight with a question attached. The hold moves it to waiting
on you, and the
exit is 3: an outcome, not an error, fufu's precedent. The envelope
is a full success envelope with the held event in it; only the code
says the flight stopped with a question. Holding is stopping: the
flight is no longer in progress, and the answer returns it to ready
or waiting for whoever pulls it next.

-m carries the question, and a missing one is tower's coded refusal,
never clap usage. `atc answer <flight> -m <answer>` releases
the hold, and `atc done <flight>` finishes a waiting flight anyway —
abandoning the question is deliberate when the flight itself is
over.";

pub const HOLD_EXAMPLES: &str = "\
Examples:
  atc hold 17 -m \"which auth flow wins?\"   stop, and ask
  atc answer 17 -m \"…\"      the release
  atc                       the question, under waiting on you";

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
  atc answer 17 -m \"the cookie flow; SSO is #21\"   release the hold
  atc brief 17              question and answer, on the record
  atc hold 17 -m \"…\"        the other half";

pub const DONE: &str = "\
Finish a flight: off the board, out of the count, out of the JSON —
and on the record, in full. Comments and links still land on a done
flight, its id still resolves, and it still briefs; the board shows
what is live and the log keeps everything else.

Finishing a waiting flight is allowed: abandoning the question is
deliberate when the flight itself is over. Done is asserted, never
derived — a smoke test that went fine leaves no trace for tower to
read.";

pub const DONE_EXAMPLES: &str = "\
Examples:
  atc done 17               by name, from anywhere
  atc brief 17              a done flight still briefs";

pub const EXPLAIN: &str = "\
Look up an error id — the prose behind every coded refusal: the
summary the refusal printed, the detail behind it, and the try: block
of exits. --list is the whole catalog, one line per id.

A pure registry lookup: no store, no repository, no fufu spawn — it
answers on a machine where nothing else does. Every refusal tower
prints carries an id shaped namespace/name, pasted whole out of an
envelope. The namespace picks the exit code: usage/* exits 2,
ref/contended exits 4 — run it again — and everything else 1. hold's
3 is not among them — an outcome, not an error. A refusal fufu shaped
itself keeps fufu's own id, and `ff explain <id>` is where its prose
lives.";

pub const EXPLAIN_EXAMPLES: &str = "\
Examples:
  atc explain flight/not-found  one id, in full
  atc explain --list            every id tower knows";

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
Spelling is forgiving: servePort, tower.servePort, and SERVEPORT all
name one setting.

Five settings ship — defaultFileStatus, where a bare `atc file`
lands; serveHost and servePort, the address
and the port `atc serve` binds; updateCheck, how often the
background release check runs; autoUpdate, whether a new release
installs itself silently. This verb opens no store and spawns no
fufu, so settings stay reachable on a half-configured machine, before
an identity exists.";

pub const CONFIG_EXAMPLES: &str = "\
Examples:
  atc config                every setting, defaults marked
  atc config servePort      what port serve binds
  atc config servePort 7777   set it, this repo
  atc config defaultFileStatus backlog   bare filings park for a person
  atc config --global autoUpdate false   set it, every repo
  atc config --unset servePort   back to the default";

pub const VERSION: &str = "\
Which tower this is: the release, the commit and date it was built
from, and the project's home under it. atc is dispatch plumbing
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
`atc -v` is the verb spelled as a flag: same cache, same line,
same fields.";

pub const VERSION_EXAMPLES: &str = "\
Examples:
  atc version               the release, the build, the update lane
  atc -v                    the same, spelled as a flag
  atc version --json        the same, as fields";

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
  atc update                update now
  atc config autoUpdate false        keep checking, only notice
  atc config updateCheck false       turn the whole lane off
  atc version               is a newer release already cached?";

pub const DOCTOR: &str = "\
The seam, the log, and the registries: doctor observes and
complains, never enforces — read-only, with no --fix and no writes.
The seam row comes first: whether ff is on PATH, its version, and
whether it speaks the contract tower reads. Absent is information —
fufu is optional, and the board runs without it; drift is a finding,
because a drifted contract fails every spawn, and doctor is the verb
that reports the broken seam rather than dying of it.

Then the log: every event the fold could not place, which the board
can only count — a chain this repository has yet to fetch, a kind a
newer tower wrote, a kind tower has retired, and the two shapes only
a hand-edited log produces. Then the registries — the installed
procedures and skills, and the update lane's cache. Then one row per
agent client `atc hook` knows: a client not on this machine earns no
row, wired is ok, not wired is information, and skills an older
tower wrote are a finding, because `atc hook -u` is the only thing
that rewrites them.

Rows come at three levels: ok counts nothing, info is news rather
than a problem, WARN is a finding. Findings drive the exit — 0
healthy, 1 findings — an outcome on the success path, so a script
gates on the code, and --json emits the same rows.";

pub const DOCTOR_EXAMPLES: &str = "\
Examples:
  atc doctor                read the seam, the log, and the registries
  atc doctor --json         the same rows, for machines";

pub const SERVE: &str = "\
Run tower's standing process: the server behind the browser board.
It serves the read API, the verb API, the change feed, and the board
itself — the web app is embedded in the binary at build time, and
every path outside /api answers a build file or the app shell.

The read API is four GET routes — /api/board, /api/brief/<flight>,
/api/procedures, bare or /<name>, and /api/views — each
answering the same envelope the matching verb emits under --json,
folded fresh per request; nothing is cached. /api/board takes the
query string atc's views store, on its URL
(/api/board?status=ready,in_progress&group=assignee&closed=7d), and
answers the groups plus the two counts, hidden and filtered; no
query is the board's own grouping, and a query it cannot parse is a
400 under the query's own id. The verb API is twelve POST routes —
/api/file, /api/assign, /api/status, /api/hold, /api/answer,
/api/done, /api/cancel, /api/comment, /api/decompose, /api/edit,
/api/link, /api/unlink — plus the three under /api/views, each
taking the verb's arguments as a small JSON body ({\"flight\": …}
with an optional \"message\", file's {\"subject\": …}, assign's
{\"assignee\": …}, status's {\"status\": …}, decompose's
{\"parts\": […]}, edit's {\"target\": …} plus any of \"subject\",
\"message\", \"priority\", \"labels\", \"skill\", and link's
and unlink's {\"flight\": …, \"dependency\": …}), appending to the
log, and answering the verb's own data envelope; hold answers 200,
its exit-3 outcome being the CLI's channel, and done requires the
flight named.
A refusal is the same one-line error envelope: 400 for a reference
or body that does not parse, 404 for a reference that names nothing,
409 when the board's standing state refuses the write, 503 when the
log is contended, 500 when the pipeline itself failed.

The change feed is GET /api/feed, one SSE stream per query: it
takes the same query on its URL, folds the current board on connect,
then an event whenever the repository moves, each subscriber's frame
being /api/board?<query>'s body minus the trailing newline. A
different query is a new subscription. Updates arrive whoever wrote
— this server's own POSTs, the CLI, an agent, a push
landing — including writes that never touched this server.

It runs in the foreground the way `ff watch` does, and Ctrl-C ends
it. It holds no state the log does not, decides nothing, and
dispatches nothing, so every other interface keeps working with it
down — just staler. A person starts it; tower never starts it for
you.

Two lanes settle where it listens, each resolving through four
sources, highest first. The address: --host, then ATC_HOST, then
tower.serveHost in git config, then 127.0.0.1. The port: --port, then
ATC_PORT, then tower.servePort, then 7420. A value none of them can
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
  atc serve                 bind 127.0.0.1:7420 until Ctrl-C
  atc serve --port 9000     bind somewhere else, once
  atc serve --host 0.0.0.0  every interface — reachable, unguarded
  atc config servePort 9000          the same, remembered
  atc serve --json          the address as an envelope, then serve";

pub const TRIGGER: &str = "\
What a wired client runs on every event it offers, and how a person
reads the notice. Bare `atc trigger` prints the notice for the
repository you are in: what tower is, the four gestures of the loop,
and one line that is this repository's — how many flights are ready,
or that nothing is filed here yet. When the process has a callsign —
ATC_CALLSIGN when set, else the client it runs under, else the login
name at a terminal — and a flight is In Progress under it, the line
is the resume line instead: the flight you are on, and the brief to
run.

Named for a source — `atc trigger claude` — it is the command
`atc hook` wrote into that client's config under every event in the
client's table, and the payload on stdin says which one fired. At a
context boundary — a fresh session, a resume, a clear, a compaction
— it renews the session's lease and prints the notice wrapped the
way that client reads it. On activity — a prompt, a tool call, the
end of a turn — it renews the lease and says nothing. At the
session's end it releases the lease and says nothing. A payload with
no hook_event_name is a boundary, so an older config's entry keeps
doing what it did. The lease is one file per session under the
machine's state directory, keyed by the session variable the client
exports, else the payload's session_id; renewing it opens no store,
so the activity path costs a few milliseconds on every tool call.

Any failure — no repository, a store that will not open, a source or
an event it does not know — exits 0 with nothing said, because a
hook's stderr is noise in someone else's terminal. `briefing` is the
spelling this verb replaced, kept for the configs that still carry
it: it prints the notice and touches no lease.

--json carries the text as `data.text`, with `ready`, `filed`, `on`
— the flights In Progress under your callsign — and `callsign`
beside it. The advanced surface is not here: it is in the `tower`
skill `atc hook` writes beside the hook, read only when wanted.";

pub const TRIGGER_EXAMPLES: &str = "\
Examples:
  atc trigger               what an agent is told here
  atc trigger claude        the same, as Claude Code's hook runs it
  atc trigger --json        the text as a field, with the counts";

pub const BRIEFING: &str = "\
The notice alone, the way `atc trigger` printed it before it was
wired to every event: bare, for the repository you are in; named for
a client, wrapped the way that client reads it, silent on any
failure, and refusing a client it does not know. It touches no lease
and reads no event, so it is the spelling a stored config may still
carry and not one to write anew — `atc hook -u` rewrites it.";

pub const HOOK: &str = "\
Wire tower into the agent clients on this machine, so every session an
agent starts in a repository is told the board is here and how to
pull from it. The hooks carry three things: the notice at every
context boundary the client reports — session start, resume, clear,
compact — a heartbeat on the session's lease at every prompt, tool
call, and turn, and the lease's release at the session's end. The
notice is silent outside a git repository, and the heartbeat opens no
store, so a wired client costs nothing elsewhere.

Bare `atc hook` reports what it found and then asks. Name clients to
wire exactly those; --all takes everything detected without asking;
-l reports and stops either way. -u rewrites what is already wired
and adds nothing: the install is re-run for every client already
wired, on whatever mechanism it is on, so an upgraded binary
refreshes the machine. The clients are flat names:

  claude  codex  cursor  gemini

What gets written is not a choice you make. Claude Code takes a
plugin directory tower owns outright, ~/.claude/skills/tower; the
other three take entries merged into their own hooks file, and
whatever else that file holds is left as it was — an entry you wrote
yourself included. --settings is
Claude Code's escape hatch: entries in ~/.claude/settings.json
instead of the plugin, and no skills.

Claude Code and Codex also take the manual, `tower` — typed
/tower:tower in Claude Code and $tower in Codex; a skill an older
tower shipped and this one does not is removed on the next write.
Cursor and Gemini read no skills directory and get the notice alone.
Codex trusts a hook by its hash, so after wiring it review the hook
with /hooks there, or it is skipped.";

pub const HOOK_EXAMPLES: &str = "\
Examples:
  atc hook                  what is on this machine, then asks
  atc hook claude codex     wire exactly those
  atc hook --all            everything detected, no question
  atc hook -l               report and stop
  atc hook -u               rewrite what is wired, after an update
  atc unhook claude         take back exactly what hook added
  atc doctor                one row per client, with its state";

pub const UNHOOK: &str = "\
Remove exactly what hook added: Claude Code's plugin directory, and
the settings entries an older install left; the other clients'
entries and skill directories. Anything else in a client's file is
left as it was, an entry you wrote yourself included. Name clients,
or --all for every client detected on this machine.";

pub const UNHOOK_EXAMPLES: &str = "\
Examples:
  atc unhook claude         take back exactly what hook added
  atc unhook --all          every client detected
  atc hook -l               what is wired now";

/// The extractors the prose guards share: every `atc …` a text spells,
/// as argv, and the check that holds one against the clap tree. Here
/// rather than in `tests` because the notice in `integ/briefing.rs` is
/// prose an agent reads as instructions too, and one reading of the
/// tree is what keeps the two guards from disagreeing.
#[cfg(test)]
pub(crate) mod guard {
    use clap::CommandFactory;

    use crate::cli::Cli;

    /// The clap tree with its built-ins materialized, so the `help`
    /// subcommand and the auto flags exist to be walked.
    pub(crate) fn tree() -> clap::Command {
        let mut root = Cli::command();
        root.build();
        root
    }

    /// Every `atc …` span between backticks, as argv-shaped tokens.
    pub(crate) fn quoted(text: &str) -> Vec<Vec<String>> {
        text.split('`')
            // Odd fields are the ones between a pair of backticks.
            .skip(1)
            .step_by(2)
            .filter(|span| *span == "atc" || span.starts_with("atc "))
            .map(argv)
            .collect()
    }

    /// Example rows: the command column of every line spelling
    /// `atc …` — everything before the two-space gutter.
    pub(crate) fn example_rows(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("atc "))
            .map(|line| argv(line.split("  ").next().unwrap_or(line)))
            .collect()
    }

    /// An invocation as argv. Double-quoted spans collapse to one
    /// placeholder before the whitespace split — a subject is one value
    /// however many words it holds — and `<…>` tokens become one after;
    /// the grammar around a placeholder is what is under test.
    pub(crate) fn argv(text: &str) -> Vec<String> {
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
    pub(crate) fn check(root: &clap::Command, tokens: &[String], whose: &str) {
        let line = tokens.join(" ");
        let rest = &tokens[1..];
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
        let mut parse = vec!["atc".to_string()];
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
}

#[cfg(test)]
mod tests {
    use super::guard::{check, example_rows, quoted, tree};

    struct Page {
        path: String,
        long_about: Option<String>,
        examples: Option<String>,
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
        walk(&tree, "atc", &mut out);
        out
    }

    /// The three worked examples under docs/skills/: prose that spells
    /// commands, held to the tree like the manual is, though tower does
    /// not ship them.
    const DOCS: [(&str, &str); 3] = [
        ("plan", include_str!("../../../docs/skills/plan.md")),
        ("work", include_str!("../../../docs/skills/work.md")),
        ("review", include_str!("../../../docs/skills/review.md")),
    ];

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
        // The skills are prose that spells commands too, and they are
        // held to the same tree: every backticked `atc …` span and every
        // code-block line that starts with one. So is the notice, which
        // is the one text every wired session reads.
        for skill in &crate::integ::skill::SKILLS {
            let mut spans = quoted(skill.text);
            spans.extend(example_rows(skill.text));
            for tokens in &spans {
                check(&tree, tokens, &format!("{} skill", skill.name));
                found += 1;
            }
        }
        for (name, text) in DOCS {
            let mut spans = quoted(text);
            spans.extend(example_rows(text));
            for tokens in &spans {
                check(&tree, tokens, &format!("{name} doc skill"));
                found += 1;
            }
        }
        for tokens in &quoted(crate::integ::briefing::NOTICE) {
            check(&tree, tokens, "notice");
            found += 1;
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

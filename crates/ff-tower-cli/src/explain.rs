//! Curated error ids with prose. The single source of truth for what each
//! id means and how to leave it — fufu's registry, tower's ids.
//!
//! Only tower-owned ids live here. A refusal fufu shaped itself passes
//! through the seam verbatim — its id, message, and exits are fufu's
//! words, and fufu's own envelopes already carry the `ff explain <id>`
//! hint — so entering them here would be a second copy waiting to drift.

use serde::Serialize;

use crate::error::CliError;

#[derive(Serialize)]
pub struct Entry {
    pub id: &'static str,
    /// One line: what this error means.
    pub summary: &'static str,
    /// A short paragraph: why it happens and what the exits do.
    pub detail: &'static str,
    pub exits: &'static [&'static str],
}

pub static ENTRIES: &[Entry] = &[
    Entry {
        id: "usage/bad-flags",
        summary: "the command line does not combine into one command",
        detail: "The line was answerable but not runnable: `-V` where tower's version flag is \
                 lowercase `-v`, the version flag riding another verb — two commands on one line \
                 — or `explain` with neither an id nor `--list`. The refusal itself names the \
                 spellings that would each be right alone.",
        exits: &[],
    },
    Entry {
        id: "usage/bad-body",
        summary: "the POST body is not the verb's JSON",
        detail: "Only the server raises this — the CLI has clap where the verb API has JSON. A \
                 write route takes the verb's arguments as a small JSON object (`{\"flight\": \
                 …}`, and `file`'s `{\"subject\": …}`), and a body that is not JSON, misses a \
                 required field, or carries a field the verb does not take is refused whole, \
                 before anything is read or written. The refusal's message quotes what the \
                 parser objected to.",
        exits: &[],
    },
    Entry {
        id: "usage/bad-flight",
        summary: "that is not a flight reference",
        detail: "A flight is named by its bare number `<n>`, by `<writer>#<n>` when boards span \
                 writers, or by the full wire id `<writer>.<seq>`. A leading `#` is tolerated \
                 because that is how tower prints them — what tower prints, tower accepts. \
                 Anything else is refused before any store is opened.",
        exits: &["ff tower"],
    },
    Entry {
        id: "usage/bad-count",
        summary: "`-n 0` asks for no flights",
        detail: "The count starts at 1. `next` with `-n 0` would be a walk that admits nothing \
                 by construction, and an empty pick that means \"you asked for none\" would be \
                 indistinguishable from the exit-1 and exit-3 outcomes that mean something.",
        exits: &[],
    },
    Entry {
        id: "usage/bad-host",
        summary: "that is not an address",
        detail: "An address here is an IP literal — 127.0.0.1, 0.0.0.0, ::1, or a tailnet \
                 100.x.y.z — and never a name, so `localhost` is refused and 127.0.0.1 is how \
                 it is spelled. Nothing resolves DNS on the way to a bind: a startup path that \
                 waited on a nameserver would be a new way to hang, and tower.serveHost stays \
                 validatable offline through the same parser its reader runs. `ff tower serve` \
                 takes the address from the first of four lanes that speaks — --host, then \
                 TOWER_HOST, then tower.serveHost, then the compiled 127.0.0.1 — and the \
                 refusal names the lane, which is the one to go fix. Whether this machine has \
                 that address is a later and separate answer, from the socket at bind.",
        exits: &["ff tower serve --host <addr>"],
    },
    Entry {
        id: "usage/bad-port",
        summary: "that is not a port",
        detail: "A port is a number from 0 to 65535, and `ff tower serve` takes one from the \
                 first of four lanes that speaks: --port, then TOWER_PORT, then \
                 tower.servePort in git config, then the compiled default. All four run one \
                 parser, so the refusal reads the same wherever the value came from, and it \
                 names the lane — that is the one to go fix. Whether the port can actually be \
                 had is a later and separate answer, from the socket at bind.",
        exits: &[],
    },
    Entry {
        id: "usage/needs-path",
        summary: "bare `warm` needs a pool root",
        detail: "`bay warm` with no path mints the next slot under the `tower.bays` setting, and \
                 the setting is unset — there is nowhere to mint. Either set the pool root once \
                 and let bare `warm` number the slots, or name the path explicitly this time.",
        exits: &["ff tower config bays <dir>", "ff tower bay warm <path>"],
    },
    Entry {
        id: "usage/needs-message",
        summary: "the verb needs `-m`",
        detail: "`hold` carries a question, `answer` an answer, `comment` a note — all through \
                 `-m`, and an empty one would put a blank line on the permanent record. The flag \
                 is optional in the parser on purpose: a `--json` caller gets this envelope \
                 instead of clap's usage text.",
        exits: &[],
    },
    Entry {
        id: "usage/needs-edit",
        summary: "`edit` has nothing to change",
        detail: "`edit` rewords through its flags — `-s` the subject, `-m` the body or a \
                 comment's text, `--priority`, `--label`, `--skill`, `--bay` the fields — and \
                 none was given, so there is no overlay to write. Any flag alone is a \
                 complete edit; every other field stands unchanged.",
        exits: &[
            "ff tower edit <target> -s <subject>",
            "ff tower edit <target> -m <msg>",
        ],
    },
    Entry {
        id: "usage/subject-on-comment",
        summary: "a comment carries no fields",
        detail: "The target resolved to a comment, and `-s`, `--priority`, `--label`, \
                 `--skill`, and `--bay` name fields only flights carry. A comment is one text, \
                 and `-m` is how it rewords; the flight the comment sits on has the fields, \
                 and editing those is a different target.",
        exits: &["ff tower edit <target> -m <msg>"],
    },
    Entry {
        id: "usage/needs-flight",
        summary: "bare `done` could not derive the flight",
        detail: "`done` with no argument reads the invoking worktree's newest session-tagged \
                 work and finishes the flight that tag names. No tagged work here, or a tag that \
                 names no filed flight, means the derivation has nothing to stand on — name the \
                 flight yourself.",
        exits: &["ff tower done <flight>"],
    },
    Entry {
        id: "usage/empty-subject",
        summary: "the subject is empty",
        detail: "`file`, `decompose`, and `edit -s` trim each subject, and one trimmed to \
                 nothing would put a blank line on the board. The refusal is at write time \
                 because that is when a typo is cheap to fix; the fold stays tolerant of \
                 whatever got into the log anyway.",
        exits: &[],
    },
    Entry {
        id: "usage/empty-procedure",
        summary: "the procedure name is empty",
        detail: "A procedure argument was given but trims to nothing. File bare — one argument, \
                 straight into Triage — or name one of the installed procedures.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "usage/no-parts",
        summary: "there is nothing to split into",
        detail: "`decompose` takes an installed procedure's name, or the sub-flights as \
                 arguments, one subject each — and nothing was given. A decompose with nothing \
                 to mint would append nothing and change nothing, so it refuses rather than \
                 pretending to have worked.",
        exits: &[],
    },
    Entry {
        id: "usage/bad-status",
        summary: "that is not a status",
        detail: "The statuses are a closed vocabulary — triage, waiting, ready, in_progress, \
                 held, done, canceled — and the word given is not among them. The refusal lives \
                 at the verb so the log never has to guess: the wire itself stays a free \
                 string, and a newer tower's value folds through untouched.",
        exits: &[],
    },
    Entry {
        id: "usage/bad-assignee",
        summary: "that is not a lane",
        detail: "The lane is deliberately coarse — `me` or `agent`, plus `none` to clear it — \
                 because whose queue this is in is all the field carries. Which agent actually \
                 flies a flight needs no field: every event carries the byline of whoever \
                 wrote it.",
        exits: &[],
    },
    Entry {
        id: "usage/self-link",
        summary: "a flight cannot depend on itself",
        detail: "`link a b` declares that `a` waits on `b`, and both references resolved to the \
                 same flight. A self-edge would make the flight permanently not-ready — a cycle \
                 of length one — so it is refused at the verb, where the two spellings of one \
                 flight are still visible.",
        exits: &[],
    },
    Entry {
        id: "usage/unknown-key",
        summary: "no setting by that name",
        detail: "Settings live in a typed registry, fufu's model: only declared keys exist, \
                 with or without the `tower.` prefix, case-insensitive. Bare `ff tower config` \
                 lists every key the registry declares, beside its current value and where the \
                 value came from.",
        exits: &["ff tower config"],
    },
    Entry {
        id: "usage/bad-value",
        summary: "the value does not fit the setting",
        detail: "Every setting declares what shape its values take, and the value is validated \
                 against that shape before anything touches disk — a config file never holds a \
                 value tower cannot read back. The refusal names what the setting wants; asking \
                 for the key alone shows its current value and default.",
        exits: &["ff tower config <key>"],
    },
    Entry {
        id: "usage/unknown-error-id",
        summary: "no such error id",
        detail: "`explain` looks ids up in tower's own registry, and this one is not in it. \
                 `--list` shows every id tower can explain. An id another tool raised lives in \
                 that tool's registry — fufu's refusals, for one, answer to `ff explain`.",
        exits: &["ff tower explain --list"],
    },
    Entry {
        id: "flight/not-found",
        summary: "no such flight on the board",
        detail: "The reference parsed, but nothing filed matches it — across every writer's \
                 chain, done flights included. The board shows what is actually filed, in the \
                 same display form the reference grammar accepts. `edit` also takes a \
                 comment's event id, printed on the brief's comment rows; a full id matching \
                 neither a flight nor a comment is this same refusal.",
        exits: &["ff tower"],
    },
    Entry {
        id: "flight/ambiguous",
        summary: "that bare number names more than one flight",
        detail: "Flight numbers are dense per writer, so two writers can both have a #3. A bare \
                 number must match exactly one flight across writers; when it does not, the \
                 refusal lists every candidate in `writer#n` form — typing one of those names \
                 exactly one.",
        exits: &["ff tower"],
    },
    Entry {
        id: "flight/done",
        summary: "the flight is already closed",
        detail: "The lifecycle verbs stop at a closed status — done or canceled: moving, \
                 assigning, holding, or answering a closed flight would write motion onto a \
                 closed record. The log keeps the record — `brief` still reads it whole — and \
                 `comment`, `link`, and `edit` stay permissive on purpose: a note on the \
                 record is fine, and a wrong word in a closed record is exactly what `edit` is \
                 for.",
        exits: &["ff tower brief <flight>"],
    },
    Entry {
        id: "link/exists",
        summary: "the dependency is already declared",
        detail: "This exact edge is already on the board, and declaring it twice would put a \
                 duplicate row on both flights' records. The brief shows each flight's declared \
                 edges in both directions.",
        exits: &["ff tower brief <flight>"],
    },
    Entry {
        id: "hold/exists",
        summary: "the flight is already held",
        detail: "One open question at a time: a second hold would bury the first, and the \
                 refusal quotes the question already standing. Answering releases the hold; \
                 hold again after that if a new question arises.",
        exits: &["ff tower answer <flight> -m <answer>"],
    },
    Entry {
        id: "answer/not-held",
        summary: "no open question to answer",
        detail: "`answer` releases a hold, and this flight has none standing — either it was \
                 never held or the question was already answered. The board's waiting-on-you \
                 section is where open questions live.",
        exits: &["ff tower"],
    },
    Entry {
        id: "status/held",
        summary: "an open question blocks the move",
        detail: "The flight is held on a question, and a status move short of closing it \
                 would bury the question unanswered. `answer` is the release — it clears the \
                 question and sets the flight Ready — while `done` and `cancel` override the \
                 hold, because abandoning the question is deliberate when the flight itself \
                 is over.",
        exits: &["ff tower answer <flight> -m <answer>"],
    },
    Entry {
        id: "bay/pool-root",
        summary: "the pool root could not be prepared",
        detail: "Minting a slot creates and canonicalizes the directory `tower.bays` points at, \
                 and the filesystem said no — a permission, a missing parent, a path that is \
                 not a directory. The message carries the OS's own words; pointing the setting \
                 somewhere writable is the usual fix.",
        exits: &["ff tower config bays <dir>"],
    },
    Entry {
        id: "bay/occupied",
        summary: "a live flight keeps its bay",
        detail: "Releasing a bay tears its worktree down, and a live flight is sitting in it — \
                 releasing anyway would pull the floor out from under work in motion. Finish \
                 the flight first; fufu captures the tree before teardown either way, so \
                 nothing is lost, but the occupancy rule is tower's to keep.",
        exits: &["ff tower done <flight>", "ff tower bay"],
    },
    Entry {
        id: "procedure/not-found",
        summary: "no procedure by that name is installed",
        detail: "Procedures resolve against the installed registry — the built-ins, the user \
                 layer, and the repository's `.tower/procedures/` — and this name is in none of \
                 them. The refusal lists what is installed; `procedures` shows each one in full \
                 and where to fork it.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/invalid",
        summary: "the definition would not parse",
        detail: "TOML that does not parse, a key nothing declares, or a directory that cannot \
                 be read. The message carries the path and toml's own words, which point at \
                 the line — an old-grammar `[[part]]` file lands here too, loudly, rather than \
                 loading as half a definition. Definitions are validated whole at load so a \
                 broken file fails here, not halfway through a filing.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/no-parts",
        summary: "the procedure declares no flights",
        detail: "A procedure is the flights it stamps out — a definition with none has \
                 nothing to file and nothing to hand out. Declare at least one `[[flight]]`, \
                 and remember the last one must be assigned to `me`: every procedure ends \
                 with a human.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/empty-rule",
        summary: "a match rule declares no predicates",
        detail: "A `[[match]]` rule is its predicates — source and event for an adapter's \
                 signal, label, priority, skill, or assignee for what a person files — and \
                 this one declares none, so it could never say what it matches. Give it at \
                 least one predicate, or delete it.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/duplicate-rule",
        summary: "two match rules wear one name",
        detail: "The routing event records which rule fired by name, and two rules under one \
                 name would make that record ambiguous. Rename one of the two; rule names \
                 only need to be unique within their own procedure.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/duplicate-part",
        summary: "two flights wear one id",
        detail: "Flight ids name edges: `after` says which flight precedes which, and a \
                 duplicated id makes that reference ambiguous. Rename one of the two; ids \
                 only need to be unique within their own procedure.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/unknown-after",
        summary: "`after` names a flight that does not exist",
        detail: "An edge points at a flight id the definition never declares — a typo, every \
                 time. The message names the flight carrying the edge and the id it reached \
                 for; the fix is almost always spelling.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/cyclic",
        summary: "`after` closes a cycle",
        detail: "The flight order came back around to itself, so no flight in the cycle could \
                 ever start — each is waiting on the others. The message names a flight on \
                 the cycle; removing or redirecting one edge breaks it.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "procedure/no-human-end",
        summary: "the procedure ends on an agent flight",
        detail: "Every procedure ends with you — the final flight must be assigned to `me`, \
                 so that finished work always crosses a human's desk before it counts as \
                 done. Add a closing flight assigned to `me`, or re-assign the last one.",
        exits: &["ff tower procedures"],
    },
    Entry {
        id: "skill/not-found",
        summary: "no skill by that name is installed",
        detail: "The name is in no layer — built-in, user, or repository. Bare \
                 `ff tower skills` lists what is installed and where each came from, and the \
                 refusal itself names the set.",
        exits: &["ff tower skills"],
    },
    Entry {
        id: "skill/invalid",
        summary: "a skill layer would not read",
        detail: "A directory or file in a skill layer cannot be read, or its bytes are not \
                 UTF-8. The message carries the path and the system's own words. A skill you \
                 cannot see is worse than a refusal, so the layer fails whole rather than \
                 skipping the file.",
        exits: &["ff tower skills"],
    },
    Entry {
        id: "serve/address-in-use",
        summary: "something is already serving on that port",
        detail: "The socket would not bind: another `ff tower serve`, or anything else holding \
                 the number. tower takes no lock of its own — a second server is just another \
                 writer, which the log already handles, and a lock file would go stale on every \
                 Ctrl-C because Drop does not run on a default SIGINT. So the port is the one \
                 conflict worth naming, and the operating system is what names it. Serve \
                 somewhere else, or stop what is there.",
        exits: &["ff tower serve --port <n>"],
    },
    Entry {
        id: "serve/failed",
        summary: "the server would not start",
        detail: "Not the port being taken — something else: a runtime that would not build, a \
                 bind the system refused for its own reason, or an accept loop that stopped \
                 with an error. The message carries the system's own words, which are the whole \
                 of what is known. The common one is a permission denial below port 1024: those \
                 numbers parse fine and fail here, because whether this user may have that port \
                 is the machine's rule rather than tower's. The same id answers an API request \
                 whose fold panicked — the server survives it, and the one request says so — and \
                 covers the change feed's watcher failing to install, which stops startup \
                 because a feed blind to the log's own refs would push stale boards.",
        exits: &["ff tower serve --port <n>"],
    },
    Entry {
        id: "serve/no-such-address",
        summary: "nothing on this machine has that address",
        detail: "The address parsed and the bind was refused: a socket can only listen on an \
                 address one of this machine's interfaces actually holds. Usually a typo, or an \
                 address that belongs to another machine, or an interface that is not up yet — \
                 a tailnet 100.x.y.z with tailscale down does this. `ip addr` (`ifconfig` on \
                 macOS) lists what is available. 127.0.0.1 always works, 0.0.0.0 binds every \
                 interface including the ones that appear later, and reaching this board from \
                 another device needs no wide bind at all if `tailscale serve` is proxying the \
                 loopback socket.",
        exits: &["ff tower serve --host <addr>"],
    },
    Entry {
        id: "update/failed",
        summary: "the update did not complete",
        detail: "Somewhere between the release check, the download, the checksum, and the \
                 atomic swap, a step failed; the message says which. The binary you are running \
                 is untouched — the swap is last and all-or-nothing — so trying again is always \
                 safe.",
        exits: &["ff tower update"],
    },
    Entry {
        id: "update/source-build",
        summary: "this tower was built from source",
        detail: "The self-updater swaps in release binaries, and a source build is not one — \
                 replacing it would silently undo whatever local state made you build from \
                 source. Rebuild through cargo instead, from the same repository.",
        exits: &["cargo install --git https://github.com/tyler-johnson/tower ff-tower-cli"],
    },
    Entry {
        id: "update/homebrew",
        summary: "this tower is Homebrew's to update",
        detail: "The binary lives in a Homebrew cellar, and two updaters disagreeing about one \
                 file is how installs corrupt — brew would overwrite the swap on its next \
                 upgrade anyway. Let brew do it.",
        exits: &["brew upgrade ff-tower"],
    },
    Entry {
        id: "identity/missing",
        summary: "git has no email to file events under",
        detail: "Every event tower writes carries an author, because events belong to a person \
                 — tower's own machinery signs its plumbing commits, but a filing without an \
                 author would be a record nobody made. The same stance as fufu's \
                 `identity/missing`. Set it once globally, or per repository.",
        exits: &["git config user.email <email>"],
    },
    Entry {
        id: "log/contended",
        summary: "another writer holds the tower log",
        detail: "Appends to one writer's chain are serialized, and another process held the \
                 lock through the whole wait — or the compare-and-swap kept losing through \
                 every retry. Both mean the same thing: the other writer is mid-append, and the \
                 verb refuses fast rather than waiting forever. Run it again in a moment.",
        exits: &[],
    },
    Entry {
        id: "log/ref-name",
        summary: "that name cannot be a log ref",
        detail: "Chains live under `refs/tower/log/<author>/<writer>`, and the author or writer \
                 in hand cannot be a ref-name component — git's rules, checked by round-trip \
                 rather than by a list tower keeps. The message says which value and what git \
                 objected to; `git config tower.writer` is where a writer id comes from.",
        exits: &[],
    },
    Entry {
        id: "log/not-tower",
        summary: "a commit on a tower ref is not tower's",
        detail: "Something wrote to `refs/tower/log/` that tower did not, or a payload no \
                 longer decodes. The chain is refused whole rather than skipped past: folding \
                 an unreadable commit into a board would launder it into authored intent. The \
                 message names the commit; inspecting it with git is the way in.",
        exits: &[],
    },
    Entry {
        id: "log/version",
        summary: "the chain is from a newer tower",
        detail: "A commit carries a chain format this binary does not read — someone on this \
                 board writes with a newer tower. The check runs before the payload is touched, \
                 the same discipline as the seam's contract number, and the fix is to upgrade.",
        exits: &["ff tower update"],
    },
    Entry {
        id: "repo/error",
        summary: "git failed underneath tower",
        detail: "Not a decision waiting on you: gix, the config layer, or the store hit a \
                 repository-level failure — a broken ref, a permissions problem, an object that \
                 would not read. The message carries the underlying words, which are the whole \
                 of what is known. If it reproduces, it is worth reporting.",
        exits: &[],
    },
    Entry {
        id: "config/no-global",
        summary: "nowhere to write the global config",
        detail: "`--global` writes to the user-level git config, found through HOME, and HOME \
                 is not set in this environment. Set HOME, or drop `--global` and write the \
                 repository's config instead.",
        exits: &[],
    },
    Entry {
        id: "config/locked",
        summary: "a concurrent git holds the config lock",
        detail: "Writing a setting takes git's own `config.lock`, and another process holds it. \
                 The verb refuses fast rather than waiting it out; the lock is short-lived, so \
                 running the command again usually just works. A stale `.git/config.lock` left \
                 by a crashed process is the exception, and deleting it is safe once nothing is \
                 running.",
        exits: &[],
    },
    Entry {
        id: "ff/not-installed",
        summary: "no `ff` on PATH",
        detail: "tower runs on fufu — every fact about the repository arrives through `ff \
                 <verb> --json` — so a machine without it has nothing for tower to read. fufu \
                 is tower's one runtime dependency, and this is the most likely way a fresh \
                 install fails: install fufu from \
                 https://github.com/tyler-johnson/fufu and the rest follows.",
        exits: &[],
    },
    Entry {
        id: "ff/spawn",
        summary: "`ff` exists and would not start",
        detail: "The binary is on PATH but the launch failed — the OS error in the message says \
                 how: a permission bit, a wrong architecture, an interpreter that is not there. \
                 This is about the file itself, not about fufu's answer; running `ff -v` by \
                 hand reproduces it in isolation.",
        exits: &[],
    },
    Entry {
        id: "ff/contract",
        summary: "fufu speaks a contract tower does not read",
        detail: "Every fufu envelope leads with a contract number, checked before the payload \
                 is touched — a shape tower cannot parse should say so in one line, not surface \
                 as a missing field three levels down. The two tools have drifted; upgrade \
                 whichever is behind.",
        exits: &["ff tower update", "ff update"],
    },
    Entry {
        id: "ff/mismatched",
        summary: "the envelope answered for a different verb",
        detail: "tower asked one verb and the envelope names another. fufu cannot produce this \
                 on its own — it means something on PATH answering to `ff` is not fufu, or a \
                 shim rewrote the call. `which ff` is the first thing to check.",
        exits: &[],
    },
    Entry {
        id: "ff/unparsable",
        summary: "no JSON envelope came back",
        detail: "The spawn ran, but what came back does not parse as fufu's envelope. The \
                 refusal carries what the process actually said, because that is what you debug \
                 from: a usage error on stderr, a shim printing its own banner, an `ff` too old \
                 to have the verb. Running the same `ff` command by hand shows it directly.",
        exits: &[],
    },
];

/// Find an entry by id, or None.
pub fn find(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|entry| entry.id == id)
}

/// The `try:` block a failure prints: what the raise site said, or what
/// the id means when the site said nothing — fufu's `exits_for`, verbatim
/// in spirit.
///
/// Most `CliError::coded` calls pass no exits, and that was never a claim
/// that there is no way out — the way out is a property of the id, and
/// the id has one written down here. Both failure surfaces (`report` and
/// the JSON envelope) read this, and a raise site only carries exits of
/// its own when it knows something the id does not, which is why the
/// narrower list wins when there is one. The last resort is the registry
/// itself: a coded failure with nothing to suggest is still a failure
/// with prose behind it, and naming the lookup beats a dead end. A
/// forwarded fufu refusal falls through to `None` — its exits are fufu's,
/// already carried verbatim, and its prose lives in fufu's registry.
pub fn exits_for(err: &CliError) -> Vec<String> {
    let exits = err.exits();
    if !exits.is_empty() {
        return exits;
    }
    let id = err.id();
    match find(id) {
        Some(entry) if entry.exits.is_empty() => vec![format!("ff tower explain {id}")],
        Some(entry) => entry.exits.iter().map(|exit| (*exit).to_string()).collect(),
        None => Vec::new(),
    }
}

/// One entry, rendered: id line, summary, a blank, the detail wrapped at
/// 80 columns, and the `try:` block when there are exits.
pub fn render(entry: &Entry) -> String {
    let mut out = String::new();
    out.push_str(entry.id);
    out.push('\n');
    out.push_str(entry.summary);
    out.push('\n');
    out.push('\n');
    out.push_str(&wrap(entry.detail, 80));
    if !entry.exits.is_empty() {
        out.push('\n');
        out.push_str("  try:\n");
        for hint in entry.exits {
            out.push_str(&format!("    {hint}\n"));
        }
    }
    out
}

/// Every entry as `id  summary`, ids padded to align — no header, the
/// list is its own explanation.
pub fn render_list() -> String {
    let width = ENTRIES
        .iter()
        .map(|entry| entry.id.len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for entry in ENTRIES {
        out.push_str(&format!("{:<width$}  {}\n", entry.id, entry.summary));
    }
    out
}

/// The refusal for an id the registry does not carry. Exit 2 by the
/// `usage/` rule. The exits meet the asker where they are: the list
/// always; fufu's lookup when the id is slash-shaped, because forwarded
/// fufu ids live in fufu's registry; and the brief when the argument
/// parses as a flight reference — someone reaching for the verb's old
/// meaning.
pub fn unknown_id(id: &str) -> CliError {
    let mut exits = vec!["ff tower explain --list".to_string()];
    if id.contains('/') {
        exits.push(format!("ff explain {id}"));
    }
    if crate::cmd::parse_ref(id).is_ok() {
        exits.push(format!("ff tower brief {id}"));
    }
    CliError::coded(
        "usage/unknown-error-id",
        format!("no such error id: {id}"),
        exits,
    )
}

/// Word-wrap `text` to `width` columns: break at spaces, never mid-word.
fn wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    for word in text.split_whitespace() {
        if col > 0 && col + 1 + word.len() > width {
            out.push('\n');
            col = 0;
        }
        if col > 0 {
            out.push(' ');
            col += 1;
        }
        out.push_str(word);
        col += word.len();
    }
    if col > 0 {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registry_entry_has_prose() {
        let mut seen = Vec::new();
        for entry in ENTRIES {
            assert!(!seen.contains(&entry.id), "duplicate id: {}", entry.id);
            seen.push(entry.id);
            assert!(!entry.summary.is_empty(), "{}: summary is empty", entry.id);
            assert!(!entry.detail.is_empty(), "{}: detail is empty", entry.id);
        }
    }

    /// The registry is a promise, and a promise nothing checks is a
    /// promise that rots. Every id raised anywhere in the workspace must
    /// be explainable, so adding a coded error without an entry fails
    /// here rather than at a user's terminal.
    ///
    /// Tower's ids come from two places fufu's come from one: the
    /// `CliError::coded(` literals, and the `fn id()` match tables that
    /// name wrapped errors, listed by file in `raised_everywhere`. The
    /// tables are scanned by file name,
    /// which is what keeps the walk off `board/doctor.rs` — its check
    /// names wear the same `category/kebab-case` shape and are not
    /// errors. Forwarded fufu refusals never enter either scan:
    /// `error.rs` returns `&refusal.id`, a variable the literal walk
    /// cannot see, which is exactly right — fufu's words, fufu's
    /// registry.
    #[test]
    fn every_raised_id_is_in_the_registry() {
        let mut missing: Vec<(String, String)> = Vec::new();
        let mut found = 0usize;
        for (id, file) in raised_everywhere() {
            found += 1;
            if !ENTRIES.iter().any(|entry| entry.id == id) {
                missing.push((id, file));
            }
        }
        // A walker that silently found nothing would pass this test while
        // checking nothing at all, so it has to prove it read the tree.
        assert!(
            found > 25,
            "only {found} raised ids found — the source walk is broken, not the registry"
        );
        assert!(
            missing.is_empty(),
            "raised ids with no registry entry: {missing:#?}"
        );
    }

    /// Ids the registry carries that nothing raises.
    ///
    /// Each one would be an id tower cannot produce and must explain for
    /// a different reason, stated per entry so a genuinely dead entry
    /// cannot hide behind a habit of adding names here. Empty today:
    /// every entry traces to a `CliError::coded(` call or an `fn id()`
    /// table arm.
    const UNRAISED: &[(&str, &str)] = &[];

    /// The mirror of the guard above, and the reason both ship: the
    /// forward guard catches an id added without prose, and cannot catch
    /// prose left behind by an id that was removed — removing a raise
    /// site only makes its job easier.
    #[test]
    fn every_registry_entry_is_reachable() {
        let raised: Vec<String> = raised_everywhere().into_iter().map(|(id, _)| id).collect();
        assert!(
            raised.len() > 25,
            "only {} raised ids found — the source walk is broken, not the registry",
            raised.len()
        );

        let orphans: Vec<&str> = ENTRIES
            .iter()
            .map(|entry| entry.id)
            .filter(|id| !raised.iter().any(|raised| raised == id))
            .filter(|id| !UNRAISED.iter().any(|(allowed, _)| allowed == id))
            .collect();
        assert!(
            orphans.is_empty(),
            "registry entries nothing raises — delete the prose or keep the raise site: \
             {orphans:#?}"
        );
    }

    /// Every exit string this workspace hands a user, from both places
    /// one can come from: the registry entry, and the raise site that
    /// overrode it. Checked together on purpose — they are the same
    /// promise written twice, "type this next", and a verb renamed out
    /// from under either half fails a user identically.
    #[test]
    fn every_exit_names_live_surface() {
        let mut checked = 0usize;
        for entry in ENTRIES {
            for exit in entry.exits {
                check_exit(exit, &format!("registry entry {}", entry.id));
                checked += 1;
            }
        }
        for file in rust_sources(&crates_root()) {
            let text = std::fs::read_to_string(&file).expect("read source");
            for (id, exit) in raised_exits(&production_source(&text)) {
                check_exit(&exit, &format!("{id} raised in {}", file.display()));
                checked += 1;
            }
        }
        // Same reason the id walks prove they read the tree: a scanner
        // that quietly matched nothing would pass while checking nothing.
        assert!(
            checked > 40,
            "only {checked} exits found — the walk is broken, not the exits"
        );
    }

    /// One exit string, held to what the CLI actually declares.
    ///
    /// Tower's exits are spelled `ff tower …`, so the `ff tower` prefix
    /// strips and the rest parses against tower's own clap tree, with
    /// `<…>` placeholders standing in as values and bare `ff tower`
    /// being the board. `ff …` without `tower` is fufu's surface and
    /// fufu's guards hold it; `git`, `cargo`, and `brew` are the other
    /// tools an exit may legitimately name, and their surfaces are not
    /// ours to check.
    fn check_exit(exit: &str, whose: &str) {
        let tokens = argv(exit);
        let Some(first) = tokens.first() else {
            panic!("{whose}: an empty exit");
        };
        if ["git", "cargo", "brew"].contains(&first.as_str()) {
            return;
        }
        assert_eq!(
            first, "ff",
            "{whose}: `{exit}` names no surface this guard vouches for"
        );
        if tokens.get(1).map(String::as_str) != Some("tower") {
            return;
        }
        let mut argv = vec!["ff-tower".to_string()];
        argv.extend(tokens[2..].iter().cloned());
        if let Err(err) = <crate::cli::Cli as clap::Parser>::try_parse_from(&argv) {
            // Not every non-Ok is a failure: clap reports `-v` and
            // `--help` as errors carrying the text they printed, which is
            // exactly what those exits are for.
            use clap::error::ErrorKind::{DisplayHelp, DisplayVersion};
            assert!(
                matches!(err.kind(), DisplayVersion | DisplayHelp),
                "{whose}: `{exit}` does not parse:\n{err}"
            );
        }
    }

    /// An exit string as argv: whitespace split, `<…>`/`{…}` placeholders
    /// becoming a value — the grammar around them is what is under test.
    fn argv(exit: &str) -> Vec<String> {
        exit.split_whitespace()
            .map(|token| {
                if token.starts_with('<') || token.starts_with('{') {
                    "x".to_string()
                } else {
                    token.to_string()
                }
            })
            .collect()
    }

    /// Every raised id in the workspace, with the file it came from:
    /// `CliError::coded(` first-literals across every crate, plus the
    /// slash-shaped literals inside the wrapped-error `fn id()` tables —
    /// each of which must be the first `fn id(` in its file.
    fn raised_everywhere() -> Vec<(String, String)> {
        let crates = crates_root();
        let mut found = Vec::new();
        for file in rust_sources(&crates) {
            let text = std::fs::read_to_string(&file).expect("read source");
            for id in raised_ids(&production_source(&text)) {
                found.push((id, file.display().to_string()));
            }
        }
        for table in [
            "ff-tower-cli/src/error.rs",
            "ff-tower-serve/src/api.rs",
            "ff-tower-serve/src/error.rs",
            "ff-tower-core/src/board/resolve.rs",
            "ff-tower-core/src/config.rs",
            "ff-tower-core/src/verb/error.rs",
            "ff-tower-core/src/ff/error.rs",
            "ff-tower-core/src/log/error.rs",
            "ff-tower-core/src/procedure/mod.rs",
            "ff-tower-core/src/skill/mod.rs",
        ] {
            let path = crates.join(table);
            let text = std::fs::read_to_string(&path).expect("read id table");
            for id in id_table_ids(&production_source(&text)) {
                found.push((id, path.display().to_string()));
            }
        }
        found
    }

    fn crates_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ is the manifest dir's parent")
            .to_path_buf()
    }

    /// A file with its inline test module cut off. Test modules are
    /// allowed placeholder ids — they exercise the namespace rule, not
    /// the registry — and what marks one is `#[cfg(test)] mod tests`
    /// specifically, fufu's own hard-won lesson about the bare attribute.
    fn production_source(text: &str) -> String {
        let mut out = text;
        for (idx, _) in text.match_indices("#[cfg(test)]") {
            let rest = text[idx + "#[cfg(test)]".len()..].trim_start();
            if rest.starts_with("mod tests") {
                out = &text[..idx];
                break;
            }
        }
        out.to_string()
    }

    /// Every `.rs` file under `dir`, recursively.
    fn rust_sources(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(rust_sources(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                found.push(path);
            }
        }
        found
    }

    /// The first string literal after each `CliError::coded(` — which is
    /// the id, whether the call sits on one line or wraps across several.
    /// The qualified spelling is the marker on purpose: `error.rs`
    /// declares `pub fn coded(` and the definition must not scan as a
    /// raise.
    fn raised_ids(text: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for (idx, _) in text.match_indices("CliError::coded(") {
            let rest = &text[idx..];
            let Some(open) = rest.find('"') else { continue };
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else {
                continue;
            };
            ids.push(after[..close].to_string());
        }
        ids
    }

    /// The slash-shaped id literals inside a file's `fn id()` body — the
    /// match tables that name wrapped errors. Shape-filtered so prose in
    /// a doc string could never scan as an id.
    fn id_table_ids(text: &str) -> Vec<String> {
        let Some(at) = text.find("fn id(") else {
            return Vec::new();
        };
        let rest = &text[at..];
        let Some(open) = rest.find('{') else {
            return Vec::new();
        };
        let Some(end) = call_end_brace(&rest[open + 1..]) else {
            return Vec::new();
        };
        literals(&rest[open + 1..open + 1 + end])
            .into_iter()
            .filter(|lit| {
                lit.matches('/').count() == 1
                    && !lit.starts_with('/')
                    && !lit.ends_with('/')
                    && lit
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '-' || c == '/')
            })
            .collect()
    }

    /// The exits each `CliError::coded(` call passes, paired with its id:
    /// the call's last `vec![` is the exits argument, and each element's
    /// *first* literal is one exit — a `format!` template counts whole,
    /// because the placeholder grammar is still what the user is shown. A
    /// call passing a variable instead of a `vec![` is simply not seen,
    /// the same under-inclusion fufu's scanner accepts.
    fn raised_exits(text: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for (idx, _) in text.match_indices("CliError::coded(") {
            let body = &text[idx + "CliError::coded(".len()..];
            let Some(end) = call_end(body) else { continue };
            let body = &body[..end];
            let Some(id) = literals(body).into_iter().next() else {
                continue;
            };
            let Some(vec_at) = body.rfind("vec![") else {
                continue;
            };
            for element in elements(&body[vec_at + "vec![".len()..]) {
                if let Some(exit) = literals(&element).into_iter().next() {
                    found.push((id.clone(), exit));
                }
            }
        }
        found
    }

    /// A `vec![…]` body split into its elements, on commas that sit
    /// outside every bracket and every string.
    fn elements(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = String::new();
        let mut depth = 0i32;
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    current.push(ch);
                    while let Some(c) = chars.next() {
                        current.push(c);
                        if c == '\\' {
                            if let Some(escaped) = chars.next() {
                                current.push(escaped);
                            }
                        } else if c == '"' {
                            break;
                        }
                    }
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' | '}' => {
                    depth -= 1;
                    current.push(ch);
                }
                ']' if depth == 0 => break,
                ']' => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if depth == 0 => out.push(std::mem::take(&mut current)),
                _ => current.push(ch),
            }
        }
        if !current.trim().is_empty() {
            out.push(current);
        }
        out
    }

    /// Offset of the `)` closing a call whose `(` was just consumed, with
    /// string literals skipped so a paren inside prose does not close it.
    fn call_end(text: &str) -> Option<usize> {
        delimited_end(text, b'(', b')')
    }

    /// The same, for a `{` already consumed.
    fn call_end_brace(text: &str) -> Option<usize> {
        delimited_end(text, b'{', b'}')
    }

    fn delimited_end(text: &str, open: u8, close: u8) -> Option<usize> {
        let bytes = text.as_bytes();
        let mut depth = 1usize;
        let mut i = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += if bytes[i] == b'\\' { 2 } else { 1 };
                    }
                }
                b if b == open => depth += 1,
                b if b == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Every string literal in `text`, escapes resolved to the character.
    fn literals(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != '"' {
                i += 1;
                continue;
            }
            i += 1;
            let mut lit = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                lit.push(chars[i]);
                i += 1;
            }
            i += 1;
            out.push(lit);
        }
        out
    }
}

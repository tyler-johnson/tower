//! The command line. Bare `ff tower` is the board; `board` is an alias for
//! the bare form, so the muscle-memory spelling and the explicit one agree
//! by construction.
//!
//! `-m` follows fufu exactly: short-only, always `Option<String>`, and a
//! missing-but-required message is a coded refusal from the verb rather
//! than a clap `required = true` — a `--json` caller gets an envelope,
//! not clap's usage text.

use clap::{Parser, Subcommand};

use crate::help;

/// The name the tool has, as opposed to what it is typed as. The version
/// is the one place the project name is worth a line: it is what somebody
/// searches for — it matches the release titles and the README — and
/// `ff-tower` is dispatch plumbing, not a name.
pub const NAME: &str = "tower";

/// What `ff tower -v` and `ff tower version` both print: the release, the
/// commit it was built from, and the project's home under it. Both
/// spellings go through the verb, which prepends the name by hand — clap
/// no longer prints this. One constant, so the spellings cannot answer
/// the same question differently.
///
/// The URL comes from the manifest rather than from a literal here — there
/// is already one place that records where this lives, and a second would
/// be a place to forget.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    env!("TOWER_BUILD_INFO"),
    "\n",
    env!("CARGO_PKG_REPOSITORY")
);

#[derive(Parser)]
#[command(
    name = "ff-tower",
    // Pinned, not derived from argv[0]: without it usage lines read
    // `Usage: ff-tower brief`, a spelling nobody types — the binary is
    // reached through fufu's dispatch, and what you type is `ff tower`.
    bin_name = "ff tower",
    about = "tower: the board over fufu",
    long_about = help::ROOT,
    after_long_help = help::ROOT_EXAMPLES,
    version = VERSION,
    // Declared by hand below, for the short letter: clap's own flag is `-V`.
    disable_version_flag = true
)]
pub struct Cli {
    /// Emit tower's machine envelope instead of the human render.
    #[arg(long, global = true)]
    pub json: bool,
    // `-v`, not clap's default `-V`. tower has no verbose flag to reserve
    // the lowercase letter for — verbosity here is `--json` or a different
    // verb — so the shifted spelling would buy nothing and cost every
    // person who typed the lowercase one first. `-V` is gone rather than
    // kept as an alias: a second spelling for a one-line answer is surface
    // with no reader.
    /// Print the version and the commit it was built from
    #[arg(short = 'v', long = "version", action = clap::ArgAction::SetTrue)]
    pub version: bool,
    /// Retired: the version flag is lowercase `-v`
    ///
    /// Declared only to stay hidden: `-V` is what almost every other tool
    /// spells this, so typing it is a question rather than a typo, and
    /// clap's bare "unexpected argument" answers a different one.
    #[arg(short = 'V', hide = true, action = clap::ArgAction::SetTrue)]
    pub version_shouted: bool,
    /// The board's own flags, declared here as well as on the variant so
    /// bare `ff tower --closed 7d` parses: bare `ff tower` is the board,
    /// but it never constructs `Command::Board` from a parse, so a flag
    /// declared on the variant alone would be typeable one way and not
    /// the other. Not `global`: it would then be in every verb's help,
    /// and it belongs to one.
    #[command(flatten)]
    pub board: BoardArgs,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The board's flags, shared by the bare form and the `board` alias so
/// the two agree by construction.
#[derive(clap::Args, Default)]
pub struct BoardArgs {
    // A string, not a parsed value, for the same reason `--port` is one:
    // a bad value has to be the verb's own coded refusal, so a `--json`
    // caller gets an envelope instead of clap's usage text and its exit
    // 2. `num_args` with the missing value makes bare `--closed` mean
    // everything.
    /// How much of the closed group to show: a count, a span like 7d,
    /// `all`, or `none`. The three newest when unsaid.
    #[arg(long, value_name = "n|span", num_args = 0..=1, default_missing_value = "all")]
    pub closed: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// The board — what is filed, what is moving, what is stuck.
    #[command(long_about = help::ROOT, after_long_help = help::ROOT_EXAMPLES)]
    Board {
        #[command(flatten)]
        args: BoardArgs,
    },
    /// Claim the next ready flight, or a set of `k` that collide with
    /// neither each other nor anything already flying.
    #[command(long_about = help::NEXT, after_long_help = help::NEXT_EXAMPLES)]
    Next {
        /// How many flights to hand out; one when unsaid.
        #[arg(short = 'n', value_name = "k")]
        count: Option<usize>,
        /// The same computation with no claim written.
        #[arg(long)]
        peek: bool,
    },
    /// Everything known about one flight, for whoever picks it up.
    #[command(long_about = help::BRIEF, after_long_help = help::BRIEF_EXAMPLES)]
    Brief {
        /// The flight to brief — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
    },
    /// File a flight onto the board — bare into Triage, or under a
    /// procedure.
    #[command(long_about = help::FILE, after_long_help = help::FILE_EXAMPLES)]
    File {
        /// A procedure then a subject, or a subject alone — two words
        /// name a procedure filing, one is a bare one; the split lives
        /// in the verb, so a missing subject is a coded refusal.
        #[arg(value_name = "procedure|subject")]
        first: Option<String>,
        /// The subject, when the first argument named a procedure.
        #[arg(value_name = "subject")]
        second: Option<String>,
        /// The body — detail beyond the subject.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
        /// The priority the flight is born with.
        #[arg(short = 'p', long = "priority", value_name = "priority")]
        priority: Option<String>,
        /// A label; repeat the flag for more than one.
        #[arg(long = "label", value_name = "label")]
        labels: Vec<String>,
        /// The skill the flight is flown with.
        #[arg(long = "skill", value_name = "name")]
        skill: Option<String>,
        /// The lane — me or agent.
        #[arg(long = "assignee", value_name = "lane")]
        assignee: Option<String>,
        /// `warm` builds a tree ahead of whoever flies it.
        #[arg(long = "bay", value_name = "bay")]
        bay: Option<String>,
    },
    /// A note on a flight's record.
    #[command(long_about = help::COMMENT, after_long_help = help::COMMENT_EXAMPLES)]
    Comment {
        /// The flight to comment on — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// The note.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Reword a flight or a comment — an overlay on the record; the log
    /// keeps every prior word.
    #[command(long_about = help::EDIT, after_long_help = help::EDIT_EXAMPLES)]
    Edit {
        /// The target — a flight (number, `writer#n`, event id) or a
        /// comment's event id.
        #[arg(value_name = "target")]
        target: String,
        /// The new subject; flights only.
        #[arg(short = 's', long = "subject", value_name = "subject")]
        subject: Option<String>,
        /// The new body — or the comment's new text.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
        /// The new priority; flights only.
        #[arg(short = 'p', long = "priority", value_name = "priority")]
        priority: Option<String>,
        /// The new label set, wholesale; repeat the flag for more than
        /// one.
        #[arg(long = "label", value_name = "label")]
        labels: Vec<String>,
        /// The new skill; flights only.
        #[arg(long = "skill", value_name = "name")]
        skill: Option<String>,
        /// The new bay ask; flights only.
        #[arg(long = "bay", value_name = "bay")]
        bay: Option<String>,
    },
    /// Declare a dependency: `a` depends on `b`.
    #[command(long_about = help::LINK, after_long_help = help::LINK_EXAMPLES)]
    Link {
        /// The flight that depends.
        #[arg(value_name = "a")]
        a: String,
        /// The flight it depends on.
        #[arg(value_name = "b")]
        b: String,
    },
    /// Split a flight into sub-flights: by hand with one subject per
    /// argument, or under a procedure whose definition mints them.
    #[command(long_about = help::DECOMPOSE, after_long_help = help::DECOMPOSE_EXAMPLES)]
    Decompose {
        /// The flight to split — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// One installed procedure's name, or the sub-flights' subjects.
        #[arg(value_name = "procedure|part")]
        parts: Vec<String>,
    },
    /// What procedures are installed, and where to fork one.
    #[command(long_about = help::PROCEDURES, after_long_help = help::PROCEDURES_EXAMPLES)]
    Procedures {
        /// One procedure, in full; the whole list when unsaid.
        #[arg(value_name = "name")]
        name: Option<String>,
    },
    /// What skills are installed, or one printed raw to fork or pipe.
    #[command(long_about = help::SKILLS, after_long_help = help::SKILLS_EXAMPLES)]
    Skills {
        /// One skill, raw; the whole list when unsaid.
        #[arg(value_name = "name")]
        name: Option<String>,
    },
    /// Set a flight's lane: me, agent, or none to clear it.
    #[command(long_about = help::ASSIGN, after_long_help = help::ASSIGN_EXAMPLES)]
    Assign {
        /// The flight — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// The lane: me, agent, or none.
        #[arg(value_name = "lane")]
        lane: String,
    },
    /// Move a flight to a status.
    #[command(long_about = help::STATUS, after_long_help = help::STATUS_EXAMPLES)]
    Status {
        /// The flight — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// Where it moves: triage, waiting, ready, in_progress, held,
        /// done, or canceled.
        #[arg(value_name = "status")]
        status: String,
    },
    /// Cancel a flight — off the board without the finish.
    #[command(long_about = help::CANCEL, after_long_help = help::CANCEL_EXAMPLES)]
    Cancel {
        /// The flight to cancel.
        #[arg(value_name = "flight")]
        flight: String,
        /// Why — stored on the move.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Stop a flight with a question attached — bay warm, exit 3.
    #[command(long_about = help::HOLD, after_long_help = help::HOLD_EXAMPLES)]
    Hold {
        /// The flight to hold.
        #[arg(value_name = "flight")]
        flight: String,
        /// The question.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Answer the open question and release the hold.
    #[command(long_about = help::ANSWER, after_long_help = help::ANSWER_EXAMPLES)]
    Answer {
        /// The held flight.
        #[arg(value_name = "flight")]
        flight: String,
        /// The answer.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Finish a flight — off the board, on the record.
    #[command(long_about = help::DONE, after_long_help = help::DONE_EXAMPLES)]
    Done {
        /// The flight to finish; the invoking worktree's flight when
        /// unsaid, derived from its newest session-tagged work.
        #[arg(value_name = "flight")]
        flight: Option<String>,
    },
    /// Look up an error id and see what it means.
    #[command(long_about = help::EXPLAIN, after_long_help = help::EXPLAIN_EXAMPLES)]
    Explain {
        /// The error id to look up.
        #[arg(value_name = "id")]
        id: Option<String>,
        /// List every error id tower knows.
        #[arg(long)]
        list: bool,
    },
    /// Settings, on fufu's typed-registry model: bare lists them, a key
    /// gets, key + value sets, `--unset` returns to the default.
    #[command(long_about = help::CONFIG, after_long_help = help::CONFIG_EXAMPLES)]
    Config {
        /// The setting — `bays`, or `tower.bays`; the whole list when
        /// unsaid.
        #[arg(value_name = "key")]
        key: Option<String>,
        /// The new value; validated before anything touches disk.
        #[arg(value_name = "value", conflicts_with = "unset")]
        value: Option<String>,
        /// Remove the setting — back to the default.
        #[arg(long, requires = "key")]
        unset: bool,
        /// The global git config — every repo — instead of this one.
        #[arg(long)]
        global: bool,
    },
    /// The pool: what is bootstrapped, what is occupied, what is free.
    #[command(long_about = help::BAY, after_long_help = help::BAY_EXAMPLES)]
    Bay {
        #[command(subcommand)]
        action: Option<BayAction>,
    },
    /// Which tower this is, and whether it is the current one.
    #[command(long_about = help::VERSION, after_long_help = help::VERSION_EXAMPLES)]
    Version,
    /// Download the latest release and replace this binary.
    #[command(long_about = help::UPDATE, after_long_help = help::UPDATE_EXAMPLES)]
    Update {
        /// Refresh the update cache only (used by the background check).
        #[arg(long)]
        check: bool,
    },
    /// Stale bays and drift — doctor observes and complains, never
    /// enforces.
    #[command(long_about = help::DOCTOR, after_long_help = help::DOCTOR_EXAMPLES)]
    Doctor,
    /// Run tower's standing process: a server the browser board mounts
    /// into, until Ctrl-C.
    #[command(long_about = help::SERVE, after_long_help = help::SERVE_EXAMPLES)]
    Serve {
        /// The address to bind; TOWER_HOST, then tower.serveHost, then
        /// 127.0.0.1.
        ///
        /// An IP literal and never a name, for the same reason the
        /// setting is one: no DNS in the startup path, and a stored value
        /// stays validatable offline. A string, not an `IpAddr`, so a bad
        /// value is the verb's own coded refusal rather than clap's.
        #[arg(long, value_name = "addr")]
        host: Option<String>,

        /// The port to bind; TOWER_PORT, then tower.servePort, then 7420.
        ///
        /// A string, not a `u16`, for the same reason `-m` is an
        /// `Option<String>`: a bad value has to be the verb's own coded
        /// refusal, so a `--json` caller gets an envelope instead of
        /// clap's usage text and its exit 2.
        #[arg(long, value_name = "n")]
        port: Option<String>,
    },
}

/// The ambient lanes: what rides an invocation besides the verb itself.
/// tower has three — fufu carries capture and trim as well, but tower
/// has none of those, so the table is the passive update check, its
/// voice, and the lazy pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lanes {
    /// Refresh the update cache in the background, and let an auto-install fire.
    pub update: bool,
    /// Print the "vX.Y.Z available" line on stderr when one is pending.
    pub notice: bool,
    /// Run the lazy pass before the verb, so its own fold sees the
    /// conclusions — `next` pulls a freshly routed flight.
    pub pass: bool,
}

impl Command {
    /// One exhaustive table, fufu's discipline: a verb added without
    /// deciding its lanes is a compile error rather than a verb that
    /// silently never learns about releases or runs the pass.
    pub fn lanes(&self) -> Lanes {
        match self {
            // The detached child must not recurse into another check.
            Command::Update { .. } => Lanes {
                update: false,
                notice: false,
                pass: false,
            },
            // A process that runs for hours is the wrong place for a
            // lane that fires once per invocation: nothing should spawn
            // update children at startup, a notice printed at shutdown
            // would report a release that landed hours ago, and serve
            // runs the pass in its own refold pipeline instead.
            Command::Serve { .. } => Lanes {
                update: false,
                notice: false,
                pass: false,
            },
            // The quiet verbs ride the check but suppress the generic
            // notice — each has its own voice: doctor's `tower/update`
            // row, version's dim "available" line. Neither runs the
            // pass: doctor observes and never writes, and version reads
            // no board.
            Command::Doctor | Command::Version => Lanes {
                update: true,
                notice: false,
                pass: false,
            },
            // Off the board entirely — nothing these answer changes
            // with the pass, and `procedures` must keep refusing a
            // broken rule file on its own path, loudly.
            Command::Explain { .. }
            | Command::Config { .. }
            | Command::Procedures { .. }
            | Command::Skills { .. } => Lanes {
                update: true,
                notice: true,
                pass: false,
            },
            Command::Board { .. }
            | Command::Next { .. }
            | Command::Brief { .. }
            | Command::File { .. }
            | Command::Comment { .. }
            | Command::Edit { .. }
            | Command::Link { .. }
            | Command::Decompose { .. }
            | Command::Assign { .. }
            | Command::Status { .. }
            | Command::Cancel { .. }
            | Command::Hold { .. }
            | Command::Answer { .. }
            | Command::Done { .. }
            | Command::Bay { .. } => Lanes {
                update: true,
                notice: true,
                pass: true,
            },
        }
    }
}

/// The pool's three verbs; bare `ff tower bay` is the list, the same
/// optional-subcommand mechanism as bare `ff tower` being the board.
#[derive(Subcommand)]
pub enum BayAction {
    /// Every bay: id, branch, and the live flight sitting in it.
    #[command(long_about = help::BAY_LIST, after_long_help = help::BAY_LIST_EXAMPLES)]
    List,
    /// Warm a bay — `ff worktree add`, so the chain floor is laid before
    /// the first command runs in it.
    #[command(long_about = help::BAY_WARM, after_long_help = help::BAY_WARM_EXAMPLES)]
    Warm {
        /// Where to put it; a relative path resolves against the
        /// repository, not the shell's directory. Minted under
        /// `tower.bays` when unsaid.
        #[arg(value_name = "path")]
        path: Option<String>,
        /// The branch it stands on — a new one named after the directory
        /// when unsaid.
        #[arg(value_name = "branch")]
        branch: Option<String>,
    },
    /// Release a bay — refused while a live flight sits in it; fufu
    /// captures the tree before teardown either way.
    #[command(long_about = help::BAY_RELEASE, after_long_help = help::BAY_RELEASE_EXAMPLES)]
    Release {
        /// The bay, by id or by path.
        #[arg(value_name = "bay")]
        bay: String,
    },
}

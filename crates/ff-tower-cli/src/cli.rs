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
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// The board — what is filed, what is moving, what is stuck.
    #[command(long_about = help::ROOT, after_long_help = help::ROOT_EXAMPLES)]
    Board,
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
    /// File a flight onto the board.
    #[command(long_about = help::FILE, after_long_help = help::FILE_EXAMPLES)]
    File {
        /// What the flight is about.
        #[arg(value_name = "subject")]
        subject: String,
        /// The body — detail beyond the subject.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
        /// The procedure to file under; `open` is the unclassified default.
        #[arg(short = 'p', long = "procedure", value_name = "name")]
        procedure: Option<String>,
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
    /// Split a flight into parts: each part files as a flight, and the
    /// parent waits on all of them.
    #[command(long_about = help::DECOMPOSE, after_long_help = help::DECOMPOSE_EXAMPLES)]
    Decompose {
        /// The flight to split — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// The parts, one subject each.
        #[arg(value_name = "part")]
        parts: Vec<String>,
    },
    /// What procedures are installed, and where to fork one.
    #[command(long_about = help::PROCEDURES, after_long_help = help::PROCEDURES_EXAMPLES)]
    Procedures {
        /// One procedure, in full; the whole list when unsaid.
        #[arg(value_name = "name")]
        name: Option<String>,
    },
    /// The unclassified pile, or route one flight to a procedure.
    #[command(long_about = help::TRIAGE, after_long_help = help::TRIAGE_EXAMPLES)]
    Triage {
        /// The flight to route — a number, `writer#n`, or the event id;
        /// the pile when unsaid.
        #[arg(value_name = "flight")]
        flight: Option<String>,
        /// The procedure it routes to.
        #[arg(short = 'p', long = "procedure", value_name = "name")]
        procedure: Option<String>,
        /// Why it routed there.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Claim one specific flight, out of order.
    #[command(long_about = help::CLAIM, after_long_help = help::CLAIM_EXAMPLES)]
    Claim {
        /// The flight to claim — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
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
}

/// The ambient lanes: what rides an invocation besides the verb itself.
/// tower has two — fufu carries capture and trim as well, but tower has
/// neither, so the table is the passive update check and its voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lanes {
    /// Refresh the update cache in the background, and let an auto-install fire.
    pub update: bool,
    /// Print the "vX.Y.Z available" line on stderr when one is pending.
    pub notice: bool,
}

impl Command {
    /// One exhaustive table, fufu's discipline: a verb added without
    /// deciding its lanes is a compile error rather than a verb that
    /// silently never learns about releases.
    pub fn lanes(&self) -> Lanes {
        match self {
            // The detached child must not recurse into another check.
            Command::Update { .. } => Lanes {
                update: false,
                notice: false,
            },
            // The quiet verbs ride the check but suppress the generic
            // notice — each has its own voice: doctor's `tower/update`
            // row, version's dim "available" line.
            Command::Doctor | Command::Version => Lanes {
                update: true,
                notice: false,
            },
            Command::Board
            | Command::Next { .. }
            | Command::Brief { .. }
            | Command::File { .. }
            | Command::Comment { .. }
            | Command::Edit { .. }
            | Command::Link { .. }
            | Command::Decompose { .. }
            | Command::Procedures { .. }
            | Command::Triage { .. }
            | Command::Claim { .. }
            | Command::Hold { .. }
            | Command::Answer { .. }
            | Command::Done { .. }
            | Command::Explain { .. }
            | Command::Config { .. }
            | Command::Bay { .. } => Lanes {
                update: true,
                notice: true,
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

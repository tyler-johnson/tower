//! The command line. Bare `ff tower` is the board; `board` is an alias for
//! the bare form, so the muscle-memory spelling and the explicit one agree
//! by construction.
//!
//! `-m` follows fufu exactly: short-only, always `Option<String>`, and a
//! missing-but-required message is a coded refusal from the verb rather
//! than a clap `required = true` — a `--json` caller gets an envelope,
//! not clap's usage text.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ff-tower", about = "tower: the board over fufu", version)]
pub struct Cli {
    /// Emit tower's machine envelope instead of the human render.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// The board — what is filed, what is moving, what is stuck.
    Board,
    /// Claim the next ready flight, or a set of `k` that collide with
    /// neither each other nor anything already flying.
    Next {
        /// How many flights to hand out; one when unsaid.
        #[arg(short = 'n', value_name = "k")]
        count: Option<usize>,
        /// The same computation with no claim written.
        #[arg(long)]
        peek: bool,
    },
    /// Everything known about one flight, for whoever picks it up.
    Brief {
        /// The flight to brief — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
    },
    /// File a flight onto the board.
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
    Comment {
        /// The flight to comment on — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// The note.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Declare a dependency: `a` depends on `b`.
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
    Decompose {
        /// The flight to split — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
        /// The parts, one subject each.
        #[arg(value_name = "part")]
        parts: Vec<String>,
    },
    /// What procedures are installed, and where to fork one.
    Procedures {
        /// One procedure, in full; the whole list when unsaid.
        #[arg(value_name = "name")]
        name: Option<String>,
    },
    /// The unclassified pile, or route one flight to a procedure.
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
    Claim {
        /// The flight to claim — a number, `writer#n`, or the event id.
        #[arg(value_name = "flight")]
        flight: String,
    },
    /// Stop a flight with a question attached — bay warm, exit 3.
    Hold {
        /// The flight to hold.
        #[arg(value_name = "flight")]
        flight: String,
        /// The question.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Answer the open question and release the hold.
    Answer {
        /// The held flight.
        #[arg(value_name = "flight")]
        flight: String,
        /// The answer.
        #[arg(short = 'm', value_name = "msg")]
        message: Option<String>,
    },
    /// Finish a flight — off the board, on the record.
    Done {
        /// The flight to finish; the invoking worktree's flight when
        /// unsaid, derived from its newest session-tagged work.
        #[arg(value_name = "flight")]
        flight: Option<String>,
    },
    /// The pool: what is bootstrapped, what is occupied, what is free.
    Bay {
        #[command(subcommand)]
        action: Option<BayAction>,
    },
}

/// The pool's three verbs; bare `ff tower bay` is the list, the same
/// optional-subcommand mechanism as bare `ff tower` being the board.
#[derive(Subcommand)]
pub enum BayAction {
    /// Every bay: id, branch, and the live flight sitting in it.
    List,
    /// Warm a bay — `ff worktree add`, so the chain floor is laid before
    /// the first command runs in it.
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
    Release {
        /// The bay, by id or by path.
        #[arg(value_name = "bay")]
        bay: String,
    },
}

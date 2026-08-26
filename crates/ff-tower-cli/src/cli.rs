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
        /// The flight to brief, `<writer>.<seq>`.
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
        /// The flight to comment on, `<writer>.<seq>`.
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
    /// Claim one specific flight, out of order.
    Claim {
        /// The flight to claim, `<writer>.<seq>`.
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
        /// The flight to finish; naming the current one arrives with bays.
        #[arg(value_name = "flight")]
        flight: Option<String>,
    },
}

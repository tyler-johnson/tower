//! The notice a wired client puts in front of an agent at every context
//! boundary, and the guards that keep it true.
//!
//! One text feeds every client. What differs per vendor is only how it is
//! delivered — plain stdout for Claude and Codex, a JSON field for Gemini
//! and Cursor — which is why the envelope is the adapter's job and this is
//! not. What differs per repository is one line at the end: how much is
//! ready, or that nothing is filed here yet.
//!
//! The guards at the bottom cover the shipped skills too. Both are prose
//! an agent reads as instructions, both rot the same silent way, and the
//! skills are the larger surface by an order of magnitude — so the check
//! that every command in them is one the CLI still takes belongs to both.

use std::io::{IsTerminal, Read};
use std::path::Path;

use serde::Deserialize;

use crate::error::CliError;
use atc_core::board;
use atc_core::log::Store;

/// What the agent is told at every context boundary, when a client's
/// session-start event gives tower somewhere to put it.
///
/// This is the always-on contract and nothing more: what tower is, the
/// four gestures of the loop, and where the authority is. Everything past
/// it — the model, naming, filing, the machine surface — lives in the
/// shipped `tower` skill (`integ/skill.md`), which costs nothing until a
/// client decides it is wanted. The two are budgeted differently on
/// purpose, and that is the whole reason the split exists.
///
/// Every command here is real and spelled the way the CLI takes it — a
/// retired or mistyped form teaches the agent to fail. Keep it short: this
/// is context the agent pays for on every session.
///
/// Every line is quoted from the CLI, and each source carries a matching
/// `// agent notice quotes this` marker in cli.rs — retiring a verb or
/// renaming a flag there is an edit here too. Adding a command to this text
/// means adding that marker at its definition, so the trail stays two-way:
/// `grep -rn "agent notice" crates/atc-cli/src`.
pub const NOTICE: &str = "\
tower (`atc`) keeps this repository's board. A flight is one unit of work for a person or \
an agent, stored in the repository beside history and moved from the shell. tower runs \
nothing: you pull work, fly it, and report back.

`atc` shows the board. `atc next` claims the next ready flight for you and names the skill \
it is flown with. `atc brief <flight>` is everything on record about one. \
`atc hold <flight> -m \"<question>\"` stops on a question only a person can answer. \
`atc done <flight>` finishes.

Every verb's own `--help` is the authority on it.
";

/// The one line that is this repository's: nothing filed, nothing ready,
/// or how many are.
pub fn status_line(ready: usize, filed: usize) -> String {
    if filed == 0 {
        return "Nothing filed here yet. Run `atc`.".to_string();
    }
    match ready {
        0 => "Nothing ready. Run `atc`.".to_string(),
        1 => "1 flight ready. Run `atc`.".to_string(),
        n => format!("{n} flights ready. Run `atc`."),
    }
}

/// The whole briefing: the notice, then the status line.
pub fn text(ready: usize, filed: usize) -> String {
    format!("{NOTICE}\n{}", status_line(ready, filed))
}

/// The two counts the status line is made of, folded from the store the
/// directory belongs to.
pub struct Counts {
    pub ready: usize,
    pub filed: usize,
}

/// The pipeline is the fold alone — no gather, no spawn beyond the store
/// — because the question is the board's: how much is ready.
pub fn counts(cwd: &Path) -> Result<Counts, CliError> {
    let store = Store::open(cwd)?;
    let fold = board::fold(&store.read_all()?);
    Ok(Counts {
        ready: fold
            .flights
            .iter()
            .filter(|flight| flight.status == "ready")
            .count(),
        filed: fold.flights.len(),
    })
}

// ---- the payload -----------------------------------------------------------

/// A payload larger than this is dropped rather than read into memory.
const MAX_PAYLOAD: u64 = 8 * 1024 * 1024;

/// What a client's session-start payload says that tower reads: where the
/// session is, and which session it is. Every field defaults, because a
/// payload tower cannot parse still has a session in front of it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Payload {
    pub cwd: String,
    /// Parsed and kept for the callsign line to come; nothing reads it yet.
    #[allow(dead_code)]
    pub session_id: String,
}

impl Payload {
    /// The directory the session is in. The payload's when it names one,
    /// else the process's own — a `sessionStart` with no cwd is the
    /// client's working directory, which is where the session started.
    pub fn cwd(&self) -> std::path::PathBuf {
        if self.cwd.is_empty() {
            std::env::current_dir().unwrap_or_default()
        } else {
            std::path::PathBuf::from(&self.cwd)
        }
    }
}

/// The client's payload off stdin, capped. Read only when stdin is not a
/// terminal, so a person typing the verb at a prompt is never left blocked
/// on input they were not asked for. Unparseable or empty is the default
/// payload, not a failure.
pub fn read_payload() -> Payload {
    if std::io::stdin().is_terminal() {
        return Payload::default();
    }
    let mut buf = Vec::new();
    if std::io::stdin()
        .lock()
        .take(MAX_PAYLOAD + 1)
        .read_to_end(&mut buf)
        .is_err()
        || buf.len() as u64 > MAX_PAYLOAD
    {
        return Payload::default();
    }
    serde_json::from_slice(&buf).unwrap_or_default()
}

// ---- the notice is a contract with the CLI ---------------------------------

/// `NOTICE` and the shipped skills are prose an agent reads as
/// instructions, so they rot in a way the compiler cannot see: a retired
/// verb or a renamed flag still reads fine and simply teaches the agent to
/// fail. These guards make it fail here instead — clap is the authority on
/// what either text is allowed to say. The extractors are `help.rs`'s, so
/// the pages and the notice are held to one reading of the tree.
#[cfg(test)]
mod tests {
    use super::{NOTICE, status_line};
    use crate::help::guard::{check, quoted, tree};
    use crate::integ::skill::SKILLS;

    /// cli.rs as text, for the marker trail. Read as source rather than
    /// through clap because a comment is exactly what clap discards.
    const CLI_SRC: &str = include_str!("../cli.rs");
    const MARKER: &str = "// agent notice quotes this";

    /// The notice and every shape of its status line, which together are
    /// the text a wired session reads.
    fn briefing() -> String {
        format!(
            "{NOTICE}\n{}\n{}\n{}\n{}",
            status_line(0, 0),
            status_line(0, 3),
            status_line(1, 3),
            status_line(2, 3)
        )
    }

    /// The guard that rotted spellings need. Parsing alone is not enough:
    /// retired surface is *declared*, hidden, so that typing it reaches an
    /// answer — it parses and then refuses. So hidden is disqualifying
    /// here, not just unknown; `check` already says so.
    #[test]
    fn only_live_documented_surface() {
        let root = tree();
        let commands = quoted(&briefing());
        assert!(
            commands.len() >= 5,
            "the notice stopped teaching verbs: {commands:?}"
        );
        for tokens in &commands {
            check(&root, tokens, "notice");
        }
    }

    /// The other half of the trail: a command in the notice has a marker at
    /// its definition, so whoever retires it there sees this text named.
    #[test]
    fn every_quoted_command_is_marked_in_cli_rs() {
        let markers: Vec<&str> = CLI_SRC
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with(MARKER))
            .collect();
        assert!(
            markers.len() >= 4,
            "the marker trail is gone from cli.rs; the notice has nothing pointing at it"
        );
        for tokens in quoted(&briefing()) {
            // Bare `atc` — the notice names the tool before it names a verb.
            let Some(verb) = tokens.get(1) else { continue };
            assert!(
                markers.iter().any(|m| m.contains(&format!("`atc {verb}"))),
                "the notice teaches `atc {verb}` but no `{MARKER}` in cli.rs claims it: \
                 add one at its definition"
            );
        }
    }

    /// The notice is context the agent pays for on every session. There is
    /// no exact token count to assert, so the budget is bytes: roughly 170
    /// tokens, and a rewrite that doubles it has to say so here. The number
    /// is fufu's, set when its skill took the advanced surface off the
    /// notice; growing it back is choosing to charge every session for
    /// something one session in twenty needs.
    #[test]
    fn stays_within_its_budget() {
        assert!(
            NOTICE.len() <= 750,
            "the notice is {} bytes; trim it or raise the budget deliberately",
            NOTICE.len()
        );
    }

    /// A skill is paid for only when it is read, so its budget is loose —
    /// but it is a budget, because an unwatched manual grows until it is
    /// one nobody finishes.
    #[test]
    fn every_skill_stays_within_its_budget() {
        for skill in &SKILLS {
            assert!(
                skill.text.len() <= 16_000,
                "the {} skill is {} bytes; trim it or raise the budget deliberately",
                skill.name,
                skill.text.len()
            );
        }
    }
}

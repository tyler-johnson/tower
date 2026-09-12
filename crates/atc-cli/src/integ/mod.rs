//! Integrations: how tower gets wired into the agent clients on this
//! machine, and how those clients then hear about the board.
//!
//! The slugs are what `atc hook` and `atc unhook` take — `claude`,
//! `codex`, `cursor`, `gemini`. They are flat and permanent, because they
//! end up written inside config files tower does not own. The two verbs
//! are for humans: an unknown slug is a real error, a failure is loud,
//! and `--json` emits a report envelope.
//!
//! What every client runs is `atc briefing <slug>`: tower's notice, once
//! per context boundary, wrapped the way that client reads it. That verb
//! is machine surface with one absolute contract — it always exits 0 and
//! says nothing on a failure — and `briefing.rs` holds the text and the
//! guards that keep it true.
//!
//! The skills the binary ships ride the same install for the clients
//! that read one. Each embedded constant is the staleness fingerprint —
//! byte drift on disk reads as "an older tower wrote it" — so the files
//! carry no version or hash of their own.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::CliError;

pub mod briefing;
pub mod claude;
pub mod codex;
pub mod cursor;
pub mod gemini;
pub mod settings;
pub mod skill;
pub mod verbs;

pub use verbs::{hook, unhook};

// ---- what a status says ----------------------------------------------------

/// Whether the client is on this machine at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Presence {
    Present { evidence: PathBuf },
    Absent,
}

impl Presence {
    pub fn is_present(&self) -> bool {
        matches!(self, Presence::Present { .. })
    }
}

/// How an integration is wired, when it is. Per-adapter and not a user
/// choice: each client has exactly one mechanism that works for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mechanism {
    /// A directory tower owns entirely: install writes it whole, uninstall
    /// removes it. No foreign content to preserve, because there is none.
    Plugin,
    /// Entries merged into a settings file that belongs to the user.
    Settings,
}

impl Mechanism {
    pub fn word(&self) -> &'static str {
        match self {
            Mechanism::Plugin => "plugin",
            Mechanism::Settings => "settings",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Wiring {
    NotWired,
    Wired {
        mechanism: Mechanism,
        at: PathBuf,
    },
    /// Some of what install writes is there and some is not — the shape a
    /// half-finished install, or an older tower's, leaves behind.
    Partial {
        missing: String,
        at: PathBuf,
    },
    /// The wiring cannot be read at all: no HOME, or a file that is not
    /// valid JSON. Carries the complaint.
    Unavailable(String),
}

impl Wiring {
    /// Whether the client actually runs the hook. `Partial` counts: what
    /// is there still fires.
    pub fn is_wired(&self) -> bool {
        matches!(self, Wiring::Wired { .. } | Wiring::Partial { .. })
    }

    pub fn word(&self) -> String {
        match self {
            Wiring::NotWired => "not wired".into(),
            Wiring::Wired { mechanism, .. } => format!("wired ({})", mechanism.word()),
            Wiring::Partial { missing, .. } => format!("partial — {missing} missing"),
            Wiring::Unavailable(complaint) => complaint.clone(),
        }
    }

    /// Where the wiring lives, when that is known.
    pub fn at(&self) -> Option<&Path> {
        match self {
            Wiring::Wired { at, .. } | Wiring::Partial { at, .. } => Some(at),
            _ => None,
        }
    }
}

/// The single derivation `atc hook -l` and `atc doctor` both read. Two
/// renderings of one vector, so the two commands cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub slug: &'static str,
    pub presence: Presence,
    pub wiring: Wiring,
    /// Something true about this integration that a person needs told —
    /// Codex's trust step, Cursor's missing session start for cloud agents.
    /// Without it, the notice can silently never land with nothing saying why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The shipped skills, for the clients that read them. Kept apart
    /// from `wiring` because they answer a different question: a missing
    /// skill costs an agent a manual, a missing hook costs it the notice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<Wiring>,
    /// The wiring works, but it is written in a spelling install would
    /// rewrite — a retired command name, or an install predating an event
    /// tower has since added. It keeps working, so this is never an
    /// outage; it is what `atc hook -u` repairs, because a user who never
    /// runs the installer again never gets rewritten by anything else.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
}

/// What `atc hook` was asked for beyond the slugs themselves.
///
/// One field, and it exists because exactly one adapter has two mechanisms:
/// Claude's plugin is the default, and `--settings` is the way back to
/// settings entries if the plugin path ever misbehaves. Every other adapter
/// ignores it, because a mechanism is a property of the client rather than
/// a choice the user should be asked to make.
#[derive(Debug, Default, Clone, Copy)]
pub struct InstallOptions {
    pub settings: bool,
}

/// What an install or uninstall did, and what to say about it.
pub struct Change {
    pub changed: bool,
    pub lines: Vec<String>,
}

impl Change {
    pub fn changed(line: impl Into<String>) -> Change {
        Change {
            changed: true,
            lines: vec![line.into()],
        }
    }

    pub fn unchanged(line: impl Into<String>) -> Change {
        Change {
            changed: false,
            lines: vec![line.into()],
        }
    }

    pub fn absorb(&mut self, other: Change) {
        self.changed |= other.changed;
        self.lines.extend(other.lines);
    }
}

// ---- the trait -------------------------------------------------------------

/// Everything a slug can do: install, detect, status, and the one
/// per-vendor fact about delivery — how the notice has to be wrapped for
/// this client to read it.
pub trait Integration: Sync {
    fn slug(&self) -> &'static str;

    /// Is this client on the machine?
    fn detect(&self) -> Presence;

    fn status(&self) -> Status;

    fn install(&self, opts: &InstallOptions) -> Result<Change, CliError>;

    fn uninstall(&self, opts: &InstallOptions) -> Result<Change, CliError>;

    /// The repair behind `atc hook -u`.
    ///
    /// Separate from `install` because they answer different questions.
    /// `install` is a person asking for wiring and is free to choose the
    /// best mechanism; a repair is a person asking for what is already
    /// there to be made current, and must not move them onto a mechanism
    /// their running client will not pick up until it restarts. The
    /// default is install, because for every integration with one
    /// mechanism the two are the same thing.
    fn repair(&self) -> Result<Change, CliError> {
        self.install(&InstallOptions::default())
    }

    /// The notice, wrapped however this client accepts injected context.
    /// Claude Code and Codex read plain stdout; Gemini and Cursor need a
    /// JSON field, and plain text there is discarded silently, which is
    /// the worst of the available failures.
    fn envelope(&self, text: &str) -> String;
}

// ---- the registry ----------------------------------------------------------

static CLAUDE: claude::Claude = claude::Claude;
static CODEX: codex::Codex = codex::Codex;
static CURSOR: cursor::Cursor = cursor::Cursor;
static GEMINI: gemini::Gemini = gemini::Gemini;

/// Every slug, in the order `atc hook -l` and `atc hook --all` walk them.
pub fn all() -> [&'static dyn Integration; 4] {
    [&CLAUDE, &CODEX, &CURSOR, &GEMINI]
}

pub fn by_slug(slug: &str) -> Option<&'static dyn Integration> {
    all().into_iter().find(|i| i.slug() == slug)
}

/// Every slug's name, for the error a wrong one earns.
pub fn slugs() -> String {
    all()
        .iter()
        .map(|i| i.slug())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The one status derivation. `atc hook -l` renders it as a table and
/// `atc doctor` renders it as rows, so the two cannot drift apart.
pub fn statuses() -> Vec<Status> {
    all().into_iter().map(|i| i.status()).collect()
}

// ---- the two failures ------------------------------------------------------

/// An IO failure on a client's file or directory, with the path and the
/// cause. One raise site for every adapter, so the id is spelled once.
pub(super) fn failed(path: &Path, why: impl std::fmt::Display) -> CliError {
    CliError::coded("hook/failed", format!("{}: {why}", path.display()), vec![])
}

/// A client file that is not the JSON object of hooks its client reads.
/// tower leaves it untouched; the message names the file and the shape.
pub(super) fn malformed(path: &Path, why: impl std::fmt::Display) -> CliError {
    CliError::coded(
        "hook/malformed",
        format!("{}: {why}; file untouched", path.display()),
        vec![],
    )
}

// ---- HOME ------------------------------------------------------------------

/// The user's home. Windows has `USERPROFILE` where unix has `HOME`, and
/// the rest of the tool reads its environment through `var_os` for the same
/// reason: a non-UTF-8 value is still a value.
pub fn home() -> Result<PathBuf, CliError> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
        .map(PathBuf::from)
        .ok_or_else(|| failed(Path::new("~"), "HOME is not set"))
}

/// This binary's own path, for baking into a client's config. Absolute
/// rather than a bare `atc` so the wiring does not depend on tower being
/// on whatever `PATH` the client happens to have.
pub fn exe_path() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "atc".to_string())
}

/// [`exe_path`] with arguments, quoted if the path needs to be, for a
/// client that takes one command string.
pub fn exe_command(args: &str) -> String {
    let exe = exe_path();
    if exe.contains(char::is_whitespace) {
        format!("\"{exe}\" {args}")
    } else {
        format!("{exe} {args}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skill::SKILLS;

    #[test]
    fn every_slug_is_unique_and_resolves() {
        let mut seen = std::collections::HashSet::new();
        for integration in all() {
            assert!(
                seen.insert(integration.slug()),
                "duplicate slug {:?}",
                integration.slug()
            );
            assert!(by_slug(integration.slug()).is_some());
        }
    }

    /// The callsign a client's mark resolves to is the slug `atc hook`
    /// wires it under, so the two lists cannot drift: a client tower
    /// detects is one tower can hook, by the same name.
    #[test]
    fn every_client_marker_names_a_hook_slug() {
        for (variable, callsign) in atc_core::log::CLIENT_MARKERS {
            assert!(
                by_slug(callsign).is_some(),
                "{variable} names `{callsign}`, which is not a hook slug"
            );
        }
    }

    fn front_matter<'a>(name: &str, text: &'a str) -> Vec<&'a str> {
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("---"), "{name}: front matter first");
        lines.take_while(|line| *line != "---").collect()
    }

    /// Every shipped skill is named by its key and carries the
    /// description a client loads it by.
    #[test]
    fn every_skill_leads_with_its_front_matter() {
        for skill in &SKILLS {
            let head = front_matter(skill.name, skill.text);
            assert!(
                head.contains(&format!("name: {}", skill.name).as_str()),
                "{}: the skill is named by its key: {head:?}",
                skill.name
            );
            assert!(
                head.iter().any(|line| line.starts_with("description: ")),
                "{}: the description is what the model loads it by: {head:?}",
                skill.name
            );
            assert!(
                skill.text.contains(&format!("\n# {}\n", skill.name)),
                "{}: the body opens with its own heading",
                skill.name
            );
        }
    }

    /// The drift the worked examples carry, kept out of every skill: no
    /// `requeue`, `promote`, `sync`, or `log` verb exists, and `-p` is
    /// priority rather than a procedure.
    #[test]
    fn no_skill_teaches_a_retired_verb() {
        for skill in &SKILLS {
            for retired in [
                "atc requeue",
                "atc promote",
                "atc sync",
                "atc log",
                "-p <name>",
            ] {
                assert!(
                    !skill.text.contains(retired),
                    "the {} skill teaches `{retired}`",
                    skill.name
                );
            }
        }
    }
}

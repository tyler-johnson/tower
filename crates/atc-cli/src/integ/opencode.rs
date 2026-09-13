//! OpenCode.
//!
//! OpenCode extends through a JavaScript plugin module, not a hooks
//! file: every `{plugin,plugins}/*.{js,ts}` under its config directory
//! loads at startup. tower writes one plugin file it owns whole,
//! `<config>/opencode/plugins/tower.js`, from an embedded body
//! (`opencode.js`) with this binary's absolute path baked in as a
//! string literal, and removes it whole. The `tower` skill lands in
//! OpenCode's own skills directory beside it. `opencode.json` is not
//! touched: its `plugin` field is for npm packages.
//!
//! The plugin does three things, on the three hooks it registers.
//! `experimental.chat.system.transform` runs on every model call and
//! pushes the notice onto the system prompt, so under OpenCode the
//! notice is standing rather than once per boundary: it says what the
//! board says now and survives compaction without a second event. Its
//! payload spells `SessionStart` with the session, so the spawn is the
//! same boundary every other client's is and renews the lease. `shell.env`
//! sets `OPENCODE_SESSION_ID` on every shell command — the bash tool's
//! and the TUI's `!` shell — and that one variable is both the session
//! tag and a client marker beside the `OPENCODE=1` the shell tool sets
//! on its own; the shell tool also sets `OPENCODE_PID` to the client's
//! own pid, which the session row reads, so the lease is held by the
//! process's liveness the way Claude's is (1.18.30 sets `AGENT=1` too,
//! and no session variable of its own).
//! `tool.execute.before` renews the lease on every tool call with a
//! `PreToolUse` payload and prints nothing.
//!
//! Wiring is a byte comparison of one file: equal to what this binary
//! writes is wired; tower's header with other bytes — a moved binary, an
//! older tower — is wired and stale, the repair `atc hook -u` makes; a
//! `tower.js` without the header is someone else's and is left alone.
//! The lease has no release under OpenCode: it frees when the pid dies,
//! and expires by `leaseExpiry` regardless.
//!
//! The config directory is the client's own rule: `$XDG_CONFIG_HOME/opencode`
//! when set, else `~/.config/opencode`, on every OS. `OPENCODE_CONFIG_DIR`
//! overrides it in OpenCode and is not read here.

use std::path::PathBuf;

use super::settings::{Class, Event, Need};
use super::{
    Change, InstallOptions, Integration, Mechanism, Presence, Status, Wiring, plugin, skill,
};
use crate::error::CliError;

pub struct Opencode;

/// The plugin body, with one placeholder for the binary's path.
const PLUGIN: &str = include_str!("opencode.js");

/// The first line of the body: what makes a `tower.js` tower's.
const HEADER: &str = "// Written by `atc hook opencode`.";

const FILE: &str = "tower.js";

/// The two names the plugin's payloads spell: the standing notice is a
/// boundary on every model call, and every tool call is activity.
const EVENTS: [Event; 2] = [
    Event {
        name: "SessionStart",
        matcher: None,
        class: Class::Boundary,
        need: Need::Required,
    },
    Event {
        name: "PreToolUse",
        matcher: None,
        class: Class::Activity,
        need: Need::Extra,
    },
];

const STANDING: &str =
    "the notice is standing: in the system prompt on every model call, one trigger spawn per turn";

// ---- paths -----------------------------------------------------------------

/// `$XDG_CONFIG_HOME/opencode` when set, else `~/.config/opencode` —
/// the client's own rule on every OS.
fn config_dir() -> Result<PathBuf, CliError> {
    let config = match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => super::home()?.join(".config"),
    };
    Ok(config.join("opencode"))
}

fn plugin_path() -> Result<PathBuf, CliError> {
    Ok(config_dir()?.join("plugins").join(FILE))
}

fn skills_root() -> Result<PathBuf, CliError> {
    Ok(config_dir()?.join("skills"))
}

/// The `opencode` binary: `ATC_OPENCODE` when set, the test seam — a
/// path that is not a file means no OpenCode — else `opencode` (or its
/// Windows spellings) found on `PATH`.
fn opencode_binary() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("ATC_OPENCODE").filter(|v| !v.is_empty()) {
        let named = PathBuf::from(named);
        return named.is_file().then_some(named);
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        ["opencode", "opencode.exe", "opencode.cmd"]
            .into_iter()
            .map(|name| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

// ---- the plugin ------------------------------------------------------------

/// The body this binary writes: the placeholder replaced with the
/// binary's path as a JS string literal, so a Windows path's backslashes
/// are escaped.
fn plugin_body() -> String {
    let literal = serde_json::to_string(&super::exe_path()).expect("a string serializes");
    PLUGIN.replacen("__ATC__", &literal, 1)
}

/// Byte-equal to the body this binary writes is wired; tower's header
/// with other bytes — a moved binary, an older tower — is wired and
/// stale; a `tower.js` without the header is someone else's and is
/// left alone.
fn plugin_wiring() -> (Wiring, bool) {
    let Ok(path) = plugin_path() else {
        return (Wiring::NotWired, false);
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (Wiring::NotWired, false);
    };
    let wired = Wiring::Wired {
        mechanism: Mechanism::Plugin,
        at: path.clone(),
    };
    if text == plugin_body() {
        (wired, false)
    } else if text.starts_with(HEADER) {
        (wired, true)
    } else {
        (Wiring::HandWritten { at: path }, false)
    }
}

fn skill_wiring() -> Wiring {
    match skills_root() {
        Ok(root) => skill::wiring(&root),
        Err(_) => Wiring::NotWired,
    }
}

// ---- the integration -------------------------------------------------------

impl Integration for Opencode {
    fn slug(&self) -> &'static str {
        "opencode"
    }

    fn events(&self) -> &'static [Event] {
        &EVENTS
    }

    fn detect(&self) -> Presence {
        match config_dir() {
            Ok(dir) if dir.is_dir() => return Presence::Present { evidence: dir },
            _ => {}
        }
        match opencode_binary() {
            Some(binary) => Presence::Present { evidence: binary },
            None => Presence::Absent,
        }
    }

    fn status(&self) -> Status {
        let (wiring, stale) = plugin_wiring();
        Status {
            slug: self.slug(),
            presence: self.detect(),
            note: wiring.is_wired().then(|| STANDING.to_string()),
            wiring,
            skill: Some(skill_wiring()),
            stale,
        }
    }

    fn install(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let path = plugin_path()?;
        if let (Wiring::HandWritten { .. }, _) = plugin_wiring() {
            return Err(super::failed(&path, "not tower's file; move it aside"));
        }
        let mut written = plugin::write_if_changed(&path, &plugin_body())?;
        let root = skills_root()?;
        if !matches!(skill::wiring(&root), Wiring::Wired { .. }) {
            skill::write_all(&root)?;
            written = true;
        }
        match plugin_wiring() {
            (Wiring::Wired { .. }, false) => {}
            (verified, _) => {
                return Err(super::failed(
                    &path,
                    format!(
                        "the plugin did not verify after the write ({})",
                        verified.word()
                    ),
                ));
            }
        }
        let mut change = if written {
            Change::changed(format!("plugin written to {}", path.display()))
        } else {
            Change::unchanged(format!("already wired in {}", path.display()))
        };
        if written {
            change.lines.push(format!(
                "skills written to {}: {}",
                root.display(),
                skill::names()
            ));
        }
        change.lines.push(STANDING.into());
        if written {
            change.lines.push("restart OpenCode to load it".into());
        }
        Ok(change)
    }

    /// Exactly the two paths tower wrote: the plugin file when it is
    /// tower's — a file under tower's name without tower's header is
    /// someone else's and stays — and the skill directory.
    fn uninstall(&self, _opts: &InstallOptions) -> Result<Change, CliError> {
        let path = plugin_path()?;
        let mut change = match plugin_wiring() {
            (Wiring::Wired { .. }, _) => {
                std::fs::remove_file(&path).map_err(|err| super::failed(&path, err))?;
                Change::changed(format!("removed {}", path.display()))
            }
            (Wiring::HandWritten { .. }, _) => {
                Change::unchanged(format!("{} is not tower's — left alone", path.display()))
            }
            _ => Change::unchanged("no tower plugin installed"),
        };
        let root = skills_root()?;
        if skill::remove_all(&root)? {
            change.absorb(Change::changed(format!(
                "removed {}",
                root.join(skill::names()).display()
            )));
        }
        Ok(change)
    }

    /// The plugin carries the text into the system prompt itself, so
    /// the trigger prints it plain.
    fn envelope(&self, text: &str) -> String {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One placeholder in the source, none in the body, the binary's
    /// path as a JSON string literal in its place, the header first,
    /// and the three hooks and the variable spelled.
    #[test]
    fn the_body_bakes_the_binary_as_a_string_literal() {
        assert_eq!(PLUGIN.matches("__ATC__").count(), 1);
        let body = plugin_body();
        assert!(!body.contains("__ATC__"), "{body}");
        let literal = serde_json::to_string(&super::super::exe_path()).unwrap();
        assert!(body.contains(&literal), "{body}");
        assert!(body.starts_with(HEADER), "{body}");
        for needle in [
            "\"shell.env\"",
            "\"experimental.chat.system.transform\"",
            "\"tool.execute.before\"",
            "OPENCODE_SESSION_ID",
            "trigger opencode",
        ] {
            assert!(body.contains(needle), "{needle} in {body}");
        }
    }

    #[test]
    fn the_events_are_the_two_the_plugin_spells() {
        assert_eq!(
            Opencode.class_of(Some("SessionStart")),
            Some(Class::Boundary)
        );
        assert_eq!(Opencode.class_of(Some("PreToolUse")), Some(Class::Activity));
        assert_eq!(Opencode.class_of(Some("Nonsense")), None);
    }

    #[test]
    fn the_briefing_goes_out_as_plain_text() {
        assert_eq!(Opencode.envelope("hello"), "hello");
    }
}

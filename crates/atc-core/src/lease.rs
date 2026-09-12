//! The session lease: one file per session under the machine's state
//! directory, its mtime the renewal.
//!
//! A lease is what one machine knows about its own sessions, and it
//! stays there: `$XDG_STATE_HOME/atc/leases/<session>`, defaulting to
//! `~/.local/state/atc/leases/`. The trigger keeps it fresh — renewed at
//! every context boundary and on every activity event the client
//! reports, released at the session's end — so the two operations here
//! open no store and touch no repository: a `touch` and an unlink, the
//! `atc config` class of work, since one of them runs on every tool
//! call.
//!
//! The session is keyed by [`crate::log::SESSION_VAR`] from the
//! environment, else by the `session_id` a client's payload carries. The
//! key becomes a file name, so it is held to one rule beyond the
//! session's own: no separator and no `..`.
//!
//! What this file holds beyond its mtime is not this module's to say
//! yet. Renew appends nothing and never truncates, so fields written
//! into it later survive every heartbeat.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::log::SESSION_VAR;

/// The state root: `XDG_STATE_HOME`, else `HOME` (or `USERPROFILE`) with
/// `.local/state` under it. None when neither is set, in which case
/// there is no lease and every operation is a no-op.
pub fn state_root_from(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    set("XDG_STATE_HOME").or_else(|| {
        set("HOME")
            .or_else(|| set("USERPROFILE"))
            .map(|home| home.join(".local/state"))
    })
}

/// Where the leases live on this machine.
pub fn dir() -> Option<PathBuf> {
    state_root_from(|name| std::env::var_os(name)).map(|root| root.join("atc/leases"))
}

/// Whether a session id can be a lease file name: trimmed, non-empty, at
/// most 128 bytes, no control characters, and nothing a path would read
/// as a step — `/`, `\`, or `..`.
fn usable(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let ok = !trimmed.is_empty()
        && trimmed.len() <= 128
        && !trimmed
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
        && !trimmed.contains("..");
    ok.then_some(trimmed)
}

/// The session this process is in: [`SESSION_VAR`] when set and usable,
/// else the payload's `session_id` when that is. None is no session, and
/// nothing to lease.
pub fn session_key(payload_session: &str) -> Option<String> {
    std::env::var(SESSION_VAR)
        .ok()
        .as_deref()
        .and_then(usable)
        .or_else(|| usable(payload_session))
        .map(str::to_string)
}

/// The lease file for one session, when the machine has a state root.
pub fn path(session: &str) -> Option<PathBuf> {
    let session = usable(session)?;
    dir().map(|dir| dir.join(session))
}

/// The heartbeat: create the lease if it is not there, and move its
/// mtime to now. Appends nothing and never truncates.
pub fn renew(session: &str) -> io::Result<()> {
    let Some(path) = path(session) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().append(true).create(true).open(&path)?;
    file.set_modified(SystemTime::now())
}

/// The session's end: the lease goes. A lease already gone is fine.
pub fn release(session: &str) -> io::Result<()> {
    let Some(path) = path(session) else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn the_state_root_is_xdg_then_home() {
        assert_eq!(
            state_root_from(env_of(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")])),
            Some(PathBuf::from("/s"))
        );
        assert_eq!(
            state_root_from(env_of(&[("XDG_STATE_HOME", ""), ("HOME", "/h")])),
            Some(PathBuf::from("/h/.local/state"))
        );
        assert_eq!(
            state_root_from(env_of(&[("USERPROFILE", "/u")])),
            Some(PathBuf::from("/u/.local/state"))
        );
        assert_eq!(state_root_from(env_of(&[])), None);
    }

    #[test]
    fn a_key_is_a_file_name_and_nothing_more() {
        assert_eq!(usable("  s1 "), Some("s1"));
        assert_eq!(usable(""), None);
        assert_eq!(usable("   "), None);
        assert_eq!(usable("a/b"), None);
        assert_eq!(usable("a\\b"), None);
        assert_eq!(usable(".."), None);
        assert_eq!(usable("a..b"), None);
        assert_eq!(usable("a\nb"), None);
        assert_eq!(usable(&"x".repeat(129)), None);
        assert_eq!(usable(&"x".repeat(128)).map(str::len), Some(128));
    }

    #[test]
    fn path_refuses_what_the_key_refuses() {
        assert!(path("a/b").is_none());
        assert!(path("").is_none());
    }
}

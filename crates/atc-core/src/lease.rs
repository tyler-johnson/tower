//! The session lease: one file per session under the machine's state
//! directory, its mtime the renewal and its body the session's identity.
//!
//! A lease is what one machine knows about its own sessions, and it
//! stays there: `$XDG_STATE_HOME/atc/leases/<session>`, defaulting to
//! `~/.local/state/atc/leases/`. The first `atc` call or trigger under a
//! session creates it; every `atc` call and every activity event the
//! trigger sees renews it; the trigger's end event releases it. The
//! body is JSON — the session, the client word, the pid with its start
//! time when the client hands one down, and the callsign `atc callsign`
//! fills in, empty until then. The lease is the session's, not the
//! callsign's: a session with no word is a live session holding
//! nothing, and the hold is only ever about the word.
//!
//! A lease is fresh while its mtime is inside the window — `leaseWindow`
//! in `atc config` — or its pid is alive. The pid is stored with the
//! start time `/proc/<pid>/stat` reports so a reused pid reads as dead;
//! a client that offers no pid gets the window alone. No parent-process
//! walk: a guess there holds or frees the wrong word.
//!
//! Expiry is the rule for the leases the pid rule cannot judge — no
//! pid handed down, or a pid still alive under a newer session id after
//! a `/clear` or a resume. A lease whose mtime is older than
//! `leaseExpiry` in `atc config` is dead whatever its pid says, and
//! [`sweep`] removes it: `atc session` and `atc callsign` sweep every
//! time, and the heartbeat sweeps once per `leaseSweep` through
//! [`sweep_if_due`], gated by the `sweep` marker in the lease directory.
//! The marker's body is the unix second the next sweep is due rather
//! than an mtime, so a heartbeat compares one number and never opens
//! config while the due time is ahead. One accepted edge: a session
//! idle past the expiry that heartbeats in the instant a sweep runs
//! loses its lease and is recreated fresh on the next heartbeat, without
//! its word.
//!
//! [`renew`] and [`release`] open no store and touch no repository — a
//! touch and an unlink, the `atc config` class of work, since one of
//! them runs on every tool call. Renew writes the body once, when the
//! file is empty, and never truncates after: a heartbeat is an fstat and
//! a `set_modified`.
//!
//! The session is keyed by the first [`crate::log::SESSION_VARS`] row
//! set in the environment, else by the `session_id` a client's payload
//! carries. The key becomes a file name, so it is held to one rule
//! beyond the session's own: no separator and no `..`. A terminal's
//! session is one [`mint`] made — a UUIDv7, so it sorts by birth — and
//! `atc session` lists every lease on the machine through [`all`].

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::config::{self, Config};
use crate::log::{CLIENT_MARKERS, SESSION_VARS, usable_session};

/// How long a lease stays fresh without a heartbeat when nothing is
/// configured: `leaseWindow`'s compiled default, which the registry row
/// spells and a test holds to this.
pub const DEFAULT_WINDOW: Duration = Duration::from_secs(120);

/// A lease whose mtime is older than this is dead whatever its pid
/// says: `leaseExpiry`'s compiled default, held to its registry row.
pub const DEFAULT_EXPIRY: Duration = Duration::from_secs(24 * 60 * 60);

/// How often the heartbeat looks for dead leases: `leaseSweep`'s
/// compiled default, held to its registry row.
pub const DEFAULT_SWEEP: Duration = Duration::from_secs(60 * 60);

/// The marker in the lease directory whose body is the unix second the
/// next heartbeat sweep is due. Not a lease; `all()` skips it.
const SWEEP_MARKER: &str = "sweep";

/// The body of a lease file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub session: String,
    /// The client marker's word, whatever the callsign is.
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub pid_start: Option<u64>,
    /// The word `atc callsign` wrote; empty until it did.
    #[serde(default)]
    pub callsign: Option<String>,
}

impl Lease {
    /// The stored pid as a [`Pid`], when the body carries one.
    pub fn pid(&self) -> Option<Pid> {
        Some(Pid {
            pid: self.pid?,
            start: self.pid_start?,
        })
    }

    /// The body written on creation: the session, the client detected
    /// from its mark, the pid the session's own row hands down, no word.
    fn fresh(session: &str) -> Lease {
        let pid = Pid::current();
        Lease {
            session: session.to_string(),
            client: client_word().map(str::to_string),
            pid: pid.map(|pid| pid.pid),
            pid_start: pid.map(|pid| pid.start),
            callsign: None,
        }
    }
}

/// A process, identified exactly: the pid and the start time the kernel
/// reports for it, so a pid the system reused after the session died
/// reads as dead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pid {
    pub pid: u32,
    pub start: u64,
}

impl Pid {
    /// The pid a variable names, with its start time — `None` when the
    /// variable is unset, not a number, or names no live process this
    /// target can read.
    pub fn from_env(var: &str) -> Option<Pid> {
        let pid: u32 = std::env::var(var).ok()?.trim().parse().ok()?;
        Some(Pid {
            pid,
            start: start_time(pid)?,
        })
    }

    /// The pid of this session's own row: the first [`SESSION_VARS`] row
    /// set and usable names the session, and only that row's pid
    /// variable is read — never an inherited one.
    pub fn current() -> Option<Pid> {
        SESSION_VARS
            .iter()
            .find(|row| {
                std::env::var(row.var)
                    .ok()
                    .as_deref()
                    .and_then(usable_session)
                    .is_some()
            })
            .and_then(|row| row.pid_var)
            .and_then(Pid::from_env)
    }

    /// Whether the process is still running: the pid exists and its
    /// start time is the one stored. False on a target with no reader.
    pub fn alive(&self) -> bool {
        start_time(self.pid) == Some(self.start)
    }
}

/// Field 22 of `/proc/<pid>/stat`, the start time in clock ticks since
/// boot. `None` when the process is gone.
#[cfg(target_os = "linux")]
fn start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat_start(&stat)
}

#[cfg(not(target_os = "linux"))]
fn start_time(_pid: u32) -> Option<u64> {
    None
}

/// The start time out of a stat line. The comm in field 2 is
/// parenthesized and may hold spaces or a `)`, so the parse begins after
/// the last `)`: field 3 is then the first token, and field 22 the
/// twentieth.
pub(crate) fn parse_stat_start(stat: &str) -> Option<u64> {
    let after = &stat[stat.rfind(')')? + 1..];
    after.split_whitespace().nth(19)?.parse().ok()
}

/// What a lease's timestamps and pid say about the session behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct State {
    /// The mtime is inside the window.
    pub fresh: bool,
    /// Since the last renewal.
    #[serde(serialize_with = "seconds")]
    pub age: Duration,
    /// Whether the stored pid is running; absent when no pid is stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid_alive: Option<bool>,
}

fn seconds<S: serde::Serializer>(age: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u64(age.as_secs())
}

impl State {
    /// Whether the word in this lease is held: fresh by the window, or
    /// the pid alive.
    pub fn held(&self) -> bool {
        self.fresh || self.pid_alive == Some(true)
    }

    /// The holder's condition, the way a refusal names it: `pid 961370`
    /// when the process is running, else `stale 40s`.
    pub fn detail(&self, pid: Option<Pid>) -> String {
        match (pid, self.pid_alive) {
            (Some(pid), Some(true)) => format!("pid {}", pid.pid),
            _ => format!("stale {}", span(self.age)),
        }
    }
}

/// `4s`, `2m`, `3h` — the lease's own age render, s/m/h/d.
pub fn span(age: Duration) -> String {
    let secs = age.as_secs();
    match secs {
        0..60 => format!("{secs}s"),
        60..3_600 => format!("{}m", secs / 60),
        3_600..86_400 => format!("{}h", secs / 3_600),
        _ => format!("{}d", secs / 86_400),
    }
}

/// [`State`] from a lease's mtime and pid against the window, at `now`.
pub fn state(mtime: SystemTime, pid: Option<Pid>, window: Duration) -> State {
    state_at(SystemTime::now(), mtime, pid.map(|pid| pid.alive()), window)
}

pub(crate) fn state_at(
    now: SystemTime,
    mtime: SystemTime,
    pid_alive: Option<bool>,
    window: Duration,
) -> State {
    let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
    State {
        fresh: age <= window,
        age,
        pid_alive,
    }
}

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

/// The session this process is in: the first [`SESSION_VARS`] row set
/// and usable, when its value can be a file name; else the payload's
/// `session_id` when no row is set and that can. None is no session,
/// and nothing to lease. The row walk is the store's, so the byline on
/// every event and the key of the lease are one value.
pub fn session_key(payload_session: &str) -> Option<String> {
    let row = SESSION_VARS.iter().find_map(|row| {
        std::env::var(row.var)
            .ok()
            .as_deref()
            .and_then(usable_session)
    });
    match row {
        Some(session) => usable(&session).map(str::to_string),
        None => usable(payload_session).map(str::to_string),
    }
}

/// The first [`CLIENT_MARKERS`] entry whose variable is set and
/// non-empty — the client's word.
pub fn client_word() -> Option<&'static str> {
    CLIENT_MARKERS
        .iter()
        .find(|(name, _)| std::env::var(name).is_ok_and(|value| !value.is_empty()))
        .map(|(_, callsign)| *callsign)
}

/// The lease file for one session, when the machine has a state root.
pub fn path(session: &str) -> Option<PathBuf> {
    let session = usable(session)?;
    dir().map(|dir| dir.join(session))
}

/// One session's lease and its mtime, when the file is there. A body
/// that will not parse — the empty file an older trigger left, a hand
/// edit — reads as an empty lease with its mtime, never as no lease.
pub fn read(session: &str) -> Option<(Lease, SystemTime)> {
    let path = path(session)?;
    let mtime = std::fs::metadata(&path).ok()?.modified().ok()?;
    let lease = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Lease>(&bytes).ok())
        .unwrap_or_else(|| Lease {
            session: session.to_string(),
            ..Lease::default()
        });
    Some((lease, mtime))
}

/// Write a lease whole: through a temporary file and a rename, so a
/// reader never sees half a body. The mtime is now.
pub fn write(lease: &Lease) -> io::Result<()> {
    let path = path(&lease.session).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no state directory: neither XDG_STATE_HOME nor HOME is set",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let body = serde_json::to_vec(lease).map_err(io::Error::other)?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The heartbeat: create the lease if it is not there — the body from
/// the environment, once — and move its mtime to now. Never truncates,
/// so the word `atc callsign` wrote survives every heartbeat.
pub fn renew(session: &str) -> io::Result<()> {
    use std::io::Write as _;

    let Some(path) = path(session) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().append(true).create(true).open(&path)?;
    if file.metadata()?.len() == 0 {
        let body = serde_json::to_vec(&Lease::fresh(session)).map_err(io::Error::other)?;
        file.write_all(&body)?;
    }
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

/// A fresh session id: a UUIDv7, so a session's id sorts by birth.
///
/// The 48 high bits are unix milliseconds; the 12 `rand_a` bits carry
/// the sub-millisecond fraction (RFC 9562 §6.2, method 3), so two mints
/// a moment apart still sort in order; the 62 `rand_b` bits come from a
/// SHA-1 over the pid, the wall-clock nanos, and two `RandomState`
/// hashes — the OS-seeded entropy std already pulls, through gix's
/// hasher, so there is no RNG dependency. `atc session --mint` prints
/// it and touches nothing.
pub fn mint() -> String {
    use std::hash::{BuildHasher, RandomState};

    let since_epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let millis = since_epoch.as_millis() as u64 & 0xffff_ffff_ffff;
    // The nanos inside this millisecond, scaled onto 12 bits.
    let sub_ms = (u64::from(since_epoch.subsec_nanos()) % 1_000_000) * 4096 / 1_000_000;

    let seed = format!(
        "{}\0{}\0{}\0{}",
        std::process::id(),
        since_epoch.as_nanos(),
        RandomState::new().hash_one(0u8),
        RandomState::new().hash_one(1u8),
    );
    let digest = gix::objs::compute_hash(
        gix::hash::Kind::Sha1,
        gix::objs::Kind::Blob,
        seed.as_bytes(),
    )
    .map(|id| id.as_bytes().to_vec())
    .unwrap_or_else(|_| seed.into_bytes());
    let mut rand_b = [0u8; 8];
    for (slot, byte) in rand_b.iter_mut().zip(digest.iter().cycle()) {
        *slot = *byte;
    }
    let rand_b = u64::from_be_bytes(rand_b) & 0x3fff_ffff_ffff_ffff;

    let high = (millis << 16) | (0x7 << 12) | sub_ms;
    let low = (0b10 << 62) | rand_b;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        high >> 32,
        (high >> 16) & 0xffff,
        high & 0xffff,
        low >> 48,
        low & 0xffff_ffff_ffff
    )
}

/// Whether a file in the lease directory is a lease: a temporary file
/// mid-write is not a session, and neither is the sweep marker.
fn is_lease_name(name: &str) -> bool {
    !name.contains(".tmp.") && name != SWEEP_MARKER
}

/// Remove every lease whose mtime is older than `expiry`, whatever its
/// body says: a read_dir and a stat per lease, no body reads, no pid
/// checks. A NotFound on the unlink is fine — two sessions may sweep at
/// once.
pub fn sweep(expiry: Duration) {
    if let Some(dir) = dir() {
        sweep_in(&dir, SystemTime::now(), expiry);
    }
}

fn sweep_in(dir: &Path, now: SystemTime, expiry: Duration) {
    // No directory is nothing to sweep.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_str().is_some_and(is_lease_name) {
            continue;
        }
        let Ok(mtime) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if now.duration_since(mtime).unwrap_or(Duration::ZERO) > expiry {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The heartbeat's sweep: read the marker; while its due time is ahead
/// do nothing. Past it, or with no marker, open the config once through
/// the closure, sweep with the configured expiry, and rewrite the marker
/// with now plus the configured leaseSweep. None from the closure is no
/// repository: both settings at their compiled defaults.
pub fn sweep_if_due(config: impl FnOnce() -> Option<Config>) {
    if let Some(dir) = dir() {
        sweep_if_due_in(&dir, SystemTime::now(), config);
    }
}

fn sweep_if_due_in(dir: &Path, now: SystemTime, config: impl FnOnce() -> Option<Config>) {
    let now_secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let marker = dir.join(SWEEP_MARKER);
    let due = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|body| body.trim().parse::<u64>().ok());
    if due.is_some_and(|due| due > now_secs) {
        return;
    }
    let (expiry, interval) = match config() {
        Some(config) => (config::lease_expiry(&config), config::lease_sweep(&config)),
        None => (DEFAULT_EXPIRY, DEFAULT_SWEEP),
    };
    sweep_in(dir, now, expiry);
    // A plain write: a torn body fails to parse and reads as due, which
    // is one extra sweep, not a bug.
    let _ = std::fs::write(&marker, (now_secs + interval.as_secs()).to_string());
}

/// Every lease on the machine: session, body, mtime. A missing
/// directory is no leases.
pub fn all() -> Vec<(String, Lease, SystemTime)> {
    let Some(dir) = dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut leases: Vec<(String, Lease, SystemTime)> = entries
        .flatten()
        .filter_map(|entry| {
            let session = entry.file_name().into_string().ok()?;
            if !is_lease_name(&session) {
                return None;
            }
            let (lease, mtime) = read(&session)?;
            Some((session, lease, mtime))
        })
        .collect();
    leases.sort_by(|a, b| a.0.cmp(&b.0));
    leases
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

    /// The comm is parenthesized and may hold anything, a `)` included,
    /// so the start time is counted from the last `)`.
    #[test]
    fn the_start_time_is_field_22_after_the_comm() {
        let stat = "961370 (atc (x)) S 1 961370 961370 0 -1 4194304 100 0 0 0 5 3 0 0 20 0 1 0 \
                    123456789 12345678 100 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 2 0 0 0 0 0";
        assert_eq!(parse_stat_start(stat), Some(123_456_789));
        assert_eq!(parse_stat_start("garbage"), None);
        assert_eq!(parse_stat_start("1 (short) S 1"), None);
    }

    /// Fresh is the window; held is the window or the pid; the detail
    /// names the pid when it runs and the staleness when it does not.
    #[test]
    fn state_reads_the_window_and_the_pid() {
        let now = SystemTime::now();
        let window = Duration::from_secs(120);
        let young = state_at(now, now - Duration::from_secs(40), None, window);
        assert!(young.fresh && young.held());
        assert_eq!(young.age, Duration::from_secs(40));
        assert_eq!(young.detail(None), "stale 40s");

        let old = state_at(now, now - Duration::from_secs(400), None, window);
        assert!(!old.fresh && !old.held());
        assert_eq!(old.detail(None), "stale 6m");

        let pid = Pid { pid: 7, start: 1 };
        let old_alive = state_at(now, now - Duration::from_secs(400), Some(true), window);
        assert!(!old_alive.fresh && old_alive.held());
        assert_eq!(old_alive.detail(Some(pid)), "pid 7");

        let old_dead = state_at(now, now - Duration::from_secs(400), Some(false), window);
        assert!(!old_dead.held());
        assert_eq!(old_dead.detail(Some(pid)), "stale 6m");

        // A clock that runs backwards is an age of zero, not a panic.
        let future = state_at(now, now + Duration::from_secs(5), None, window);
        assert!(future.fresh);
        assert_eq!(future.age, Duration::ZERO);
    }

    #[test]
    fn span_rounds_down_by_unit() {
        assert_eq!(span(Duration::from_secs(0)), "0s");
        assert_eq!(span(Duration::from_secs(59)), "59s");
        assert_eq!(span(Duration::from_secs(60)), "1m");
        assert_eq!(span(Duration::from_secs(3_599)), "59m");
        assert_eq!(span(Duration::from_secs(3_600)), "1h");
        assert_eq!(span(Duration::from_secs(90_000)), "1d");
    }

    /// The body round-trips whole, an empty or foreign file reads as an
    /// empty lease with its mtime, and a `Lease::pid` needs both halves.
    #[test]
    fn a_lease_body_round_trips_and_a_bad_one_reads_empty() {
        let lease = Lease {
            session: "s1".to_string(),
            client: Some("claude".to_string()),
            pid: Some(42),
            pid_start: Some(9),
            callsign: Some("alpha".to_string()),
        };
        let json = serde_json::to_string(&lease).unwrap();
        assert_eq!(serde_json::from_str::<Lease>(&json).unwrap(), lease);
        assert_eq!(lease.pid(), Some(Pid { pid: 42, start: 9 }));

        let partial: Lease = serde_json::from_str(r#"{"session":"s2"}"#).unwrap();
        assert_eq!(partial.session, "s2");
        assert!(partial.pid().is_none() && partial.callsign.is_none());
        let half: Lease = serde_json::from_str(r#"{"session":"s3","pid":5}"#).unwrap();
        assert!(half.pid().is_none(), "a pid without its start is no pid");
    }

    /// A mint is a UUIDv7: the shape, the version and variant bits, the
    /// timestamp decoding to now, and two in a row sorting in order.
    #[test]
    fn a_mint_is_a_v7_uuid_that_sorts_by_birth() {
        let before = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let first = mint();
        let second = mint();
        let after = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        for id in [&first, &second] {
            assert_eq!(id.len(), 36, "{id}");
            let groups: Vec<&str> = id.split('-').collect();
            assert_eq!(
                groups.iter().map(|g| g.len()).collect::<Vec<_>>(),
                [8, 4, 4, 4, 12],
                "{id}"
            );
            assert!(
                id.chars()
                    .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "lowercase hex: {id}"
            );
            assert!(groups[2].starts_with('7'), "version 7: {id}");
            let variant = u8::from_str_radix(&groups[3][..1], 16).unwrap();
            assert_eq!(variant & 0b1100, 0b1000, "variant 10: {id}");
            let millis = u64::from_str_radix(&format!("{}{}", groups[0], groups[1]), 16).unwrap();
            assert!(
                (before..=after).contains(&millis),
                "{id}: {millis} not in {before}..={after}"
            );
        }
        assert!(first < second, "{first} then {second}");
        assert_ne!(first, second);
    }

    /// A lease by hand in a scratch directory — never `dir()`, since the
    /// environment is process-global — with its mtime `age` seconds back.
    fn plant(dir: &Path, name: &str, age_secs: u64) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!(r#"{{"session":"{name}"}}"#)).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(age_secs))
            .unwrap();
        path
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn marker_body(dir: &Path) -> u64 {
        std::fs::read_to_string(dir.join(SWEEP_MARKER))
            .expect("a marker")
            .trim()
            .parse()
            .expect("a unix second")
    }

    /// The sweep reads mtimes and nothing else: a pidless lease past the
    /// expiry goes, one inside it stays, one with a live pid past it goes
    /// all the same, and the names that are not leases are left alone.
    #[test]
    fn sweep_removes_by_mtime_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let dead = plant(dir, "dead", 25 * 3_600);
        let old = plant(dir, "old", 400);
        let ghost = dir.join("ghost");
        let pid = std::process::id();
        std::fs::write(
            &ghost,
            format!(r#"{{"session":"ghost","pid":{pid},"pid_start":1}}"#),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&ghost)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(25 * 3_600))
            .unwrap();
        let young = plant(dir, "young", 3);
        let tmp_file = plant(dir, "x.tmp.1", 25 * 3_600);
        let marker = plant(dir, SWEEP_MARKER, 25 * 3_600);

        sweep_in(dir, SystemTime::now(), DEFAULT_EXPIRY);
        assert!(!dead.exists(), "pidless past the expiry goes");
        assert!(old.exists(), "pidless inside the expiry stays");
        assert!(
            !ghost.exists(),
            "a live pid past the expiry goes all the same"
        );
        assert!(young.exists(), "a fresh lease is never touched");
        assert!(tmp_file.exists(), "a temporary file is not a lease");
        assert!(marker.exists(), "the marker is not a lease");

        // A missing directory is nothing to sweep.
        sweep_in(&dir.join("absent"), SystemTime::now(), DEFAULT_EXPIRY);
    }

    /// A marker with its due time ahead is the whole check: the
    /// directory is not read, config is not opened, the expired lease
    /// beside it survives.
    #[test]
    fn the_heartbeat_sweep_waits_on_the_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let due = now_secs() + 600;
        std::fs::write(dir.join(SWEEP_MARKER), due.to_string()).unwrap();
        let expired = plant(dir, "expired", 25 * 3_600);
        let opened = std::cell::Cell::new(false);

        sweep_if_due_in(dir, SystemTime::now(), || {
            opened.set(true);
            None
        });
        assert!(expired.exists(), "the marker is ahead: no sweep");
        assert!(!opened.get(), "the marker is ahead: no config open");
        assert_eq!(marker_body(dir), due, "the marker is left as it was");
    }

    /// No marker, or one whose due time is past: the sweep runs with the
    /// closure's config and the marker is rewritten a leaseSweep out.
    #[test]
    fn the_heartbeat_sweep_runs_when_the_marker_is_missing_or_past() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for stale_marker in [None, Some(now_secs() - 1)] {
            if let Some(due) = stale_marker {
                std::fs::write(dir.join(SWEEP_MARKER), due.to_string()).unwrap();
            }
            let expired = plant(dir, "expired", 25 * 3_600);
            let opened = std::cell::Cell::new(false);
            let before = now_secs();
            sweep_if_due_in(dir, SystemTime::now(), || {
                opened.set(true);
                None
            });
            assert!(
                !expired.exists(),
                "{stale_marker:?}: the expired lease goes"
            );
            assert!(opened.get(), "{stale_marker:?}: config is opened once");
            let due = marker_body(dir);
            assert!(
                (before + 3_600..=before + 3_602).contains(&due),
                "{stale_marker:?}: the marker is an hour out, got {due} from {before}"
            );
        }
    }

    /// A configured leaseSweep is the marker's distance.
    #[test]
    fn a_configured_sweep_sets_the_due_time() {
        let fixture = atc_testsupport::Repo::new();
        fixture.git(&["config", "tower.leaseSweep", "5m"]);
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let before = now_secs();
        sweep_if_due_in(dir, SystemTime::now(), || {
            Some(Config::open(fixture.path()).expect("open"))
        });
        let due = marker_body(dir);
        assert!(
            (before + 300..=before + 302).contains(&due),
            "five minutes out, got {due} from {before}"
        );
    }

    #[test]
    fn all_ignores_the_marker() {
        assert!(!is_lease_name("sweep"));
        assert!(!is_lease_name("a.tmp.1"));
        assert!(is_lease_name("s1"));
    }

    /// A pid variable that is unset or not a number is no pid; this
    /// process's own pid is alive, and a pid with the wrong start time is
    /// not.
    #[test]
    fn a_pid_is_read_with_its_start_and_checked_against_it() {
        assert!(Pid::from_env("ATC_TEST_PID_UNSET_9f2c").is_none());
        // The environment is process-global and tests run in parallel,
        // so the variable is this test's own.
        let var = "ATC_TEST_PID_9f2c";
        unsafe { std::env::set_var(var, "not-a-pid") };
        assert!(Pid::from_env(var).is_none());
        unsafe { std::env::set_var(var, std::process::id().to_string()) };
        let own = Pid::from_env(var);
        unsafe { std::env::remove_var(var) };
        if cfg!(target_os = "linux") {
            let own = own.expect("this process has a start time");
            assert_eq!(own.pid, std::process::id());
            assert!(own.alive());
            let reused = Pid {
                pid: own.pid,
                start: own.start.wrapping_add(1),
            };
            assert!(!reused.alive(), "a start time that differs is a reused pid");
        } else {
            assert!(own.is_none(), "no reader on this target, no pid");
        }
    }
}

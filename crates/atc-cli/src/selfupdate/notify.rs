//! The passive lane's state file and the spawn/pending machinery. The
//! cadence grammar lives in core (`config::parse_cadence` and kin) — the
//! registry validated it, so the reader here is the same parser.
//!
//! The repository reaches this module as a `&Path` and config reads go
//! through core's [`Config`] — opened only past the state-file gates, so
//! the hot path stays one file read and zero gix.

use serde::{Deserialize, Serialize};
use std::io::IsTerminal;

use atc_core::config::{self, Config, Setting};

/// update.json — all timestamps are unix seconds. `interval_secs` caches
/// the parsed `tower.updateCheck` so the hot path is one file read, no
/// config load: 0 = unset/default, -1 = disabled, else seconds.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    pub checked_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notified: Option<String>,
    pub interval_secs: i64,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extensions: std::collections::BTreeMap<String, ExtensionState>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionState {
    pub repo: String,
    pub latest: String,
    pub notified: Option<String>,
}

pub fn refresh_extensions(
    state: &mut UpdateState,
    declared: &[crate::registry::Declared],
    mut fetch: impl FnMut(&str) -> Option<String>,
) {
    let eligible: std::collections::BTreeMap<_, _> = declared
        .iter()
        .filter_map(|entry| {
            Some((
                entry.name(),
                crate::selfupdate::release_repo(&entry.manifest)?,
            ))
        })
        .collect();
    state.extensions.retain(|name, cached| {
        eligible
            .get(name.as_str())
            .is_some_and(|repo| repo == &cached.repo)
    });
    for (name, repo) in eligible {
        if let Some(latest) = fetch(&repo)
            && crate::selfupdate::parse_tag(&latest).is_some()
        {
            let cached = state.extensions.entry(name.into()).or_default();
            cached.repo = repo;
            cached.latest = latest;
        }
    }
}

fn extension_notices(state: &UpdateState) -> Vec<String> {
    crate::registry::read()
        .declared()
        .iter()
        .filter_map(|entry| {
            let cached = state.extensions.get(entry.name())?;
            if cached.repo != crate::selfupdate::release_repo(&entry.manifest)?
                || cached.notified.as_deref() == Some(&cached.latest)
            {
                return None;
            }
            let current =
                crate::selfupdate::parse_semver(entry.manifest.version.trim_start_matches('v'))?;
            if crate::selfupdate::parse_tag(&cached.latest)? <= current {
                return None;
            }
            Some(format!(
                "atc-{}: {} is available (running {}) — atc update",
                entry.name(),
                cached.latest,
                entry.manifest.version
            ))
        })
        .collect()
}

/// Resolve the platform cache root using a pure env-lookup closure.
pub fn cache_root_from(
    os: &str,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<std::path::PathBuf> {
    let env_or_home = |ev: &str, home_fallback: &str| -> Option<std::path::PathBuf> {
        let val = env(ev)?;
        if val.is_empty() {
            return None;
        }
        let home = std::path::PathBuf::from(val);
        Some(home.join(home_fallback))
    };

    match os {
        "macos" => env_or_home("HOME", "Library/Caches"),
        "windows" => {
            let val = env("LOCALAPPDATA")?;
            if val.is_empty() {
                return None;
            }
            Some(std::path::PathBuf::from(val))
        }
        _ => match env("XDG_CACHE_HOME") {
            Some(val) if !val.is_empty() => Some(std::path::PathBuf::from(val)),
            _ => env_or_home("HOME", ".cache"),
        },
    }
}

/// Path to the passive-lane state file (`<cache_root>/tower/update.json`).
pub fn state_path() -> Option<std::path::PathBuf> {
    let root = cache_root_from(std::env::consts::OS, |n| std::env::var_os(n))?;
    Some(root.join("tower").join("update.json"))
}

/// Load the passive-lane state from `path`.
///
/// Any error (missing, unreadable, corrupt) returns [`UpdateState::default`].
pub fn load_state(path: &std::path::Path) -> UpdateState {
    std::fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

/// Save the passive-lane state to `path` using atomic temp-file + rename.
pub fn save_state(path: &std::path::Path, state: &UpdateState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_file_name("update.json.tower-tmp");
    let body = serde_json::to_string(state)?;
    std::fs::write(&tmp, &body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Current unix timestamp in seconds.
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Passive-lane gate: official build and not in CI.
fn gates_open() -> bool {
    crate::selfupdate::OFFICIAL && std::env::var_os("CI").is_none()
}

/// The registry's own rows — the lane reads through the same settings the
/// verb validates.
fn update_check() -> &'static Setting {
    config::lookup("updateCheck").expect("updateCheck is registered")
}

/// The lane's cadence on an already-open config — the detached child's
/// re-read in `update --check` rides the same registry row.
pub fn read_cadence(config: &Config) -> i64 {
    config.read_cadence(update_check())
}

/// Spawn a detached process (all stdio nulled, cwd inherited).
fn spawn_detached(exe: &std::path::Path, args: &[&str]) {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW (winbase.h) — hardcoded; no winapi dep for one flag.
        cmd.creation_flags(0x0800_0000);
    }
    // Drop the Child: the parent is short-lived, init reaps the orphan.
    let _ = cmd.spawn();
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CheckStatus {
    Unofficial,
    NoCheckYet,
    Available(String), // the newer release tag, e.g. "v0.2.0"
    UpToDate,
}

/// Pure core: no IO, no compile-time gates, fully testable.
pub(crate) fn check_status_from(
    official: bool,
    state: &UpdateState,
    current: Option<crate::selfupdate::Version>,
) -> CheckStatus {
    if !official {
        return CheckStatus::Unofficial;
    }
    let cur = match current {
        Some(v) => v,
        None => return CheckStatus::NoCheckYet,
    };
    let (latest_ver, latest_tag) = match &state.latest {
        Some(tag) => match crate::selfupdate::parse_tag(tag) {
            Some(v) => (v, tag.clone()),
            None => return CheckStatus::NoCheckYet,
        },
        None => return CheckStatus::NoCheckYet,
    };
    if latest_ver > cur {
        return CheckStatus::Available(latest_tag);
    }
    CheckStatus::UpToDate
}

/// The IO wrapper doctor calls: OFFICIAL + the state file + parse_semver.
pub(crate) fn check_status(current_version: &str) -> CheckStatus {
    let state = match state_path() {
        Some(p) => load_state(&p),
        None => UpdateState::default(),
    };
    check_status_from(
        crate::selfupdate::OFFICIAL,
        &state,
        crate::selfupdate::parse_semver(current_version),
    )
}

/// Result of the passive decision core — which actions are due.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Due {
    pub latest: String,
}

/// The pending() decision, minus all IO. None = fast path, nothing due.
pub(crate) fn compute_due(
    state: &UpdateState,
    current: crate::selfupdate::Version,
    tty: bool,
) -> Option<Due> {
    if !tty {
        return None;
    }
    let latest = state.latest.as_ref()?;
    let latest_ver = crate::selfupdate::parse_tag(latest)?;
    if latest_ver <= current {
        return None;
    }
    if state.notified.as_deref() == state.latest.as_deref() {
        return None;
    }
    Some(Due {
        latest: latest.clone(),
    })
}

/// The pending() notice, minus all IO. None = nothing to say to the caller.
pub(crate) fn notice_for(
    due: &Due,
    want_notice: bool,
    current_version: &str,
    kind: crate::selfupdate::InstallKind,
) -> Option<String> {
    if !want_notice {
        return None;
    }
    Some(format!(
        "atc: {} is available (running v{}) — update with: {}",
        due.latest,
        current_version,
        crate::selfupdate::command_for(kind)
    ))
}

/// Background cache-refresh spawn. Never errors, returns ().
pub fn maybe_spawn_check(repo: &std::path::Path) {
    if !gates_open() {
        return;
    }
    let Some(path) = state_path() else {
        return;
    };
    let mut state = load_state(&path);
    let now = now_secs();

    // Staleness gate on the CACHED interval (hot path — one file read, zero config loads).
    if now - state.checked_at < config::stale_after(state.interval_secs) {
        return;
    }

    // Only now read live config (fufu parity — the scope walk, repo wins).
    let Ok(live) = Config::open(repo) else {
        return;
    };
    let encoded = live.read_cadence(update_check());
    state.interval_secs = encoded;

    // Disabled: stamp to prevent daily config re-reads from becoming frequent file writes.
    if encoded == -1 {
        state.checked_at = now;
        let _ = save_state(&path, &state);
        return;
    }

    // Still fresh under the LIVE cadence — persist the encoding and return.
    if let Some(interval) = config::effective(encoded)
        && now - state.checked_at < interval
    {
        let _ = save_state(&path, &state);
        return;
    }

    // Stale — spawn a detached check. checked_at is NOT stamped here;
    // the spawned child stamps it first thing, which stops respawn storms when offline.
    let _ = save_state(&path, &state);
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    spawn_detached(&exe, &["update", "--check"]);
}

/// Return a release notice only when the caller wants one and this release has not been announced.
pub fn pending(repo: &std::path::Path, current_version: &str, want_notice: bool) -> Option<String> {
    if !want_notice || !gates_open() {
        return None;
    }
    let path = state_path()?;
    let state = load_state(&path);
    let tty = std::io::stderr().is_terminal();

    let current = crate::selfupdate::parse_semver(current_version)?;
    if !tty {
        return None;
    }
    let due = compute_due(&state, current, tty);
    let mut extensions = extension_notices(&state);
    if due.is_none() && extensions.is_empty() {
        return None;
    }

    // Something is due — NOW open live config.
    let live = Config::open(repo).ok()?;
    if live.read_cadence(update_check()) == -1 {
        return None;
    }

    let exe = crate::selfupdate::resolve_exe().ok()?;
    let kind = crate::selfupdate::classify_install(&exe, true);
    if let Some(own) = due.and_then(|due| notice_for(&due, want_notice, current_version, kind)) {
        extensions.insert(0, own);
    }
    Some(extensions.join("\n"))
}

/// Mark the current latest as notified — a release announces at most once, ever.
pub fn mark_notified() {
    let Some(path) = state_path() else {
        return;
    };
    let mut state = load_state(&path);
    state.notified = state.latest.clone();
    for entry in crate::registry::read().declared() {
        if let Some(cached) = state.extensions.get_mut(entry.name()) {
            cached.notified = Some(cached.latest.clone());
        }
    }
    let _ = save_state(&path, &state);
}

/// Write-through cache sync: keep the cached interval honest when config changes.
/// NOT gated on gates_open — config writes should keep the cache honest everywhere.
pub fn sync_interval(encoded: i64) {
    let Some(path) = state_path() else {
        return;
    };
    let mut state = load_state(&path);
    state.interval_secs = encoded;
    let _ = save_state(&path, &state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn extension_cache_keeps_offline_answers_and_drops_retired_or_reaimed_recipes() {
        let declared = |name: &str, repo: &str| {
            crate::registry::Declared {
            path: format!("/bin/atc-{name}").into(), declared_at: 1,
            manifest: crate::manifest::parse(serde_json::json!({"name":name,"version":"1.0.0","contract":1,"verbs":[{"name":"go","read_only":true}],"undoable":false,"update":{"releases":format!("https://github.com/owner/{repo}/releases/latest")}})).unwrap(),
        }
        };
        let mut state = UpdateState::default();
        refresh_extensions(&mut state, &[declared("probe", "probe")], |_| {
            Some("v2.0.0".into())
        });
        assert_eq!(state.extensions["probe"].latest, "v2.0.0");
        refresh_extensions(&mut state, &[declared("probe", "probe")], |_| None);
        assert_eq!(state.extensions["probe"].latest, "v2.0.0");
        refresh_extensions(&mut state, &[declared("probe", "new")], |_| None);
        assert!(state.extensions.is_empty());
        refresh_extensions(&mut state, &[declared("probe", "probe")], |_| {
            Some("bad tag".into())
        });
        assert!(state.extensions.is_empty());
        let mut source = declared("probe", "probe");
        source.manifest.build = Some(crate::manifest::Build::Source);
        refresh_extensions(&mut state, &[source], |_| {
            panic!("a source build has no release check")
        });
        refresh_extensions(&mut state, &[declared("probe", "probe")], |_| {
            Some("v3.0.0".into())
        });
        refresh_extensions(&mut state, &[], |_| panic!("no declarations"));
        assert!(state.extensions.is_empty());
    }

    #[test]
    fn cache_root_from_linux() {
        let linux_env = |key: &str| -> Option<OsString> {
            match key {
                "XDG_CACHE_HOME" => Some(OsString::from("/custom/cache")),
                "HOME" => Some(OsString::from("/home/user")),
                _ => None,
            }
        };
        assert_eq!(
            cache_root_from("linux", linux_env),
            Some(std::path::PathBuf::from("/custom/cache")),
        );

        // XDG unset + HOME
        let linux_env2 = |key: &str| -> Option<OsString> {
            if key == "HOME" {
                Some(OsString::from("/home/user"))
            } else {
                None
            }
        };
        assert_eq!(
            cache_root_from("linux", linux_env2),
            Some(std::path::PathBuf::from("/home/user/.cache")),
        );

        // XDG empty + HOME → fallback to HOME/.cache
        let linux_env3 = |key: &str| -> Option<OsString> {
            match key {
                "XDG_CACHE_HOME" => Some(OsString::from("")),
                "HOME" => Some(OsString::from("/home/user")),
                _ => None,
            }
        };
        assert_eq!(
            cache_root_from("linux", linux_env3),
            Some(std::path::PathBuf::from("/home/user/.cache")),
        );

        // Neither
        let linux_env4 = |_key: &str| -> Option<OsString> { None };
        assert_eq!(cache_root_from("linux", linux_env4), None);
    }

    #[test]
    fn cache_root_from_macos() {
        let mac_env = |key: &str| -> Option<OsString> {
            if key == "HOME" {
                Some(OsString::from("/Users/alice"))
            } else {
                None
            }
        };
        assert_eq!(
            cache_root_from("macos", mac_env),
            Some(std::path::PathBuf::from("/Users/alice/Library/Caches")),
        );
    }

    #[test]
    fn cache_root_from_windows() {
        let win_env = |key: &str| -> Option<OsString> {
            if key == "LOCALAPPDATA" {
                Some(OsString::from("C:\\Users\\alice\\AppData\\Local"))
            } else {
                None
            }
        };
        assert_eq!(
            cache_root_from("windows", win_env),
            Some(std::path::PathBuf::from("C:\\Users\\alice\\AppData\\Local")),
        );

        // Empty LOCALAPPDATA → None
        let win_env_empty = |key: &str| -> Option<OsString> {
            if key == "LOCALAPPDATA" {
                Some(OsString::from(""))
            } else {
                None
            }
        };
        assert_eq!(cache_root_from("windows", win_env_empty), None);
    }

    #[test]
    fn state_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update.json");

        // Default round-trips
        let state = UpdateState::default();
        save_state(&path, &state).unwrap();
        assert_eq!(load_state(&path), state);

        // Fully populated round-trips
        let state = UpdateState {
            checked_at: 1_700_000_000,
            latest: Some("v0.2.0".into()),
            notified: Some("v0.2.0".into()),
            interval_secs: 86_400,
            extensions: std::collections::BTreeMap::new(),
        };
        save_state(&path, &state).unwrap();
        assert_eq!(load_state(&path), state);
    }

    #[test]
    fn state_missing_and_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");
        assert_eq!(load_state(&path), UpdateState::default());

        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "not json at all {{{").unwrap();
        assert_eq!(load_state(&bad), UpdateState::default());
    }

    // ------------------------------------------------------------------
    // compute_due matrix — pure decision logic, no IO
    // ------------------------------------------------------------------

    fn state_builder(latest: Option<&str>, notified: Option<&str>) -> UpdateState {
        UpdateState {
            latest: latest.map(str::to_string),
            notified: notified.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn compute_due_no_tty() {
        let state = state_builder(Some("v0.2.0"), None);
        assert!(compute_due(&state, crate::selfupdate::Version(0, 1, 0), false).is_none());
    }

    #[test]
    fn compute_due_latest_absent() {
        let state = state_builder(None, None);
        assert!(compute_due(&state, crate::selfupdate::Version(0, 1, 0), true).is_none());
    }

    #[test]
    fn compute_due_latest_equals_current() {
        let state = state_builder(Some("v0.1.0"), None);
        assert!(compute_due(&state, crate::selfupdate::Version(0, 1, 0), true).is_none());
    }

    #[test]
    fn compute_due_latest_older() {
        let state = state_builder(Some("v0.0.9"), None);
        assert!(compute_due(&state, crate::selfupdate::Version(0, 1, 0), true).is_none());
    }

    #[test]
    fn compute_due_notice_only() {
        // A newer release that has not been announced gets one notice.
        let state = state_builder(Some("v0.2.0"), None);
        let due = compute_due(&state, crate::selfupdate::Version(0, 1, 0), true);
        assert_eq!(
            due,
            Some(Due {
                latest: "v0.2.0".into(),
            })
        );
    }

    #[test]
    fn an_announced_release_stays_silent() {
        let state = state_builder(Some("v0.2.0"), Some("v0.2.0"));
        let due = compute_due(&state, crate::selfupdate::Version(0, 1, 0), true);
        assert_eq!(due, None);
    }

    #[test]
    fn compute_due_latest_unparseable() {
        let state = state_builder(Some("not-a-version"), None);
        assert!(compute_due(&state, crate::selfupdate::Version(0, 1, 0), true).is_none());
    }

    #[test]
    fn notice_for_unwanted_is_silent() {
        // Due for a notice, but the caller does not want one: nothing.
        let due = Due {
            latest: "v0.2.0".into(),
        };
        assert!(notice_for(&due, false, "0.1.0", crate::selfupdate::InstallKind::Script).is_none());
    }

    #[test]
    fn notice_for_binary_install() {
        let due = Due {
            latest: "v0.2.0".into(),
        };
        // `latest` is the tag (v-prefixed); `current_version` is the bare
        // CARGO_PKG_VERSION — the format string supplies its v.
        let notice = notice_for(&due, true, "0.1.0", crate::selfupdate::InstallKind::Script)
            .expect("notice");
        assert!(notice.starts_with("atc: "), "{notice}");
        assert!(notice.contains("v0.2.0"), "the tag: {notice}");
        assert!(
            notice.contains("running v0.1.0"),
            "running + current: {notice}"
        );
        assert!(
            notice.ends_with(&format!(
                " — update with: {}",
                crate::selfupdate::install_command()
            )),
            "{notice}"
        );
    }

    #[test]
    fn notice_for_brew_install() {
        let due = Due {
            latest: "v0.2.0".into(),
        };
        let notice = notice_for(
            &due,
            true,
            "0.1.0",
            crate::selfupdate::InstallKind::Homebrew,
        )
        .expect("notice");
        assert!(
            notice.ends_with(" — update with: brew upgrade atc"),
            "{notice}"
        );
    }

    // ------------------------------------------------------------------
    // check_status_from matrix — pure decision logic, no IO
    // ------------------------------------------------------------------

    #[test]
    fn check_status_unofficial_wins() {
        let state = state_builder(Some("v1.0.0"), None);
        assert_eq!(
            check_status_from(false, &state, Some(crate::selfupdate::Version(0, 1, 0))),
            CheckStatus::Unofficial
        );
    }

    #[test]
    fn check_status_no_latest() {
        let state = state_builder(None, None);
        assert_eq!(
            check_status_from(true, &state, Some(crate::selfupdate::Version(0, 1, 0))),
            CheckStatus::NoCheckYet
        );
    }

    #[test]
    fn check_status_unparseable_latest() {
        let state = state_builder(Some("gibberish"), None);
        assert_eq!(
            check_status_from(true, &state, Some(crate::selfupdate::Version(0, 1, 0))),
            CheckStatus::NoCheckYet
        );
    }

    #[test]
    fn check_status_available() {
        let state = state_builder(Some("v0.2.0"), None);
        assert_eq!(
            check_status_from(true, &state, Some(crate::selfupdate::Version(0, 1, 0))),
            CheckStatus::Available("v0.2.0".into())
        );
    }

    #[test]
    fn check_status_up_to_date() {
        let state = state_builder(Some("v0.1.0"), None);
        assert_eq!(
            check_status_from(true, &state, Some(crate::selfupdate::Version(0, 1, 0))),
            CheckStatus::UpToDate
        );
    }

    #[test]
    fn check_status_no_current() {
        let state = state_builder(Some("v0.2.0"), None);
        assert_eq!(
            check_status_from(true, &state, None),
            CheckStatus::NoCheckYet
        );
    }
    #[test]
    fn old_cache_keeps_its_cadence_and_notification() {
        let state: UpdateState = serde_json::from_str(r#"{"checked_at":1700000000,"latest":"v0.2.0","notified":"v0.2.0","auto_tried_at":1700000100,"interval_secs":43200}"#).unwrap();
        assert_eq!(state.checked_at, 1_700_000_000);
        assert_eq!(state.interval_secs, 43_200);
        assert!(compute_due(&state, crate::selfupdate::Version(0, 1, 0), true).is_none());
        assert!(
            !serde_json::to_string(&state)
                .unwrap()
                .contains("auto_tried_at")
        );
    }
}

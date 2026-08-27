//! tower's settings: a typed registry over plain git config, fufu's
//! model carried across the seam.
//!
//! Storage is git config under `tower.*`, so `git config` and tower can
//! never disagree about what is set. The registry is the typed half:
//! every shipped setting declares its kind, its default, and the prose a
//! bare `ff tower config` prints, and a value is validated through the
//! same parser its reader uses before anything touches disk.
//!
//! This lives in core rather than the CLI — unlike fufu, where the whole
//! verb is CLI-side — because the CLI stays gix-free and the registry,
//! validation, and lossless writes all need gix. [`Config`] opens the
//! repository directly instead of riding `Store`: `Store::open` resolves
//! the author and fails without `user.email`, and config is the verb you
//! use on a half-configured machine.
//!
//! The write convention is fufu's `snapshot/config.rs`, verbatim: read
//! the file losslessly, mutate, write through `<path>.lock` with an
//! atomic rename, comments preserved. `mint_writer`'s retrying lock loop
//! in `log/mod.rs` stays its own — it must win eventually, where a verb
//! should refuse fast.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use gix::config::Source;
use gix::config::source::Kind;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A key the registry does not declare.
    #[error("unknown setting \"{input}\" — `ff tower config` lists them all")]
    UnknownKey { input: String },

    /// A value the setting's own parser refuses, named so the message
    /// blames the setting actually being set.
    #[error("invalid value for {name}: {want}")]
    BadValue {
        name: &'static str,
        want: &'static str,
    },

    /// `--global` with nowhere to write.
    #[error("cannot locate global git config: HOME is not set")]
    NoGlobal,

    /// A concurrent git holds `<path>.lock`. The verb refuses fast
    /// rather than waiting it out.
    #[error("config is locked: {detail}")]
    Locked { detail: String },

    /// gix, underneath everything else.
    #[error("git error: {0}")]
    Repo(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    /// The stable id, on `procedure::Error`'s pattern: `usage/*` exits 2
    /// by the CLI's namespace rule, everything else 1.
    pub fn id(&self) -> &'static str {
        match self {
            Error::UnknownKey { .. } => "usage/unknown-key",
            Error::BadValue { .. } => "usage/bad-value",
            Error::NoGlobal => "config/no-global",
            Error::Locked { .. } => "config/locked",
            Error::Repo(_) => "repo/error",
        }
    }

    fn repo(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
        Error::Repo(err.into())
    }
}

/// What shape a setting's values take. Only the kinds a shipped setting
/// needs — kinds earn existence like verbs.
#[derive(Debug)]
pub enum SettingKind {
    Dir,
    Cadence,
    Bool,
}

impl SettingKind {
    /// The wire name, lowercased, for the machine envelope.
    pub fn name(&self) -> &'static str {
        match self {
            SettingKind::Dir => "dir",
            SettingKind::Cadence => "cadence",
            SettingKind::Bool => "bool",
        }
    }
}

/// One registered setting: display name, git key, default as displayed,
/// kind, and the hand-wrapped prose the list prints.
#[derive(Debug)]
pub struct Setting {
    pub name: &'static str,
    pub key: &'static str,
    pub def: &'static str,
    pub kind: SettingKind,
    pub desc: &'static [&'static str],
}

/// Every setting tower ships, in display order. `tower.writer` is
/// deliberately absent: identity minted at first append, not a tunable —
/// setting it to another machine's id forks that writer's chain.
pub fn registry() -> &'static [Setting] {
    &[
        Setting {
            name: "bays",
            key: "tower.bays",
            def: "",
            kind: SettingKind::Dir,
            desc: &[
                "The pool root bare `bay warm` mints bay-<n> slots under: absolute,",
                "or relative to the main worktree. Unset, bare warm refuses and",
                "asks for a path.",
            ],
        },
        Setting {
            name: "updateCheck",
            key: "tower.updateCheck",
            def: "1d",
            kind: SettingKind::Cadence,
            desc: &[
                "How often tower looks for a new release in the background. false",
                "turns the whole machinery off (checks, notices, auto-install);",
                "true means daily; durations work too (12h, 7d, 2w), floored at",
                "one minute.",
            ],
        },
        Setting {
            name: "autoUpdate",
            key: "tower.autoUpdate",
            def: "true",
            kind: SettingKind::Bool,
            desc: &[
                "Install new releases silently in the background. false prints a",
                "one-line notice instead; updateCheck false disables both.",
            ],
        },
    ]
}

/// The setting a user's spelling names: case-insensitive, `tower.`
/// prefix optional, so `bays`, `tower.bays`, and `BAYS` all answer.
pub fn lookup(input: &str) -> Result<&'static Setting> {
    let stripped = if input.len() >= 6 && input[..6].eq_ignore_ascii_case("tower.") {
        &input[6..]
    } else {
        input
    };
    registry()
        .iter()
        .find(|setting| setting.name.eq_ignore_ascii_case(stripped))
        .ok_or_else(|| Error::UnknownKey {
            input: input.to_string(),
        })
}

/// Refuse a value the setting's reader could not use, through the same
/// parser that reader runs, before anything touches disk.
pub fn validate(setting: &Setting, value: &str) -> Result<()> {
    let (ok, want) = match setting.kind {
        SettingKind::Dir => (!value.trim().is_empty(), "want a directory path"),
        SettingKind::Cadence => (
            parse_cadence(value).is_some(),
            "want true, false, or a duration like 12h or 7d",
        ),
        SettingKind::Bool => (
            gix::config::Boolean::try_from(gix::bstr::BStr::new(value)).is_ok(),
            "want true or false",
        ),
    };
    if ok {
        Ok(())
    } else {
        Err(Error::BadValue {
            name: setting.name,
            want,
        })
    }
}

/// Parse a cadence string, fufu's shared value language: a bool
/// (`true` = the default cadence, `false`/`never` = off), a compact
/// duration (`12h`, `2w`), or a bare number of days.
///
/// Returns `Some(-1)` for disabled, `Some(0)` for default, `Some(secs)`
/// for explicit durations floored at a minute, or `None` for
/// unparseable input. Public because #13's update reader is the second
/// consumer — the validation here is the reader's own parser.
pub fn parse_cadence(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    match raw.to_ascii_lowercase().as_str() {
        "false" | "no" | "off" | "never" | "0" => return Some(-1),
        "true" | "yes" | "on" => return Some(0),
        _ => {}
    }
    parse_duration(raw).map(|secs| secs.max(60))
}

/// Decode an encoded cadence into an effective interval in seconds:
/// `-1` → disabled (`None`), `0` → daily default, `n` → `n` floored
/// at 60.
pub fn effective(encoded: i64) -> Option<i64> {
    match encoded {
        -1 => None,
        0 => Some(86_400),
        n => Some(n.max(60)),
    }
}

/// The duration grammar under the cadence: `<n>[smhdw]`, or a bare
/// integer of days.
fn parse_duration(raw: &str) -> Option<i64> {
    if raw.is_empty() {
        return None;
    }
    let (digits, unit) = match raw.strip_suffix(['s', 'm', 'h', 'd', 'w']) {
        Some(digits) => (digits, raw.chars().last().unwrap()),
        None => (raw, 'd'),
    };
    let n: i64 = digits.parse().ok()?;
    let secs = match unit {
        's' => n,
        'm' => n.checked_mul(60)?,
        'h' => n.checked_mul(60 * 60)?,
        'd' => n.checked_mul(24 * 60 * 60)?,
        'w' => n.checked_mul(7 * 24 * 60 * 60)?,
        _ => unreachable!(),
    };
    (secs >= 0).then_some(secs)
}

/// A setting's effective value and where it came from. `value` is
/// `None` when nothing sets it — the default applies, and the caller
/// displays `Setting::def`.
pub struct Row {
    pub value: Option<String>,
    pub source: Option<&'static str>,
}

/// The scope walk's order, highest precedence first, each scope with
/// the label the envelope carries.
const SCOPES: [(Kind, &str); 5] = [
    (Kind::Override, "env"),
    (Kind::Repository, "local"),
    (Kind::Global, "global"),
    (Kind::System, "system"),
    (Kind::GitInstallation, "system"),
];

/// The config handle: one repository, opened without `Store` on purpose
/// — no author resolution, no writer, nothing that could refuse before
/// the settings themselves are reachable.
pub struct Config {
    repo: gix::Repository,
}

impl Config {
    /// Open on the repository containing `path`.
    pub fn open(path: &Path) -> Result<Config> {
        let repo = gix::discover(path).map_err(Error::repo)?;
        Ok(Config { repo })
    }

    /// The effective value across every scope.
    pub fn read(&self, setting: &Setting) -> Row {
        self.walk(setting, None)
    }

    /// The effective value with one scope held out — what still applies
    /// after an unset of the local (`global` false) or global scope.
    /// Reads the snapshot taken at open, so it answers for the state
    /// before this process's own write.
    pub fn read_excluding(&self, setting: &Setting, global: bool) -> Row {
        let exclude = if global {
            Kind::Global
        } else {
            Kind::Repository
        };
        self.walk(setting, Some(exclude))
    }

    fn walk(&self, setting: &Setting, exclude: Option<Kind>) -> Row {
        let snap = self.repo.config_snapshot();
        let file = snap.plumbing();
        for (kind, label) in SCOPES {
            if Some(kind) == exclude {
                continue;
            }
            let found = file.string_filter(setting.key, &mut |md: &gix::config::file::Metadata| {
                md.source.kind() == kind
            });
            if let Some(value) = found {
                return Row {
                    value: Some(value.to_string()),
                    source: Some(label),
                };
            }
        }
        Row {
            value: None,
            source: None,
        }
    }

    /// Write one value into the local or global file. Validation is the
    /// caller's, ahead of this — nothing here checks the value again.
    pub fn set(&self, setting: &Setting, value: &str, global: bool) -> Result<()> {
        let (path, source) = self.target(global)?;
        let mut file = load_config_file(&path, source)?;
        file.set_raw_value_by("tower", None, setting.name, value)
            .map_err(Error::repo)?;
        write_config_file(&path, &file)
    }

    /// Remove every occurrence of the setting from the local or global
    /// file. Returns whether anything was removed; an untouched file is
    /// not rewritten.
    pub fn unset(&self, setting: &Setting, global: bool) -> Result<bool> {
        let (path, source) = self.target(global)?;
        let mut file = load_config_file(&path, source)?;
        let ids: Vec<_> = file
            .sections_and_ids_by_name("tower")
            .into_iter()
            .flatten()
            .map(|(_, id)| id)
            .collect();
        let mut removed = false;
        for id in ids {
            if let Some(mut section) = file.section_mut_by_id(id) {
                // A loop, not one call: duplicates all go.
                while section.remove(setting.name).is_some() {
                    removed = true;
                }
            }
        }
        if removed {
            write_config_file(&path, &file)?;
        }
        Ok(removed)
    }

    /// Which file a write lands in, with the source metadata a fresh
    /// file carries.
    fn target(&self, global: bool) -> Result<(PathBuf, Source)> {
        if global {
            Ok((global_config_path().ok_or(Error::NoGlobal)?, Source::User))
        } else {
            Ok((self.repo.common_dir().join("config"), Source::Local))
        }
    }
}

/// Where `--global` writes: whichever of `~/.gitconfig` and the XDG
/// location already exists, preferring the former; neither existing
/// falls back to `~/.gitconfig`, git's own creation behavior.
fn global_config_path() -> Option<PathBuf> {
    let mut env = |name: &str| std::env::var_os(name);
    let user = Source::User.storage_location(&mut env);
    let xdg = Source::Git.storage_location(&mut env);

    if let Some(path) = &user
        && path.exists()
    {
        return Some(path.to_path_buf());
    }
    if let Some(path) = &xdg
        && path.exists()
    {
        return Some(path.to_path_buf());
    }
    user.map(|path| path.to_path_buf())
}

/// Read a git config file losslessly (comments and formatting
/// preserved); an absent file is an empty one carrying the given source
/// metadata.
pub fn load_config_file(path: &Path, source: Source) -> Result<gix::config::File<'static>> {
    let metadata = gix::config::file::Metadata::from(source);
    match std::fs::read(path) {
        Ok(mut bytes) => {
            gix::config::File::from_bytes_owned(&mut bytes, metadata, Default::default())
                .map_err(Error::repo)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(gix::config::File::new(metadata))
        }
        Err(err) => Err(Error::repo(err)),
    }
}

/// Serialize and write via `<path>.lock` (git's own lock convention:
/// `create_new` fails if a concurrent git holds it) + atomic rename;
/// the lock file is removed on failure.
pub fn write_config_file(path: &Path, file: &gix::config::File<'_>) -> Result<()> {
    let mut bytes = Vec::new();
    file.write_to(&mut bytes).map_err(Error::repo)?;

    let lock = path.with_extension("lock");
    let mut lock_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|err| Error::Locked {
            detail: err.to_string(),
        })?;
    let write = lock_file
        .write_all(&bytes)
        .and_then(|()| lock_file.sync_all())
        .and_then(|()| {
            drop(lock_file);
            std::fs::rename(&lock, path)
        });
    if let Err(err) = write {
        let _ = std::fs::remove_file(&lock);
        return Err(Error::repo(err));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_accepts_all_spellings_and_refuses_unknown() {
        assert_eq!(lookup("bays").expect("bare").name, "bays");
        assert_eq!(lookup("tower.bays").expect("prefixed").name, "bays");
        assert_eq!(lookup("BAYS").expect("case-insensitive").name, "bays");
        assert_eq!(
            lookup("Tower.UpdateCheck").expect("both").name,
            "updateCheck"
        );

        let err = lookup("nope").expect_err("unknown key");
        assert_eq!(err.id(), "usage/unknown-key");
        assert!(err.to_string().contains("unknown setting \"nope\""));
    }

    #[test]
    fn cadence_parse_table() {
        assert_eq!(parse_cadence("false"), Some(-1));
        assert_eq!(parse_cadence("NO"), Some(-1));
        assert_eq!(parse_cadence("off"), Some(-1));
        assert_eq!(parse_cadence("never"), Some(-1));
        assert_eq!(parse_cadence("0"), Some(-1));
        assert_eq!(parse_cadence("true"), Some(0));
        assert_eq!(parse_cadence("YES"), Some(0));
        assert_eq!(parse_cadence("on"), Some(0));
        assert_eq!(parse_cadence("12h"), Some(43_200));
        assert_eq!(parse_cadence("2w"), Some(1_209_600));
        assert_eq!(parse_cadence("7"), Some(604_800));
        assert_eq!(parse_cadence("45s"), Some(60)); // floor
        assert_eq!(parse_cadence("  true  "), Some(0));
        assert!(parse_cadence("bogus").is_none());
        assert!(parse_cadence("5x").is_none());

        assert_eq!(effective(-1), None);
        assert_eq!(effective(0), Some(86_400));
        assert_eq!(effective(30), Some(60));
        assert_eq!(effective(7_200), Some(7_200));
    }

    #[test]
    fn every_bad_value_names_its_own_setting() {
        // The fufu quirk fixed rather than mirrored: each kind's message
        // interpolates the setting actually being set.
        for setting in registry() {
            let bad = match setting.kind {
                SettingKind::Dir => "   ",
                SettingKind::Cadence => "5x",
                SettingKind::Bool => "maybe",
            };
            let err = validate(setting, bad).expect_err("a bad value refuses");
            assert_eq!(err.id(), "usage/bad-value");
            assert!(
                err.to_string()
                    .starts_with(&format!("invalid value for {}:", setting.name)),
                "{}: {err}",
                setting.name
            );
        }
    }

    #[test]
    fn a_config_file_round_trips_with_comments_preserved() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config");
        std::fs::write(&path, "# hands off\n[tower]\n\tbays = ../pool\n").expect("seed");

        let mut file = load_config_file(&path, Source::Local).expect("load");
        file.set_raw_value_by("tower", None, "updateCheck", "12h")
            .expect("set");
        write_config_file(&path, &file).expect("write");

        let after = std::fs::read_to_string(&path).expect("read back");
        assert!(after.contains("# hands off"), "comment lost: {after}");
        assert!(after.contains("bays = ../pool"), "value lost: {after}");
        assert!(after.contains("updateCheck = 12h"), "set lost: {after}");
    }
}

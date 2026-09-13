//! The install-channel dispatcher and passive release check. The installer owns replacing the binary.

pub mod github;
pub mod notify;

use crate::error::CliError;
use std::path::{Path, PathBuf};

/// Release builds set ATC_OFFICIAL_BUILD in CI; dev, dogfood, and test builds are source installs.
pub const OFFICIAL: bool = option_env!("ATC_OFFICIAL_BUILD").is_some();

#[cfg(not(windows))]
pub const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/tyler-johnson/tower/main/install.sh";
#[cfg(windows)]
pub const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/tyler-johnson/tower/main/install.ps1";
pub const RELEASES_URL: &str = "https://github.com/tyler-johnson/tower/releases/latest";
pub const CARGO_INSTALL: &str =
    "cargo install --git https://github.com/tyler-johnson/tower atc-cli";
pub const BREW_UPGRADE: &str = "brew upgrade atc";

pub(crate) fn failed(message: impl Into<String>) -> CliError {
    CliError::coded("update/failed", message, Vec::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

/// Accept only a bare major.minor.patch, with no suffix or metadata.
pub fn parse_semver(s: &str) -> Option<Version> {
    let parts: Vec<&str> = s.split('.').collect();
    let [a, b, c] = parts[..] else { return None };
    if [a, b, c]
        .iter()
        .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return None;
    }
    Some(Version(a.parse().ok()?, b.parse().ok()?, c.parse().ok()?))
}

pub fn parse_tag(tag: &str) -> Option<Version> {
    parse_semver(tag.strip_prefix('v')?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallKind {
    Source,
    Homebrew,
    Script,
    Unmanaged,
}

/// Canonicalize before classification, including Homebrew's symlinked entry point.
pub fn resolve_exe() -> Result<PathBuf, CliError> {
    let path = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|err| failed(format!("cannot locate the running binary: {err}")))?;
    #[cfg(windows)]
    {
        if let Some(stripped) = path.to_string_lossy().strip_prefix(r"\\?\") {
            return Ok(PathBuf::from(stripped));
        }
    }
    Ok(path)
}

/// The installer's default destination. ATC_INSTALL_DIR is not evidence of how a binary got here.
pub fn script_install_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let path = {
        let local = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty())?;
        PathBuf::from(local)
            .join("Programs")
            .join("atc")
            .join("atc.exe")
    };
    #[cfg(not(windows))]
    let path = {
        let home = std::env::var_os("HOME").filter(|v| !v.is_empty())?;
        PathBuf::from(home).join(".local").join("bin").join("atc")
    };
    let path = path.canonicalize().unwrap_or(path);
    #[cfg(windows)]
    {
        if let Some(stripped) = path.to_string_lossy().strip_prefix(r"\\?\") {
            return Some(PathBuf::from(stripped));
        }
    }
    Some(path)
}

/// Both paths are resolved; an absent home is represented by an empty script path.
pub fn classify_install_at(exe: &Path, official: bool, script_path: &Path) -> InstallKind {
    if !official {
        return InstallKind::Source;
    }
    let path = exe.to_string_lossy();
    if path.contains("/Cellar/")
        || path.contains("/opt/homebrew/")
        || path.contains("/home/linuxbrew/")
    {
        return InstallKind::Homebrew;
    }
    if !script_path.as_os_str().is_empty() && exe == script_path {
        return InstallKind::Script;
    }
    InstallKind::Unmanaged
}

pub fn classify_install(exe: &Path, official: bool) -> InstallKind {
    classify_install_at(exe, official, &script_install_path().unwrap_or_default())
}

#[cfg(not(windows))]
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

pub fn install_command() -> String {
    install_command_for(INSTALL_URL)
}

pub fn install_command_for(url: &str) -> String {
    #[cfg(windows)]
    {
        format!("irm {url} | iex")
    }
    #[cfg(not(windows))]
    {
        if !on_path("curl") && on_path("wget") {
            format!("wget -qO- {url} | sh")
        } else {
            format!("curl -fsSL {url} | sh")
        }
    }
}

pub fn extension_bin_dir(bin: Option<&str>) -> Option<PathBuf> {
    let path = match bin {
        Some("~") => crate::integ::home().ok()?,
        Some(bin) if bin.starts_with("~/") => crate::integ::home().ok()?.join(&bin[2..]),
        Some(bin) => PathBuf::from(bin),
        None => script_install_path()?.parent()?.to_path_buf(),
    };
    Some(path.canonicalize().unwrap_or(path))
}

pub fn classify_extension_at(
    path: &Path,
    official: bool,
    install: bool,
    bin: &Path,
) -> InstallKind {
    let own = classify_install_at(path, official, Path::new(""));
    if own != InstallKind::Unmanaged {
        return own;
    }
    if install && !bin.as_os_str().is_empty() && path.parent() == Some(bin) {
        InstallKind::Script
    } else {
        InstallKind::Unmanaged
    }
}

pub fn recipe_for(block: &crate::manifest::Update, kind: InstallKind) -> Option<String> {
    match kind {
        InstallKind::Source => None,
        InstallKind::Homebrew => block
            .brew
            .as_ref()
            .map(|formula| format!("brew upgrade {formula}")),
        InstallKind::Script => block.install.as_deref().map(install_command_for),
        InstallKind::Unmanaged => block.releases.clone(),
    }
}

pub fn release_repo(manifest: &crate::manifest::Manifest) -> Option<String> {
    if manifest.build() == crate::manifest::Build::Source {
        return None;
    }
    let url = manifest.update.as_ref()?.releases.as_deref()?;
    let path = url.strip_prefix("https://github.com/")?;
    let parts: Vec<_> = path.trim_end_matches('/').split('/').collect();
    let [owner, repo, "releases", "latest"] = parts.as_slice() else {
        return None;
    };
    if [owner, repo].iter().any(|s| {
        s.is_empty()
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    }) {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

/// Shared by the explicit verb and the passive notice.
pub fn command_for(kind: InstallKind) -> String {
    match kind {
        InstallKind::Source => CARGO_INSTALL.into(),
        InstallKind::Homebrew => BREW_UPGRADE.into(),
        InstallKind::Script => install_command(),
        InstallKind::Unmanaged => RELEASES_URL.into(),
    }
}

/// Called only after consent. In JSON mode installer progress goes to stderr so stdout remains one envelope.
pub fn run_installer(cmd: &str, json: bool) -> Result<(), CliError> {
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("powershell");
        command.args(["-NoProfile", "-Command", cmd]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", cmd]);
        command
    };
    command
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    if json {
        command.stdout(std::io::stderr());
    } else {
        command.stdout(std::process::Stdio::inherit());
    }
    let status = command
        .status()
        .map_err(|err| failed(format!("cannot run the installer: {err}")))?;
    if !status.success() {
        return Err(failed(format!(
            "the installer failed ({status}) — run it yourself: {cmd}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_channels_and_recipes_follow_the_owners_directory() {
        let bin = Path::new("/home/u/.local/bin");
        let block = crate::manifest::Update {
            brew: Some("owner/tap/probe".into()),
            install: Some("https://example.com/install.sh".into()),
            bin: None,
            releases: Some("https://example.com/releases".into()),
        };
        for (path, kind) in [
            ("/home/u/.local/bin/atc-probe", InstallKind::Script),
            ("/home/u/.local/bin/sub/atc-probe", InstallKind::Unmanaged),
            ("/opt/homebrew/bin/atc-probe", InstallKind::Homebrew),
        ] {
            assert_eq!(
                classify_extension_at(Path::new(path), true, true, bin),
                kind
            );
            assert_eq!(
                classify_extension_at(Path::new(path), false, true, bin),
                InstallKind::Source
            );
        }
        assert_eq!(
            classify_extension_at(&bin.join("atc-probe"), true, false, bin),
            InstallKind::Unmanaged
        );
        assert_eq!(
            recipe_for(&block, InstallKind::Homebrew).as_deref(),
            Some("brew upgrade owner/tap/probe")
        );
        assert!(
            recipe_for(&block, InstallKind::Script)
                .unwrap()
                .contains("https://example.com/install.sh")
        );
        assert_eq!(recipe_for(&block, InstallKind::Unmanaged), block.releases);
        assert_eq!(recipe_for(&block, InstallKind::Source), None);
    }

    #[test]
    fn versions_are_strict_and_compare_numerically() {
        assert_eq!(parse_semver("0.1.0"), Some(Version(0, 1, 0)));
        assert!(parse_tag("v1.10.0") > parse_tag("v1.2.3"));
        for bad in [
            "0.1.0",
            "v1.2",
            "v1.2.3.4",
            "v1.2.3-rc1",
            "v1.2.3+meta",
            "v1.+2.3",
            "v1..3",
            "",
        ] {
            assert!(parse_tag(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn channels_respect_build_ownership_and_exact_script_destination() {
        let script = Path::new("/home/u/.local/bin/atc");
        for (path, kind) in [
            ("/home/u/.local/bin/atc", InstallKind::Script),
            ("/home/u/.local/bin/atc2", InstallKind::Unmanaged),
            ("/usr/local/bin/atc", InstallKind::Unmanaged),
            ("/nix/store/abc-atc/bin/atc", InstallKind::Unmanaged),
            ("/opt/homebrew/bin/atc", InstallKind::Homebrew),
            ("/home/linuxbrew/.linuxbrew/bin/atc", InstallKind::Homebrew),
            ("/usr/local/Cellar/atc/0.1.0/bin/atc", InstallKind::Homebrew),
        ] {
            assert_eq!(
                classify_install_at(Path::new(path), true, script),
                kind,
                "{path}"
            );
            assert_eq!(
                classify_install_at(Path::new(path), false, script),
                InstallKind::Source
            );
        }
        assert_eq!(
            classify_install_at(script, true, Path::new("")),
            InstallKind::Unmanaged
        );
        assert_eq!(
            classify_install_at(Path::new(""), true, Path::new("")),
            InstallKind::Unmanaged
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolved_symlink_uses_the_owners_channel() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Cellar/atc/0.1.0/bin/atc");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "binary").unwrap();
        let link = dir.path().join("atc");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            classify_install_at(
                &link.canonicalize().unwrap(),
                true,
                &link.canonicalize().unwrap()
            ),
            InstallKind::Homebrew
        );
    }

    #[test]
    fn installer_command_names_the_platform_script() {
        let cmd = install_command();
        assert!(cmd.contains(INSTALL_URL), "{cmd}");
        assert!(
            cmd.ends_with(if cfg!(windows) { " | iex" } else { " | sh" }),
            "{cmd}"
        );
    }
}

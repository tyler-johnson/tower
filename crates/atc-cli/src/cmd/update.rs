//! `atc update` dispatches to the installer that owns this binary. `--check` only refreshes the cache.

use crate::error::CliError;
use crate::{machine, selfupdate};
use atc_core::config::Config;
use selfupdate::InstallKind;

pub fn run(json: bool, check: bool, yes: bool) -> Result<(), CliError> {
    if check {
        return refresh_cache();
    }

    let exe = selfupdate::resolve_exe()?;
    let own = dispatch(
        selfupdate::classify_install(&exe, selfupdate::OFFICIAL),
        env!("CARGO_PKG_VERSION"),
        yes,
        || {
            let agent = selfupdate::github::agent();
            selfupdate::github::fetch_latest(&agent, "https://api.github.com").map(|r| r.tag_name)
        },
        |report| {
            if !json {
                report.print();
            }
        },
        || {
            if json || !machine::interactive() {
                return Ok(false);
            }
            confirm()
        },
        |cmd| selfupdate::run_installer(cmd, json),
    );
    let registry = crate::registry::read();
    if let Some(why) = &registry.unreadable {
        eprintln!("atc: the registry does not read as one: {why}");
    }
    let declared = registry.declared();
    if declared.is_empty() {
        let report = own?;
        if json {
            println!("{}", machine::emit("update", &report));
        }
        return Ok(());
    }
    let mut trouble = Vec::new();
    if let Err(err) = &own {
        trouble.push(err.to_string());
    }
    let mut extensions = Vec::new();
    let mut moved = false;
    for entry in declared {
        let report = extension(entry, json, yes, &mut trouble)?;
        moved |= report.status == "installed";
        extensions.push(report);
    }
    if moved {
        // The path from before installation still names the installed binary after its predecessor was replaced.
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["hook", "-u"]);
        if json {
            cmd.stdout(std::io::stderr());
        }
        match cmd.status() {
            Ok(status) if status.success() => {}
            result => trouble.push(format!("atc hook -u did not finish: {result:?}")),
        }
    }
    if !trouble.is_empty() {
        return Err(selfupdate::failed(format!(
            "not everything moved:\n  {}",
            trouble.join("\n  ")
        )));
    }
    if json {
        let mut data = serde_json::to_value(own.expect("no trouble")).expect("report serializes");
        data["extensions"] = serde_json::to_value(extensions).expect("reports serialize");
        println!("{}", machine::emit("update", &data));
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct ExtensionReport {
    name: String,
    current: String,
    channel: Option<InstallKind>,
    command: Option<String>,
    status: &'static str,
    message: String,
}

fn extension(
    entry: &crate::registry::Declared,
    json: bool,
    yes: bool,
    trouble: &mut Vec<String>,
) -> Result<ExtensionReport, CliError> {
    let mut report = ExtensionReport {
        name: entry.name().into(),
        current: entry.manifest.version.clone(),
        channel: None,
        command: None,
        status: "instructions",
        message: String::new(),
    };
    let path = entry
        .resolve()
        .map(|path| path.canonicalize().unwrap_or(path));
    if path.is_none() {
        report.message = format!(
            "atc-{} is not on PATH any more; recorded at {}",
            entry.name(),
            entry.path.display()
        );
    } else if entry.manifest.build() == crate::manifest::Build::Source {
        report.channel = Some(InstallKind::Source);
        report.message = format!(
            "atc-{} was built from source — rebuild it the way you built it",
            entry.name()
        );
    } else if let Some(block) = &entry.manifest.update {
        let bin = selfupdate::extension_bin_dir(block.bin.as_deref()).unwrap_or_default();
        let kind = selfupdate::classify_extension_at(
            path.as_deref().expect("resolved"),
            true,
            block.install.is_some(),
            &bin,
        );
        report.channel = Some(kind);
        report.command = selfupdate::recipe_for(block, kind);
        report.message = if report.command.is_some() {
            format!("atc-{} {}", entry.name(), entry.manifest.version)
        } else {
            format!(
                "atc-{} has no recipe for its {kind:?} channel",
                entry.name()
            )
        };
    } else {
        report.message = format!(
            "atc-{} is one tower cannot move: its manifest carries no update recipes",
            entry.name()
        );
    }
    if !json {
        println!("\n{}.", report.message);
        if let Some(command) = &report.command {
            println!("update with:\n  {command}");
        }
    }
    let runnable = report.channel == Some(InstallKind::Script) && report.command.is_some();
    if runnable {
        if yes || (!json && machine::interactive() && confirm()?) {
            match selfupdate::run_installer(report.command.as_deref().expect("recipe"), json) {
                Ok(()) => report.status = "installed",
                Err(err) => {
                    report.status = "failed";
                    trouble.push(format!("atc-{}: {err}", entry.name()));
                }
            }
        }
    } else if yes {
        trouble.push(report.message.clone());
    }
    Ok(report)
}

#[derive(Debug, serde::Serialize)]
struct Report {
    channel: InstallKind,
    current: String,
    latest: Option<String>,
    command: String,
    status: &'static str,
}

fn reason(kind: InstallKind) -> &'static str {
    match kind {
        InstallKind::Source => "atc was built from source",
        InstallKind::Homebrew => "atc was installed with Homebrew",
        InstallKind::Unmanaged => {
            "atc was installed by something else — whatever placed this binary replaces it"
        }
        InstallKind::Script => "atc sits where its install script puts it",
    }
}

impl Report {
    fn print(&self) {
        if self.status == "current" {
            println!("already up to date (v{})", self.current);
            return;
        }
        if let Some(latest) = &self.latest {
            println!("atc {latest} is available (running v{}).", self.current);
        } else {
            println!("{}.", reason(self.channel));
        }
        println!("update with:\n  {}", self.command);
    }
}

/// Inject the three effects so tests prove which channels can reach the network, prompt, and installer.
fn dispatch(
    kind: InstallKind,
    current: &str,
    yes: bool,
    fetch: impl FnOnce() -> Result<String, CliError>,
    show: impl FnOnce(&Report),
    confirm: impl FnOnce() -> Result<bool, CliError>,
    install: impl FnOnce(&str) -> Result<(), CliError>,
) -> Result<Report, CliError> {
    let mut report = Report {
        channel: kind,
        current: current.into(),
        latest: None,
        command: selfupdate::command_for(kind),
        status: "instructions",
    };
    if kind != InstallKind::Script {
        if yes {
            let message = format!("{} — update with: {}", reason(kind), report.command);
            let exits = vec![report.command];
            return Err(match kind {
                InstallKind::Source => CliError::coded("update/source-build", message, exits),
                InstallKind::Homebrew => CliError::coded("update/homebrew", message, exits),
                InstallKind::Unmanaged => CliError::coded("update/unmanaged", message, exits),
                InstallKind::Script => unreachable!(),
            });
        }
        show(&report);
        return Ok(report);
    }
    let current_version = selfupdate::parse_semver(current)
        .ok_or_else(|| selfupdate::failed(format!("cannot parse current version \"{current}\"")))?;
    let tag = fetch()?;
    let latest = selfupdate::parse_tag(&tag)
        .ok_or_else(|| selfupdate::failed(format!("unexpected release tag \"{tag}\"")))?;
    report.latest = Some(tag);
    if latest <= current_version {
        report.status = "current";
        show(&report);
        return Ok(report);
    }
    report.status = "available";
    show(&report);
    if yes || confirm()? {
        install(&report.command)?;
        report.status = "installed";
    }
    Ok(report)
}

/// Consent is explicit: an empty line, EOF, and an unrecognized answer all decline.
fn confirmed(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn confirm() -> Result<bool, CliError> {
    use std::io::Write;
    eprint!("\nrun it now? [y/N] ");
    std::io::stderr()
        .flush()
        .map_err(|err| selfupdate::failed(format!("stderr: {err}")))?;
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return Ok(false);
    }
    Ok(confirmed(&answer))
}

/// The detached child's whole job. Stamping `checked_at` first thing is
/// the offline backoff: a spawn that finds no network still moves the
/// clock, so the parent does not respawn a storm of children.
fn refresh_cache() -> Result<(), CliError> {
    let Some(path) = selfupdate::notify::state_path() else {
        return Ok(());
    };

    let mut state = selfupdate::notify::load_state(&path);

    // Stamp checked_at = now
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    state.checked_at = now;

    // Re-read cadence if we can discover a repo
    if let Ok(repo) = std::env::current_dir()
        && let Ok(config) = Config::open(&repo)
    {
        state.interval_secs = selfupdate::notify::read_cadence(&config);
    }
    let _ = selfupdate::notify::save_state(&path, &state);

    // Fetch latest — failures are silent
    let _ = (|| -> Result<(), CliError> {
        let agent = selfupdate::github::agent();
        let release = selfupdate::github::fetch_latest(&agent, "https://api.github.com")?;
        if selfupdate::parse_tag(&release.tag_name).is_some() {
            state.latest = Some(release.tag_name);
        }
        let _ = selfupdate::notify::save_state(&path, &state);
        Ok(())
    })();
    // Every failure is silent
    let agent = selfupdate::github::agent();
    selfupdate::notify::refresh_extensions(
        &mut state,
        crate::registry::read().declared(),
        |repo| {
            selfupdate::github::fetch_latest_of(&agent, "https://api.github.com", repo)
                .ok()
                .map(|release| release.tag_name)
        },
    );
    let _ = selfupdate::notify::save_state(&path, &state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn other_channels_never_check_prompt_or_install() {
        for (kind, id, command) in [
            (
                InstallKind::Source,
                "update/source-build",
                selfupdate::CARGO_INSTALL,
            ),
            (
                InstallKind::Homebrew,
                "update/homebrew",
                selfupdate::BREW_UPGRADE,
            ),
            (
                InstallKind::Unmanaged,
                "update/unmanaged",
                selfupdate::RELEASES_URL,
            ),
        ] {
            for yes in [false, true] {
                let shown = Cell::new(false);
                let result = dispatch(
                    kind,
                    "0.1.0",
                    yes,
                    || panic!("network"),
                    |_| shown.set(true),
                    || panic!("prompt"),
                    |_| panic!("installer"),
                );
                if yes {
                    let err = result.unwrap_err();
                    assert_eq!(err.id(), id);
                    assert!(err.to_string().contains(command));
                    assert!(!shown.get());
                } else {
                    let report = result.unwrap();
                    assert_eq!(report.status, "instructions");
                    assert_eq!(report.command, command);
                    assert!(shown.get());
                }
            }
        }
    }

    #[test]
    fn script_requires_consent_after_the_command_is_shown() {
        for yes in [false, true] {
            for answer in [false, true] {
                let shown = Cell::new(false);
                let prompted = Cell::new(false);
                let installed = Cell::new(false);
                let report = dispatch(
                    InstallKind::Script,
                    "0.1.0",
                    yes,
                    || Ok("v0.2.0".into()),
                    |report| {
                        assert_eq!(report.status, "available");
                        shown.set(true);
                    },
                    || {
                        assert!(shown.get());
                        prompted.set(true);
                        Ok(answer)
                    },
                    |cmd| {
                        assert!(shown.get());
                        assert_eq!(cmd, selfupdate::install_command());
                        installed.set(true);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(prompted.get(), !yes);
                assert_eq!(installed.get(), yes || answer);
                assert_eq!(
                    report.status,
                    if yes || answer {
                        "installed"
                    } else {
                        "available"
                    }
                );
            }
        }
    }

    #[test]
    fn current_or_older_releases_never_prompt_or_install() {
        for tag in ["v0.1.0", "v0.0.9"] {
            let report = dispatch(
                InstallKind::Script,
                "0.1.0",
                true,
                || Ok(tag.into()),
                |_| {},
                || panic!("prompt"),
                |_| panic!("installer"),
            )
            .unwrap();
            assert_eq!(report.status, "current");
        }
    }

    #[test]
    fn invalid_tags_and_network_failures_stop_before_consent() {
        for release in [
            Ok("not-a-version".into()),
            Err(selfupdate::failed("offline")),
        ] {
            let err = dispatch(
                InstallKind::Script,
                "0.1.0",
                true,
                || release,
                |_| panic!("show"),
                || panic!("prompt"),
                |_| panic!("installer"),
            )
            .unwrap_err();
            assert_eq!(err.id(), "update/failed");
        }
    }

    #[test]
    fn installer_failure_is_not_reported_as_success() {
        let err = dispatch(
            InstallKind::Script,
            "0.1.0",
            true,
            || Ok("v0.2.0".into()),
            |_| {},
            || panic!("prompt"),
            |_| Err(selfupdate::failed("installer failed")),
        )
        .unwrap_err();
        assert_eq!(err.id(), "update/failed");
        assert_eq!(err.to_string(), "installer failed");
    }

    #[test]
    fn consent_requires_a_typed_yes() {
        for answer in ["", "\n", "no", "n", "sure", "yesterday"] {
            assert!(!confirmed(answer));
        }
        for answer in ["y", "yes", " YES\n", "Y"] {
            assert!(confirmed(answer));
        }
    }
}

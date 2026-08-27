//! `ff tower update` — download the latest release and replace this
//! binary. `--check` is the background lane: refresh the update cache,
//! print nothing.

use crate::error::CliError;
use crate::{machine, selfupdate};
use ff_tower_core::config::Config;
use ff_tower_core::ff::Ff;

pub fn run(json: bool, check: bool) -> Result<(), CliError> {
    if check {
        return refresh_cache();
    }

    let exe = selfupdate::swap::resolve_exe()?;
    let updater = selfupdate::Updater {
        api_base: "https://api.github.com".into(),
        exe,
        current_version: env!("CARGO_PKG_VERSION").into(),
        official: selfupdate::OFFICIAL,
    };

    match updater.run()? {
        selfupdate::Outcome::UpToDate { current } => {
            if json {
                println!(
                    "{}",
                    machine::emit("update", &serde_json::json!({ "current": current }))
                );
            } else {
                println!("already up to date (v{current})");
            }
        }
        selfupdate::Outcome::Updated { from, to } => {
            if json {
                println!(
                    "{}",
                    machine::emit(
                        "update",
                        &serde_json::json!({ "updated": { "from": from, "to": to } })
                    )
                );
            } else {
                println!("updated {from} → {to}");
            }
        }
    }
    Ok(())
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
    if let Ok(ff) = Ff::here()
        && let Ok(config) = Config::open(ff.repo())
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
    Ok(())
}

//! `ff tower config` — read and write tower settings stored as plain
//! git config under `tower.*`.
//!
//! No subcommands: arity decides. Bare lists the registry, a key gets,
//! key + value sets, `--unset` returns to the default, `--global`
//! widens to every repo.
//!
//! No `Store` and no fufu spawn: config is the verb you use on a
//! half-configured machine, and `Store::open` resolves the author and
//! fails without `user.email`. `Config::open` goes to gix directly, so
//! the settings stay reachable before identity exists.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::config::{self, Config, Setting, registry};

pub fn run(
    json: bool,
    key: Option<&str>,
    value: Option<String>,
    unset: bool,
    global: bool,
) -> Result<(), CliError> {
    let config = Config::open(super::ff()?.repo())?;

    let Some(key) = key else {
        return list(json, &config);
    };
    let setting = config::lookup(key)?;
    if unset {
        return unset_run(json, &config, setting, global);
    }
    match value {
        None => get(json, &config, setting),
        Some(value) => set(json, &config, setting, &value, global),
    }
}

fn list(json: bool, config: &Config) -> Result<(), CliError> {
    if json {
        let entries: Vec<serde_json::Value> = registry()
            .iter()
            .map(|setting| {
                let row = config.read(setting);
                let is_default = row.value.is_none();
                serde_json::json!({
                    "key": setting.name,
                    "git_key": setting.key,
                    "kind": setting.kind.name(),
                    "value": row.value.as_deref().unwrap_or(setting.def),
                    "source": row.source,
                    "default": is_default,
                    "description": setting.desc.join("\n"),
                })
            })
            .collect();
        println!(
            "{}",
            machine::emit("config", &serde_json::json!({ "settings": entries }))
        );
        return Ok(());
    }

    let colored = render::colored();
    for setting in registry() {
        let row = config.read(setting);
        let default_tag = if row.value.is_none() {
            format!(" {}", render::paint_dim("(default)", colored))
        } else {
            String::new()
        };
        println!(
            "{}  {}{}",
            setting.name,
            row.value.as_deref().unwrap_or(setting.def),
            default_tag
        );
        for line in setting.desc {
            println!("  {line}");
        }
        println!();
    }
    println!(
        "{}",
        render::paint_dim(
            "Set with:     ff tower config <key> <value>   (--global: every repo)",
            colored
        )
    );
    println!(
        "{}",
        render::paint_dim("Remove with:  ff tower config --unset <key>", colored)
    );
    println!(
        "{}",
        render::paint_dim("Stored as plain git config under tower.<key>", colored)
    );
    Ok(())
}

fn get(json: bool, config: &Config, setting: &'static Setting) -> Result<(), CliError> {
    let row = config.read(setting);
    let is_default = row.value.is_none();
    let display = row.value.as_deref().unwrap_or(setting.def);
    if json {
        println!(
            "{}",
            machine::emit(
                "config",
                &serde_json::json!({
                    "key": setting.name,
                    "git_key": setting.key,
                    "value": display,
                    "source": row.source,
                    "default": is_default,
                })
            )
        );
    } else {
        println!("{display}");
    }
    Ok(())
}

fn set(
    json: bool,
    config: &Config,
    setting: &'static Setting,
    value: &str,
    global: bool,
) -> Result<(), CliError> {
    // Validate first: nothing touches disk on a refusal.
    config::validate(setting, value)?;
    config.set(setting, value, global)?;

    if json {
        println!(
            "{}",
            machine::emit(
                "config",
                &serde_json::json!({
                    "key": setting.name,
                    "value": value,
                    "global": global,
                })
            )
        );
    } else {
        let scope = if global { "every repo" } else { "this repo" };
        println!("{} = {} ({scope})", setting.name, value);
    }
    Ok(())
}

fn unset_run(
    json: bool,
    config: &Config,
    setting: &'static Setting,
    global: bool,
) -> Result<(), CliError> {
    let removed = config.unset(setting, global)?;
    // The snapshot from open, with the written scope held out — what
    // still applies now that this scope no longer answers.
    let still = config.read_excluding(setting, global);

    if json {
        let still_json = still
            .value
            .as_ref()
            .map(|value| serde_json::json!({ "value": value, "source": still.source }));
        println!(
            "{}",
            machine::emit(
                "config",
                &serde_json::json!({
                    "key": setting.name,
                    "global": global,
                    "removed": removed,
                    "still_applies": still_json,
                })
            )
        );
        return Ok(());
    }

    match (removed, still.value.as_deref()) {
        (true, None) => println!(
            "{} unset — back to the default ({})",
            setting.name, setting.def
        ),
        (true, Some(value)) => println!(
            "{} unset here — {} still applies from {}",
            setting.name,
            value,
            scope_human_label(still.source.unwrap_or(""))
        ),
        (false, Some(value)) => {
            let suffix = if !global && still.source == Some("global") {
                " — try --global"
            } else {
                ""
            };
            println!(
                "{} is not set here, but {} applies from {}{}",
                setting.name,
                value,
                scope_human_label(still.source.unwrap_or("")),
                suffix
            );
        }
        (false, None) => println!(
            "{} is not set — the default ({}) applies",
            setting.name, setting.def
        ),
    }
    Ok(())
}

/// The scope labels in the refusal grammar's register.
fn scope_human_label(source: &str) -> &str {
    match source {
        "local" => "this repo",
        "global" => "global config",
        "system" => "system config",
        "env" => "the environment",
        _ => source,
    }
}

//! `ff tower serve` — the standing process, in the foreground.
//!
//! The verb's own work is small: resolve the address and the port, hand
//! them and the repository to `ff-tower-serve`, say where it landed, and
//! then not return until Ctrl-C. Everything about what is served lives in
//! that crate; everything about how a person types it lives here.
//!
//! Each lane has four sources and one parser. `--host` and `--port` take
//! an `Option<String>` rather than a parsed type for the reason `-m` is an
//! `Option<String>`: a value clap refuses is clap's usage text and clap's
//! exit 2, and a `--json` caller has to get an envelope. So the flags, the
//! two environment variables, and the two settings all run the parsers
//! core exports and all refuse identically — and the refusal names the
//! lane, which is the part the person has to go fix.
//!
//! The one sentence this verb says on its own is the warning: a bind
//! beyond the loopback puts a board with no authentication on every
//! interface it reaches, and that is worth stating once. On stderr, so a
//! machine caller's stdout is still exactly one envelope.

use std::net::IpAddr;
use std::path::Path;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::config::{self, Config, DEFAULT_HOST, DEFAULT_PORT};

pub fn run(json: bool, host: Option<&str>, port: Option<&str>) -> Result<(), CliError> {
    let ff = super::ff()?;
    let (host, port) = resolve(host, port, ff.repo())?;

    // Bound, not yet serving: the address is settled and nothing has been
    // answered, which is what makes one startup envelope possible.
    let bound = ff_tower_serve::run(ff.repo(), host, port)?;
    let addr = bound.addr();

    if json {
        println!(
            "{}",
            machine::emit(
                "serve",
                &serde_json::json!({
                    "address": addr.to_string(),
                    "host": addr.ip().to_string(),
                    "port": addr.port(),
                    "url": format!("http://{addr}/"),
                })
            )
        );
    } else {
        println!("tower serving on http://{addr}/");
        println!(
            "{}",
            render::paint_dim("Ctrl-C to stop.", render::colored())
        );
    }

    // Observe and complain, never enforce: the bind already happened and
    // this changes nothing about it. `is_loopback` is the whole test, so
    // ::1 is as quiet as 127.0.0.1.
    if !addr.ip().is_loopback() {
        eprintln!(
            "{}",
            render::paint_dim(
                "reachable from the network — this board has no authentication in front of it",
                render::colored(),
            )
        );
    }

    bound.serve()?;
    Ok(())
}

/// Both lanes at once, from the highest source that speaks: the flag,
/// then the environment, then git config, then the compiled default.
///
/// One resolver rather than two because both want the same handle, and
/// one startup should open `Config` once — lazily, so a `--host` and a
/// `--port` that both bind never need a repository whose config parses.
fn resolve(
    flag_host: Option<&str>,
    flag_port: Option<&str>,
    repo: &Path,
) -> Result<(IpAddr, u16), CliError> {
    let mut config = None;

    let host = match loud(flag_host, "--host", "TOWER_HOST") {
        Some((raw, lane)) => parse_host(&raw, lane)?,
        None => match stored(&mut config, repo, "serveHost")? {
            Some(raw) => parse_host(&raw, "tower.serveHost")?,
            // The default is a string like every other lane's value, and
            // core's test pins that it parses.
            None => parse_host(DEFAULT_HOST, "the default")?,
        },
    };

    let port = match loud(flag_port, "--port", "TOWER_PORT") {
        Some((raw, lane)) => parse_port(&raw, lane)?,
        None => match stored(&mut config, repo, "servePort")? {
            Some(raw) => parse_port(&raw, "tower.servePort")?,
            None => DEFAULT_PORT,
        },
    };

    Ok((host, port))
}

/// The two lanes above config, with the name of whichever spoke: the
/// flag, then the environment variable. An empty variable is silence
/// rather than a value nothing can parse.
fn loud<'a>(flag: Option<&str>, flag_name: &'a str, var: &'a str) -> Option<(String, &'a str)> {
    if let Some(raw) = flag {
        return Some((raw.to_string(), flag_name));
    }
    if let Some(raw) = std::env::var_os(var)
        && !raw.is_empty()
    {
        return Some((raw.to_string_lossy().into_owned(), var));
    }
    None
}

/// The config lane, opening the repository's config on first use and
/// reusing it after.
fn stored(config: &mut Option<Config>, repo: &Path, key: &str) -> Result<Option<String>, CliError> {
    let config = match config {
        Some(config) => config,
        empty => empty.insert(Config::open(repo)?),
    };
    Ok(config.read(config::lookup(key)?).value)
}

/// One parser, four sources. A name is not an address: `localhost` is
/// refused here rather than resolved, so nothing in the startup path
/// waits on DNS and a stored value stays validatable offline.
fn parse_host(raw: &str, lane: &str) -> Result<IpAddr, CliError> {
    config::parse_host(raw).ok_or_else(|| {
        CliError::coded(
            "usage/bad-host",
            format!(
                "{lane}: \"{raw}\" is not an address — want an IP like 127.0.0.1, 0.0.0.0, or ::1"
            ),
            vec!["ff tower serve --host <addr>".to_string()],
        )
    })
}

/// One parser, four sources. A port below 1024 parses here and fails at
/// bind — whether this machine lets this user have it is the operating
/// system's answer, not a second opinion held here.
fn parse_port(raw: &str, lane: &str) -> Result<u16, CliError> {
    config::parse_port(raw).ok_or_else(|| {
        CliError::coded(
            "usage/bad-port",
            format!("{lane}: \"{raw}\" is not a port — want a number from 0 to 65535"),
            vec!["ff tower serve --port <n>".to_string()],
        )
    })
}

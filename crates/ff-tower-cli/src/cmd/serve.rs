//! `ff tower serve` — the standing process, in the foreground.
//!
//! The verb's own work is small: resolve the port, hand the repository
//! and the port to `ff-tower-serve`, say where it landed, and then not
//! return until Ctrl-C. Everything about what is served lives in that
//! crate; everything about how a person types it lives here.
//!
//! The port has four lanes and one parser. `--port` takes an
//! `Option<String>` rather than an `Option<u16>` for the reason `-m` is
//! an `Option<String>`: a value clap refuses is clap's usage text and
//! clap's exit 2, and a `--json` caller has to get an envelope. So the
//! flag, `TOWER_PORT`, and `tower.servePort` all run the same parser and
//! all refuse `usage/bad-port` identically — and the refusal names the
//! lane, which is the part the person has to go fix.

use std::path::Path;

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::config::{self, Config, DEFAULT_PORT};

pub fn run(json: bool, port: Option<&str>) -> Result<(), CliError> {
    let ff = super::ff()?;
    let port = resolve(port, ff.repo())?;

    // Bound, not yet serving: the address is settled and nothing has been
    // answered, which is what makes one startup envelope possible.
    let bound = ff_tower_serve::run(ff.repo(), port)?;
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

    bound.serve()?;
    Ok(())
}

/// The port, from the highest lane that speaks: the flag, then
/// `TOWER_PORT`, then `tower.servePort`, then the compiled default.
///
/// Config is read only when the two louder lanes are silent — a `--port`
/// that binds should not need a repository whose config parses.
fn resolve(flag: Option<&str>, repo: &Path) -> Result<u16, CliError> {
    if let Some(raw) = flag {
        return parse_port(raw, "--port");
    }
    if let Some(raw) = std::env::var_os("TOWER_PORT")
        && !raw.is_empty()
    {
        return parse_port(&raw.to_string_lossy(), "TOWER_PORT");
    }
    let config = Config::open(repo)?;
    match config.read(config::lookup("servePort")?).value {
        Some(raw) => parse_port(&raw, "tower.servePort"),
        None => Ok(DEFAULT_PORT),
    }
}

/// One parser, three sources. A port below 1024 parses here and fails at
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

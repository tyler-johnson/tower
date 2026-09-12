//! Help pages against the real binary: the root page renders, `help`
//! resolves at depth, and short help stays short. All of it in a plain
//! tempdir outside any repository with `ATC_FF=/nonexistent` — help
//! must work on a machine where nothing else does. Whether every command
//! has a page is `help.rs`'s clap-tree guard, not a list here.

use std::path::Path;
use std::process::{Command, Output};

fn atc(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_atc"))
        .args(args)
        .current_dir(dir)
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("ATC_CALLSIGN")
        .env("ATC_FF", "/nonexistent")
        .env("XDG_CACHE_HOME", dir.join("cache"))
        // The update cache root forks to `LOCALAPPDATA` on Windows.
        .env("LOCALAPPDATA", dir.join("cache"))
        .env("XDG_CONFIG_HOME", dir.join("xdg"))
        .env("HOME", dir)
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", dir)
        .output()
        .expect("spawn atc")
}

fn stdout(out: &Output) -> String {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn the_long_flag_prints_the_root_page() {
    let dir = tempfile::TempDir::new().unwrap();
    let body = stdout(&atc(dir.path(), &["--help"]));
    // Usage lines spell what you type.
    assert!(body.contains("Usage: atc"), "{body}");
    // The one-line about survives as the page's first line — clap does
    // not print `about` once `long_about` is set.
    assert!(body.contains("tower: the board over fufu"), "{body}");
    // The flight-ref grammar is taught at the root.
    assert!(body.contains("<writer>#<n>"), "{body}");
    assert!(body.contains("<writer>.<seq>"), "{body}");
    assert!(body.contains("Examples:"), "{body}");
}

#[test]
fn help_command_equals_the_long_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let help_out = atc(dir.path(), &["help", "brief"]);
    let flag_out = atc(dir.path(), &["brief", "--help"]);
    assert_eq!(
        help_out.stdout, flag_out.stdout,
        "atc help brief != atc brief --help"
    );
}

#[test]
fn help_resolves_the_show_alias_to_brief() {
    let dir = tempfile::TempDir::new().unwrap();
    let help_out = atc(dir.path(), &["help", "show"]);
    let flag_out = atc(dir.path(), &["brief", "--help"]);
    assert_eq!(
        help_out.stdout, flag_out.stdout,
        "atc help show != atc brief --help"
    );
}

#[test]
fn help_board_prints_the_root_page() {
    let dir = tempfile::TempDir::new().unwrap();
    let body = stdout(&atc(dir.path(), &["help", "board"]));
    assert!(body.contains("<writer>#<n>"), "missing the grammar: {body}");
    assert!(body.contains("Examples:"), "{body}");
}

#[test]
fn short_help_stays_short() {
    let dir = tempfile::TempDir::new().unwrap();
    let short = stdout(&atc(dir.path(), &["next", "-h"]));
    assert!(
        !short.contains("Examples:"),
        "short help (-h) should not carry Examples: {short}"
    );
    let long = stdout(&atc(dir.path(), &["next", "--help"]));
    assert!(
        long.contains("Examples:"),
        "long help (--help) should carry Examples: {long}"
    );
}

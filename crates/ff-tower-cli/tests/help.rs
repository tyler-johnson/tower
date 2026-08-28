//! Help pages against the real binary: the root page renders, `help`
//! resolves at depth, and short help stays short. All of it in a plain
//! tempdir outside any repository with `TOWER_FF=/nonexistent` — help
//! must work on a machine where nothing else does. Whether every command
//! has a page is `help.rs`'s clap-tree guard, not a list here.

use std::path::Path;
use std::process::{Command, Output};

fn ff_tower(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff-tower"))
        .args(args)
        .current_dir(dir)
        .env("TOWER_FF", "/nonexistent")
        .env("XDG_CACHE_HOME", dir.join("cache"))
        // The update cache root forks to `LOCALAPPDATA` on Windows.
        .env("LOCALAPPDATA", dir.join("cache"))
        .env("XDG_CONFIG_HOME", dir.join("xdg"))
        .env("HOME", dir)
        // Windows' `HOME`: gix and git.exe read the profile from it, so
        // setting `HOME` alone leaves the runner's real one reachable.
        .env("USERPROFILE", dir)
        .env_remove("FF_REPO")
        .output()
        .expect("spawn ff-tower")
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
    let body = stdout(&ff_tower(dir.path(), &["--help"]));
    // The bin_name pin: usage lines spell what you type, not argv[0].
    assert!(body.contains("Usage: ff tower"), "{body}");
    assert!(!body.contains("Usage: ff-tower"), "{body}");
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
    let help_out = ff_tower(dir.path(), &["help", "brief"]);
    let flag_out = ff_tower(dir.path(), &["brief", "--help"]);
    assert_eq!(
        help_out.stdout, flag_out.stdout,
        "ff tower help brief != ff tower brief --help"
    );
}

#[test]
fn help_board_prints_the_root_page() {
    let dir = tempfile::TempDir::new().unwrap();
    let body = stdout(&ff_tower(dir.path(), &["help", "board"]));
    assert!(body.contains("<writer>#<n>"), "missing the grammar: {body}");
    assert!(body.contains("Examples:"), "{body}");
}

#[test]
fn help_resolves_nested_subcommands() {
    let dir = tempfile::TempDir::new().unwrap();
    let body = stdout(&ff_tower(dir.path(), &["bay", "warm", "--help"]));
    assert!(body.contains("Usage: ff tower bay warm"), "{body}");
    assert!(body.contains("Examples:"), "{body}");
}

#[test]
fn short_help_stays_short() {
    let dir = tempfile::TempDir::new().unwrap();
    let short = stdout(&ff_tower(dir.path(), &["next", "-h"]));
    assert!(
        !short.contains("Examples:"),
        "short help (-h) should not carry Examples: {short}"
    );
    let long = stdout(&ff_tower(dir.path(), &["next", "--help"]));
    assert!(
        long.contains("Examples:"),
        "long help (--help) should carry Examples: {long}"
    );
}

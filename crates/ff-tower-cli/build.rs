//! Emit the build provenance so `ff tower -v` and `ff tower version` can
//! name the commit the binary came from.
//!
//! Three variables, one fact. `TOWER_BUILD_INFO` is the display half —
//! either ` (<sha> <date>)` or the empty string — and `TOWER_BUILD_SHA` /
//! `TOWER_BUILD_DATE` are the same two values on their own, because
//! `ff tower version --json` reports them as fields and parsing them back
//! out of the display string would make that envelope depend on how the
//! line is spelled.
//!
//! All three are empty together, never one without the others: the binary
//! was built without git available (source tarball, crates.io vendor,
//! docker context without `.git`), and a sha with no date is a half-answer
//! the envelope has no honest shape for.
//!
//! **Rerun directives.** We watch the git state that determines the sha,
//! not the package files: the value depends on the commit, not on the
//! sources. Each path is resolved through `git rev-parse --git-path` so
//! the directive is worktree-safe (`.git` is a file in a worktree, not a
//! directory).
//!
//! Watched paths:
//! - `HEAD` — branch switches and detached checkouts.
//! - The current branch ref file (e.g. `refs/heads/main`) — an ordinary
//!   commit. Silently skipped when the ref is packed and the file does not
//!   exist.
//! - `packed-refs` — packed ref updates and `git gc`.
//!
//! Known limit: a commit made while a build is in flight, or dirty
//! working-tree changes, are not reflected — the sha names a commit, and
//! an uncommitted tree has none.
//!
//! A git failure (missing `.git`, `git` not on `PATH`, non-zero exit,
//! non-UTF-8 output) is deliberately silent — the build succeeds with an
//! empty `TOWER_BUILD_INFO`.
//!
//! Spawning git here is fine: the zero-spawn discipline is the binary's,
//! not the build's.
//!
//! fufu's build.rs also reserves an 8 MiB main-thread stack on Windows,
//! because its clap tree outgrew the platform's 1 MiB default. Tower's
//! tree is a fraction of that size, and fufu's own porting note says take
//! the block only for a comparably large one — so it stays behind. CI runs
//! the suite on Windows, so an overflow would surface there as a test
//! binary dying with `STATUS_STACK_OVERFLOW` before any user's release
//! binary — and this is where the `reserve_windows_stack()` block goes.

use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let dir = std::path::Path::new(&manifest_dir);

    // Emit rerun-if-changed for the git paths that determine the sha.
    // --git-path resolves through worktrees so the directive is correct
    // whether .git is a directory or a file.
    watch_git_path(dir, "HEAD");

    // Current branch ref file catches an ordinary commit.
    if let Some(sym) = run_git(dir, &["symbolic-ref", "-q", "HEAD"]) {
        watch_git_path(dir, &sym);
    }

    // packed-refs covers packed refs and git gc.
    watch_git_path(dir, "packed-refs");

    let sha = run_git(dir, &["rev-parse", "--short=7", "HEAD"]);
    let date = run_git(dir, &["log", "-1", "--format=%cs"]);

    let (sha, date, info) = match (sha, date) {
        (Some(sha), Some(date)) => {
            let info = format!(" ({} {})", sha, date);
            (sha, date, info)
        }
        _ => (String::new(), String::new(), String::new()),
    };

    println!("cargo::rustc-env=TOWER_BUILD_INFO={info}");
    println!("cargo::rustc-env=TOWER_BUILD_SHA={sha}");
    println!("cargo::rustc-env=TOWER_BUILD_DATE={date}");
}

/// Run a `git` command in `dir`. Returns `None` on any failure instead of
/// aborting the build.
fn run_git(dir: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .trim_end()
        .lines()
        .next()
        .map(|l| l.trim().to_string())
}

/// Resolve a git internal path through `--git-path` and emit
/// `cargo::rerun-if-changed` if the resulting file exists.
fn watch_git_path(dir: &std::path::Path, name: &str) {
    let resolved = match run_git(dir, &["rev-parse", "--git-path", name]) {
        Some(p) => p,
        None => return,
    };
    let path = dir.join(&resolved);
    if path.exists() {
        println!("cargo::rerun-if-changed={}", path.display());
    }
}

//! `ff tower version` against the real binary. Every run sits in a plain
//! tempdir outside any repository with `TOWER_FF=/nonexistent` — the verb
//! reads the binary and a cache file and nothing else, so a reach for
//! fufu or a repository would be the failure these tests notice. Test
//! builds are never official, so the update lane's status is always
//! `unofficial` and the "available" line never fires.

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
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn version_names_the_build() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["--version"]);

    assert!(
        out.status.success(),
        "exit status: {}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let out_str = stdout(&out).trim_end().to_string();
    let mut lines = out_str.lines();

    // Line one names the tool by the project name, not the binary's
    // dispatch name: `tower` is what the release titles and the README
    // say, and this is the output a bug report gets pasted from.
    let first = lines.next().unwrap_or_default();
    let prefix = format!("tower {}", env!("CARGO_PKG_VERSION"));
    assert!(
        first.starts_with(&prefix),
        "stdout did not start with \"{prefix}\": {out_str}"
    );

    // Line two is where to go next, and it comes from the manifest rather
    // than from a literal in the source.
    assert_eq!(
        lines.next(),
        Some(env!("CARGO_PKG_REPOSITORY")),
        "second line is the project's home: {out_str}"
    );
    assert_eq!(lines.next(), None, "two lines and no more: {out_str}");

    let rest = &first[prefix.len()..];

    if !rest.is_empty() {
        assert!(
            rest.starts_with(" (") && rest.ends_with(')'),
            "build info should be parenthesised: {rest:?}"
        );

        let inner = &rest[2..rest.len() - 1];
        let parts: Vec<&str> = inner.splitn(2, ' ').collect();
        assert_eq!(
            parts.len(),
            2,
            "build info inner should have exactly two space-separated parts: {inner:?}"
        );

        let sha = parts[0];
        assert!(
            sha.len() >= 7
                && sha
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "sha part is not 7+ lowercase hex: {sha:?}"
        );

        let date = parts[1];
        assert!(
            date.len() == 10
                && date.as_bytes()[4] == b'-'
                && date.as_bytes()[7] == b'-'
                && date
                    .chars()
                    .enumerate()
                    .all(|(i, c)| (i == 4 || i == 7) || c.is_ascii_digit()),
            "date part is not YYYY-MM-DD: {date:?}"
        );
    }

    assert!(
        out.stderr.is_empty(),
        "stderr was not empty: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The three spellings of one question. `-v` is the verb itself, so it
/// cannot drift from `ff tower version`, and `-V` — what almost every
/// other tool spells this — must be answered rather than met with clap's
/// unknown-argument error.
#[test]
fn the_version_is_asked_three_ways_and_answered_once() {
    let dir = tempfile::TempDir::new().unwrap();

    let long = ff_tower(dir.path(), &["--version"]);
    let short = ff_tower(dir.path(), &["-v"]);
    let verb = ff_tower(dir.path(), &["version"]);
    for out in [&long, &short, &verb] {
        assert!(out.status.success(), "exit 0: {:?}", out.status);
    }

    assert_eq!(stdout(&short), stdout(&long), "-v is --version");
    // The flag is the verb, so the spellings match line for line.
    let line = stdout(&long);
    assert!(
        stdout(&verb).starts_with(line.trim_end()),
        "ff tower version does not lead with the flag's line: {:?} vs {line:?}",
        stdout(&verb)
    );

    // `-V` is gone as a spelling and present as an answer.
    let shouted = ff_tower(dir.path(), &["-V"]);
    assert!(!shouted.status.success(), "-V no longer prints a version");
    let err = String::from_utf8_lossy(&shouted.stderr).to_string();
    assert!(err.contains("ff tower -v"), "names the spelling: {err}");
    assert!(err.contains("ff tower version"), "names the verb: {err}");
}

/// The envelope names the verb that ran. `ff tower -v --json` settles as
/// the version verb and not as the board, so the flag cannot answer a
/// different question from the verb it spells.
#[test]
fn the_version_flag_takes_the_envelope() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["-v", "--json"]);
    assert!(out.status.success(), "exit 0: {:?}", out.status);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("valid json");
    assert_eq!(v["tower"], 1);
    assert_eq!(
        v["cmd"], "version",
        "the flag settled as the verb, not the board"
    );
    assert_eq!(v["data"]["version"], env!("CARGO_PKG_VERSION"));
}

/// The flag does not ride another verb: `-v board` is two commands on one
/// line, refused with the two spellings that would each be right alone.
#[test]
fn the_version_flag_does_not_ride_another_verb() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["-v", "board"]);
    assert_eq!(out.status.code(), Some(2), "usage error: {:?}", out.status);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(err.contains("ff tower -v"), "names the flag: {err}");
    assert!(err.contains("ff tower version"), "names the verb: {err}");

    let out = ff_tower(dir.path(), &["-v", "board", "--json"]);
    assert_eq!(out.status.code(), Some(2), "usage error: {:?}", out.status);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("valid json");
    assert_eq!(v["error"]["id"], "usage/bad-flags");
}

/// The envelope carries the line as fields, so a caller never takes the
/// display string apart.
#[test]
fn version_json_splits_the_line_into_fields() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = ff_tower(dir.path(), &["version", "--json"]);
    assert!(out.status.success(), "exit 0: {:?}", out.status);
    let v: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("valid json");
    assert_eq!(v["tower"], 1);
    assert_eq!(v["cmd"], "version");
    assert_eq!(v["data"]["version"], env!("CARGO_PKG_VERSION"));

    // Commit and date are both recorded or both null — never one alone,
    // which is what the build script's "both or neither" rule buys.
    let commit = &v["data"]["commit"];
    let date = &v["data"]["date"];
    assert_eq!(
        commit.is_null(),
        date.is_null(),
        "half a provenance: {commit} / {date}"
    );
    if let Some(commit) = commit.as_str() {
        assert!(
            commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit()),
            "not a short sha: {commit}"
        );
        assert_eq!(date.as_str().map(str::len), Some(10), "not YYYY-MM-DD");
        // And the display line is built from exactly these two.
        let line = stdout(&ff_tower(dir.path(), &["-v"]));
        assert!(line.contains(commit), "the line drops the commit: {line}");
    }

    // The update lane always reports one of its four states, and names a
    // tag only when there is one to name.
    let status = v["data"]["update"]["status"].as_str().expect("a status");
    assert!(
        ["unofficial", "unchecked", "available", "current"].contains(&status),
        "unknown update status: {status}"
    );
    assert_eq!(
        v["data"]["update"]["latest"].is_null(),
        status != "available",
        "a tag is named exactly when one is available"
    );
}

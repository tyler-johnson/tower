//! Shared fixtures for tower's crates.
//!
//! Its own crate rather than fufu's `ff-testsupport`, which is
//! `publish = false` and stopped being reachable the moment tower left that
//! workspace.
//!
//! Two fixtures, for the two halves of the seam. [`Repo`] is a real
//! repository with a real `ff` run against it, which is the only way to
//! prove tower parses what fufu actually emits — a hand-written envelope
//! proves tower parses tower. [`FakeFf`] is a script standing in for `ff`,
//! which is the only way to reach the paths a working fufu will not
//! produce on demand: a contract from the future, a held exit, output that
//! is not an envelope.

pub mod mock_model;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Every variable the store reads to stamp a session or a callsign: the
/// session table and its pid variables, the launcher's callsign
/// override, and the marks the agent clients leave on their shells. A
/// cargo run inside a Claude Code session inherits `CLAUDECODE=1` and
/// its session id, so a harness that did not scrub these would stamp
/// `claude` on every fixture's events and key every fixture's lease by
/// the developer's own session. Core's tests assert this list covers
/// everything the store reads.
pub const AGENT_ENV: &[&str] = &[
    "ATC_SESSION",
    "ATC_PID",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_PID",
    "CODEX_SESSION_ID",
    "QWEN_CODE_SESSION_ID",
    "OPENCODE_SESSION_ID",
    "OPENCODE_PID",
    "ATC_SHELL_SESSION",
    "ATC_SHELL_PID",
    "ATC_CALLSIGN",
    "CLAUDECODE",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_SANDBOX",
    "CURSOR_AGENT",
    "QWEN_CODE",
    "OPENCODE",
];

/// Remove every [`AGENT_ENV`] variable from a spawn, so the events a
/// harness writes are stamped by what the test sets and nothing else.
/// `XDG_STATE_HOME` goes too, so a fixture's leases land under the
/// `HOME` the harness points into the fixture and never under the
/// developer's state directory; and `SHELL`, so a shell is present in
/// a fixture only when its rc file is there, whatever the developer's
/// login shell is.
pub fn scrub(command: &mut Command) {
    for name in AGENT_ENV {
        command.env_remove(name);
    }
    command.env_remove("XDG_STATE_HOME");
    command.env_remove("SHELL");
}

/// A real repository with real fufu history on it.
///
/// Requires `ff` on PATH. That is deliberate rather than unfortunate: fufu
/// is tower's runtime dependency, so a suite that mocked it away would pass
/// against a contract that had moved.
///
/// The tempdir is a root holding `repo/`, and [`Repo::path`] answers the
/// subdirectory: anything a test makes *inside* the working tree — a
/// worktree, a config root — would ride into every capture as untracked
/// noise, so the root leaves sibling slots beside `repo/` instead, inside
/// the fixture's lifetime.
pub struct Repo {
    /// Held for its lifetime alone: the root goes when the fixture does.
    _dir: tempfile::TempDir,
    repo: PathBuf,
}

impl Repo {
    /// A repository with one commit on it, initialized through `ff init` so
    /// fufu's own floor is armed the way it would be in daily use.
    pub fn new() -> Repo {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("repo");
        std::fs::create_dir(&path).expect("mkdir repo");
        let repo = Repo {
            _dir: dir,
            repo: path,
        };

        repo.ff(&["init"]);
        // Identity, a quiet default branch, and byte-for-byte line endings:
        // a test repository must not read the machine's git config for any
        // of them, or the assertions below depend on whose machine it is.
        // The line-ending pin earns its keep on the Windows runners, where
        // Git for Windows' system config sets `core.autocrlf=true`; local
        // config wins, and rides with the repository through every later
        // spawn.
        repo.git(&["config", "user.name", "tower tests"]);
        repo.git(&["config", "user.email", "tests@tower.invalid"]);
        repo.git(&["config", "core.autocrlf", "false"]);
        repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        repo.write("README.md", "tower test fixture\n");
        repo.ff(&["commit", "-m", "first: a file to have history about"]);
        repo
    }

    pub fn path(&self) -> &Path {
        &self.repo
    }

    /// Write a file, creating parent directories.
    pub fn write(&self, path: impl AsRef<Path>, contents: &str) {
        let path = self.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, contents).expect("write");
    }

    /// Run a fufu verb against this repository, and fail loudly if it did
    /// not succeed — a fixture that half-built is worse than no fixture.
    pub fn ff(&self, args: &[&str]) -> String {
        self.run("ff", args)
    }

    /// Pin this repository's tower writer id, so log ids in assertions
    /// read `pi.1` instead of whatever the machine would mint. Shell-based
    /// on purpose: testsupport stays free of gix and serde.
    pub fn pin_writer(&self, writer: &str) {
        self.git(&["config", "tower.writer", writer]);
    }

    /// Run git directly. Only for setup fufu has no verb for.
    pub fn git(&self, args: &[&str]) -> String {
        self.run("git", args)
    }

    fn run(&self, program: &str, args: &[&str]) -> String {
        let output = Command::new(program)
            .args(args)
            .current_dir(self.path())
            .env("FF_NONINTERACTIVE", "1")
            // The operator's git config must not reach fixture spawns: a
            // machine with `fufu.gitPolicy strict` would refuse the `ff git`
            // setup some tests do, and any global default could bend an
            // assertion. The local-config pins in `new` stay all the same —
            // they cover the spawns tower's own binary makes, which do not
            // inherit this env.
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_CONFIG_SYSTEM", null_device())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("FF_SESSION")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .output()
            .unwrap_or_else(|err| panic!("`{program} {}`: {err}", args.join(" ")));
        assert!(
            output.status.success(),
            "`{program} {}` exited {:?}\nstdout: {}\nstderr: {}",
            args.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

impl Default for Repo {
    fn default() -> Repo {
        Repo::new()
    }
}

/// A script standing in for `ff`, for the answers a real one will not give
/// on request.
///
/// Handed to `Ff::program()` by path rather than dropped on PATH: PATH is
/// process-global, and tower's tests run in parallel in one process.
pub struct FakeFf {
    dir: tempfile::TempDir,
    path: PathBuf,
}

impl FakeFf {
    /// A fake that writes `stdout` verbatim and exits `code`.
    pub fn saying(stdout: &str, code: i32) -> FakeFf {
        FakeFf::script(&format!(
            "printf '%s\\n' {}\nexit {code}\n",
            shell_quote(stdout)
        ))
    }

    /// A fake with an arbitrary body. `$@` is the argv tower built, which is
    /// how the command-line assertions read it back.
    ///
    /// The body is POSIX sh on every platform: unix runs it directly, and
    /// Windows runs the same body under Git Bash through a `.cmd`
    /// trampoline, so call sites write one dialect.
    #[cfg(unix)]
    pub fn script(body: &str) -> FakeFf {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ff");
        // Written by a child process, not this one. Tests run in parallel
        // in one process, and an executable written here would leave a
        // briefly-open write fd that a concurrently forking test inherits —
        // the exec of the fake then fails ETXTBSY, at random. A child's fd
        // never enters this process's table, so the race cannot exist.
        let status = Command::new("sh")
            .arg("-c")
            .arg(r#"printf '%s' "$1" > "$0" && chmod 755 "$0""#)
            .arg(&path)
            .arg(format!("#!/bin/sh\n{body}"))
            .status()
            .expect("write fake");
        assert!(status.success(), "writing the fake failed");
        FakeFf { dir, path }
    }

    /// A fake with an arbitrary body. `$@` is the argv tower built, which is
    /// how the command-line assertions read it back.
    ///
    /// The body is POSIX sh on every platform: unix runs it directly, and
    /// Windows runs the same body under Git Bash through a `.cmd`
    /// trampoline, so call sites write one dialect.
    #[cfg(windows)]
    pub fn script(body: &str) -> FakeFf {
        let dir = tempfile::tempdir().expect("tempdir");
        // CreateProcess cannot exec a shebang, so the sh body is not the
        // spawned program; the `.cmd` trampoline below hands it to bash.
        // The body lands at `ff` — extensionless, exactly the unix name —
        // because bodies write `"$0.argv"` and `$(dirname "$0")/calls.log`
        // and tests read those paths back through `dir()`, so `$0` must be
        // `.../ff` exactly. Written directly where unix needs the
        // child-process dance: ETXTBSY is a fork/exec artifact, and Windows
        // handles are not inherited unless marked, so no concurrent spawn
        // ever holds this write handle open.
        let body_path = dir.path().join("ff");
        std::fs::write(&body_path, body).expect("write fake body");
        // The trampoline flips `%~dp0`'s backslashes to forward slashes
        // before handing bash the path: with backslashes, `dirname "$0"`
        // answers `.` and the $0-relative writes above land in the spawn's
        // working directory instead of the fake's.
        let path = dir.path().join("ff.cmd");
        let trampoline = [
            "@echo off",
            "setlocal",
            "set \"DIR=%~dp0\"",
            "set \"DIR=%DIR:\\=/%\"",
            "bash \"%DIR%ff\" %*",
            "exit /b %ERRORLEVEL%",
            "",
        ]
        .join("\r\n");
        std::fs::write(&path, trampoline).expect("write trampoline");
        FakeFf { dir, path }
    }

    /// The path to hand `Ff::program()`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A directory the fake can write to, for a script that records what it
    /// was called with.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }
}

/// The OS's null device, for pointing git config paths at nothing.
pub fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

/// A fixture path the way a verb reports it: the main worktree gix
/// discovers from the working directory. On macOS the tempdir sits
/// behind the `/var` link to `/private/var` and the working directory
/// comes back resolved, so unix resolves too; Windows hands a directory
/// back as it was entered, 8.3 aliases and all, so it is left alone —
/// `std::fs::canonicalize` there would expand the alias and add the
/// `\\?\` prefix, and the verb prints neither.
pub fn as_reported(path: &Path) -> PathBuf {
    if cfg!(unix) {
        std::fs::canonicalize(path)
            .unwrap_or_else(|err| panic!("canonicalize {}: {err}", path.display()))
    } else {
        path.to_path_buf()
    }
}

/// A string with backslashes flipped to `/`, for containment assertions
/// where ff chose the separators on one side and the test built the other
/// natively.
pub fn slashes(text: &str) -> String {
    text.replace('\\', "/")
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

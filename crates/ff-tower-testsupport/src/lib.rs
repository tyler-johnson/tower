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

use std::path::{Path, PathBuf};
use std::process::Command;

/// A real repository with real fufu history on it.
///
/// Requires `ff` on PATH. That is deliberate rather than unfortunate: fufu
/// is tower's runtime dependency, so a suite that mocked it away would pass
/// against a contract that had moved.
pub struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    /// A repository with one commit on it, initialized through `ff init` so
    /// fufu's own floor is armed the way it would be in daily use.
    pub fn new() -> Repo {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = Repo { dir };

        repo.ff(&["init"]);
        // Identity and a quiet default branch: a test repository must not
        // read the machine's git config for either, or the assertions below
        // depend on whose machine it is.
        repo.git(&["config", "user.name", "tower tests"]);
        repo.git(&["config", "user.email", "tests@tower.invalid"]);
        repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);

        repo.write("README.md", "tower test fixture\n");
        repo.ff(&["commit", "-m", "first: a file to have history about"]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Write a file, creating parent directories.
    pub fn write(&self, path: impl AsRef<Path>, contents: &str) {
        let path = self.dir.path().join(path);
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

    /// Run git directly. Only for setup fufu has no verb for.
    pub fn git(&self, args: &[&str]) -> String {
        self.run("git", args)
    }

    fn run(&self, program: &str, args: &[&str]) -> String {
        let output = Command::new(program)
            .args(args)
            .current_dir(self.dir.path())
            .env("FF_NONINTERACTIVE", "1")
            .env_remove("FF_SESSION")
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
    pub fn script(body: &str) -> FakeFf {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ff");
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("write fake");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod fake");
        }
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

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

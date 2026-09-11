//! The seam: everything tower knows about a repository, read by spawning
//! `ff <verb> --json` and parsing the envelope.
//!
//! tower spawns fufu; it does not link it. The reasons are in this crate's
//! own docs, and the consequence is this module: one place that builds a
//! command line, one place that checks a contract version, one place that
//! turns an error envelope into a Rust error. Nothing else in tower shells
//! out to `ff`.
//!
//! # The envelope
//!
//! Every `--json` emission is `{"ff": <version>, "cmd": <verb>, …}` with
//! either `data` or `error` and never both. tower checks the version before
//! it looks at the payload, which is the reason the number is there.
//!
//! # Exit codes are an outcome, not a verdict on the envelope
//!
//! fufu's codes are 0 done, 1 no, 2 bad command line, 3 held. A command can
//! exit 3 and still have emitted a perfectly good `data` envelope — held is
//! a thing that happened, with a report. So the code is carried on [`Run`]
//! beside the payload rather than being treated as a failure, and only an
//! `error` envelope (or no envelope at all) is an [`Error`].
//!
//! # The streaming half
//!
//! `ff watch` is newline-delimited JSON over a process that does not exit,
//! so the buffered spawn behind every other verb cannot carry it.
//! [`Ff::watch_all`] builds that command line and hands it over unspawned;
//! the subscriber — serve's change feed — owns the child, its lines, and
//! its respawn.

mod error;
mod payload;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

pub use error::{Error, Refusal, Result};
pub use payload::Version;

use serde::Deserialize;
use serde_json::value::RawValue;

/// The JSON contract tower reads.
///
/// Checked for equality, not for a floor. The version exists to be looked at
/// before an envelope is parsed, and fufu moves it when the shape breaks —
/// so a number tower has not been taught is a payload tower should refuse in
/// one line rather than guess at three levels down.
pub const CONTRACT: u32 = 1;

/// How much of a process's output an [`Error::Unparsable`] carries. Enough
/// to recognize a usage error or a shim's banner; short of pasting a log.
const SNIPPET: usize = 400;

/// A fufu invocation that returned an answer, and the code it exited with.
#[derive(Debug, Clone)]
pub struct Run<T> {
    pub data: T,
    pub exit: Exit,
}

/// What the shell was told, in fufu's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0 — done, or yes.
    Done,
    /// 1 — no: it failed, or the check's answer is negative.
    No,
    /// 2 — the command line was wrong. tower built it, so this is tower's
    /// bug and not the user's.
    Usage,
    /// 3 — held: nothing was touched and a human decision is required.
    Held,
    /// Anything else, or termination by a signal (`None`).
    Other(Option<i32>),
}

impl Exit {
    fn of(status: std::process::ExitStatus) -> Exit {
        match status.code() {
            Some(0) => Exit::Done,
            Some(1) => Exit::No,
            Some(2) => Exit::Usage,
            Some(3) => Exit::Held,
            other => Exit::Other(other),
        }
    }
}

/// A repository, addressed for asking fufu questions about.
///
/// Every call carries `-C <dir>`, so one tower process can ask every bay in
/// the pool without any of them being the current directory. That is the
/// whole reason the handle holds a path rather than relying on where the
/// process happens to be standing.
#[derive(Debug, Clone)]
pub struct Ff {
    program: OsString,
    repo: PathBuf,
}

impl Ff {
    /// A specific worktree — a repository under test, or the one a verb
    /// was invoked from.
    pub fn at(repo: impl Into<PathBuf>) -> Ff {
        Ff {
            program: OsString::from("ff"),
            repo: repo.into(),
        }
    }

    /// Point at a different `ff`. For tests, and for the day a config key
    /// has to name one.
    #[must_use]
    pub fn program(mut self, program: impl Into<OsString>) -> Ff {
        self.program = program.into();
        self
    }

    /// [`program`](Ff::program) from the environment: a non-empty
    /// `ATC_FF` names the `ff` to spawn. The test seam for every
    /// surface that answers requests — environment carries addressing,
    /// argv carries verbs, the seam's own discipline — and an env var
    /// cannot leak into an interactive shell the way a hidden flag one
    /// autocomplete away could.
    #[must_use]
    pub fn env_program(mut self) -> Ff {
        if let Some(program) = std::env::var_os("ATC_FF")
            && !program.is_empty()
        {
            self.program = program;
        }
        self
    }

    /// The worktree this handle asks about.
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// `ff version --json` — what is installed, repo-independent.
    ///
    /// The doctor's drift check: a fufu speaking another contract fails
    /// here as [`Error::Contract`] before any other read is attempted,
    /// and a missing `ff` as [`Error::NotInstalled`] — for doctor those
    /// are findings, not failures.
    pub fn version(&self) -> Result<Version> {
        Ok(self.run::<Version>("version", &[] as &[&str])?.data)
    }

    /// `ff watch --all` — the streaming half, built and handed over
    /// rather than spawned: watch never exits, so [`Ff::run`]'s buffered
    /// spawn cannot carry it and the caller owns the child.
    ///
    /// It mirrors the buffered spawn's conventions where they apply —
    /// `-C <repo>`, `FF_NONINTERACTIVE=1`, and `FF_SESSION` scrubbed —
    /// and drops the one that does not: no `--json`, because watch is
    /// always JSON.
    #[must_use]
    pub fn watch_all(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.arg("-C").arg(&self.repo);
        command.arg("watch").arg("--all");
        command.env("FF_NONINTERACTIVE", "1");
        command.env_remove("FF_SESSION");
        command
    }

    /// Run one fufu verb and deserialize its `data` payload.
    ///
    /// Public because the seam is meant to be widened by callers rather than
    /// by guesswork here: a verb earns a typed wrapper above when something
    /// in tower reads it, and until then this is how it is reached.
    pub fn run<T: for<'de> Deserialize<'de>>(
        &self,
        verb: &str,
        args: &[impl AsRef<OsStr>],
    ) -> Result<Run<T>> {
        let output = self.spawn(verb, args)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit = Exit::of(output.status);

        let line = stdout.trim();
        if line.is_empty() {
            return Err(self.unparsable(verb, "no output", "", &stderr));
        }

        // Two passes, and the order is the point. The payload stays a
        // `RawValue` until the version and the verb have been checked, so a
        // contract tower does not read reports itself as one line about the
        // contract rather than as a missing field inside a shape that
        // changed under it.
        let envelope: Envelope = match serde_json::from_str(line) {
            Ok(envelope) => envelope,
            Err(err) => return Err(self.unparsable(verb, &err.to_string(), line, &stderr)),
        };

        if envelope.ff != CONTRACT {
            return Err(Error::Contract {
                verb: verb.to_string(),
                expected: CONTRACT,
                found: envelope.ff,
            });
        }
        if envelope.cmd != verb {
            return Err(Error::Mismatched {
                asked: verb.to_string(),
                answered: envelope.cmd,
            });
        }

        // An error envelope is fufu saying no, whatever the exit code said.
        // The two agree in practice; the envelope is the one with words in
        // it, so it wins.
        if let Some(refusal) = envelope.error {
            return Err(Error::Ff(refusal));
        }

        // An absent `data` and a `null` one are the same fact — there is no
        // payload — so both are handed to `T` as `null` rather than being
        // refused here. A caller asking for `Option<_>` gets `None`; one
        // asking for a struct gets the same parse failure it would have got
        // from any other malformed envelope.
        let raw = envelope.data.map_or("null", RawValue::get);
        match serde_json::from_str::<T>(raw) {
            Ok(data) => Ok(Run { data, exit }),
            Err(err) => Err(self.unparsable(verb, &err.to_string(), line, &stderr)),
        }
    }

    fn spawn(&self, verb: &str, args: &[impl AsRef<OsStr>]) -> Result<std::process::Output> {
        let mut command = Command::new(&self.program);

        // fufu's own flags first, the verb's after it. `-C` is global and
        // would parse in either place; `--json` is the verb's and would
        // not, so it goes directly after the verb where no positional
        // argument can ever swallow it.
        command.arg("-C").arg(&self.repo);
        // A verb can be two words — `op log` — and each word is its own
        // argv token. The envelope still answers with the full string,
        // which is what `run` compares against.
        for word in verb.split(' ') {
            command.arg(word);
        }
        command.arg("--json");
        for arg in args {
            command.arg(arg);
        }

        // Nothing tower calls may prompt or open an editor. A board render
        // that blocked on a question from a subprocess would be a board
        // nobody types twice.
        command.env("FF_NONINTERACTIVE", "1");

        // tower tags nothing: an inherited `FF_SESSION` from tower's own
        // dispatch would otherwise reach an adapter two processes down,
        // naming a flight that is not the one being flown.
        command.env_remove("FF_SESSION");

        command.output().map_err(|source| {
            let program = self.program.to_string_lossy().into_owned();
            if source.kind() == std::io::ErrorKind::NotFound {
                Error::NotInstalled { program }
            } else {
                Error::Spawn { program, source }
            }
        })
    }

    fn unparsable(&self, verb: &str, detail: &str, stdout: &str, stderr: &str) -> Error {
        Error::Unparsable {
            verb: verb.to_string(),
            detail: detail.to_string(),
            stdout: snippet(stdout),
            stderr: snippet(stderr),
        }
    }
}

/// The envelope, with the payload still unparsed. `data` and `error` are
/// never both present; both are optional here so that whichever arrived is
/// read after the version check rather than before it.
#[derive(Deserialize)]
struct Envelope<'a> {
    ff: u32,
    cmd: String,
    #[serde(borrow, default)]
    data: Option<&'a RawValue>,
    #[serde(default)]
    error: Option<Refusal>,
}

fn snippet(text: &str) -> String {
    let text = text.trim();
    match text.char_indices().nth(SNIPPET) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The watch command line, read back without a spawn: the argv is
    /// exactly `-C <repo> watch --all`, the environment is
    /// noninteractive, and the session is scrubbed.
    #[test]
    fn watch_all_builds_the_argv_and_spawns_nothing() {
        let command = Ff::at("/somewhere").watch_all();

        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-C", "/somewhere", "watch", "--all"]);

        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs.contains(&(OsStr::new("FF_NONINTERACTIVE"), Some(OsStr::new("1")))));
        assert!(envs.contains(&(OsStr::new("FF_SESSION"), None)));
    }
}

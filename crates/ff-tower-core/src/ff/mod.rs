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
//! # Reads are not side-effect-free
//!
//! `ff status --json` takes a capture first, like every fufu verb. Folding
//! the board against a dirty tree appends an operation to that worktree's
//! chain. It is a no-op when nothing changed, and when something did change
//! it is fufu's floor doing its job — but "tower only reads" is true of
//! tower's own store and not of the repository underneath it.
//!
//! # Not the streaming half
//!
//! `ff watch` is newline-delimited JSON over a process that does not exit,
//! and it needs a subscription rather than a call. It gets its own path when
//! there is something to subscribe on its behalf.

mod error;
mod payload;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

pub use error::{Error, Refusal, Result};
pub use payload::{
    At, ChangeKind, Collisions, Editing, FileStat, Head, Held, Open, Pair, Pairing, Side, Status,
    UnknownReason,
};

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
    session: Option<String>,
}

impl Ff {
    /// The repository tower was invoked against.
    ///
    /// `FF_REPO` when fufu's dispatch set it — absolute, resolved, and
    /// absent rather than empty outside a worktree — and the current
    /// directory otherwise, which is the case when `ff-tower` is run
    /// directly instead of through `ff tower`.
    ///
    /// Reading it back is what the handshake is for: an extension that had
    /// to rediscover its own repository would be guessing at the answer
    /// fufu already told it. This is not git's `GIT_DIR` footgun in
    /// reverse — tower re-exports nothing, and passes what it read as an
    /// explicit `-C` on each call rather than through the environment.
    pub fn here() -> Result<Ff> {
        let repo = match std::env::var_os("FF_REPO") {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => std::env::current_dir().map_err(|source| Error::Spawn {
                program: "ff".into(),
                source,
            })?,
        };
        Ok(Ff::at(repo))
    }

    /// A specific worktree — a bay in the pool, or a repository under test.
    pub fn at(repo: impl Into<PathBuf>) -> Ff {
        Ff {
            program: OsString::from("ff"),
            repo: repo.into(),
            session: None,
        }
    }

    /// Tag every call from this handle with a session name.
    ///
    /// The design's rule verbatim: every fufu call tower makes carries
    /// `--session <flight>`, so a flight's captures group into one `ff undo`
    /// step and `ff watch --session` narrows to that flight's own motion.
    /// Per-flight capture chains fall out of the tagging and cost nothing
    /// else.
    #[must_use]
    pub fn session(mut self, name: impl Into<String>) -> Ff {
        self.session = Some(name.into());
        self
    }

    /// Point at a different `ff`. For tests, and for the day a config key
    /// has to name one.
    #[must_use]
    pub fn program(mut self, program: impl Into<OsString>) -> Ff {
        self.program = program.into();
        self
    }

    /// The worktree this handle asks about.
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// `ff status --json` for this worktree.
    ///
    /// Returns the payload rather than a [`Run`]: status reports a hold, it
    /// never *is* one, so there is no exit code here a caller could act on.
    pub fn status(&self) -> Result<Status> {
        Ok(self.run::<Status>("status", &[] as &[&str])?.data)
    }

    /// `ff collide --json` — which branches in flight would collide.
    ///
    /// `names` empty means the ones the map would rank; `limit` is how many
    /// branches to rank in, newest tip first, with `None` meaning fufu's own
    /// default rather than all of them. A collision is a finding and not a
    /// failure, so this exits 0 either way.
    pub fn collide(&self, names: &[String], limit: Option<usize>) -> Result<Collisions> {
        let mut args: Vec<OsString> = Vec::new();
        if let Some(limit) = limit {
            args.push("--max-count".into());
            args.push(limit.to_string().into());
        }
        args.extend(names.iter().map(OsString::from));
        Ok(self.run::<Collisions>("collide", &args)?.data)
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

        // fufu's own flags first, the verb's after it. `-C` and `--session`
        // are global and would parse in either place; `--json` is the verb's
        // and would not, so it goes directly after the verb where no
        // positional argument can ever swallow it.
        command.arg("-C").arg(&self.repo);
        if let Some(session) = &self.session {
            command.arg("--session").arg(session);
        }
        command.arg(verb).arg("--json");
        for arg in args {
            command.arg(arg);
        }

        // Nothing tower calls may prompt or open an editor. A board render
        // that blocked on a question from a subprocess would be a board
        // nobody types twice.
        command.env("FF_NONINTERACTIVE", "1");

        // fufu emits `FF_SESSION` and never reads it back, so the flag above
        // is what actually tags the call. This keeps the environment from
        // disagreeing with it anyway: an inherited tag from tower's own
        // dispatch would otherwise reach an adapter two processes down,
        // naming a flight that is not the one being flown.
        match &self.session {
            Some(session) => command.env("FF_SESSION", session),
            None => command.env_remove("FF_SESSION"),
        };

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

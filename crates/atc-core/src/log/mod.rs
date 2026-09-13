//! tower's own store: an append-only event log on
//! `refs/tower/log/<author>/<writer>`, an orphan commit chain that never
//! touches the working tree and never touches `refs/fufu/*`.
//!
//! Stored intent, never derived state. Derived fields have zero merge
//! surface and self-heal when someone works around tower; what a person
//! authored is derivable from nothing, which makes this the first tower
//! state that is not a cache — and why the store had to be right before
//! anything was written into it.
//!
//! The log is partitioned per writer, so merging divergent logs is a
//! union, not a merge — conflict-free by construction. The board (slice 3)
//! is a fold over that union. One ref per author alone would break the
//! moment two machines append under one email: both chains diverge and a
//! push is rejected with no merge available, because a commit chain has no
//! union. The writer component makes every push a fast-forward.
//!
//! The union fold orders by `(time, writer, seq)` — last-writer-wins by
//! wall clock with a stable tiebreak. Clocks disagree across machines and
//! that is accepted: both events survive in the log regardless. Within one
//! writer, order is a fact rather than an estimate, so append clamps its
//! clock to the tip's, and a clock stepping backwards can never contradict
//! `seq`.

mod chain;
mod error;
mod event;
mod lock;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gix::refs::transaction::PreviousValue;

use crate::config::Config;
use crate::lease;

pub use chain::{ATC_EMAIL, ATC_NAME, CHAIN_VERSION};
pub use error::{Error, Result};
pub use event::{Event, EventId, Kind, RETIRED_KINDS};

/// The store, open on one repository.
///
/// The author is resolved at open — a store that cannot say who is filing
/// should say so before anything leans on it. The writer is resolved from
/// config at open and minted on the first append when there is none, so
/// opening a store writes nothing to the repository; the one thing it
/// touches is the session's lease, outside it.
pub struct Store {
    repo: gix::Repository,
    author: String,
    identity: Identity,
    /// The word `atc callsign` wrote after open, so the move it appends
    /// carries the new byline.
    adopted: std::cell::OnceCell<String>,
    writer: std::cell::OnceCell<String>,
}

impl Store {
    /// Open the store on the repository containing `path`.
    ///
    /// Opening resolves the identity, and that is the one write an open
    /// makes: the session's lease is renewed — created on the first
    /// call, its body rewritten when the pid changed or when this
    /// repository is not the last it was seen in — so every `atc` call
    /// under a session is a heartbeat. A lease that will not write is
    /// not the verb's problem.
    pub fn open(path: &Path) -> Result<Store> {
        let repo = gix::discover(path).map_err(Error::repo)?;
        let author = resolve_author(&repo)?;
        let identity = identity_from_environment(&repo);
        let writer = std::cell::OnceCell::new();
        if let Some(configured) = configured_writer(&repo) {
            validate_component("writer", &configured)?;
            let _ = writer.set(configured);
        }
        Ok(Store {
            repo,
            author,
            identity,
            adopted: std::cell::OnceCell::new(),
            writer,
        })
    }

    /// Who events file under: git `user.email`.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// The session every append is tagged with, when there is one: the
    /// first [`SESSION_VARS`] row set, or the login name at a terminal.
    pub fn session(&self) -> Option<&str> {
        self.identity.session.as_deref()
    }

    /// The callsign every append is stamped with, when there is one:
    /// [`CALLSIGN_VAR`] as the override, else the word this session's
    /// lease holds, else the client detected from its own mark
    /// ([`CLIENT_MARKERS`]), else the login name at a terminal. The
    /// pilot — which the session, a run, and the author, an email,
    /// never say.
    pub fn callsign(&self) -> Option<&str> {
        self.adopted
            .get()
            .map(String::as_str)
            .or(self.identity.callsign.as_deref())
    }

    /// Everything identity-related, resolved at open: the session and
    /// where it came from, the pid, the callsign and its source, the
    /// lease's state, and the one notice a stale lease may have earned.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// The word `atc callsign` just wrote into the lease: every append
    /// from here on is stamped with it. Once per store.
    pub fn adopt_callsign(&self, word: String) {
        let _ = self.adopted.set(word);
    }

    /// This machine's writer id, once one exists — `None` until the first
    /// append mints it.
    pub fn writer(&self) -> Option<&str> {
        self.writer.get().map(String::as_str)
    }

    /// The settings registry over this store's repository. Core reads
    /// config, the CLI stays gix-free. A verb that needs a setting reads
    /// it here rather than discovering the repository a second time.
    pub fn config(&self) -> Config {
        Config::from_repo(self.repo.clone())
    }

    /// The main worktree's path — the parent of the common dir — or
    /// `None` for a bare repository, which has no tree to anchor to.
    ///
    /// The CLI stays gix-free, and this is the anchor `.tower/procedures`
    /// resolves against. The main worktree rather than the invoking one,
    /// because every worktree must read the same definitions the
    /// repository does. No spawn — `file` keeps the property that it
    /// never runs fufu.
    pub fn main_worktree(&self) -> Option<PathBuf> {
        if self.repo.is_bare() {
            return None;
        }
        self.repo
            .common_dir()
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }

    /// The filesystem paths a change feed watches to notice this store
    /// moving, however it was moved — this process, another tower, a
    /// push landing, `git pack-refs`.
    ///
    /// Beside [`Store::main_worktree`] and on its pattern: everything is
    /// derived from the common dir, which `common_dir()` already
    /// resolves through the `.git`-file of a linked worktree. `refs`
    /// always exists; `log` may not before the first append; the
    /// packed-refs file may never. The append lock lives at
    /// `<common>/tower/`, outside all three, so lock churn never reaches
    /// a watch.
    pub fn watch_paths(&self) -> WatchPaths {
        let common = self.repo.common_dir();
        WatchPaths {
            refs: common.join("refs"),
            log: common.join(chain::LOG_PREFIX.trim_end_matches('/')),
            packed_refs: common.join("packed-refs"),
        }
    }

    /// Append events as one commit, assigning ids `<writer>.<seq>` in
    /// order. Returns the assigned ids; an empty batch writes nothing.
    pub fn append(&self, kinds: Vec<Kind>) -> Result<Vec<EventId>> {
        // Ahead of the delegation, so an empty append still mints no
        // writer.
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        self.append_with(move |_| kinds.clone())
    }

    /// Append a batch whose events name each other's ids: `plan` receives a
    /// minter — `mint(n)` is the id this batch's `n`th event will take — and
    /// returns the kinds in that same order. Re-run per attempt, because a
    /// lost CAS reparents the batch onto a new tip with fresh seqs.
    ///
    /// The plan cannot be built ahead of the append and handed to
    /// [`Store::append`]: seqs are assigned inside the retry loop below.
    /// Two appends would do instead — file, then link — but they leave a
    /// window where the parent is live, unlinked, and claimable.
    pub fn append_with(
        &self,
        plan: impl Fn(&dyn Fn(usize) -> EventId) -> Vec<Kind>,
    ) -> Result<Vec<EventId>> {
        let writer = self.writer_or_mint()?;
        let name = chain::log_ref(&self.author, &writer);

        // The lock spans read-tip → write-objects → move-ref, because that
        // span is the whole of the race: gix checks `MustExistAndMatch`
        // against a value it read before locking, so the CAS alone would
        // let a second writer overwrite an append that already reported
        // success. See `log/lock.rs`.
        let _held = lock::acquire(&self.repo, &writer)?;

        // A lost CAS under the lock means a foreign mover — a push landing
        // here, a hand-moved ref. Retrying is always safe: the same events
        // re-parented onto the new tip, with fresh seqs to match.
        for _ in 0..3 {
            let tip = ref_target(&self.repo, &name)?;
            let (next_seq, tip_time) = match tip {
                Some(tip) => {
                    let tip = chain::decode(&self.repo, tip)?;
                    (tip.next_seq, tip.time)
                }
                None => (1, 0),
            };
            // Clamped to the tip so one writer's events stay monotonic in
            // the sort key no matter what the clock does.
            let now = wall_clock().max(tip_time);

            let kinds = plan(&|offset| EventId {
                writer: writer.clone(),
                seq: next_seq + offset as u64,
            });
            if kinds.is_empty() {
                return Ok(Vec::new());
            }

            let events: Vec<Event> = kinds
                .into_iter()
                .enumerate()
                .map(|(offset, kind)| Event {
                    id: EventId {
                        writer: writer.clone(),
                        seq: next_seq + offset as u64,
                    },
                    author: self.author.clone(),
                    writer: writer.clone(),
                    time: now,
                    session: self.identity.session.clone(),
                    callsign: self.callsign().map(str::to_string),
                    kind,
                })
                .collect();

            let commit = chain::write_events(
                &self.repo,
                &events,
                next_seq + events.len() as u64,
                tip,
                now,
            )?;
            let expected = match tip {
                Some(tip) => PreviousValue::MustExistAndMatch(gix::refs::Target::Object(tip)),
                None => PreviousValue::MustNotExist,
            };
            let reflog = format!("append: {} event(s)", events.len());
            match chain::move_ref(&self.repo, &name, commit, expected, now, &reflog)? {
                chain::EditOutcome::Applied => {
                    return Ok(events.into_iter().map(|event| event.id).collect());
                }
                chain::EditOutcome::Contended => continue,
            }
        }
        Err(Error::Contended { writer })
    }

    /// This writer's chain, oldest first. No writer yet means nothing was
    /// ever written here: an empty log, not an error.
    pub fn read(&self) -> Result<Vec<Event>> {
        let Some(writer) = self.writer() else {
            return Ok(Vec::new());
        };
        let name = chain::log_ref(&self.author, writer);
        match ref_target(&self.repo, &name)? {
            None => Ok(Vec::new()),
            Some(tip) => walk(&self.repo, tip),
        }
    }

    /// The union of every chain under `refs/tower/log/`, ordered by
    /// `(time, writer, seq)`.
    pub fn read_all(&self) -> Result<Vec<Event>> {
        // Pointers first: ref iteration must not overlap ref edits.
        let mut tips = Vec::new();
        {
            let platform = self.repo.references().map_err(Error::repo)?;
            let iter = platform.prefixed(chain::LOG_PREFIX).map_err(Error::repo)?;
            for reference in iter {
                let reference = reference.map_err(Error::repo)?;
                if let Some(tip) = reference.target().try_id() {
                    tips.push(tip.to_owned());
                }
            }
        }
        let mut events = Vec::new();
        for tip in tips {
            events.extend(walk(&self.repo, tip)?);
        }
        events.sort_by(|a, b| (a.time, &a.writer, a.id.seq).cmp(&(b.time, &b.writer, b.id.seq)));
        Ok(events)
    }

    fn writer_or_mint(&self) -> Result<String> {
        if let Some(writer) = self.writer.get() {
            return Ok(writer.clone());
        }
        let minted = mint_writer(&self.repo)?;
        validate_component("writer", &minted)?;
        let _ = self.writer.set(minted.clone());
        Ok(minted)
    }
}

/// Where the log shows up on disk, for a watcher. Loose ref writes land
/// under `log`, `git pack-refs` moves tips into `packed_refs`, and
/// `refs` is the directory that exists before either does.
pub struct WatchPaths {
    /// `<common>/refs` — always present, the root a recursive watch
    /// installs on so it sees `refs/tower/log` born.
    pub refs: PathBuf,
    /// `<common>/refs/tower/log` — where every chain's loose ref lives.
    pub log: PathBuf,
    /// `<common>/packed-refs` — where `git pack-refs` moves tips.
    pub packed_refs: PathBuf,
}

/// Parent walk from the tip, newest-first, reversed at the end. The decode
/// gate runs on every commit, so nothing tower did not write gets past it.
fn walk(repo: &gix::Repository, tip: gix::ObjectId) -> Result<Vec<Event>> {
    let mut batches = Vec::new();
    let mut cursor = Some(tip);
    while let Some(id) = cursor {
        let decoded = chain::decode(repo, id)?;
        cursor = decoded.parent;
        batches.push(decoded.events);
    }
    batches.reverse();
    Ok(batches.into_iter().flatten().collect())
}

/// The direct target of a ref, if it exists. tower writes only direct
/// refs, so a symbolic one on a tower path is somebody else's doing.
fn ref_target(repo: &gix::Repository, name: &str) -> Result<Option<gix::ObjectId>> {
    match repo.try_find_reference(name).map_err(Error::repo)? {
        Some(reference) => match reference.target().try_id() {
            Some(id) => Ok(Some(id.to_owned())),
            None => Err(Error::repo(format!(
                "{name} is symbolic; tower writes only direct refs"
            ))),
        },
        None => Ok(None),
    }
}

fn wall_clock() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// git `user.email`, the partition key for everything this store writes.
/// Unset is fatal by design: tower's own machinery signs as tower, but
/// events belong to a person.
fn resolve_author(repo: &gix::Repository) -> Result<String> {
    let sig = repo
        .committer()
        .transpose()
        .map_err(Error::repo)?
        .ok_or(Error::Identity)?;
    let author = sig.email.to_string();
    validate_component("author", &author)?;
    Ok(author)
}

/// The session an append is tagged with, by three rules in order: the
/// first set and usable row of the session table, the login name when
/// a person is at the terminal, else none. Beside the id, where it came
/// from: the row's source, or `login`.
///
/// A row's value is accepted under one rule: trimmed, non-empty, no
/// control characters, at most 128 bytes — [`usable_session`]. An
/// unusable value is ignored rather than fatal — the session is a
/// byline, not identity — and the walk moves to the next row. `login`
/// is the first set login variable, and `git_name` the committer's
/// name, the fallback when a terminal has no login variable.
pub(crate) fn resolve_session(
    rows: &[(Option<&str>, &'static str)],
    interactive: bool,
    login: Option<&str>,
    git_name: Option<&str>,
) -> Option<(String, &'static str)> {
    if let Some(found) = rows
        .iter()
        .find_map(|(value, source)| value.and_then(usable_session).map(|id| (id, *source)))
    {
        return Some(found);
    }
    if !interactive {
        return None;
    }
    login
        .and_then(usable_session)
        .or_else(|| git_name.and_then(usable_session))
        .map(|id| (id, "login"))
}

/// The one rule a session id is held to: trimmed, non-empty, no
/// control characters, at most 128 bytes. `Some` is the usable id.
pub fn usable_session(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && !trimmed.chars().any(char::is_control) && trimmed.len() <= 128)
        .then(|| trimmed.to_string())
}

/// One way a session names itself in the process environment: the
/// variable, the variable its pid rides in when the client hands one
/// down, and the word the source is reported as.
#[derive(Debug)]
pub struct SessionVar {
    pub var: &'static str,
    pub pid_var: Option<&'static str>,
    pub source: &'static str,
}

/// The session table, walked in order, first set and usable wins.
/// `ATC_SESSION` is the launcher's own tag, set per worker by a
/// person's orchestrator, and beats the client's; the clients' own
/// follow — Claude Code's on every process it spawns, Codex's, Qwen
/// Code's on hooks and shell tools alike, and the one tower's OpenCode
/// plugin sets, with `OPENCODE_PID` beside it, which OpenCode's shell
/// tool sets to its own pid; `ATC_SHELL_SESSION` is a
/// terminal's, minted by its rc lines, and last on purpose: a
/// terminal's variables are inherited by every agent launched from it,
/// and the agent's own row must win. A pid is read from the row that
/// named the session and never from an inherited one; a row with no
/// pid variable gets the lease window alone. No parent-process walk:
/// Codex runs commands in a pid namespace where the client is pid 1,
/// and a guess there holds or frees the wrong word.
pub const SESSION_VARS: &[SessionVar] = &[
    SessionVar {
        var: "ATC_SESSION",
        pid_var: Some("ATC_PID"),
        source: "launcher",
    },
    SessionVar {
        var: "CLAUDE_CODE_SESSION_ID",
        pid_var: Some("CLAUDE_PID"),
        source: "claude",
    },
    SessionVar {
        var: "CODEX_SESSION_ID",
        pid_var: None,
        source: "codex",
    },
    SessionVar {
        var: "QWEN_CODE_SESSION_ID",
        pid_var: None,
        source: "qwen",
    },
    SessionVar {
        var: "OPENCODE_SESSION_ID",
        pid_var: Some("OPENCODE_PID"),
        source: "opencode",
    },
    SessionVar {
        var: "ATC_SHELL_SESSION",
        pid_var: Some("ATC_SHELL_PID"),
        source: "shell",
    },
];

/// [`resolve_session`] over the process: [`SESSION_VARS`] in order,
/// then stdin a terminal, with `USER`, `LOGNAME`, or `USERNAME` as the
/// login name.
fn session_from_environment(repo: &gix::Repository) -> Option<(String, &'static str)> {
    let values: Vec<Option<String>> = SESSION_VARS
        .iter()
        .map(|row| std::env::var(row.var).ok())
        .collect();
    let rows: Vec<(Option<&str>, &'static str)> = SESSION_VARS
        .iter()
        .zip(&values)
        .map(|(row, value)| (value.as_deref(), row.source))
        .collect();
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let login = login_name();
    let git_name = repo
        .committer()
        .and_then(|sig| sig.ok())
        .map(|sig| sig.name.to_string());
    resolve_session(&rows, interactive, login.as_deref(), git_name.as_deref())
}

/// Everything identity-related about the process, resolved once at
/// open. What `atc callsign` reads bare, and what `atc whoami` will.
#[derive(Debug, Clone)]
pub struct Identity {
    pub session: Option<String>,
    /// The [`SessionVar`] source the session came from, or `login`.
    pub session_source: Option<&'static str>,
    /// This process's pid, from the session row's own pid variable.
    pub pid: Option<lease::Pid>,
    pub callsign: Option<String>,
    /// `env`, `session`, `client`, or `login`.
    pub callsign_source: Option<&'static str>,
    /// The client marker's word, whatever the callsign is.
    pub client: Option<&'static str>,
    /// The lease's state as read at open, before this call renewed it.
    pub lease: Option<lease::State>,
    /// The roots this session was seen in, this one last; empty with no
    /// lease.
    pub repos: Vec<String>,
    /// The one line the CLI prints on stderr: the word this session
    /// held was taken while it was idle.
    pub notice: Option<String>,
}

impl Identity {
    /// The session a lease is keyed by: one a table row named. A login
    /// name at a terminal is a byline, not a session to lease.
    pub fn leased_session(&self) -> Option<&str> {
        match self.session_source {
            Some("login") | None => None,
            Some(_) => self.session.as_deref(),
        }
    }

    /// The variable the session came from, for a render that names it.
    pub fn session_var(&self) -> Option<&'static str> {
        SESSION_VARS
            .iter()
            .find(|row| Some(row.source) == self.session_source)
            .map(|row| row.var)
    }
}

/// [`Identity`] over the process: the session and its row's pid, the
/// client, the lease read and renewed, and the callsign by the rule.
fn identity_from_environment(repo: &gix::Repository) -> Identity {
    let (session, session_source) = match session_from_environment(repo) {
        Some((id, source)) => (Some(id), Some(source)),
        None => (None, None),
    };
    let pid = session_source
        .and_then(|source| SESSION_VARS.iter().find(|row| row.source == source))
        .and_then(|row| row.pid_var)
        .and_then(lease::Pid::from_env);
    let client = lease::client_word();
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());
    let login = login_name();
    let tag = std::env::var(CALLSIGN_VAR).ok();
    let env_word = tag.as_deref().and_then(usable_callsign);

    let leased = match session_source {
        Some("login") | None => None,
        Some(_) => session.as_deref(),
    };
    let fallback = client
        .map(str::to_string)
        .or_else(|| login.clone().filter(|_| interactive));
    let own = leased.map(|session| {
        own_lease(
            session,
            pid,
            env_word.is_none(),
            || lease_window_of(repo),
            fallback.as_deref().unwrap_or("nobody"),
            repo.workdir(),
        )
    });
    if leased.is_some() {
        lease::sweep_if_due(|| Some(Config::from_repo(repo.clone())));
    }
    let (lease_word, lease_state, repos, notice) = match own {
        Some(own) => (own.word, Some(own.state), own.repos, own.notice),
        None => (None, None, Vec::new(), None),
    };

    let (callsign, callsign_source) = match resolve_callsign(
        tag.as_deref(),
        lease_word.as_deref(),
        client,
        interactive,
        login.as_deref(),
    ) {
        Some((word, source)) => (Some(word), Some(source)),
        None => (None, None),
    };
    Identity {
        session,
        session_source,
        pid,
        callsign,
        callsign_source,
        client,
        lease: lease_state,
        repos,
        notice,
    }
}

/// `leaseWindow` for the repository a store opened on, read only when a
/// lease has a word to weigh.
fn lease_window_of(repo: &gix::Repository) -> std::time::Duration {
    crate::config::lease_window(&Config::from_repo(repo.clone()))
}

struct OwnLease {
    word: Option<String>,
    state: lease::State,
    repos: Vec<String>,
    notice: Option<String>,
}

/// This session's lease, read and renewed. The word it holds is the
/// callsign's second slot, and it is not checked against the pid or
/// the window except for one thing: a lease that has gone stale is
/// checked once, here, for whether another session took the word
/// meanwhile. If not, it is renewed and nothing happened. If so, the
/// word is dropped — the lease is rewritten empty, and the notice says
/// who took it — and the verb proceeds under the client word. Two
/// sessions never both hold a word believing it; at worst one learns
/// late. The check is skipped when `ATC_CALLSIGN` is set: the launcher's
/// word wins and the lease's is never read, so nothing is lost by
/// keeping it.
///
/// The renewal rewrites the body when the pid differs from what the
/// file holds — a resumed session, a new process — or when `root`, the
/// repository this store opened on, is not the last the session was
/// seen in; and touches otherwise. The state reported is the lease's as
/// found, before this renewal; the repos are as written. Beside it the
/// heartbeat sweeps dead leases once per `leaseSweep`, through the
/// marker [`lease::sweep_if_due`] reads.
fn own_lease(
    session: &str,
    pid: Option<lease::Pid>,
    check_word: bool,
    window: impl FnOnce() -> std::time::Duration,
    fallback: &str,
    root: Option<&Path>,
) -> OwnLease {
    let root_str = root.map(lease::root_string);
    let Some((mut held, mtime)) = lease::read(session) else {
        let _ = lease::renew(session, root);
        return OwnLease {
            word: None,
            state: lease::state(SystemTime::now(), pid, lease::DEFAULT_WINDOW),
            repos: root_str.map(|root| vec![root]).unwrap_or_default(),
            notice: None,
        };
    };
    // The window is git config, read only when there is a word to
    // weigh: a lease with no word contends with nothing.
    let window = if held.callsign.is_some() {
        window()
    } else {
        lease::DEFAULT_WINDOW
    };
    let state = lease::state(mtime, held.pid(), window);
    let mut notice = None;
    if check_word
        && let Some(word) = held.callsign.clone()
        && !state.held()
        && let Some(taker) = taken_by(session, &word, window)
    {
        held.callsign = None;
        notice = Some(format!(
            "callsign {word} was taken by session {taker} while idle; you are {fallback}"
        ));
    }
    if held.pid() != pid || notice.is_some() {
        // The rewrite under the lock, and the same edits on the local
        // copy, so what is returned is what was written.
        let dropped = notice.is_some();
        let edit = |held: &mut lease::Lease| {
            held.pid = pid.map(|pid| pid.pid);
            held.pid_start = pid.map(|pid| pid.start);
            if held.client.is_none() {
                held.client = lease::client_word().map(str::to_string);
            }
            if dropped {
                held.callsign = None;
            }
            if let Some(root) = &root_str {
                held.saw(root);
            }
            true
        };
        let _ = lease::update(session, edit);
        edit(&mut held);
    } else {
        let _ = lease::renew(session, root);
        if let Some(root) = &root_str {
            held.saw(root);
        }
    }
    OwnLease {
        word: held.callsign,
        state,
        repos: held.repos,
        notice,
    }
}

/// The other session holding `word` with a fresh lease or a live pid,
/// when there is one.
fn taken_by(own: &str, word: &str, window: std::time::Duration) -> Option<String> {
    lease::all()
        .into_iter()
        .filter(|(session, held, _)| session != own && held.callsign.as_deref() == Some(word))
        .find(|(_, held, mtime)| lease::state(*mtime, held.pid(), window).held())
        .map(|(session, _, _)| session)
}

/// The launcher's override on the callsign. Set it in the agent's
/// environment — `ATC_CALLSIGN=qwen-review` — and every event that
/// agent appends carries the name, whatever client mark sits beneath
/// it. Without it the client is detected from [`CLIENT_MARKERS`], and a
/// person at a terminal gets the login name.
pub const CALLSIGN_VAR: &str = "ATC_CALLSIGN";

/// The marks the agent clients leave on the shell they run commands in,
/// and the callsign each names: a variable that is set and non-empty
/// names its client, first match in table order. Claude Code sets
/// `CLAUDECODE`, Qwen Code `QWEN_CODE`, Cursor's IDE agent
/// `CURSOR_AGENT`, and Codex's shell tool `CODEX_SANDBOX_NETWORK_DISABLED`
/// and `CODEX_SANDBOX` under its sandbox. OpenCode's shell tool sets
/// `OPENCODE=1` (1.18.30, captured on this machine), and tower's own
/// plugin sets `OPENCODE_SESSION_ID` on every shell command, so the
/// session variable is a marker too. Best-effort on purpose: a client
/// that leaves no mark, or Codex with sandboxing off, resolves like a
/// bare shell, and the launcher's [`CALLSIGN_VAR`] is the way to name
/// it. The callsigns are the words the clients are wired under —
/// an `atc hook` slug, or the source a retired adapter wrote — so the
/// client a hook wires is the one its events name.
pub const CLIENT_MARKERS: &[(&str, &str)] = &[
    ("CLAUDECODE", "claude"),
    ("QWEN_CODE", "qwen"),
    ("CURSOR_AGENT", "cursor"),
    ("CODEX_SANDBOX_NETWORK_DISABLED", "codex"),
    ("CODEX_SANDBOX", "codex"),
    ("OPENCODE", "opencode"),
    ("OPENCODE_SESSION_ID", "opencode"),
];

/// The one rule a callsign is held to, at every boundary that takes one:
/// trimmed, non-empty, at most 64 bytes, no whitespace or control
/// characters, and not one of the lane words `me`, `agent`, `none`,
/// which name a lane rather than a pilot. `Some` is the usable word.
pub fn usable_callsign(word: &str) -> Option<String> {
    let trimmed = word.trim();
    let usable = !trimmed.is_empty()
        && trimmed.len() <= 64
        && !trimmed.chars().any(|c| c.is_whitespace() || c.is_control())
        && !matches!(trimmed, "me" | "agent" | "none");
    usable.then(|| trimmed.to_string())
}

/// The callsign an append is stamped with, by five rules in order: the
/// variable when it is set and usable, the word this session's lease
/// holds, the detected client, the login name when a person is at the
/// terminal, else none — with the source beside the word: `env`,
/// `session`, `client`, or `login`. The lease's word and the client are
/// already usable words — the verb held one to the rule, the table the
/// other — so they skip it. No git-name fallback — a committer name
/// has spaces and is not a callsign. An unusable variable is ignored
/// rather than fatal, the session's rule: under a hook with no mark
/// and nothing set, the callsign is none and folds like any other.
pub(crate) fn resolve_callsign(
    tag: Option<&str>,
    lease_word: Option<&str>,
    client: Option<&str>,
    interactive: bool,
    login: Option<&str>,
) -> Option<(String, &'static str)> {
    if let Some(tag) = tag.and_then(usable_callsign) {
        return Some((tag, "env"));
    }
    if let Some(word) = lease_word {
        return Some((word.to_string(), "session"));
    }
    if let Some(client) = client {
        return Some((client.to_string(), "client"));
    }
    if !interactive {
        return None;
    }
    login.and_then(usable_callsign).map(|word| (word, "login"))
}

/// The first set login variable — `USER`, `LOGNAME`, or `USERNAME` —
/// the name a terminal's session and callsign both fall back to.
fn login_name() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
}

/// `tower.writer` from config, when set. Local config lives in the common
/// dir and is shared across a repository's linked worktrees, which is
/// exactly the granularity wanted: every worktree on one machine is one writer
/// writing one chain, serialized by the lock.
fn configured_writer(repo: &gix::Repository) -> Option<String> {
    repo.config_snapshot()
        .string("tower.writer")
        .map(|value| value.to_string())
}

/// A single ref-name component under `refs/tower/log/`, checked by asking
/// gix to parse the full ref it would produce — the same rules
/// `git check-ref-format` applies. An ordinary email passes.
fn validate_component(what: &'static str, value: &str) -> Result<()> {
    let refuse = |detail: String| Error::RefName {
        what,
        value: value.to_string(),
        detail,
    };
    if value.is_empty() {
        return Err(refuse("it is empty".to_string()));
    }
    if value.contains('/') {
        return Err(refuse("`/` would split it into two components".to_string()));
    }
    let name = format!("{}{value}", chain::LOG_PREFIX);
    match TryInto::<gix::refs::FullName>::try_into(name.as_str()) {
        Ok(_) => Ok(()),
        Err(err) => Err(refuse(err.to_string())),
    }
}

/// Mint this machine's writer id into local config, once, under git's own
/// config lock so two racing first writes agree instead of minting twice.
/// The write convention is fufu's `snapshot/config.rs`: read the file
/// losslessly, append only what is missing, write through `<path>.lock`,
/// atomic rename.
fn mint_writer(repo: &gix::Repository) -> Result<String> {
    use std::io::Write as _;

    let path = repo.common_dir().join("config");
    let lock_path = path.with_extension("lock");

    // `create_new` is git's own convention: holding the file *is* the lock.
    let mut lock_file = {
        let mut tries = 0;
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(file) => break file,
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && tries < 40 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(err) => return Err(Error::repo(format!("config is locked: {err}"))),
            }
        }
    };

    let outcome = (|| -> Result<(String, bool)> {
        // Re-read under the lock: the other first write may have won, and
        // its writer is then this machine's writer.
        let mut file = load_config_file(&path)?;
        if let Some(existing) = file.string("tower.writer") {
            return Ok((existing.to_string(), false));
        }
        let writer = fresh_writer_name(repo);
        let mut section = file
            .section_mut_or_create_new("tower", None)
            .map_err(Error::repo)?;
        section.push(
            "writer".try_into().map_err(Error::repo)?,
            Some(writer.as_str().into()),
        );
        drop(section);
        let mut bytes = Vec::new();
        file.write_to(&mut bytes).map_err(Error::repo)?;
        lock_file
            .write_all(&bytes)
            .and_then(|()| lock_file.sync_all())
            .map_err(Error::repo)?;
        Ok((writer, true))
    })();

    drop(lock_file);
    match outcome {
        Ok((writer, wrote)) => {
            if wrote {
                if let Err(err) = std::fs::rename(&lock_path, &path) {
                    let _ = std::fs::remove_file(&lock_path);
                    return Err(Error::repo(err));
                }
            } else {
                let _ = std::fs::remove_file(&lock_path);
            }
            Ok(writer)
        }
        Err(err) => {
            let _ = std::fs::remove_file(&lock_path);
            Err(err)
        }
    }
}

/// Read the local config losslessly (comments and formatting preserved);
/// an absent file is an empty one.
fn load_config_file(path: &Path) -> Result<gix::config::File<'static>> {
    let metadata = gix::config::file::Metadata::from(gix::config::Source::Local);
    match std::fs::read(path) {
        Ok(mut bytes) => {
            gix::config::File::from_bytes_owned(&mut bytes, metadata, Default::default())
                .map_err(Error::repo)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(gix::config::File::new(metadata))
        }
        Err(err) => Err(Error::repo(err)),
    }
}

/// `<hostname>-<4 chars>`. The suffix is not decoration: two machines both
/// named `pi` under one email would otherwise share a ref and reintroduce
/// exactly the divergence the writer component exists to prevent. It is
/// hashed from hostname + pid + wall-clock nanos through gix's hasher, so
/// it needs no RNG dependency.
fn fresh_writer_name(repo: &gix::Repository) -> String {
    let host = hostname();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let seed = format!("{host}\0{}\0{nanos}", std::process::id());
    let suffix =
        gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, seed.as_bytes())
            .map(|id| id.to_string()[..4].to_string())
            .unwrap_or_else(|_| format!("{:04x}", std::process::id() as u16));
    format!("{host}-{suffix}")
}

/// The machine's name, from spawning `hostname` — nothing in std or the
/// declared dependencies provides one, and tower already lives by spawning
/// processes. Sanitized to a legal ref component and truncated; a machine
/// that cannot say its name is `host`.
fn hostname() -> String {
    let raw = std::process::Command::new("hostname")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    let cleaned: String = raw
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-');
    let cleaned = cleaned[..cleaned.len().min(24)].trim_matches('-');
    if cleaned.is_empty() {
        "host".to_string()
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three watch paths hang off one common dir: `refs` and
    /// `packed-refs` directly, the log two components under `refs`.
    #[test]
    fn watch_paths_share_one_common_dir() {
        let fixture = atc_testsupport::Repo::new();
        let store = Store::open(fixture.path()).expect("open");
        let paths = store.watch_paths();

        assert!(paths.refs.ends_with("refs"), "{:?}", paths.refs);
        assert!(paths.log.ends_with("refs/tower/log"), "{:?}", paths.log);
        assert!(
            paths.packed_refs.ends_with("packed-refs"),
            "{:?}",
            paths.packed_refs
        );

        let common = paths.refs.parent().expect("a common dir");
        assert_eq!(paths.packed_refs.parent(), Some(common));
        assert_eq!(
            paths.log.parent().and_then(Path::parent),
            Some(paths.refs.as_path())
        );
    }

    /// The three rules in order: the first set and usable row beats
    /// everything and names its source, a terminal gives the login name
    /// or the git name behind it as `login`, and a non-terminal with no
    /// row gives none. An unusable row is skipped, not fatal, and the
    /// walk moves to the next.
    #[test]
    fn a_session_resolves_by_row_then_terminal_then_none() {
        let uuid = "95b36d9d-efdc-4564-9b06-91842f51ef6b";
        fn one(value: Option<&str>) -> Vec<(Option<&str>, &'static str)> {
            vec![(value, "claude")]
        }
        assert_eq!(
            resolve_session(&one(Some(uuid)), true, Some("tyler"), Some("Tyler")),
            Some((uuid.to_string(), "claude"))
        );
        assert_eq!(
            resolve_session(&one(Some(uuid)), false, None, None),
            Some((uuid.to_string(), "claude"))
        );
        assert_eq!(
            resolve_session(&one(Some("  hand-typed  ")), false, None, None),
            Some(("hand-typed".to_string(), "claude"))
        );
        assert_eq!(
            resolve_session(&one(None), true, Some("tyler"), Some("Tyler")),
            Some(("tyler".to_string(), "login"))
        );
        assert_eq!(
            resolve_session(&one(None), true, None, Some("Tyler Johnson")),
            Some(("Tyler Johnson".to_string(), "login"))
        );
        assert_eq!(resolve_session(&one(None), true, None, None), None);
        assert_eq!(
            resolve_session(&one(None), false, Some("tyler"), Some("Tyler")),
            None
        );
        assert_eq!(
            resolve_session(&one(Some("   ")), true, Some("tyler"), None),
            Some(("tyler".to_string(), "login"))
        );
        assert_eq!(
            resolve_session(&one(Some("a\nb")), true, Some("tyler"), None),
            Some(("tyler".to_string(), "login"))
        );
        let long = "x".repeat(129);
        assert_eq!(
            resolve_session(&one(Some(&long)), true, Some("tyler"), None),
            Some(("tyler".to_string(), "login")),
            "a row past 128 bytes is ignored"
        );

        // The table's order: the launcher's row beats the client's, the
        // client's beats the shell's, and an unusable row falls through
        // to the next one set.
        let rows = |launcher, claude, shell| {
            vec![
                (launcher, "launcher"),
                (claude, "claude"),
                (None, "codex"),
                (None, "opencode"),
                (shell, "shell"),
            ]
        };
        assert_eq!(
            resolve_session(
                &rows(Some("w1"), Some(uuid), Some("t1")),
                true,
                Some("tyler"),
                None
            ),
            Some(("w1".to_string(), "launcher"))
        );
        assert_eq!(
            resolve_session(
                &rows(None, Some(uuid), Some("t1")),
                true,
                Some("tyler"),
                None
            ),
            Some((uuid.to_string(), "claude"))
        );
        assert_eq!(
            resolve_session(&rows(None, None, Some("t1")), true, Some("tyler"), None),
            Some(("t1".to_string(), "shell"))
        );
        assert_eq!(
            resolve_session(&rows(Some("  "), None, Some("t1")), false, None, None),
            Some(("t1".to_string(), "shell"))
        );
    }

    /// The table's sources are distinct words — the identity reports a
    /// session by its source, and the pid row is found by it.
    #[test]
    fn every_session_row_has_its_own_source() {
        let mut seen = Vec::new();
        for row in SESSION_VARS {
            assert!(!seen.contains(&row.source), "{} twice", row.source);
            assert_ne!(row.source, "login", "login is the terminal's word");
            seen.push(row.source);
        }
        assert_eq!(
            SESSION_VARS[0].var, "ATC_SESSION",
            "the launcher's row is first"
        );
        assert_eq!(
            SESSION_VARS.last().map(|row| row.var),
            Some("ATC_SHELL_SESSION"),
            "the terminal's row is last"
        );
    }

    /// The five rules in order: the variable beats everything, the
    /// lease's word beats the client, the client beats the terminal, a
    /// terminal gives the login name, and a non-terminal with no
    /// variable, no lease, and no client gives none — each with its
    /// source. There is no git-name fallback, and an unusable variable
    /// — blank, spaced, a lane word — is ignored, not fatal: it falls
    /// through to the lease, then the client.
    #[test]
    fn a_callsign_resolves_by_variable_then_lease_then_client_then_terminal_then_none() {
        struct Case {
            tag: Option<&'static str>,
            lease: Option<&'static str>,
            client: Option<&'static str>,
            interactive: bool,
            login: Option<&'static str>,
            want: Option<(&'static str, &'static str)>,
        }
        let case = |tag, lease, client, interactive, login, want| Case {
            tag,
            lease,
            client,
            interactive,
            login,
            want,
        };
        let cases = [
            case(
                Some("claude"),
                None,
                None,
                true,
                Some("tyler"),
                Some(("claude", "env")),
            ),
            case(
                Some("claude"),
                None,
                None,
                false,
                None,
                Some(("claude", "env")),
            ),
            case(
                Some("  qwen-review  "),
                None,
                None,
                false,
                None,
                Some(("qwen-review", "env")),
            ),
            case(
                None,
                None,
                None,
                true,
                Some("tyler"),
                Some(("tyler", "login")),
            ),
            case(None, None, None, true, None, None),
            case(None, None, None, false, Some("tyler"), None),
            case(
                Some("   "),
                None,
                None,
                true,
                Some("tyler"),
                Some(("tyler", "login")),
            ),
            case(
                Some("two words"),
                None,
                None,
                true,
                Some("tyler"),
                Some(("tyler", "login")),
            ),
            case(Some("a\tb"), None, None, false, None, None),
            case(
                Some("me"),
                None,
                None,
                true,
                Some("tyler"),
                Some(("tyler", "login")),
            ),
            case(Some("agent"), None, None, false, None, None),
            case(Some("none"), None, None, false, None, None),
            case(None, None, None, true, Some("Tyler Johnson"), None),
            // The client rows: the tag beats the client, the client
            // beats the login, a client with no terminal still resolves,
            // and an unusable tag falls through to the client.
            case(
                Some("qwen-review"),
                None,
                Some("claude"),
                false,
                None,
                Some(("qwen-review", "env")),
            ),
            case(
                None,
                None,
                Some("claude"),
                true,
                Some("tyler"),
                Some(("claude", "client")),
            ),
            case(
                None,
                None,
                Some("claude"),
                false,
                None,
                Some(("claude", "client")),
            ),
            case(
                Some("   "),
                None,
                Some("qwen"),
                false,
                None,
                Some(("qwen", "client")),
            ),
            case(
                Some("me"),
                None,
                Some("codex"),
                true,
                Some("tyler"),
                Some(("codex", "client")),
            ),
            // The lease rows: the tag beats the lease, the lease beats
            // the client and the login, and an unusable tag falls
            // through to the lease.
            case(
                Some("qwen-review"),
                Some("alpha"),
                Some("claude"),
                true,
                Some("tyler"),
                Some(("qwen-review", "env")),
            ),
            case(
                None,
                Some("alpha"),
                Some("claude"),
                true,
                Some("tyler"),
                Some(("alpha", "session")),
            ),
            case(
                None,
                Some("alpha"),
                None,
                false,
                None,
                Some(("alpha", "session")),
            ),
            case(
                Some("me"),
                Some("alpha"),
                Some("claude"),
                false,
                None,
                Some(("alpha", "session")),
            ),
        ];
        for Case {
            tag,
            lease,
            client,
            interactive,
            login,
            want,
        } in cases
        {
            assert_eq!(
                resolve_callsign(tag, lease, client, interactive, login),
                want.map(|(word, source)| (word.to_string(), source)),
                "tag {tag:?}, lease {lease:?}, client {client:?}, interactive {interactive}, login {login:?}"
            );
        }
        let long = "x".repeat(65);
        assert_eq!(
            resolve_callsign(Some(&long), None, None, true, Some("tyler")),
            Some(("tyler".to_string(), "login")),
            "a callsign past 64 bytes is ignored"
        );
        assert_eq!(
            usable_callsign(&"x".repeat(64)).as_deref(),
            Some("x".repeat(64).as_str()),
            "64 bytes is the last usable length"
        );
    }

    /// Every callsign in the marker table passes the rule the variable
    /// is held to, so skipping the rule on the client loses nothing.
    #[test]
    fn every_client_marker_names_a_usable_callsign() {
        for (variable, callsign) in CLIENT_MARKERS {
            assert_eq!(
                usable_callsign(callsign).as_deref(),
                Some(*callsign),
                "{variable}"
            );
        }
    }

    /// The harness scrub list covers every variable the store reads for
    /// a session, its pid, or a callsign, or a test run inside a Claude
    /// Code session would stamp `claude` on every fixture's events and
    /// key every fixture's lease by the developer's own session.
    /// testsupport cannot depend on core, so the guard runs from here.
    #[test]
    fn the_test_scrub_covers_every_agent_variable() {
        let names = [CALLSIGN_VAR]
            .into_iter()
            .chain(SESSION_VARS.iter().map(|row| row.var))
            .chain(SESSION_VARS.iter().filter_map(|row| row.pid_var))
            .chain(CLIENT_MARKERS.iter().map(|(name, _)| *name));
        for name in names {
            assert!(
                atc_testsupport::AGENT_ENV.contains(&name),
                "{name} is read by the store but not scrubbed by atc_testsupport::AGENT_ENV"
            );
        }
    }
}

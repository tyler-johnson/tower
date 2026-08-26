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

use gix::refs::transaction::PreviousValue;

pub use chain::{CHAIN_VERSION, TOWER_EMAIL, TOWER_NAME};
pub use error::{Error, Result};
pub use event::{Event, EventId, Kind};

/// The store, open on one repository.
///
/// The author is resolved at open — a store that cannot say who is filing
/// should say so before anything leans on it. The writer is resolved from
/// config at open and minted on the first append when there is none, so
/// opening a store never writes anything.
pub struct Store {
    repo: gix::Repository,
    author: String,
    writer: std::cell::OnceCell<String>,
}

impl Store {
    /// Open the store on the repository containing `path`.
    pub fn open(path: &Path) -> Result<Store> {
        let repo = gix::discover(path).map_err(Error::repo)?;
        let author = resolve_author(&repo)?;
        let writer = std::cell::OnceCell::new();
        if let Some(configured) = configured_writer(&repo) {
            validate_component("writer", &configured)?;
            let _ = writer.set(configured);
        }
        Ok(Store {
            repo,
            author,
            writer,
        })
    }

    /// Who events file under: git `user.email`.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// This machine's writer id, once one exists — `None` until the first
    /// append mints it.
    pub fn writer(&self) -> Option<&str> {
        self.writer.get().map(String::as_str)
    }

    /// `tower.bays` from config, when set: the pool root bare `warm`
    /// mints slots under. Same granularity story as `tower.writer`:
    /// local config lives in the common dir and is shared across a
    /// repository's linked worktrees, so every bay sees the same pool
    /// root.
    pub fn pool_root(&self) -> Option<String> {
        self.repo
            .config_snapshot()
            .string("tower.bays")
            .map(|value| value.to_string())
    }

    /// The main worktree's path — the parent of the common dir — or
    /// `None` for a bare repository, which has no tree to anchor to.
    ///
    /// Beside `pool_root` and for its reason: the CLI stays gix-free, and
    /// this is the anchor `.tower/procedures` resolves against. The main
    /// worktree rather than the invoking one, because a bay must read the
    /// same definitions the repository does. No spawn — `file` keeps the
    /// property that it never runs fufu.
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

/// `tower.writer` from config, when set. Local config lives in the common
/// dir and is shared across a repository's linked worktrees, which is
/// exactly the granularity wanted: every bay on one machine is one writer
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

//! The change feed: the loop that settles the log when the repository
//! moves and stamps motion for every SSE stream to fold against.
//!
//! The rule this module exists to keep: all board updates flow through
//! this loop, whoever wrote — GUI, CLI, MCP, adapters, agents in bays.
//! The only stamp is the loop below, and its only triggers are the
//! repository moving; a POST handler publishes nothing directly, so its
//! write reaches the feed through the same watcher every other writer's
//! does, and the browser can never see a board the log does not back.
//!
//! The channel carries a stamp and not a board. Every subscriber holds
//! its own query, so there is no one envelope to broadcast: on each
//! stamp a subscriber folds its own frame against its own query, one
//! fold per subscriber per motion — the trade the query surface names,
//! a fold being a handful of local ff spawns. The loop's own work per
//! motion is the lazy pass, run once here so every subscriber's fold
//! reads a settled log.
//!
//! Two motion sources, one dirty flag. A filesystem watcher covers
//! tower's own store — loose refs under `refs/tower/log`, and
//! `packed-refs` for the day `git pack-refs` moves the tips — which is
//! how a write from another process on this machine, or a push landing,
//! is seen. An `ff watch --all` child covers the repository underneath:
//! its lines are never parsed, because any line means motion and the
//! fold is the arbiter of what changed. Both collapse into one
//! [`Notify`], debounced by [`QUIET`] of silence, so a burst folds once.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use ff_tower_core::ff::Ff;
use ff_tower_core::log::WatchPaths;
use notify::Watcher;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{Notify, watch};

/// How long the repository must stay quiet before a refold. Under fufu's
/// 200ms watch poll cadence on purpose: sustained motion arrives at that
/// beat, and a window wider than the beat would chain ticks into an
/// indefinite wait.
const QUIET: Duration = Duration::from_millis(150);

/// How long a dead watch child stays dead before the respawn.
const RESPAWN: Duration = Duration::from_secs(1);

/// What the feed's channel carries: a motion stamp.
#[derive(Clone)]
pub(crate) enum Latest {
    /// The seed value before the first stamp lands. Streams wait on it.
    Pending,
    /// The repository moved and the log is settled. It carries nothing:
    /// the fold is the arbiter of what changed, and each subscriber runs
    /// its own.
    Moved,
    /// Shutdown: every stream ends, so graceful shutdown can drain
    /// connections an SSE stream would otherwise hold open forever.
    Closing,
}

/// Install the watcher and spawn the feed's tasks on the current
/// runtime. The one hard failure is the watcher not installing: a feed
/// that silently missed tower's own ref writes is the outcome this
/// module forbids, so the caller treats it as a startup failure.
pub(crate) fn start(
    repo: PathBuf,
    paths: WatchPaths,
    tx: Arc<watch::Sender<Latest>>,
) -> Result<(), notify::Error> {
    let dirty = Arc::new(Notify::new());
    let watcher = fs_watcher(&paths, Arc::clone(&dirty))?;
    tokio::spawn(watch_child(repo.clone(), Arc::clone(&dirty)));
    tokio::spawn(refold_loop(repo, tx, dirty, watcher));
    Ok(())
}

/// The filesystem half: a watcher whose callback — on notify's own
/// thread, where the sync-callable `notify_one` is exactly right — marks
/// the flag when a tower log ref or packed-refs moves. `refs` is watched
/// recursively because it always exists and sees `refs/tower/log` born;
/// the common dir non-recursively, for packed-refs alone. Everything
/// else under `refs` — branch churn, fufu's own refs — is filtered out
/// here, because the watch child already covers the repository and a
/// flag marked twice folds once anyway.
fn fs_watcher(
    paths: &WatchPaths,
    dirty: Arc<Notify>,
) -> Result<notify::RecommendedWatcher, notify::Error> {
    // Both spellings of the log's path, because a backend decides which
    // one it reports: macOS hands back canonical paths, so a repository
    // reached through a symlink — `/var/folders` into `/private` under
    // every macOS tempdir, and any developer whose checkout sits under
    // one — arrives under a prefix the registered spelling never
    // matches, and the filesystem lane silently covers nothing.
    let log = paths.log.clone();
    let resolved = resolve(&paths.log);
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            let Ok(event) = event else { return };
            if matches!(event.kind, notify::EventKind::Access(_)) {
                return;
            }
            let moved = event.paths.iter().any(|path| {
                path.starts_with(&log)
                    || path.starts_with(&resolved)
                    || path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("packed-refs"))
            });
            if moved {
                dirty.notify_one();
            }
        })?;
    watcher.watch(&paths.refs, notify::RecursiveMode::Recursive)?;
    let common = paths
        .packed_refs
        .parent()
        .expect("packed-refs sits in the common dir");
    watcher.watch(common, notify::RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

/// A path with its symlinks resolved, for comparing against what a
/// watcher reports. `refs/tower/log` need not exist yet — the recursive
/// watch is installed to see it born — so the nearest ancestor that does
/// exist is canonicalized and the rest re-joined onto it. A path nothing
/// on it resolves comes back unchanged, which leaves the raw comparison
/// beside this one to answer.
fn resolve(path: &Path) -> PathBuf {
    let mut tail = Vec::new();
    let mut cursor = path;
    loop {
        if let Ok(real) = cursor.canonicalize() {
            return tail
                .iter()
                .rev()
                .fold(real, |resolved, part| resolved.join(part));
        }
        match (cursor.parent(), cursor.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                cursor = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The process half: `ff watch --all`, respawned for as long as the
/// server lives. Every stdout line marks the flag and nothing is parsed.
/// A spawn that fails because there is no `ff` ends the task after one
/// stderr line — every fold is failing for the same reason, and a retry
/// loop would only repeat the sentence — while an exit or EOF says one
/// line, waits [`RESPAWN`], and tries again. `kill_on_drop` reaps the
/// child when the runtime goes down with the server.
async fn watch_child(repo: PathBuf, dirty: Arc<Notify>) {
    loop {
        let mut command = tokio::process::Command::from(Ff::at(&repo).env_program().watch_all());
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("the feed's `ff watch --all` found no ff to spawn: {err}");
                return;
            }
            Err(err) => {
                eprintln!("the feed's `ff watch --all` would not spawn: {err}");
                tokio::time::sleep(RESPAWN).await;
                continue;
            }
        };
        if let Some(stdout) = child.stdout.take() {
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(_line)) = lines.next_line().await {
                dirty.notify_one();
            }
        }
        let _ = child.wait().await;
        eprintln!("the feed's `ff watch --all` exited; respawning");
        tokio::time::sleep(RESPAWN).await;
    }
}

/// The one stamper. It owns the watcher — dropping a
/// `RecommendedWatcher` uninstalls it, so it lives exactly as long as
/// the loop does — seeds the channel with the first stamp, and then
/// alternates settling and stamping forever.
async fn refold_loop(
    repo: PathBuf,
    tx: Arc<watch::Sender<Latest>>,
    dirty: Arc<Notify>,
    watcher: notify::RecommendedWatcher,
) {
    let _watcher = watcher;
    stamp(&repo, &tx).await;
    loop {
        settle(&dirty).await;
        stamp(&repo, &tx).await;
    }
}

/// Wait for motion, then for [`QUIET`] of silence after it. `notify_one`
/// stores at most one permit, which is the whole debounce: a burst
/// coalesces into one wakeup, and motion landing while a fold runs is
/// held as the permit the next `notified` returns from.
async fn settle(dirty: &Notify) {
    dirty.notified().await;
    loop {
        tokio::select! {
            _ = dirty.notified() => continue,
            _ = tokio::time::sleep(QUIET) => return,
        }
    }
}

/// One motion, stamped. The lazy pass runs first, on a blocking thread
/// for the reason every handler's does — it spawns ff processes and
/// `Store` is not `Sync` — so every subscriber's fold reads a settled
/// log, the same pass `GET /api/board` runs ahead of its read. The
/// pass's own append re-trips the watcher, the next pass concludes
/// nothing, and the loop stamps once more and stops. A pass that fails,
/// panic included, says one stderr line and the stamp still lands; the
/// loop lives.
async fn stamp(repo: &Path, tx: &watch::Sender<Latest>) {
    let repo = repo.to_path_buf();
    match tokio::task::spawn_blocking(move || crate::api::pass(&repo)).await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            eprintln!("the feed's pass did not run: {err}");
        }
        Err(_panicked) => {
            eprintln!("the feed's pass did not run: the pass panicked");
        }
    }
    let _ = tx.send(Latest::Moved);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The debounce, on a paused clock — the only honest pin, because
    /// end-to-end the window races the fold. `settle` waits for motion,
    /// holds while notifies land inside [`QUIET`], returns after that
    /// much silence — and a notify with no waiter, the shape of motion
    /// during a fold, is the stored permit the next `settle` returns
    /// from.
    #[tokio::test(start_paused = true)]
    async fn settle_debounces_and_the_stored_permit_survives_a_fold() {
        let dirty = Arc::new(Notify::new());
        let settled = tokio::spawn({
            let dirty = Arc::clone(&dirty);
            async move { settle(&dirty).await }
        });

        // No motion: no amount of quiet releases it.
        tokio::time::sleep(QUIET * 10).await;
        assert!(!settled.is_finished(), "settled with nothing moving");

        // Motion inside the window keeps holding it.
        dirty.notify_one();
        tokio::time::sleep(QUIET / 2).await;
        assert!(!settled.is_finished(), "settled inside the quiet window");
        dirty.notify_one();
        tokio::time::sleep(QUIET / 2).await;
        assert!(!settled.is_finished(), "the second notify did not re-arm");

        // A full window of silence releases it.
        tokio::time::sleep(QUIET).await;
        settled.await.expect("settle returns after quiet");

        // Motion with no waiter — a fold in progress — stores the
        // permit, and the next settle rides it out without new motion.
        dirty.notify_one();
        tokio::time::sleep(QUIET * 10).await;
        let again = tokio::spawn({
            let dirty = Arc::clone(&dirty);
            async move { settle(&dirty).await }
        });
        tokio::time::sleep(QUIET * 2).await;
        assert!(again.is_finished(), "the stored permit was lost");
        again.await.expect("settle returns from the permit");
    }

    /// The comparison the filesystem lane stands on: a repository
    /// reached through a symlink — every macOS tempdir — must resolve to
    /// what the watcher will report, and a ref that does not exist yet
    /// must resolve anyway, because the recursive watch is installed to
    /// see it born.
    #[test]
    fn a_ref_path_resolves_through_a_symlink_whether_or_not_it_exists() {
        let dir = tempfile::TempDir::new().expect("a tempdir");
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("refs")).expect("the refs dir");
        let link = dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).expect("a symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real, &link).expect("a symlink");

        let real = real.canonicalize().expect("the real path");
        assert_eq!(resolve(&link.join("refs")), real.join("refs"));
        assert_eq!(
            resolve(&link.join("refs").join("tower").join("log")),
            real.join("refs").join("tower").join("log"),
            "an unborn ref resolves through its nearest existing ancestor"
        );
    }
}

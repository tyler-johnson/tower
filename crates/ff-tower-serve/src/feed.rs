//! The change feed: the loop that refolds the board when the repository
//! moves and publishes the envelope to every SSE stream.
//!
//! The rule this module exists to keep: all board updates flow through
//! this loop, whoever wrote — GUI, CLI, MCP, adapters, agents in bays.
//! The only publisher is the refold below, and its only triggers are the
//! repository moving; a POST handler publishes nothing directly, so its
//! write reaches the feed through the same watcher every other writer's
//! does, and the browser can never see a board the log does not back.
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

/// What the feed's channel carries.
#[derive(Clone)]
pub(crate) enum Latest {
    /// The seed value before the first fold lands. Streams skip it.
    Pending,
    /// The board envelope — the bytes `GET /api/board` answers, minus
    /// its trailing newline, because that newline is `println!` framing
    /// and SSE frames itself.
    Board(String),
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
    let log = paths.log.clone();
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            let Ok(event) = event else { return };
            if matches!(event.kind, notify::EventKind::Access(_)) {
                return;
            }
            let moved = event.paths.iter().any(|path| {
                path.starts_with(&log)
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

/// The one publisher. It owns the watcher — dropping a
/// `RecommendedWatcher` uninstalls it, so it lives exactly as long as
/// the loop does — seeds the channel with the first fold, and then
/// alternates settling and publishing forever.
async fn refold_loop(
    repo: PathBuf,
    tx: Arc<watch::Sender<Latest>>,
    dirty: Arc<Notify>,
    watcher: notify::RecommendedWatcher,
) {
    let _watcher = watcher;
    publish(&repo, &tx).await;
    loop {
        settle(&dirty).await;
        publish(&repo, &tx).await;
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

/// One refold, published. The fold runs on a blocking thread for the
/// reason every handler's does — it spawns ff processes and `Store` is
/// not `Sync` — and through the exact pipeline `GET /api/board` runs, so
/// a pushed board and a pulled one can only be the same bytes. A fold
/// that fails, panic included, says one stderr line and leaves the last
/// board standing; the loop lives.
async fn publish(repo: &Path, tx: &watch::Sender<Latest>) {
    let repo = repo.to_path_buf();
    match tokio::task::spawn_blocking(move || crate::api::board_envelope(&repo)).await {
        Ok(Ok(envelope)) => {
            let _ = tx.send(Latest::Board(envelope));
        }
        Ok(Err(err)) => {
            eprintln!("the feed's refold failed; the last board stands: {err}");
        }
        Err(_panicked) => {
            eprintln!("the feed's refold failed; the last board stands: the fold panicked");
        }
    }
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
}

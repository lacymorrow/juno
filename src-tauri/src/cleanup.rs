//! Application cleanup module
//! Handles resource cleanup on application exit

use crate::state::AppState;
use crate::utils::resource_manager::ResourceManager;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use tauri::Manager;
use tracing::{error, info, warn};

/// The most the exit path will wait for cleanup. Quitting must always finish:
/// a browser that will not close is not worth a Juno that will not quit.
const EXIT_CLEANUP_BUDGET: Duration = Duration::from_secs(5);

/// How long to wait for the browser controller's lock before leaving it. A
/// browser tool call holds it while it works.
const BROWSER_LOCK_WAIT: Duration = Duration::from_secs(1);

/// The most *all* MCP servers together may take to stop. Shared rather than
/// per-server on purpose: servers are stopped one at a time (`stop_server`
/// takes the connection map's write lock), so a per-server budget large enough
/// to be useful means two wedged servers can spend the whole of
/// [`EXIT_CLEANUP_BUDGET`] between them and the third is never reached.
const MCP_STAGE_BUDGET: Duration = Duration::from_secs(3);

/// Set once cleanup has run, so every way out does the work once between them.
static EXIT_CLEANUP_DONE: AtomicBool = AtomicBool::new(false);

/// True for the first caller only.
fn claim_exit_cleanup(done: &AtomicBool) -> bool {
    !done.swap(true, Ordering::SeqCst)
}

/// Install the Ctrl-C handler for CLI and `tauri dev` runs.
///
/// Cleanup itself is not registered here. It used to hang off the app handle's
/// `tauri://destroyed` listener, which never fired: Tauri sends that event to
/// the destroyed window only, and an app-level listener is not a window. So a
/// normal quit skipped the whole of [`cleanup_application`] and left the
/// browser Juno launched running. Cleanup now runs from `RunEvent::Exit`
/// ([`run_exit_cleanup`]), which every way out reaches, this one included.
pub fn init_cleanup_handlers(app_handle: tauri::AppHandle) {
    if let Err(e) = ctrlc::set_handler(move || {
        info!("Received interrupt signal, exiting...");
        // Tauri's managed exit, which ends in `RunEvent::Exit` and so in cleanup.
        app_handle.exit(0);
    }) {
        error!("Failed to set Ctrl-C handler: {}", e);
    }
}

/// What the exit path saw when it stopped waiting for cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupOutcome {
    /// Cleanup ran to the end.
    Finished,
    /// The budget ran out first. Cleanup may still be running; quitting is not
    /// waiting for it.
    TimedOut,
    /// The cleanup task ended without reporting (a panic, or a runtime that is
    /// no longer running tasks).
    TaskGone,
}

/// Clean up on the way out. Called from `RunEvent::Exit`, once, bounded by
/// [`EXIT_CLEANUP_BUDGET`].
pub fn run_exit_cleanup(app_handle: &tauri::AppHandle) {
    if !claim_exit_cleanup(&EXIT_CLEANUP_DONE) {
        return;
    }
    let handle = app_handle.clone();
    run_bounded("Application cleanup", EXIT_CLEANUP_BUDGET, async move {
        cleanup_application(&handle).await;
    });
}

/// Run `work` on the async runtime and wait for it, for no longer than
/// `budget`, then carry on either way.
///
/// The wait is a blocking channel receive on the calling thread, deliberately,
/// and swapping it back for `tokio::time::timeout` reopens LAC-8220.
/// `tokio::time::timeout` polls its inner future on its own task, so it bounds
/// the future's *awaits*, not the wall clock: anything that blocks the task
/// between await points — a `block_on`, a blocking syscall, an await that is
/// never woken on a runtime that has stopped driving timers — takes the timer
/// branch down with it and the timeout never fires.
///
/// That was fatal here rather than merely slow, because `RunEvent::Exit` runs
/// on the main thread inside AppKit's `applicationWillTerminate`. A cleanup
/// that outlived its budget parked the main thread forever inside
/// `-[NSApplication terminate:]`: Juno never exited, stopped answering Apple
/// Events, and the next `open -a Juno` just re-activated the stuck process.
///
/// `Receiver::recv_timeout` is a condvar with an OS deadline. It is not the
/// runtime's to miss, nothing `work` does can hold it past `budget`, and the
/// work happens on a worker thread rather than on the thread AppKit is waiting
/// on. Threads left behind do not keep the process alive: the event loop exits
/// it once this returns.
///
/// There is deliberately no cancellation signal: on a timeout `work` keeps
/// running while the process exits around it. That is safe here because this is
/// only ever called from `RunEvent::Exit`, where everything after it either
/// exits the process or (in the updater's case) touches nothing cleanup holds,
/// and because the alternative — a cancellation `work` has to poll — is the
/// same dependence on cooperative yielding that caused LAC-8220. The cost is
/// that a timed-out stage may leave what it was about to tidy up; the process
/// is ending, so the kernel gets it instead.
pub(crate) fn run_bounded<F>(what: &str, budget: Duration, work: F) -> CleanupOutcome
where
    F: Future<Output = ()> + Send + 'static,
{
    let (done_tx, done_rx) = mpsc::channel::<()>();
    tauri::async_runtime::spawn(async move {
        work.await;
        // The receiver is gone once the budget has run out. Nothing to report.
        let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(budget) {
        Ok(()) => CleanupOutcome::Finished,
        Err(RecvTimeoutError::Timeout) => {
            warn!(
                "{what} did not finish within {}s; exiting anyway",
                budget.as_secs()
            );
            CleanupOutcome::TimedOut
        }
        Err(RecvTimeoutError::Disconnected) => {
            warn!("{what} ended without reporting; exiting anyway");
            CleanupOutcome::TaskGone
        }
    }
}

/// Perform full application cleanup
pub async fn cleanup_application(app_handle: &tauri::AppHandle) {
    info!("Starting application cleanup...");

    // Cancel every armed timer first. Screen and file monitors poll on an
    // interval and start a turn of their own when they fire, so leaving one
    // running through shutdown means capturing the screen on the way out.
    let cancelled_timers = crate::agent::tools::timer_tools::timer_manager()
        .cancel_all_timers(Some(app_handle))
        .await;
    if !cancelled_timers.is_empty() {
        info!(
            "Cancelled {} armed timer(s) during cleanup",
            cancelled_timers.len()
        );
    }

    // Get AppState
    if let Some(app_state) = app_handle.try_state::<AppState>() {
        // Cancel any ongoing agent execution
        app_state.signal_cancel();

        // Clean up browser controllers. Waits a moment for the lock rather
        // than skipping on contention: a browser tool call holding it is the
        // moment a Chrome Juno launched is most likely to be open.
        match tokio::time::timeout(BROWSER_LOCK_WAIT, app_state.browser_controller.lock()).await {
            Ok(mut controller_guard) => {
                if let Some(controller) = controller_guard.take() {
                    info!("Cleaning up browser controller from AppState...");
                    if let Err(e) = controller.cleanup().await {
                        error!("Failed to cleanup browser controller: {}", e);
                    }
                }
            }
            Err(_) => warn!("Browser controller is busy; leaving it to the process exit"),
        }

        // MCP server children: stdio servers are separate processes that the
        // exit would otherwise leave to notice their stdin closing.
        //
        // The whole stage shares one deadline ([`MCP_STAGE_BUDGET`]) and each
        // server gets whatever is left of it, so every server is reached even
        // when an earlier one will not die. A server reached with nothing left
        // still usually gets its signal: `timeout` polls its inner future once
        // before it checks the deadline, and `disconnect` runs synchronously
        // through `start_kill()` before its first await. "Usually" because that
        // first poll stops early if the connection map's lock is held by a tool
        // call, which is why the log below does not claim the signal went out.
        //
        // The deadline starts before the manager's lock, not after, because an
        // agent turn can be holding that lock and waiting for it is the stall
        // most likely to swallow the whole stage. Timing out here gives the
        // steps after this one their turn instead of spending the outer budget
        // on a lock that is not coming.
        info!("Stopping MCP servers...");
        let stage_ends_at = tokio::time::Instant::now() + MCP_STAGE_BUDGET;
        let manager = app_state.get_mcp_manager().await;
        // Bound to a local rather than matched inline. Matched inline, this is the
        // tail expression of the enclosing block, so the
        // `Result<MutexGuard<'_, MCPManager>, Elapsed>` temporary lives to the end
        // of the block and outlives the `Arc` it borrows from: E0597. A local
        // declared after `manager` is dropped before it.
        let locked = tokio::time::timeout(MCP_STAGE_BUDGET, manager.lock()).await;
        match locked {
            Ok(manager) => {
                for config in manager.get_server_configs().await {
                    let left = stage_ends_at.saturating_duration_since(tokio::time::Instant::now());
                    match tokio::time::timeout(left, manager.stop_server(&config.id)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            warn!("Failed to stop MCP server '{}': {}", config.name, e)
                        }
                        Err(_) => warn!(
                            "MCP server '{}' did not finish stopping inside the {}s stop budget \
                             shared by all servers; leaving it to the process exit",
                            config.name,
                            MCP_STAGE_BUDGET.as_secs()
                        ),
                    }
                }
                info!("MCP servers stopped");
            }
            Err(_) => warn!(
                "MCP manager is busy after {}s; leaving its servers to the process exit",
                MCP_STAGE_BUDGET.as_secs()
            ),
        }

        // Note: the CDP browser connection is cleaned up when AppState is dropped
        // TTS provider is part of AudioSettings and doesn't need explicit cleanup
    }

    // Clean up global resources
    let resource_manager = ResourceManager::global();
    resource_manager.cleanup_all().await;

    // Clean up temporary directories
    cleanup_temp_directories();

    info!("Application cleanup completed");
}

/// Clean up temporary directories created by the application
fn cleanup_temp_directories() {
    // Clean up Juno temp directories
    if let Ok(temp_dir) = std::env::temp_dir().read_dir() {
        for entry in temp_dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                // Browser profiles belong to the browser controller, which
                // removes its own and sweeps only those whose owner has died.
                if (name.starts_with("juno_") || name.starts_with("juno-"))
                    && !name.starts_with("juno-browser-")
                {
                    if let Ok(metadata) = entry.metadata() {
                        // Only delete if older than 1 hour (in case multiple instances are running)
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed.as_secs() > 3600 {
                                    info!("Cleaning up old temp directory: {:?}", entry.path());
                                    let _ = std::fs::remove_dir_all(entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_cleanup_is_claimed_once() {
        let done = AtomicBool::new(false);
        assert!(claim_exit_cleanup(&done));
        assert!(!claim_exit_cleanup(&done));
    }

    /// Cleanup that finishes is waited for, not cut off.
    #[test]
    fn a_cleanup_that_finishes_reports_finished() {
        assert_eq!(
            run_bounded("test cleanup", Duration::from_secs(5), async {}),
            CleanupOutcome::Finished
        );
    }

    /// LAC-8220: quitting hung forever with a stdio MCP server connected.
    ///
    /// The budget was a `tokio::time::timeout` wrapped around the cleanup
    /// future and awaited on the main thread. That bounds the future's awaits,
    /// not the clock: cleanup that stops yielding takes the timer branch down
    /// with it, so the "timeout" never fires and the main thread never comes
    /// back out of `-[NSApplication terminate:]`.
    ///
    /// The future below models exactly that — it blocks its own task and never
    /// reaches an await point. Under the old implementation this returns
    /// `Finished`, a whole sleep late. The budget has to hold anyway.
    ///
    /// It needs a worker thread to spare while that one is blocked, which it
    /// has: Tauri's global runtime is a multi-threaded tokio runtime
    /// (`async_runtime::default_runtime`), and nothing else in this test binary
    /// occupies it. A `#[test]` rather than a `#[tokio::test]` for the same
    /// reason — the wait is a blocking receive and must not sit on a worker.
    #[test]
    fn cleanup_that_never_yields_does_not_hold_up_quitting() {
        // Long enough to dwarf the budget, short enough that the worker it
        // blocks is back before anything else in the binary wants it.
        let blocked_for = Duration::from_millis(900);
        let budget = Duration::from_millis(100);

        let started = std::time::Instant::now();
        let outcome = run_bounded("test cleanup", budget, async move {
            std::thread::sleep(blocked_for);
        });
        let waited = started.elapsed();

        assert_eq!(
            outcome,
            CleanupOutcome::TimedOut,
            "a cleanup that blocks its own task must hit the budget"
        );
        assert!(
            waited < blocked_for / 2,
            "quitting waited {waited:?}, which is the stuck cleanup's time, not the {budget:?} budget"
        );
    }

    /// Cleanup used to be registered on `tauri://destroyed` through the app
    /// handle. Tauri emits that event to the destroyed window alone, so the
    /// listener never ran and a normal quit skipped cleanup entirely. The exit
    /// path has to be what calls it.
    #[test]
    fn the_exit_event_runs_cleanup_and_nothing_listens_for_destroyed() {
        let lib = include_str!("lib.rs");
        let exit_arm = lib
            .split_once("tauri::RunEvent::Exit =>")
            .map(|(_, rest)| rest)
            .expect("lib.rs handles RunEvent::Exit");
        let exit_arm = exit_arm
            .split_once("_ => {}")
            .map(|(arm, _)| arm)
            .unwrap_or(exit_arm);
        assert!(
            exit_arm.contains("cleanup::run_exit_cleanup"),
            "RunEvent::Exit must run application cleanup"
        );

        let own = include_str!("cleanup.rs");
        let production = own.split("#[cfg(test)]").next().unwrap_or(own);
        assert!(
            !production.contains("TAURI_DESTROYED"),
            "an app-level listener never receives window-scoped tauri://destroyed"
        );
    }
}

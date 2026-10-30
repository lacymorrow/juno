//! Application cleanup module
//! Handles resource cleanup on application exit

use crate::state::AppState;
use crate::utils::resource_manager::ResourceManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;
use tracing::{error, info, warn};

/// The most the exit path will wait for cleanup. Quitting must always finish:
/// a browser that will not close is not worth a Juno that will not quit.
const EXIT_CLEANUP_BUDGET: Duration = Duration::from_secs(5);

/// How long to wait for the browser controller's lock before leaving it. A
/// browser tool call holds it while it works.
const BROWSER_LOCK_WAIT: Duration = Duration::from_secs(1);

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

/// Clean up on the way out. Called from `RunEvent::Exit`, once, bounded by
/// [`EXIT_CLEANUP_BUDGET`].
pub fn run_exit_cleanup(app_handle: &tauri::AppHandle) {
    if !claim_exit_cleanup(&EXIT_CLEANUP_DONE) {
        return;
    }
    tauri::async_runtime::block_on(async {
        if tokio::time::timeout(EXIT_CLEANUP_BUDGET, cleanup_application(app_handle))
            .await
            .is_err()
        {
            warn!(
                "Application cleanup did not finish within {}s; exiting anyway",
                EXIT_CLEANUP_BUDGET.as_secs()
            );
        }
    });
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
                if name.starts_with("juno_") || name.starts_with("juno-") {
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

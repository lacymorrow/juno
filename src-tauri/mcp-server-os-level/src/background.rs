//! Background-operation switch and the process the agent is currently acting on.
//!
//! The source of truth for the switch is the app's `AgentSettings.background_mode`
//! setting. The app mirrors it here at startup and on every settings change so the
//! platform layer can consult it without an async settings read on the click path.
//!
//! `last_target_pid` exists because keystrokes carry no coordinate. A no-warp mouse
//! action already resolves the process under the target point, so remembering it
//! lets the following `type`/`key` reach the same app instead of whatever happens
//! to be frontmost.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Mirrors `AgentSettings.background_mode`, whose default is true.
static BACKGROUND_MODE: AtomicBool = AtomicBool::new(true);

/// 0 means "no process targeted yet".
static LAST_TARGET_PID: AtomicI32 = AtomicI32::new(0);

/// Mirror the app's `background_mode` setting into the platform layer.
pub fn set_background_mode(enabled: bool) {
    BACKGROUND_MODE.store(enabled, Ordering::Relaxed);
    if !enabled {
        // Turning the setting off puts every path back on the physical tap, so a
        // remembered process would only be a stale route for later keystrokes.
        forget_target_pid();
    }
}

/// True when the agent must avoid taking the cursor and the frontmost app.
pub fn is_background_mode() -> bool {
    BACKGROUND_MODE.load(Ordering::Relaxed)
}

/// Record the process a background action just reached, so keystrokes that
/// follow can be routed to the same app.
pub fn remember_target_pid(pid: i32) {
    if pid > 0 {
        LAST_TARGET_PID.store(pid, Ordering::Relaxed);
    }
}

/// Forget the remembered process (used when a target is known to be gone).
pub fn forget_target_pid() {
    LAST_TARGET_PID.store(0, Ordering::Relaxed);
}

/// The process the agent last acted on, if it is still alive.
///
/// Always `None` while background mode is off, so callers cannot accidentally
/// route input to a remembered process when the setting says to use the physical
/// tap. A dead pid is cleared rather than returned: recycled pids would send the
/// user's keystrokes into an unrelated process.
pub fn target_pid() -> Option<i32> {
    if !is_background_mode() {
        return None;
    }
    let pid = LAST_TARGET_PID.load(Ordering::Relaxed);
    if pid <= 0 {
        return None;
    }
    if process_is_alive(pid) {
        Some(pid)
    } else {
        forget_target_pid();
        None
    }
}

#[cfg(unix)]
fn process_is_alive(pid: i32) -> bool {
    // Signal 0 performs the permission and existence checks without delivering
    // anything, which is the standard liveness probe.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
fn process_is_alive(_pid: i32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Both pieces of state here are process-global, so the tests take turns.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Run a test against a known starting state and put the globals back.
    fn with_clean_state(background: bool, body: impl FnOnce()) {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_background_mode(background);
        forget_target_pid();
        body();
        set_background_mode(true);
        forget_target_pid();
        drop(guard);
    }

    #[test]
    fn background_mode_round_trips() {
        with_clean_state(true, || {
            assert!(is_background_mode());
            set_background_mode(false);
            assert!(!is_background_mode());
            set_background_mode(true);
            assert!(is_background_mode());
        });
    }

    #[test]
    fn non_positive_pids_are_never_remembered() {
        with_clean_state(true, || {
            remember_target_pid(0);
            assert_eq!(target_pid(), None);
            remember_target_pid(-5);
            assert_eq!(target_pid(), None);
        });
    }

    #[test]
    fn a_live_process_is_returned() {
        with_clean_state(true, || {
            let own_pid = std::process::id() as i32;
            remember_target_pid(own_pid);
            assert_eq!(target_pid(), Some(own_pid));
        });
    }

    #[test]
    fn no_target_is_offered_while_background_mode_is_off() {
        with_clean_state(true, || {
            let own_pid = std::process::id() as i32;
            remember_target_pid(own_pid);
            set_background_mode(false);
            assert_eq!(
                target_pid(),
                None,
                "a remembered process must not route input once the setting is off"
            );
        });
    }
}

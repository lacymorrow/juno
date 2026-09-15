//! Dock icon visibility, and surviving menu-bar-only mode.
//!
//! Turning the Dock icon off is the setting a non-technical person is most
//! likely to regret: Juno vanishes from the Dock and the app switcher and the
//! only way back is the menu bar. So this module does two things. It persists
//! and applies the preference, and it watches for the "I opened it again and
//! nothing happened" panic so the UI can offer the way out.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::constants::events;
use crate::constants::ui::window_labels;
use crate::settings::manager::SettingsManager;

/// Reopen attempts this close together belong to the same burst.
pub const REOPEN_WINDOW: Duration = Duration::from_secs(60);

/// Attempts inside that window before a quiet hint becomes a way out.
/// Three tries in a minute is what scrambling looks like.
pub const REOPEN_ESCALATION_THRESHOLD: usize = 3;

/// Timestamps of the recent reopen attempts, newest last.
static REOPEN_ATTEMPTS: Mutex<Vec<Instant>> = Mutex::new(Vec::new());

/// Drop the attempts that have aged out, add this one, return the new count.
///
/// Pure on purpose: the counting rule is the whole escalation decision, and it
/// is tested directly rather than through a running app.
pub fn record_attempt(history: &mut Vec<Instant>, now: Instant, window: Duration) -> usize {
    history.retain(|seen| now.duration_since(*seen) < window);
    history.push(now);
    history.len()
}

/// Register a reopen attempt and return how many happened in the last minute.
pub fn note_reopen_attempt() -> usize {
    let mut history = match REOPEN_ATTEMPTS.lock() {
        Ok(guard) => guard,
        // A poisoned lock here means a previous panic while counting clicks.
        // The count is advisory; recover rather than lose the escalation.
        Err(poisoned) => poisoned.into_inner(),
    };
    record_attempt(&mut history, Instant::now(), REOPEN_WINDOW)
}

/// Forget the recent attempts; used once the user has been helped.
pub fn reset_reopen_attempts() {
    let mut history = match REOPEN_ATTEMPTS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    history.clear();
}

/// The saved preference, defaulting to visible when it cannot be read: a Juno
/// nobody can find is the worse failure.
pub async fn read_dock_icon_visible(app: &AppHandle) -> bool {
    let manager = match SettingsManager::new(app.clone()) {
        Ok(manager) => manager,
        Err(e) => {
            warn!("[DockIcon] Failed to open settings: {}", e);
            return true;
        }
    };
    match manager.get_agent_settings().await {
        Ok(settings) => settings.dock_icon_visible,
        Err(e) => {
            warn!("[DockIcon] Failed to read the Dock icon setting: {}", e);
            true
        }
    }
}

/// Apply the saved preference once, at startup.
pub fn apply_saved_dock_icon_policy(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let visible = read_dock_icon_visible(&app).await;
        // Regular is already the default, so only a hidden icon needs applying.
        if !visible {
            apply_dock_icon_policy(&app, false);
        }
    });
}

/// Put the process in (or out of) the Dock and the app switcher.
#[cfg(target_os = "macos")]
pub fn apply_dock_icon_policy(app: &AppHandle, visible: bool) {
    let policy = if visible {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    };
    match app.set_activation_policy(policy) {
        Ok(()) => info!(
            "[DockIcon] Activation policy set to {}",
            if visible { "regular" } else { "accessory" }
        ),
        Err(e) => warn!("[DockIcon] Failed to set activation policy: {}", e),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_dock_icon_policy(_app: &AppHandle, _visible: bool) {
    // Only macOS has an activation policy; elsewhere the setting is inert.
}

/// Tell the Juno pane that somebody tried to open an already-running Juno.
///
/// Only meaningful while the Dock icon is hidden: with a Dock icon there is
/// nothing to explain, the window simply comes forward.
pub fn announce_reopen_attempt(app: &AppHandle, count: usize) {
    if let Err(e) = app.emit(
        events::app_lifecycle::REOPEN_ATTEMPT,
        serde_json::json!({ "count": count }),
    ) {
        warn!("[DockIcon] Failed to emit reopen attempt: {}", e);
    }

    // The banner points at the menu bar, which is the one place the app still
    // is. It says the same thing the pane says, in case the pane is not up.
    let body = if count >= REOPEN_ESCALATION_THRESHOLD {
        "Open it from the Juno icon in the menu bar. The Juno window can put the Dock icon back."
    } else {
        "It lives in the menu bar at the top of your screen. Click the Juno icon there."
    };
    let shown = app
        .notification()
        .builder()
        .title("Juno is already running")
        .body(body)
        .show();
    if let Err(e) = shown {
        warn!("[DockIcon] Failed to show the menu bar hint: {}", e);
    }
}

/// React to a reopen of an app that has no Dock icon: count it, say where
/// Juno is, and let the UI escalate once the count says the person is stuck.
pub fn handle_reopen(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let visible = read_dock_icon_visible(&app).await;
        // With a Dock icon there is nothing to explain: the window came
        // forward, which is exactly what the click asked for.
        if visible {
            return;
        }
        let count = note_reopen_attempt();
        info!("[DockIcon] Reopen attempt {} while menu-bar only", count);

        // The floating bar counts as a visible window, so the main window was
        // left alone above. Bring it up: opening the app is the person asking
        // to see Juno, and the notice they need is in that window.
        if let Some(window) = app.get_webview_window(window_labels::MAIN) {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }

        announce_reopen_attempt(&app, count);
    });
}

/// Is Juno currently shown in the Dock?
#[tauri::command]
pub async fn get_dock_icon_visible(
    settings_manager: State<'_, SettingsManager>,
) -> Result<bool, String> {
    let agent_settings = settings_manager.get_agent_settings().await?;
    Ok(agent_settings.dock_icon_visible)
}

/// Show or hide Juno in the Dock, and remember the choice.
#[tauri::command]
pub async fn set_dock_icon_visible(
    app: AppHandle,
    settings_manager: State<'_, SettingsManager>,
    visible: bool,
) -> Result<(), String> {
    let mut agent_settings = settings_manager.get_agent_settings().await?;
    agent_settings.dock_icon_visible = visible;
    settings_manager.set_agent_settings(&agent_settings).await?;

    apply_dock_icon_policy(&app, visible);
    // Back in the Dock means the user is found again; the panic counter goes.
    if visible {
        reset_reopen_attempts();
    }
    info!(
        "[DockIcon] Juno is {} in the Dock",
        if visible { "shown" } else { "hidden" }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_the_first_attempt_as_one() {
        let mut history = Vec::new();
        let now = Instant::now();
        assert_eq!(record_attempt(&mut history, now, REOPEN_WINDOW), 1);
    }

    #[test]
    fn counts_attempts_inside_the_window() {
        let mut history = Vec::new();
        let start = Instant::now();
        assert_eq!(record_attempt(&mut history, start, REOPEN_WINDOW), 1);
        assert_eq!(
            record_attempt(&mut history, start + Duration::from_secs(10), REOPEN_WINDOW),
            2
        );
        assert_eq!(
            record_attempt(&mut history, start + Duration::from_secs(20), REOPEN_WINDOW),
            REOPEN_ESCALATION_THRESHOLD
        );
    }

    #[test]
    fn forgets_attempts_older_than_the_window() {
        let mut history = Vec::new();
        let start = Instant::now();
        record_attempt(&mut history, start, REOPEN_WINDOW);
        record_attempt(&mut history, start + Duration::from_secs(1), REOPEN_WINDOW);
        // Two minutes later the earlier burst is history; this is a fresh one.
        assert_eq!(
            record_attempt(
                &mut history,
                start + Duration::from_secs(120),
                REOPEN_WINDOW
            ),
            1
        );
    }

    #[test]
    fn a_slow_drip_never_escalates() {
        let mut history = Vec::new();
        let start = Instant::now();
        for minute in 0..5 {
            let at = start + Duration::from_secs(minute * 61);
            assert_eq!(record_attempt(&mut history, at, REOPEN_WINDOW), 1);
        }
    }
}

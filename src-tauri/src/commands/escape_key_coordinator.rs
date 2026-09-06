//! # Escape / stop-key coordinator
//!
//! Ref-counts the parts of Juno that currently need the stop key (agent runs,
//! TTS, dictation, onboarding) and keeps a stop-key *observer* alive while any
//! of them is active.
//!
//! Which observer depends on the configured `stop_current_task` shortcut:
//!
//! * a bare key (the default, `Escape`) is watched with a **passive NSEvent
//!   monitor** (`platform::stop_key_monitor`). It never consumes the key, so
//!   every other app — and Juno's own web views — still receive Escape;
//! * a modified chord (e.g. `Cmd+Escape`) is registered through the
//!   `tauri-plugin-global-shortcut` hot-key path as before.
//!
//! The ledger (`StopKeyLedger`) is pure and unit-tested; the coordinator is a
//! thin wrapper that turns ledger transitions into observer install/remove.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut};
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Pure state: who needs the stop key
// ---------------------------------------------------------------------------

/// What the coordinator must do to the observer after a ledger change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerTransition {
    /// First user arrived — install the observer.
    Activate,
    /// Last user left — remove the observer.
    Deactivate,
    /// Observer state is unchanged.
    Unchanged,
}

/// Set of named users that currently want the stop key, with the time each
/// registered. The set *is* the ref count — no separate counter to drift.
#[derive(Debug, Default)]
pub struct StopKeyLedger {
    users: HashMap<String, Instant>,
}

impl StopKeyLedger {
    pub fn register(&mut self, user: &str, now: Instant) -> LedgerTransition {
        let was_empty = self.users.is_empty();
        let already = self.users.insert(user.to_string(), now).is_some();
        if already {
            LedgerTransition::Unchanged
        } else if was_empty {
            LedgerTransition::Activate
        } else {
            LedgerTransition::Unchanged
        }
    }

    pub fn unregister(&mut self, user: &str) -> LedgerTransition {
        if self.users.remove(user).is_none() {
            return LedgerTransition::Unchanged;
        }
        if self.users.is_empty() {
            LedgerTransition::Deactivate
        } else {
            LedgerTransition::Unchanged
        }
    }

    pub fn clear(&mut self) -> LedgerTransition {
        if self.users.is_empty() {
            LedgerTransition::Unchanged
        } else {
            self.users.clear();
            LedgerTransition::Deactivate
        }
    }

    pub fn contains(&self, user: &str) -> bool {
        self.users.contains_key(user)
    }

    pub fn len(&self) -> usize {
        self.users.len()
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }

    pub fn users(&self) -> Vec<String> {
        let mut out: Vec<String> = self.users.keys().cloned().collect();
        out.sort();
        out
    }

    /// Users registered longer than `max_age` ago — but only when nothing in
    /// the app is actually running. A long agent run (or a long TTS read) is
    /// *not* stale just because it is old; sweeping it would silently disarm
    /// the stop key mid-run.
    pub fn stale_users(
        &self,
        now: Instant,
        max_age: Duration,
        work_in_progress: bool,
    ) -> Vec<String> {
        if work_in_progress {
            return Vec::new();
        }
        let mut out: Vec<String> = self
            .users
            .iter()
            .filter(|(_, registered_at)| now.saturating_duration_since(**registered_at) > max_age)
            .map(|(user, _)| user.clone())
            .collect();
        out.sort();
        out
    }
}

// ---------------------------------------------------------------------------
// Pure decision: how to observe the configured stop key
// ---------------------------------------------------------------------------

/// How the configured stop shortcut is observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopKeyBinding {
    /// Bare key watched passively (never consumed). `key_code` is the macOS
    /// virtual key code.
    PassiveMonitor { key_code: u16, label: String },
    /// Modified chord registered as an exclusive global hot key.
    GlobalShortcut(Shortcut),
}

impl StopKeyBinding {
    pub fn describe(&self) -> String {
        match self {
            StopKeyBinding::PassiveMonitor { label, key_code } => {
                format!("passive_monitor:{} (key code {})", label, key_code)
            }
            StopKeyBinding::GlobalShortcut(shortcut) => {
                format!("global_shortcut:{:?}", shortcut)
            }
        }
    }
}

/// macOS virtual key codes for the unmodified keys we are willing to watch
/// passively. Anything else (letters, digits...) is a poor stop key and keeps
/// the exclusive hot-key behaviour so it at least still works.
pub fn macos_key_code(code: Code) -> Option<u16> {
    Some(match code {
        Code::Escape => 53,
        Code::F1 => 122,
        Code::F2 => 120,
        Code::F3 => 99,
        Code::F4 => 118,
        Code::F5 => 96,
        Code::F6 => 97,
        Code::F7 => 98,
        Code::F8 => 100,
        Code::F9 => 101,
        Code::F10 => 109,
        Code::F11 => 103,
        Code::F12 => 111,
        _ => return None,
    })
}

/// Resolve the `stop_current_task` setting into a binding. Unparseable input
/// falls back to a passive Escape monitor (matching the historical fallback
/// to Escape).
pub fn resolve_stop_key_binding(stop_setting: &str) -> StopKeyBinding {
    let shortcut = crate::parse_shortcut_string(stop_setting).unwrap_or_else(|| {
        warn!(
            "[EscapeKeyCoordinator] Failed to parse stop_current_task '{}', falling back to Escape",
            stop_setting
        );
        Shortcut::new(None, Code::Escape)
    });
    binding_for_shortcut(shortcut, stop_setting)
}

fn binding_for_shortcut(shortcut: Shortcut, label: &str) -> StopKeyBinding {
    if !cfg!(target_os = "macos") {
        return StopKeyBinding::GlobalShortcut(shortcut);
    }
    if !shortcut.mods.is_empty() {
        return StopKeyBinding::GlobalShortcut(shortcut);
    }
    match macos_key_code(shortcut.key) {
        Some(key_code) => StopKeyBinding::PassiveMonitor {
            key_code,
            label: if label.trim().is_empty() {
                "Escape".to_string()
            } else {
                label.trim().to_string()
            },
        },
        None => StopKeyBinding::GlobalShortcut(shortcut),
    }
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

pub struct EscapeKeyCoordinator {
    ledger: Mutex<StopKeyLedger>,
    /// The binding that is currently observing, if any. Remembered so removal
    /// tears down exactly what was installed even if the setting changed
    /// mid-run.
    active_binding: Mutex<Option<StopKeyBinding>>,
}

#[allow(clippy::new_without_default)]
impl EscapeKeyCoordinator {
    pub fn new() -> Self {
        Self {
            ledger: Mutex::new(StopKeyLedger::default()),
            active_binding: Mutex::new(None),
        }
    }

    fn ledger(&self) -> std::sync::MutexGuard<'_, StopKeyLedger> {
        self.ledger.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn active(&self) -> std::sync::MutexGuard<'_, Option<StopKeyBinding>> {
        self.active_binding
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Register a user for stop-key handling (idempotent).
    pub async fn register_escape_user(
        &self,
        app_handle: &AppHandle,
        user_id: &str,
    ) -> Result<(), String> {
        let transition = self.ledger().register(user_id, Instant::now());
        let count = self.ledger().len();
        match transition {
            LedgerTransition::Unchanged | LedgerTransition::Deactivate => {
                debug!(
                    "[EscapeKeyCoordinator] User '{}' registered (count: {}, observer unchanged)",
                    user_id, count
                );
                Ok(())
            }
            LedgerTransition::Activate => {
                info!(
                    "[EscapeKeyCoordinator] User '{}' registered, count: {} — activating stop key",
                    user_id, count
                );
                if let Err(e) = self.activate(app_handle) {
                    warn!("[EscapeKeyCoordinator] Failed to activate stop key: {}", e);
                    self.ledger().unregister(user_id);
                    return Err(e);
                }
                Ok(())
            }
        }
    }

    /// Unregister a user (idempotent — a stale unregister is a no-op).
    pub async fn unregister_escape_user(
        &self,
        app_handle: &AppHandle,
        user_id: &str,
    ) -> Result<(), String> {
        let transition = self.ledger().unregister(user_id);
        let count = self.ledger().len();
        match transition {
            LedgerTransition::Unchanged | LedgerTransition::Activate => {
                debug!(
                    "[EscapeKeyCoordinator] Unregister '{}' (count: {}, observer unchanged)",
                    user_id, count
                );
                Ok(())
            }
            LedgerTransition::Deactivate => {
                info!(
                    "[EscapeKeyCoordinator] User '{}' unregistered, count: 0 — releasing stop key",
                    user_id
                );
                self.deactivate(app_handle)
            }
        }
    }

    /// Remove every user and release the stop key.
    pub async fn unregister_all_users(&self, app_handle: &AppHandle) -> Result<(), String> {
        let users = self.ledger().users();
        let transition = self.ledger().clear();
        if transition == LedgerTransition::Deactivate {
            info!(
                "[EscapeKeyCoordinator] Released stop key for all users: {:?}",
                users
            );
            self.deactivate(app_handle)?;
        } else {
            debug!("[EscapeKeyCoordinator] No users registered, nothing to release");
            // Defensive: an observer with no users is a leak — tear it down.
            if self.active().is_some() {
                warn!("[EscapeKeyCoordinator] Observer active with no users — removing");
                self.deactivate(app_handle)?;
            }
        }
        Ok(())
    }

    /// Force reset the escape key state (emergency cleanup).
    pub async fn force_reset(&self, app_handle: &AppHandle) -> Result<(), String> {
        warn!("[EscapeKeyCoordinator] Force reset requested");
        self.ledger().clear();
        self.deactivate(app_handle)?;
        info!("[EscapeKeyCoordinator] Force reset completed");
        Ok(())
    }

    /// Sweep registrations older than `max_age` — only when the app is idle.
    ///
    /// Previously this unregistered any user older than five minutes even
    /// while its agent run was still going, which silently disarmed Escape
    /// partway through every long run. Now a sweep is skipped whenever an
    /// agent, TTS, dictation or onboarding is live.
    pub async fn check_and_cleanup_stale(&self, app_handle: &AppHandle, max_age: Duration) {
        let work_in_progress = something_to_stop(app_handle).await;
        let stale = self
            .ledger()
            .stale_users(Instant::now(), max_age, work_in_progress);
        if stale.is_empty() {
            return;
        }
        warn!(
            "[EscapeKeyCoordinator] Sweeping {} stale registrations (older than {:?}, app idle): {:?}",
            stale.len(),
            max_age,
            stale
        );
        for user_id in &stale {
            if let Err(e) = self.unregister_escape_user(app_handle, user_id).await {
                warn!(
                    "[EscapeKeyCoordinator] Failed to clean up stale user '{}': {}",
                    user_id, e
                );
            }
        }
    }

    /// Coordinator status for debugging.
    pub async fn get_status(&self) -> serde_json::Value {
        let (users, count) = {
            let ledger = self.ledger();
            (ledger.users(), ledger.len())
        };
        let binding = self.active().as_ref().map(StopKeyBinding::describe);
        serde_json::json!({
            "user_count": count,
            "is_registered": binding.is_some(),
            "binding": binding,
            "passive_monitor_installed": crate::platform::stop_key_monitor::is_installed(),
            "registered_users": users,
        })
    }

    fn activate(&self, app_handle: &AppHandle) -> Result<(), String> {
        if let Some(existing) = self.active().as_ref() {
            debug!(
                "[EscapeKeyCoordinator] Stop key already active ({})",
                existing.describe()
            );
            return Ok(());
        }
        let binding = current_binding(app_handle);
        info!(
            "[EscapeKeyCoordinator] Activating stop key: {}",
            binding.describe()
        );
        match &binding {
            StopKeyBinding::PassiveMonitor { key_code, .. } => {
                crate::platform::stop_key_monitor::install(app_handle, *key_code)?;
            }
            StopKeyBinding::GlobalShortcut(shortcut) => {
                app_handle
                    .global_shortcut()
                    .register(*shortcut)
                    .map_err(|e| {
                        error!(
                            "[EscapeKeyCoordinator] Failed to register stop shortcut {:?}: {}",
                            shortcut, e
                        );
                        format!("Failed to register stop shortcut: {}", e)
                    })?;
            }
        }
        *self.active() = Some(binding);
        Ok(())
    }

    fn deactivate(&self, app_handle: &AppHandle) -> Result<(), String> {
        let Some(binding) = self.active().take() else {
            return Ok(());
        };
        info!(
            "[EscapeKeyCoordinator] Releasing stop key: {}",
            binding.describe()
        );
        match &binding {
            StopKeyBinding::PassiveMonitor { .. } => {
                crate::platform::stop_key_monitor::remove(app_handle)
            }
            StopKeyBinding::GlobalShortcut(shortcut) => app_handle
                .global_shortcut()
                .unregister(*shortcut)
                .map_err(|e| {
                    error!(
                        "[EscapeKeyCoordinator] Failed to unregister stop shortcut {:?}: {}",
                        shortcut, e
                    );
                    format!("Failed to unregister stop shortcut: {}", e)
                }),
        }
    }
}

/// Resolve the binding for the user's current `stop_current_task` setting.
fn current_binding(app_handle: &AppHandle) -> StopKeyBinding {
    let setting = app_handle
        .try_state::<crate::state::AppState>()
        .and_then(|state| state.get_keyboard_shortcuts().ok())
        .map(|shortcuts| shortcuts.stop_current_task)
        .unwrap_or_else(|| "Escape".to_string());
    resolve_stop_key_binding(&setting)
}

/// Is anything live that a stop-key press would need to stop?
async fn something_to_stop(app_handle: &AppHandle) -> bool {
    let Some(state) = app_handle.try_state::<crate::state::AppState>() else {
        return false;
    };
    if state.is_agent_executing()
        || state.is_dictation_active()
        || state.is_onboarding_active()
        || crate::tts::is_tts_playing()
    {
        return true;
    }
    state.agent_sessions().len().await > 0
}

// Global coordinator instance
static ESCAPE_KEY_COORDINATOR: Lazy<EscapeKeyCoordinator> = Lazy::new(EscapeKeyCoordinator::new);

/// Get the global escape key coordinator
pub fn get_escape_key_coordinator() -> &'static EscapeKeyCoordinator {
    &ESCAPE_KEY_COORDINATOR
}

/// Tauri command to register escape key user
#[tauri::command]
pub async fn register_escape_key_user(
    app_handle: AppHandle,
    user_id: String,
) -> Result<(), String> {
    get_escape_key_coordinator()
        .register_escape_user(&app_handle, &user_id)
        .await
}

/// Tauri command to unregister escape key user
#[tauri::command]
pub async fn unregister_escape_key_user(
    app_handle: AppHandle,
    user_id: String,
) -> Result<(), String> {
    get_escape_key_coordinator()
        .unregister_escape_user(&app_handle, &user_id)
        .await
}

/// Tauri command to force reset escape key state
#[tauri::command]
pub async fn force_reset_escape_key(app_handle: AppHandle) -> Result<(), String> {
    get_escape_key_coordinator().force_reset(&app_handle).await
}

/// Tauri command to get escape key coordinator status
#[tauri::command]
pub async fn get_escape_key_coordinator_status() -> Result<serde_json::Value, String> {
    Ok(get_escape_key_coordinator().get_status().await)
}

/// Force unregister escape key for debugging/emergency cleanup
#[tauri::command]
pub async fn force_unregister_escape_key(app_handle: AppHandle) -> Result<String, String> {
    info!("[EscapeKeyCoordinator] Force unregister escape key requested manually");
    get_escape_key_coordinator()
        .force_reset(&app_handle)
        .await?;
    Ok("Stop key observer removed".to_string())
}

/// Test escape key flow: register -> stop all operations -> verify unregistered
#[tauri::command]
pub async fn test_escape_key_flow(app_handle: AppHandle) -> Result<String, String> {
    info!("[EscapeKeyCoordinator] Testing escape key registration/unregistration flow");

    let coordinator = get_escape_key_coordinator();
    coordinator
        .register_escape_user(&app_handle, "test_user")
        .await?;
    let status_after_register = coordinator.get_status().await;

    let stop_coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
    stop_coordinator
        .stop_all_operations(&app_handle, "Test escape key flow")
        .await?;

    let status_after_stop = coordinator.get_status().await;

    Ok(format!(
        "Escape key flow test completed:\n- After register: {}\n- After stop: {}",
        serde_json::to_string_pretty(&status_after_register)
            .unwrap_or_else(|_| "failed to serialize".to_string()),
        serde_json::to_string_pretty(&status_after_stop)
            .unwrap_or_else(|_| "failed to serialize".to_string())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Modifiers;

    fn now() -> Instant {
        Instant::now()
    }

    fn ago(secs: u64) -> Instant {
        now()
            .checked_sub(Duration::from_secs(secs))
            .unwrap_or_else(now)
    }

    // --- ledger: when should the observer be active ---

    #[test]
    fn first_user_activates_last_user_deactivates() {
        let mut ledger = StopKeyLedger::default();
        assert_eq!(ledger.register("agent", now()), LedgerTransition::Activate);
        assert_eq!(ledger.register("tts", now()), LedgerTransition::Unchanged);
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger.unregister("agent"), LedgerTransition::Unchanged);
        assert_eq!(ledger.unregister("tts"), LedgerTransition::Deactivate);
        assert!(ledger.is_empty());
    }

    #[test]
    fn duplicate_register_does_not_inflate_and_refreshes_timestamp() {
        let mut ledger = StopKeyLedger::default();
        let early = ago(600);
        assert_eq!(ledger.register("agent", early), LedgerTransition::Activate);
        assert_eq!(ledger.register("agent", now()), LedgerTransition::Unchanged);
        assert_eq!(ledger.len(), 1);
        // Timestamp refreshed: no longer stale.
        assert!(ledger
            .stale_users(now(), Duration::from_secs(300), false)
            .is_empty());
        // One unregister releases it — the duplicate did not add a second hold.
        assert_eq!(ledger.unregister("agent"), LedgerTransition::Deactivate);
    }

    #[test]
    fn stale_unregister_is_a_noop() {
        let mut ledger = StopKeyLedger::default();
        assert_eq!(ledger.unregister("ghost"), LedgerTransition::Unchanged);
        ledger.register("agent", now());
        ledger.unregister("agent");
        // A second, late unregister from the same run must not touch anyone else.
        ledger.register("next_run", now());
        assert_eq!(ledger.unregister("agent"), LedgerTransition::Unchanged);
        assert!(ledger.contains("next_run"));
    }

    #[test]
    fn clear_deactivates_once() {
        let mut ledger = StopKeyLedger::default();
        assert_eq!(ledger.clear(), LedgerTransition::Unchanged);
        ledger.register("a", now());
        ledger.register("b", now());
        assert_eq!(ledger.clear(), LedgerTransition::Deactivate);
        assert_eq!(ledger.clear(), LedgerTransition::Unchanged);
    }

    // --- stale sweep must never disarm a live run ---

    #[test]
    fn old_registrations_are_stale_only_when_idle() {
        let mut ledger = StopKeyLedger::default();
        let old = ago(400);
        ledger.register("agent_execution", old);
        ledger.register("fresh", now());
        let max_age = Duration::from_secs(300);

        // Agent still running: nothing is stale, no matter how old.
        assert!(ledger.stale_users(now(), max_age, true).is_empty());
        // Idle: only the old one is swept.
        assert_eq!(
            ledger.stale_users(now(), max_age, false),
            vec!["agent_execution".to_string()]
        );
    }

    // --- binding resolution ---

    #[cfg(target_os = "macos")]
    #[test]
    fn bare_escape_uses_passive_monitor() {
        assert_eq!(
            resolve_stop_key_binding("Escape"),
            StopKeyBinding::PassiveMonitor {
                key_code: 53,
                label: "Escape".to_string()
            }
        );
        assert!(matches!(
            resolve_stop_key_binding("esc"),
            StopKeyBinding::PassiveMonitor { key_code: 53, .. }
        ));
        assert!(matches!(
            resolve_stop_key_binding("F5"),
            StopKeyBinding::PassiveMonitor { key_code: 96, .. }
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unparseable_setting_falls_back_to_passive_escape() {
        assert!(matches!(
            resolve_stop_key_binding("NotAKey"),
            StopKeyBinding::PassiveMonitor { key_code: 53, .. }
        ));
        assert!(matches!(
            resolve_stop_key_binding(""),
            StopKeyBinding::PassiveMonitor { key_code: 53, .. }
        ));
    }

    #[test]
    fn modified_chord_keeps_global_shortcut() {
        match resolve_stop_key_binding("Cmd+Escape") {
            StopKeyBinding::GlobalShortcut(shortcut) => {
                assert_eq!(shortcut.key, Code::Escape);
                // global-hotkey normalises META to SUPER on macOS; either is a chord.
                assert!(shortcut.mods.intersects(Modifiers::META | Modifiers::SUPER));
            }
            other => panic!("expected global shortcut, got {:?}", other),
        }
        assert!(matches!(
            resolve_stop_key_binding("Ctrl+Shift+F12"),
            StopKeyBinding::GlobalShortcut(_)
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unmapped_bare_key_keeps_global_shortcut() {
        // A bare letter has no passive mapping; keep the old behaviour rather
        // than silently doing nothing.
        assert!(matches!(
            resolve_stop_key_binding("Q"),
            StopKeyBinding::GlobalShortcut(_)
        ));
    }

    #[test]
    fn key_code_table_covers_escape_and_function_keys() {
        assert_eq!(macos_key_code(Code::Escape), Some(53));
        assert_eq!(macos_key_code(Code::F1), Some(122));
        assert_eq!(macos_key_code(Code::F12), Some(111));
        assert_eq!(macos_key_code(Code::KeyA), None);
        assert_eq!(macos_key_code(Code::Space), None);
    }
}

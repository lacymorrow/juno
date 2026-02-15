use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tracing::{info, warn, error, debug};
use tokio::sync::RwLock;
use std::collections::HashMap;
use once_cell::sync::Lazy;



/// Simplified escape key coordinator with reference counting
pub struct EscapeKeyCoordinator {
    /// Current user count with atomic operations
    user_count: AtomicI32,
    /// Track if escape key is currently registered
    is_registered: AtomicBool,
    /// Track who has registered for escape key (for debugging)
    registered_users: RwLock<HashMap<String, Instant>>,
}

#[allow(clippy::new_without_default)]
impl EscapeKeyCoordinator {
    pub fn new() -> Self {
        Self {
            user_count: AtomicI32::new(0),
            is_registered: AtomicBool::new(false),
            registered_users: RwLock::new(HashMap::new()),
        }
    }

    // Simplified - no complex timing or operation tracking needed

    /// Register a user for escape key handling (idempotent — safe to call multiple times)
    pub async fn register_escape_user(&self, app_handle: &AppHandle, user_id: &str) -> Result<(), String> {
        debug!("[EscapeKeyCoordinator] Register escape user requested: {}", user_id);

        // Check if user is already registered — if so, just update timestamp (no count change)
        {
            let mut users = self.registered_users.write().await;
            if users.contains_key(user_id) {
                info!("[EscapeKeyCoordinator] User '{}' already registered, updating timestamp", user_id);
                users.insert(user_id.to_string(), Instant::now());
                return Ok(());
            }
            users.insert(user_id.to_string(), Instant::now());
        }

        let new_count = self.user_count.fetch_add(1, Ordering::SeqCst) + 1;
        info!("[EscapeKeyCoordinator] User '{}' registered, count: {}", user_id, new_count);

        // Only register global shortcut if this is the first user and not already registered
        if new_count == 1 && !self.is_registered.load(Ordering::SeqCst) {
            if let Err(e) = self.register_global_shortcut(app_handle).await {
                warn!("[EscapeKeyCoordinator] Failed to register global shortcut: {}", e);
                // Rollback user count and HashMap entry
                self.user_count.fetch_sub(1, Ordering::SeqCst);
                let mut users = self.registered_users.write().await;
                users.remove(user_id);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Unregister a user from escape key handling (idempotent — safe to call if already unregistered)
    pub async fn unregister_escape_user(&self, app_handle: &AppHandle, user_id: &str) -> Result<(), String> {
        debug!("[EscapeKeyCoordinator] Unregister escape user requested: {}", user_id);

        // Check if user is actually registered — if not, no-op (prevents stale unregister from decrementing)
        {
            let mut users = self.registered_users.write().await;
            if !users.contains_key(user_id) {
                debug!("[EscapeKeyCoordinator] User '{}' not registered, skipping unregister", user_id);
                return Ok(());
            }
            users.remove(user_id);
        }

        // Atomically decrement count, but never let it go below 0
        let new_count = self.user_count.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if current > 0 {
                Some(current - 1)
            } else {
                // Already at 0, don't decrement further
                None
            }
        });

        match new_count {
            Ok(previous_count) => {
                let actual_new_count = previous_count - 1;
                info!("[EscapeKeyCoordinator] User '{}' unregistered, count: {} -> {}", user_id, previous_count, actual_new_count);

                // Only unregister global shortcut if no users remain and currently registered
                if actual_new_count == 0 && self.is_registered.load(Ordering::SeqCst) {
                    if let Err(e) = self.unregister_global_shortcut(app_handle).await {
                        warn!("[EscapeKeyCoordinator] Failed to unregister global shortcut: {}", e);
                        return Err(e);
                    }
                }
            }
            Err(current_count) => {
                // Counter was already at 0 despite HashMap having the user — count was out of sync
                warn!("[EscapeKeyCoordinator] User '{}' was in HashMap but count was already 0 (current: {})", user_id, current_count);
            }
        }

        Ok(())
    }

    /// Resolve the configured stop shortcut from AppState, falling back to Escape
    fn resolve_stop_shortcut(app_handle: &AppHandle) -> Shortcut {
        let app_state = app_handle.state::<crate::state::AppState>();
        if let Ok(shortcuts) = app_state.get_keyboard_shortcuts() {
            if let Some(shortcut) = crate::parse_shortcut_string(&shortcuts.stop_current_task) {
                return shortcut;
            }
            warn!("[EscapeKeyCoordinator] Failed to parse stop_current_task '{}', falling back to Escape", shortcuts.stop_current_task);
        }
        Shortcut::new(None, tauri_plugin_global_shortcut::Code::Escape)
    }

    /// Register the global stop shortcut (configured via stop_current_task setting)
    async fn register_global_shortcut(&self, app_handle: &AppHandle) -> Result<(), String> {
        let stop_shortcut = Self::resolve_stop_shortcut(app_handle);
        info!("[EscapeKeyCoordinator] Registering global stop shortcut: {:?}", stop_shortcut);

        let result = app_handle.global_shortcut().register(stop_shortcut);

        match result {
            Ok(_) => {
                self.is_registered.store(true, Ordering::SeqCst);
                info!("[EscapeKeyCoordinator] Global stop shortcut registered successfully");
                Ok(())
            }
            Err(e) => {
                error!("[EscapeKeyCoordinator] Failed to register global stop shortcut: {}", e);
                Err(format!("Failed to register stop shortcut: {}", e))
            }
        }
    }

    /// Unregister the global stop shortcut
    async fn unregister_global_shortcut(&self, app_handle: &AppHandle) -> Result<(), String> {
        let stop_shortcut = Self::resolve_stop_shortcut(app_handle);
        info!("[EscapeKeyCoordinator] Unregistering global stop shortcut: {:?}", stop_shortcut);

        let result = app_handle.global_shortcut().unregister(stop_shortcut);

        match result {
            Ok(_) => {
                self.is_registered.store(false, Ordering::SeqCst);
                info!("[EscapeKeyCoordinator] Global stop shortcut unregistered successfully");
                Ok(())
            }
            Err(e) => {
                error!("[EscapeKeyCoordinator] Failed to unregister global stop shortcut: {}", e);
                Err(format!("Failed to unregister stop shortcut: {}", e))
            }
        }
    }

    /// Cooperatively unregister all users (safe for coordinated cleanup)
    /// Each user is removed from HashMap first, so stale unregister calls from
    /// those users will harmlessly no-op (Phase 1 idempotency).
    pub async fn unregister_all_users(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("[EscapeKeyCoordinator] Unregistering all users cooperatively");

        // Collect user IDs to unregister (avoid holding write lock while calling unregister)
        let user_ids: Vec<String> = {
            let users = self.registered_users.read().await;
            users.keys().cloned().collect()
        };

        if user_ids.is_empty() {
            debug!("[EscapeKeyCoordinator] No users registered, nothing to unregister");
            // Defensive: if count is out of sync, force it to 0 and unregister shortcut
            let count = self.user_count.load(Ordering::SeqCst);
            if count > 0 {
                warn!("[EscapeKeyCoordinator] No users in HashMap but count={}, forcing to 0", count);
                self.user_count.store(0, Ordering::SeqCst);
                if self.is_registered.load(Ordering::SeqCst) {
                    if let Err(e) = self.unregister_global_shortcut(app_handle).await {
                        warn!("[EscapeKeyCoordinator] Failed to unregister global shortcut during defensive cleanup: {}", e);
                    }
                }
            }
            return Ok(());
        }

        for user_id in &user_ids {
            if let Err(e) = self.unregister_escape_user(app_handle, user_id).await {
                warn!("[EscapeKeyCoordinator] Failed to unregister user '{}': {}", user_id, e);
            }
        }

        // Defensive fallback: if count > 0 after all users removed, force it to 0
        let remaining_count = self.user_count.load(Ordering::SeqCst);
        if remaining_count > 0 {
            warn!("[EscapeKeyCoordinator] Count still {} after unregister_all_users, forcing to 0", remaining_count);
            self.user_count.store(0, Ordering::SeqCst);
            if self.is_registered.load(Ordering::SeqCst) {
                if let Err(e) = self.unregister_global_shortcut(app_handle).await {
                    warn!("[EscapeKeyCoordinator] Failed to unregister global shortcut during fallback: {}", e);
                }
            }
        }

        info!("[EscapeKeyCoordinator] All users unregistered cooperatively");
        Ok(())
    }

    /// Force reset the escape key state (for emergency cleanup)
    pub async fn force_reset(&self, app_handle: &AppHandle) -> Result<(), String> {
        warn!("[EscapeKeyCoordinator] Force reset requested");

        // Reset all state
        self.user_count.store(0, Ordering::SeqCst);

        {
            let mut users = self.registered_users.write().await;
            users.clear();
        }

        // Unregister if currently registered
        if self.is_registered.load(Ordering::SeqCst) {
            self.unregister_global_shortcut(app_handle).await?;
        }

        info!("[EscapeKeyCoordinator] Force reset completed");
        Ok(())
    }

    /// Check for and clean up stale registrations older than max_age
    pub async fn check_and_cleanup_stale(&self, app_handle: &AppHandle, max_age: Duration) {
        let stale_users: Vec<String> = {
            let users = self.registered_users.read().await;
            users.iter()
                .filter(|(_, registered_at)| registered_at.elapsed() > max_age)
                .map(|(user_id, _)| user_id.clone())
                .collect()
        };

        if stale_users.is_empty() {
            return;
        }

        warn!("[EscapeKeyCoordinator] Found {} stale registrations (older than {:?}): {:?}",
            stale_users.len(), max_age, stale_users);

        for user_id in &stale_users {
            if let Err(e) = self.unregister_escape_user(app_handle, user_id).await {
                warn!("[EscapeKeyCoordinator] Failed to clean up stale user '{}': {}", user_id, e);
            }
        }
    }

    /// Get coordinator status for debugging
    pub async fn get_status(&self) -> serde_json::Value {
        let user_count = self.user_count.load(Ordering::SeqCst);
        let is_registered = self.is_registered.load(Ordering::SeqCst);
        let registration_in_progress = false; // Simplified - no registration tracking

        let users = self.registered_users.read().await;
        let user_list: Vec<String> = users.keys().cloned().collect();

        let last_operation_time: Option<u128> = None; // Simplified - no operation timing

        serde_json::json!({
            "user_count": user_count,
            "is_registered": is_registered,
            "registration_in_progress": registration_in_progress,
            "registered_users": user_list,
            "last_operation_ms_ago": last_operation_time
        })
    }
}

// Global coordinator instance
static ESCAPE_KEY_COORDINATOR: Lazy<EscapeKeyCoordinator> =
    Lazy::new(EscapeKeyCoordinator::new);

/// Get the global escape key coordinator
pub fn get_escape_key_coordinator() -> &'static EscapeKeyCoordinator {
    &ESCAPE_KEY_COORDINATOR
}

/// Tauri command to register escape key user
#[tauri::command]
pub async fn register_escape_key_user(app_handle: AppHandle, user_id: String) -> Result<(), String> {
    let coordinator = get_escape_key_coordinator();
    coordinator.register_escape_user(&app_handle, &user_id).await
}

/// Tauri command to unregister escape key user
#[tauri::command]
pub async fn unregister_escape_key_user(app_handle: AppHandle, user_id: String) -> Result<(), String> {
    let coordinator = get_escape_key_coordinator();
    coordinator.unregister_escape_user(&app_handle, &user_id).await
}

/// Tauri command to force reset escape key state
#[tauri::command]
pub async fn force_reset_escape_key(app_handle: AppHandle) -> Result<(), String> {
    let coordinator = get_escape_key_coordinator();
    coordinator.force_reset(&app_handle).await
}

/// Tauri command to get escape key coordinator status
#[tauri::command]
pub async fn get_escape_key_coordinator_status() -> Result<serde_json::Value, String> {
    let coordinator = get_escape_key_coordinator();
    Ok(coordinator.get_status().await)
}

/// Force unregister escape key for debugging/emergency cleanup
#[tauri::command]
pub async fn force_unregister_escape_key(app_handle: AppHandle) -> Result<String, String> {
    info!("[EscapeKeyCoordinator] Force unregister escape key requested manually");

    let coordinator = get_escape_key_coordinator();
    coordinator.force_reset(&app_handle).await?;

    Ok("Escape key forcefully unregistered and released to other applications".to_string())
}

/// Test escape key flow: register -> stop all operations -> verify unregistered
#[tauri::command]
pub async fn test_escape_key_flow(app_handle: AppHandle) -> Result<String, String> {
    info!("[EscapeKeyCoordinator] Testing escape key registration/unregistration flow");

    let coordinator = get_escape_key_coordinator();

    // 1. Register escape key for test user
    coordinator.register_escape_user(&app_handle, "test_user").await?;
    let status_after_register = coordinator.get_status().await;

    // 2. Stop all operations (should unregister escape key)
    let stop_coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
    stop_coordinator.stop_all_operations(&app_handle, "Test escape key flow").await?;

    // 3. Check status after stop
    let status_after_stop = coordinator.get_status().await;

    Ok(format!(
        "Escape key flow test completed:\n- After register: {}\n- After stop: {}",
        serde_json::to_string_pretty(&status_after_register).unwrap_or_else(|_| "failed to serialize".to_string()),
        serde_json::to_string_pretty(&status_after_stop).unwrap_or_else(|_| "failed to serialize".to_string())
    ))
}

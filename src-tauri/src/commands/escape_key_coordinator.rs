use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};
use std::time::Instant;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, Code};
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

    /// Register a user for escape key handling
    pub async fn register_escape_user(&self, app_handle: &AppHandle, user_id: &str) -> Result<(), String> {
        debug!("[EscapeKeyCoordinator] Register escape user requested: {}", user_id);

        // Add user to tracking
        {
            let mut users = self.registered_users.write().await;
            users.insert(user_id.to_string(), Instant::now());
        }

        let new_count = self.user_count.fetch_add(1, Ordering::SeqCst) + 1;
        info!("[EscapeKeyCoordinator] User '{}' registered, count: {}", user_id, new_count);

        // Only register global shortcut if this is the first user and not already registered
        if new_count == 1 && !self.is_registered.load(Ordering::SeqCst) {
            if let Err(e) = self.register_global_shortcut(app_handle).await {
                warn!("[EscapeKeyCoordinator] Failed to register global shortcut: {}", e);
                // Rollback user count
                self.user_count.fetch_sub(1, Ordering::SeqCst);
                let mut users = self.registered_users.write().await;
                users.remove(user_id);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Unregister a user from escape key handling
    pub async fn unregister_escape_user(&self, app_handle: &AppHandle, user_id: &str) -> Result<(), String> {
        debug!("[EscapeKeyCoordinator] Unregister escape user requested: {}", user_id);

        // Remove user from tracking
        {
            let mut users = self.registered_users.write().await;
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
                // Counter was already at 0, nothing to decrement
                warn!("[EscapeKeyCoordinator] Attempted to unregister user '{}' but count was already 0 (current: {})", user_id, current_count);
            }
        }

        Ok(())
    }

    /// Register the global escape key shortcut
    async fn register_global_shortcut(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("[EscapeKeyCoordinator] Registering global Escape key shortcut");

        let escape_shortcut = Shortcut::new(None, Code::Escape);
        let result = app_handle.global_shortcut().register(escape_shortcut);

        match result {
            Ok(_) => {
                self.is_registered.store(true, Ordering::SeqCst);
                info!("[EscapeKeyCoordinator] Global Escape key shortcut registered successfully");
                Ok(())
            }
            Err(e) => {
                error!("[EscapeKeyCoordinator] Failed to register global Escape key shortcut: {}", e);
                Err(format!("Failed to register escape key: {}", e))
            }
        }
    }

    /// Unregister the global escape key shortcut
    async fn unregister_global_shortcut(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("[EscapeKeyCoordinator] Unregistering global Escape key shortcut");

        let escape_shortcut = Shortcut::new(None, Code::Escape);
        let result = app_handle.global_shortcut().unregister(escape_shortcut);

        match result {
            Ok(_) => {
                self.is_registered.store(false, Ordering::SeqCst);
                info!("[EscapeKeyCoordinator] Global Escape key shortcut unregistered successfully");
                Ok(())
            }
            Err(e) => {
                error!("[EscapeKeyCoordinator] Failed to unregister global Escape key shortcut: {}", e);
                Err(format!("Failed to unregister escape key: {}", e))
            }
        }
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

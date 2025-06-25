use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use log::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub notification_type: String, // "system", "toast", "both", or "disabled"
    pub sound_enabled: bool,
    pub duration: u32, // Duration in milliseconds for toast notifications
    pub position: String, // Position for toast notifications
    pub show_icons: bool,
    pub persist_important: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationData {
    pub title: String,
    pub message: String,
    pub level: String, // "info", "success", "warning", "error"
    pub important: Option<bool>,
    pub timeout: Option<u32>, // Override default duration
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemNotificationPermission {
    pub granted: bool,
    pub denied: bool,
    pub default: bool,
}

/// Get notification settings
#[tauri::command]
pub async fn get_notification_settings(
    state: tauri::State<'_, AppState>,
) -> Result<NotificationSettings, String> {
    let notification_type = state.notification_type.lock().map_err(|e| format!("Failed to get notification type: {}", e))?.clone();
    let sound_enabled = *state.notification_sound_enabled.lock().map_err(|e| format!("Failed to get sound enabled: {}", e))?;
    let duration = *state.notification_duration.lock().map_err(|e| format!("Failed to get duration: {}", e))?;
    let position = state.notification_position.lock().map_err(|e| format!("Failed to get position: {}", e))?.clone();
    let show_icons = *state.notification_show_icons().lock().map_err(|e| format!("Failed to get show icons: {}", e))?;
    let persist_important = *state.notification_persist_important().lock().map_err(|e| format!("Failed to get persist important: {}", e))?;

    Ok(NotificationSettings {
        notification_type,
        sound_enabled,
        duration,
        position,
        show_icons,
        persist_important,
    })
}

/// Set notification type
#[tauri::command]
pub async fn set_notification_type(
    state: tauri::State<'_, AppState>,
    notification_type: String,
) -> Result<(), String> {
    let mut type_guard = state.notification_type.lock().map_err(|e| format!("Failed to lock notification type: {}", e))?;
    *type_guard = notification_type;
    Ok(())
}

/// Set notification sound enabled
#[tauri::command]
pub async fn set_notification_sound_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let mut sound_guard = state.notification_sound_enabled.lock().map_err(|e| format!("Failed to lock sound enabled: {}", e))?;
    *sound_guard = enabled;
    Ok(())
}

/// Set notification duration
#[tauri::command]
pub async fn set_notification_duration(
    state: tauri::State<'_, AppState>,
    duration: u32,
) -> Result<(), String> {
    let mut duration_guard = state.notification_duration.lock().map_err(|e| format!("Failed to lock duration: {}", e))?;
    *duration_guard = duration;
    Ok(())
}

/// Set notification position
#[tauri::command]
pub async fn set_notification_position(
    state: tauri::State<'_, AppState>,
    position: String,
) -> Result<(), String> {
    let mut position_guard = state.notification_position.lock().map_err(|e| format!("Failed to lock position: {}", e))?;
    *position_guard = position;
    Ok(())
}

/// Set notification show icons
#[tauri::command]
pub async fn set_notification_show_icons(
    state: tauri::State<'_, AppState>,
    show_icons: bool,
) -> Result<(), String> {
    let mut icons_guard = state.notification_show_icons().lock().map_err(|e| format!("Failed to lock show icons: {}", e))?;
    *icons_guard = show_icons;
    Ok(())
}

/// Set notification persist important
#[tauri::command]
pub async fn set_notification_persist_important(
    state: tauri::State<'_, AppState>,
    persist_important: bool,
) -> Result<(), String> {
    let mut persist_guard = state.notification_persist_important().lock().map_err(|e| format!("Failed to lock persist important: {}", e))?;
    *persist_guard = persist_important;
    Ok(())
}

/// Check system notification permission
#[tauri::command]
pub async fn check_notification_permission(
    app: AppHandle,
) -> Result<SystemNotificationPermission, String> {
    match app.notification().permission_state() {
        Ok(permission) => {
            info!("Notification permission state: {:?}", permission);
            
            // Convert the permission state to a boolean representation
            // Since PermissionState is likely an enum, we'll handle the main states
            let granted = permission.to_string().to_lowercase().contains("granted");
            let denied = permission.to_string().to_lowercase().contains("denied");
            let default = !granted && !denied;
            
            Ok(SystemNotificationPermission {
                granted,
                denied,
                default,
            })
        },
        Err(e) => {
            error!("Failed to check notification permission: {}", e);
            Err(format!("Failed to check notification permission: {}", e))
        }
    }
}

/// Request notification permission
#[tauri::command]
pub async fn request_notification_permission(
    app: AppHandle,
) -> Result<SystemNotificationPermission, String> {
    match app.notification().request_permission() {
        Ok(permission) => {
            info!("Notification permission after request: {:?}", permission);
            
            // Convert the permission state to a boolean representation
            let granted = permission.to_string().to_lowercase().contains("granted");
            let denied = permission.to_string().to_lowercase().contains("denied");
            let default = !granted && !denied;
            
            Ok(SystemNotificationPermission {
                granted,
                denied,
                default,
            })
        },
        Err(e) => {
            error!("Failed to request notification permission: {}", e);
            Err(format!("Failed to request notification permission: {}", e))
        }
    }
}

/// Send a notification
#[tauri::command]
pub async fn send_notification(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    data: NotificationData,
) -> Result<(), String> {
    let settings = get_notification_settings(state.clone()).await?;
    
    // Check if notifications are disabled
    if settings.notification_type == "disabled" {
        return Ok(());
    }
    
    // Send system notification if enabled
    if settings.notification_type == "system" || settings.notification_type == "both" {
        let permission = check_notification_permission(app.clone()).await?;
        
        if permission.granted {
            let notification_result = app.notification()
                .builder()
                .title(&data.title)
                .body(&data.message)
                .show();
                
            if let Err(e) = notification_result {
                error!("Failed to send system notification: {}", e);
            } else {
                info!("System notification sent successfully");
            }
        }
    }
    
    // Send toast notification if enabled (emitted to frontend)
    if settings.notification_type == "toast" || settings.notification_type == "both" {
        app.emit("toast-notification", &data).map_err(|e| format!("Failed to emit toast notification: {}", e))?;
        info!("Toast notification emitted successfully");
    }
    
    Ok(())
}

/// Test notification - sends a test notification with current settings
#[tauri::command]
pub async fn test_notification(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let test_data = NotificationData {
        title: "Test Notification".to_string(),
        message: "This is a test notification to verify your settings.".to_string(),
        level: "info".to_string(),
        important: Some(false),
        timeout: None,
    };
    
    send_notification(app, state, test_data).await
}
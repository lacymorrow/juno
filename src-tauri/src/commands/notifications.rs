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
    let notification_type = state.get_notification_type().map_err(|e| format!("Failed to get notification type: {}", e))?;
    let sound_enabled = state.get_notification_sound_enabled().map_err(|e| format!("Failed to get sound enabled: {}", e))?;
    let duration = state.get_notification_duration().map_err(|e| format!("Failed to get duration: {}", e))?;
    let position = state.get_notification_position().map_err(|e| format!("Failed to get position: {}", e))?;
    let show_icons = state.get_notification_show_icons().map_err(|e| format!("Failed to get show icons: {}", e))?;
    let persist_important = state.get_notification_persist_important().map_err(|e| format!("Failed to get persist important: {}", e))?;

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
    state.set_notification_type(notification_type).map_err(|e| format!("Failed to set notification type: {}", e))?;
    Ok(())
}

/// Set notification sound enabled
#[tauri::command]
pub async fn set_notification_sound_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    state.set_notification_sound_enabled(enabled).map_err(|e| format!("Failed to set sound enabled: {}", e))?;
    Ok(())
}

/// Set notification duration
#[tauri::command]
pub async fn set_notification_duration(
    state: tauri::State<'_, AppState>,
    duration: u32,
) -> Result<(), String> {
    state.set_notification_duration(duration).map_err(|e| format!("Failed to set duration: {}", e))?;
    Ok(())
}

/// Set notification position
#[tauri::command]
pub async fn set_notification_position(
    state: tauri::State<'_, AppState>,
    position: String,
) -> Result<(), String> {
    state.set_notification_position(position).map_err(|e| format!("Failed to set position: {}", e))?;
    Ok(())
}

/// Set notification show icons
#[tauri::command]
pub async fn set_notification_show_icons(
    state: tauri::State<'_, AppState>,
    show_icons: bool,
) -> Result<(), String> {
    state.set_notification_show_icons(show_icons).map_err(|e| format!("Failed to set show icons: {}", e))?;
    Ok(())
}

/// Set notification persist important
#[tauri::command]
pub async fn set_notification_persist_important(
    state: tauri::State<'_, AppState>,
    persist_important: bool,
) -> Result<(), String> {
    state.set_notification_persist_important(persist_important).map_err(|e| format!("Failed to set persist important: {}", e))?;
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

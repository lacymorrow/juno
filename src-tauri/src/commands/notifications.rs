use crate::state::AppState;
use log::{error, info};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// What there is to decide about notifications.
///
/// One switch, because one switch is what Juno can actually act on. The
/// previous six (type, sound, duration, position, show icons, persist
/// important) were stored, read back by the screen that drew them, and
/// consulted by nothing on the way to a notification. Four of them could
/// never have worked: macOS owns how a user notification is presented, so an
/// app does not get to pick its duration or its corner. The "toast" half of
/// the type setting emitted an event no window listened for, and choosing
/// "Disabled" changed nothing at all while promising, in as many words, that
/// Juno would stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub enabled: bool,
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
    Ok(NotificationSettings {
        enabled: state.get_notifications_enabled()?,
    })
}

/// Turn Juno's notifications on or off.
#[tauri::command]
pub async fn set_notifications_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    state.set_notifications_enabled(enabled)
}

/// Show a system notification, unless the person has switched them off.
///
/// The one door. Everything that notifies goes through here, so "off" is a
/// single check in a single place rather than a promise each call site has to
/// remember to keep. It used to be the other way round: this module honoured
/// the setting and the four things that actually notify (the scheduler, a new
/// automation, the request for the physical mouse, the menu-bar-only hint)
/// each called the plugin directly and honoured nothing.
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let enabled = app
        .try_state::<AppState>()
        .map(|state| state.get_notifications_enabled().unwrap_or(true))
        // No state means we are too early or headless. Speak up rather than
        // swallow: a missed notification is worse than an extra one.
        .unwrap_or(true);
    if !enabled {
        info!("Notifications are off; not showing: {}", title);
        return;
    }

    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        error!("Failed to show notification '{}': {}", title, e);
    }
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
        }
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
        }
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
    _state: tauri::State<'_, AppState>,
    data: NotificationData,
) -> Result<(), String> {
    notify(&app, &data.title, &data.message);
    Ok(())
}

/// Test notification - sends a test notification with current settings
#[tauri::command]
pub async fn test_notification(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _ = state;
    notify(
        &app,
        "Juno",
        "This is what a notification from Juno looks like.",
    );
    Ok(())
}

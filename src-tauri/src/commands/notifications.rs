use crate::state::AppState;
use log::{error, info, warn};
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

/// What macOS says about Juno's notifications, read from
/// `UNUserNotificationCenter.getNotificationSettings`.
///
/// This is the real authorization, the same one System Settings >
/// Notifications > Juno shows. The notification plugin's own permission check
/// is a hard-coded `granted` on desktop and asks macOS nothing, so it is not
/// used here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAuthorization {
    /// The person allowed notifications (including quiet delivery).
    Authorized,
    /// The person turned notifications off for Juno.
    Denied,
    /// macOS has never asked. `request_notification_permission` asks.
    NotDetermined,
    /// This process cannot post notifications as Juno (a development build or
    /// a binary outside Juno.app), so there is nothing to ask or to change.
    Unavailable,
}

impl NotificationAuthorization {
    /// Map `UNAuthorizationStatus`'s raw value.
    ///
    /// 0 notDetermined, 1 denied, 2 authorized, 3 provisional, 4 ephemeral.
    /// Provisional and ephemeral both deliver, so both read as authorized. A
    /// value this build does not know is `Unavailable`, never a guess.
    pub fn from_raw(raw: isize) -> Self {
        match raw {
            0 => Self::NotDetermined,
            1 => Self::Denied,
            2..=4 => Self::Authorized,
            _ => Self::Unavailable,
        }
    }

    /// Whether macOS will show a notification from Juno.
    pub fn allows_notifications(self) -> bool {
        self == Self::Authorized
    }
}

/// Everything the notifications row draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationStatus {
    pub authorization: NotificationAuthorization,
    /// Why nothing can be posted, when `authorization` is `unavailable`.
    pub unavailable_reason: Option<String>,
}

/// The two links that open System Settings > Notifications, most specific
/// first: the pane scoped to Juno's bundle id, then the pane itself.
pub fn notification_settings_urls(bundle_id: &str) -> [String; 2] {
    [
        format!(
            "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={}",
            bundle_id
        ),
        "x-apple.systempreferences:com.apple.preference.notifications".to_string(),
    ]
}

const DENIED_REASON: &str = "Notifications are off for Juno in System Settings.";
const NOT_ASKED_REASON: &str = "Allow notifications for Juno first.";

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

/// Is the switch on?
///
/// No state means we are too early or headless. Speak up rather than swallow:
/// a missed notification is worse than an extra one.
fn notifications_enabled(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .map(|state| state.get_notifications_enabled().unwrap_or(true))
        .unwrap_or(true)
}

/// Whether this process is the executable inside a `.app` bundle.
///
/// macOS attributes a user notification to a bundle. A loose binary has none,
/// and `UNUserNotificationCenter` raises when asked for one, so nothing here
/// touches it until this is true.
#[cfg(target_os = "macos")]
fn running_from_app_bundle() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    exe.parent()
        .is_some_and(|dir| dir.ends_with("Contents/MacOS"))
}

#[cfg(not(target_os = "macos"))]
fn running_from_app_bundle() -> bool {
    true
}

/// Why this process cannot post notifications as Juno, if it cannot.
fn unavailable_reason() -> Option<&'static str> {
    if tauri::is_dev() {
        return Some(
            "This is a development build, so notifications appear under Terminal rather than Juno.",
        );
    }
    if !running_from_app_bundle() {
        return Some("Juno is running outside Juno.app, so macOS has no app to notify as.");
    }
    None
}

/// Ask macOS for Juno's authorization. `None` means it did not answer in time.
#[cfg(target_os = "macos")]
fn read_authorization() -> Option<NotificationAuthorization> {
    use block2::RcBlock;
    use objc2_user_notifications::{UNNotificationSettings, UNUserNotificationCenter};
    use std::ptr::NonNull;

    let (tx, rx) = std::sync::mpsc::channel::<isize>();
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        // SAFETY: macOS hands the handler a live settings object.
        let raw = unsafe { settings.as_ref() }.authorizationStatus().0;
        let _ = tx.send(raw);
    });
    center.getNotificationSettingsWithCompletionHandler(&block);
    rx.recv_timeout(std::time::Duration::from_secs(3))
        .ok()
        .map(NotificationAuthorization::from_raw)
}

#[cfg(not(target_os = "macos"))]
fn read_authorization() -> Option<NotificationAuthorization> {
    Some(NotificationAuthorization::Authorized)
}

/// Show the system permission prompt and wait for the answer.
#[cfg(target_os = "macos")]
fn request_authorization() -> Result<(), String> {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::NSError;
    use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let block = RcBlock::new(move |_granted: Bool, _error: *mut NSError| {
        let _ = tx.send(());
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &block,
    );
    // The prompt waits on a person, so this is generous.
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| "macOS did not answer the notification prompt.".to_string())
}

#[cfg(not(target_os = "macos"))]
fn request_authorization() -> Result<(), String> {
    Ok(())
}

/// What macOS currently allows for Juno.
pub fn current_status() -> NotificationStatus {
    if let Some(reason) = unavailable_reason() {
        return NotificationStatus {
            authorization: NotificationAuthorization::Unavailable,
            unavailable_reason: Some(reason.to_string()),
        };
    }
    match read_authorization() {
        Some(authorization) => NotificationStatus {
            authorization,
            unavailable_reason: None,
        },
        None => {
            warn!("macOS did not report notification settings in time");
            NotificationStatus {
                authorization: NotificationAuthorization::Unavailable,
                unavailable_reason: Some(
                    "macOS did not report Juno's notification settings.".into(),
                ),
            }
        }
    }
}

/// The refusal [`deliver`] returns before it reaches the plugin, if any.
fn refusal(enabled: bool, status: &NotificationStatus) -> Result<(), String> {
    if !enabled {
        return Err("Notifications are off in Juno. Turn on Show notifications.".to_string());
    }
    match status.authorization {
        NotificationAuthorization::Authorized => Ok(()),
        NotificationAuthorization::Denied => Err(DENIED_REASON.to_string()),
        NotificationAuthorization::NotDetermined => Err(NOT_ASKED_REASON.to_string()),
        NotificationAuthorization::Unavailable => Err(status
            .unavailable_reason
            .clone()
            .unwrap_or_else(|| "Notifications are unavailable.".to_string())),
    }
}

/// Hand one notification to macOS, or say why it cannot be handed over.
///
/// The one door. Everything that notifies goes through here, so "off" is a
/// single check in a single place rather than a promise each call site has to
/// remember to keep. `Ok(())` means macOS allows Juno's notifications and the
/// plugin took this one; the plugin posts asynchronously, so it still does not
/// mean anyone saw it.
pub fn deliver(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    if let Err(reason) = refusal(notifications_enabled(app), &current_status()) {
        info!("Withholding '{}': {}", title, reason);
        return Err(reason);
    }

    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("macOS would not take the notification: {}", e))
}

/// Notify from a background task, where there is no one to tell.
///
/// The scheduler, a new automation, a finished timer, the request for the
/// physical mouse and the menu-bar-only hint all land here. None of them has
/// anything to do with a failure but record it, so this logs the reason and
/// moves on. Anything a person pressed calls [`deliver`] and shows them what
/// came back.
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(reason) = deliver(app, title, body) {
        warn!("Notification '{}' did not go out: {}", title, reason);
    }
}

/// What macOS currently allows for Juno, for the notifications row.
#[tauri::command]
pub async fn check_notification_permission() -> Result<NotificationStatus, String> {
    tauri::async_runtime::spawn_blocking(current_status)
        .await
        .map_err(|e| format!("Could not read notification settings: {}", e))
}

/// Show the real macOS permission prompt, then report the answer.
#[tauri::command]
pub async fn request_notification_permission() -> Result<NotificationStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        if let Some(reason) = unavailable_reason() {
            return Err(reason.to_string());
        }
        request_authorization()?;
        Ok(current_status())
    })
    .await
    .map_err(|e| format!("Could not ask for notifications: {}", e))?
}

/// Open System Settings > Notifications for Juno.
///
/// Tries the pane scoped to Juno's bundle id, then the general pane.
#[tauri::command]
pub async fn open_notification_settings(app: AppHandle) -> Result<(), String> {
    let bundle_id = app.config().identifier.clone();
    for url in notification_settings_urls(&bundle_id) {
        match std::process::Command::new("open").arg(&url).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => warn!("`open {}` exited with {}", url, status),
            Err(e) => error!("`open {}` failed: {}", url, e),
        }
    }
    Err("Could not open System Settings".to_string())
}

/// Send a notification.
///
/// Returns the reason when there is one, rather than the `Ok(())` it used to
/// return whatever happened.
#[tauri::command]
pub async fn send_notification(
    app: AppHandle,
    _state: tauri::State<'_, AppState>,
    data: NotificationData,
) -> Result<(), String> {
    deliver(&app, &data.title, &data.message)
}

/// Send one notification because someone asked for one, and say what happened.
#[tauri::command]
pub async fn test_notification(app: AppHandle) -> Result<(), String> {
    deliver(
        &app,
        "Juno",
        "This is what a notification from Juno looks like.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_status_maps_to_authorization() {
        use NotificationAuthorization::*;
        assert_eq!(NotificationAuthorization::from_raw(0), NotDetermined);
        assert_eq!(NotificationAuthorization::from_raw(1), Denied);
        assert_eq!(NotificationAuthorization::from_raw(2), Authorized);
        // Provisional and ephemeral both deliver.
        assert_eq!(NotificationAuthorization::from_raw(3), Authorized);
        assert_eq!(NotificationAuthorization::from_raw(4), Authorized);
        // Unknown is never read as allowed.
        assert_eq!(NotificationAuthorization::from_raw(9), Unavailable);
        assert_eq!(NotificationAuthorization::from_raw(-1), Unavailable);
    }

    #[test]
    fn only_authorized_allows_notifications() {
        use NotificationAuthorization::*;
        assert!(Authorized.allows_notifications());
        for a in [Denied, NotDetermined, Unavailable] {
            assert!(!a.allows_notifications());
        }
    }

    fn status(authorization: NotificationAuthorization) -> NotificationStatus {
        NotificationStatus {
            authorization,
            unavailable_reason: None,
        }
    }

    #[test]
    fn a_send_is_refused_unless_juno_and_macos_both_allow_it() {
        use NotificationAuthorization::*;
        assert!(refusal(true, &status(Authorized)).is_ok());
        assert!(refusal(false, &status(Authorized)).is_err());
        assert_eq!(refusal(true, &status(Denied)).unwrap_err(), DENIED_REASON);
        assert_eq!(
            refusal(true, &status(NotDetermined)).unwrap_err(),
            NOT_ASKED_REASON
        );
        assert!(refusal(true, &status(Unavailable)).is_err());
    }

    /// The reported defect: the Open Settings button failed. The row now opens
    /// the pane scoped to Juno, with the general pane as the fallback.
    #[test]
    fn settings_links_target_juno_then_the_general_pane() {
        let urls = notification_settings_urls("com.juno.desktop");
        assert_eq!(
            urls[0],
            "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=com.juno.desktop"
        );
        assert_eq!(
            urls[1],
            "x-apple.systempreferences:com.apple.preference.notifications"
        );
    }
}

use crate::state::AppState;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};

/// The System Settings pane that holds the one switch a person can change.
///
/// Matches the `"notifications"` arm of
/// [`crate::commands::permissions::open_system_preferences`], so the screen can
/// send someone straight there.
pub const SYSTEM_SETTINGS_PANE: &str = "notifications";

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

/// What the notification plugin reports about permission, as a variant.
///
/// It used to be three booleans derived by stringifying the plugin's value and
/// substring-matching it:
///
/// ```ignore
/// let granted = permission.to_string().to_lowercase().contains("granted");
/// let denied  = permission.to_string().to_lowercase().contains("denied");
/// let default = !granted && !denied;
/// ```
///
/// Any value whose text held neither word landed in `default` with nothing
/// said, and a `Display` impl that changed shape upstream would have moved the
/// answer without moving a line of Juno. Matching the variants puts that
/// decision where the compiler can see it.
///
/// Worth knowing what this value is and is not: on desktop the plugin's
/// `permission_state()` is a hard-coded `Ok(PermissionState::Granted)`. It asks
/// macOS nothing. So this is a report of what the plugin said, never evidence
/// that a banner will appear, which is why nothing built from it is allowed to
/// use the word "allowed".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    Granted,
    Denied,
    MustAsk,
    /// The plugin could not answer. Its own state, not a third guess folded
    /// into one of the three above.
    Unknown,
}

impl From<PermissionState> for PluginPermission {
    fn from(state: PermissionState) -> Self {
        // Exhaustive on purpose, with no catch-all arm. `PermissionState` is a
        // closed enum, so a variant added upstream breaks this build instead of
        // quietly reading as one of the answers below.
        match state {
            PermissionState::Granted => Self::Granted,
            PermissionState::Denied => Self::Denied,
            PermissionState::Prompt | PermissionState::PromptWithRationale => Self::MustAsk,
        }
    }
}

/// Whether a notification Juno posts can reach the screen at all.
///
/// Rust decides this and the screen draws the answer. Three of the four states
/// are facts Juno can establish about itself; the fourth is the honest edge of
/// what a desktop app can know, because macOS does not report a per-app
/// notification choice back to an app posting through this API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAvailability {
    /// Juno will hand the notification to macOS, and macOS decides from there.
    MacosDecides,
    /// Switched off in Juno's own settings.
    OffInJuno,
    /// A development build posts under `com.apple.Terminal`, never under Juno.
    DevBuild,
    /// Running outside the `.app` bundle, so macOS has no app to attribute a
    /// notification to.
    Unbundled,
}

impl NotificationAvailability {
    /// Why nothing will appear, in a sentence someone can act on, or `None`
    /// when Juno has no reason to think nothing will appear.
    pub fn blocked_reason(self) -> Option<&'static str> {
        match self {
            Self::MacosDecides => None,
            Self::OffInJuno => {
                Some("Notifications are off in Juno. Turn on Show notifications above.")
            }
            Self::DevBuild => Some(
                "This is a development build. macOS posts its notifications under Terminal \
                 rather than Juno, so a Juno banner cannot appear. Test from an installed Juno.",
            ),
            Self::Unbundled => Some(
                "Juno is running outside Juno.app, so macOS has no app to attribute a \
                 notification to and shows nothing. Run the installed Juno.app.",
            ),
        }
    }

    /// The short line at the right edge of the row.
    pub fn headline(self) -> &'static str {
        match self {
            Self::MacosDecides => "macOS decides",
            Self::OffInJuno => "Off in Juno",
            Self::DevBuild => "Development build",
            Self::Unbundled => "Not running from Juno.app",
        }
    }

    /// The sentence under the label. A blocked state explains itself; the
    /// unblocked one says exactly how far Juno's knowledge goes, because the
    /// row that used to sit here read "Allowed" on a machine where no
    /// notification had appeared for weeks.
    pub fn detail(self) -> &'static str {
        self.blocked_reason().unwrap_or(
            "Juno hands every notification to macOS. Whether one appears is your choice in \
             System Settings > Notifications > Juno, and macOS does not report that choice \
             back to an app, so Juno cannot promise you will see one.",
        )
    }

    /// Whether Juno should try at all.
    pub fn can_notify(self) -> bool {
        self.blocked_reason().is_none()
    }

    /// The System Settings pane worth opening, if any. A development build and
    /// an unbundled binary have no row in that pane, so the screen does not
    /// send anyone there to look for one.
    pub fn system_settings_pane(self) -> Option<&'static str> {
        match self {
            Self::MacosDecides => Some(SYSTEM_SETTINGS_PANE),
            Self::OffInJuno | Self::DevBuild | Self::Unbundled => None,
        }
    }
}

/// Everything the notifications pane draws, decided in Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationStatus {
    pub availability: NotificationAvailability,
    pub plugin_permission: PluginPermission,
    pub headline: String,
    pub detail: String,
    pub can_notify: bool,
    pub system_settings_pane: Option<String>,
}

/// Assemble the pane's copy from the two things Juno knows.
pub fn status_for(
    availability: NotificationAvailability,
    plugin_permission: PluginPermission,
) -> NotificationStatus {
    NotificationStatus {
        availability,
        plugin_permission,
        headline: availability.headline().to_string(),
        detail: availability.detail().to_string(),
        can_notify: availability.can_notify(),
        system_settings_pane: availability.system_settings_pane().map(str::to_string),
    }
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
/// so there is nothing for the person to authorize and nothing appears.
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

/// What Juno can establish, right now, about whether a notification will show.
pub fn availability(app: &AppHandle) -> NotificationAvailability {
    if !notifications_enabled(app) {
        return NotificationAvailability::OffInJuno;
    }
    // Bound to a value so this reads as the runtime question it is: `is_dev()`
    // is a `const fn` over the `custom-protocol` feature.
    let dev_build = tauri::is_dev();
    if dev_build {
        return NotificationAvailability::DevBuild;
    }
    if !running_from_app_bundle() {
        return NotificationAvailability::Unbundled;
    }
    NotificationAvailability::MacosDecides
}

/// The refusal [`deliver`] returns before it reaches the plugin, if any.
fn refusal(availability: NotificationAvailability) -> Result<(), String> {
    match availability.blocked_reason() {
        Some(reason) => Err(reason.to_string()),
        None => Ok(()),
    }
}

/// Hand one notification to macOS, or say why it cannot be handed over.
///
/// The one door. Everything that notifies goes through here, so "off" is a
/// single check in a single place rather than a promise each call site has to
/// remember to keep.
///
/// What this cannot promise, and why: `tauri_plugin_notification` posts through
/// `NSUserNotificationCenter` inside `tauri::async_runtime::spawn` and returns
/// `Ok(())` before the post is attempted, discarding whatever the post did. So
/// a delivery failure never comes back here, and `Ok(())` from this function
/// means Juno handed the notification over, never that anyone saw it. Only the
/// refusals Juno can establish itself are errors, and they are checked first.
pub fn deliver(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    let availability = availability(app);
    if let Err(reason) = refusal(availability) {
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

/// What the notifications pane should say.
#[tauri::command]
pub async fn check_notification_permission(app: AppHandle) -> Result<NotificationStatus, String> {
    let plugin_permission = match app.notification().permission_state() {
        Ok(state) => PluginPermission::from(state),
        Err(e) => {
            // Not fatal, and not "granted" either: the state is unknown and the
            // pane says so rather than picking one of the other answers.
            error!("Could not read the notification permission state: {}", e);
            PluginPermission::Unknown
        }
    };

    let availability = availability(&app);
    info!(
        "Notifications: {:?} (the plugin reports {:?})",
        availability, plugin_permission
    );

    Ok(status_for(availability, plugin_permission))
}

/// Send a notification.
///
/// Returns the reason when there is one, rather than the `Ok(())` it used to
/// return whatever happened. Its two in-process callers, a finished background
/// agent session and one waiting for input, already log what comes back.
#[tauri::command]
pub async fn send_notification(
    app: AppHandle,
    _state: tauri::State<'_, AppState>,
    data: NotificationData,
) -> Result<(), String> {
    deliver(&app, &data.title, &data.message)
}

/// Send one notification because someone asked for one.
///
/// It returns what happened. The body it replaces was `notify(...)` followed by
/// `Ok(())`, so the command could not fail from the screen's point of view no
/// matter what the notification did, and pressing the button looked like
/// success on a machine where notifications had not worked in weeks.
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

    const EVERY_AVAILABILITY: [NotificationAvailability; 4] = [
        NotificationAvailability::MacosDecides,
        NotificationAvailability::OffInJuno,
        NotificationAvailability::DevBuild,
        NotificationAvailability::Unbundled,
    ];

    const EVERY_PERMISSION: [PluginPermission; 4] = [
        PluginPermission::Granted,
        PluginPermission::Denied,
        PluginPermission::MustAsk,
        PluginPermission::Unknown,
    ];

    const BLOCKED: [NotificationAvailability; 3] = [
        NotificationAvailability::OffInJuno,
        NotificationAvailability::DevBuild,
        NotificationAvailability::Unbundled,
    ];

    /// Every state the plugin can hand us maps to a state of ours. `From` has
    /// no catch-all arm, so a variant added upstream fails to compile rather
    /// than reading as one of these.
    #[test]
    fn every_plugin_permission_maps_to_a_variant() {
        assert_eq!(
            PluginPermission::from(PermissionState::Granted),
            PluginPermission::Granted
        );
        assert_eq!(
            PluginPermission::from(PermissionState::Denied),
            PluginPermission::Denied
        );
        assert_eq!(
            PluginPermission::from(PermissionState::Prompt),
            PluginPermission::MustAsk
        );
        assert_eq!(
            PluginPermission::from(PermissionState::PromptWithRationale),
            PluginPermission::MustAsk
        );
    }

    /// An unreadable permission state is its own answer and survives to the
    /// screen as one. The substring match this replaces computed
    /// `default = !granted && !denied`, so anything it did not recognise
    /// became "Not asked" and the pane offered a button to fix it.
    #[test]
    fn an_unknown_permission_stays_unknown() {
        let status = status_for(
            NotificationAvailability::MacosDecides,
            PluginPermission::Unknown,
        );
        assert_eq!(status.plugin_permission, PluginPermission::Unknown);
    }

    /// The reported defect: "it doesn't show anything, even though it says
    /// allowed". No state may say that word, in any combination, because the
    /// plugin's desktop `permission_state()` is a hard-coded `Granted` that
    /// asks macOS nothing.
    #[test]
    fn no_status_claims_notifications_are_allowed() {
        for availability in EVERY_AVAILABILITY {
            for permission in EVERY_PERMISSION {
                let status = status_for(availability, permission);
                let copy = format!("{} {}", status.headline, status.detail).to_lowercase();
                assert!(
                    !copy.contains("allowed"),
                    "{:?} with {:?} claims allowed: {}",
                    availability,
                    permission,
                    copy
                );
            }
        }
    }

    /// A blocked state is a sentence someone can act on, not a shrug, and it
    /// does not send anyone to a System Settings pane that holds no row for it.
    #[test]
    fn a_blocked_state_explains_itself() {
        for availability in BLOCKED {
            let reason = availability
                .blocked_reason()
                .expect("a blocked state has a reason");
            assert!(reason.ends_with('.'), "{:?}: {}", availability, reason);
            assert!(!availability.can_notify());

            let status = status_for(availability, PluginPermission::Granted);
            assert!(!status.can_notify);
            assert_eq!(status.detail, reason);
            assert!(status.system_settings_pane.is_none());
        }
    }

    /// The one unblocked state still promises nothing, and is the only one that
    /// points at the place a person can change the answer.
    #[test]
    fn the_unblocked_state_points_at_system_settings_without_promising() {
        let status = status_for(
            NotificationAvailability::MacosDecides,
            PluginPermission::Granted,
        );
        assert!(status.can_notify);
        assert_eq!(
            status.system_settings_pane.as_deref(),
            Some(SYSTEM_SETTINGS_PANE)
        );
        assert!(status.detail.contains("System Settings"));
    }

    /// What `test_notification` returns when nothing will appear. `deliver`
    /// runs this before it reaches the plugin, so the error reaches the screen
    /// instead of the old unconditional `Ok(())`.
    #[test]
    fn a_blocked_send_is_an_error_and_not_ok() {
        for availability in BLOCKED {
            let result = refusal(availability);
            assert_eq!(
                result.as_ref().err().map(String::as_str),
                availability.blocked_reason(),
                "{:?} did not refuse with its own reason",
                availability
            );
        }
        assert!(refusal(NotificationAvailability::MacosDecides).is_ok());
    }
}

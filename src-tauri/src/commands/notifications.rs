use crate::state::AppState;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager};

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
///
/// macOS shows the prompt only while authorization is not determined; after
/// that this returns the standing answer at once.
#[cfg(target_os = "macos")]
fn request_authorization() -> Result<(), String> {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::NSError;
    use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};

    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let block = RcBlock::new(move |_granted: Bool, error: *mut NSError| {
        // SAFETY: a non-null error is a live NSError for this call.
        let error = unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string());
        let _ = tx.send(error);
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &block,
    );
    // The prompt waits on a person, so this is generous.
    match rx.recv_timeout(std::time::Duration::from_secs(300)) {
        Ok(None) => Ok(()),
        Ok(Some(error)) => Err(format!("macOS refused the notification request: {}", error)),
        Err(_) => Err("macOS did not answer the notification prompt.".to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn request_authorization() -> Result<(), String> {
    Ok(())
}

/// A request identifier no other notification from this process shares.
///
/// macOS replaces a pending or delivered notification that has the same
/// identifier, so two timers finishing together must not collide.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn request_identifier(sequence: u64, millis: u128) -> String {
    format!("juno.{}.{}", millis, sequence)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn next_request_identifier() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    request_identifier(SEQUENCE.fetch_add(1, Ordering::Relaxed), millis)
}

/// Hand one notification to `UNUserNotificationCenter` and wait for its answer.
///
/// This is the same center the authorization comes from, so what the
/// notifications row shows and what posting does can no longer disagree.
#[cfg(target_os = "macos")]
fn post(title: &str, body: &str) -> Result<(), String> {
    use block2::RcBlock;
    use objc2_foundation::{NSError, NSString};
    use objc2_user_notifications::{
        UNMutableNotificationContent, UNNotificationRequest, UNNotificationSound,
        UNUserNotificationCenter,
    };

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));

    let identifier = NSString::from_str(&next_request_identifier());
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);

    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    let block = RcBlock::new(move |error: *mut NSError| {
        // SAFETY: a non-null error is a live NSError for this call.
        let error = unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string());
        let _ = tx.send(error);
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, Some(&*block));

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(None) => Ok(()),
        Ok(Some(error)) => Err(format!("macOS would not take the notification: {}", error)),
        Err(_) => {
            // The request is queued; macOS was slow to confirm. Not a failure.
            warn!("macOS did not confirm notification '{}' in time", title);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn post(title: &str, _body: &str) -> Result<(), String> {
    info!("Notification '{}' has no presenter on this platform", title);
    Ok(())
}

/// How a notification is presented while Juno is the frontmost app.
///
/// Without a delegate saying otherwise, macOS posts a frontmost app's
/// notifications silently: no banner, no sound. Juno's bar and Settings window
/// make it frontmost often, which is exactly when the test button is pressed.
#[cfg(target_os = "macos")]
fn foreground_presentation() -> objc2_user_notifications::UNNotificationPresentationOptions {
    use objc2_user_notifications::UNNotificationPresentationOptions as Options;
    Options::Banner | Options::List | Options::Sound
}

#[cfg(target_os = "macos")]
mod presenter {
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AnyThread};
    use objc2_user_notifications::{
        UNNotification, UNNotificationPresentationOptions, UNUserNotificationCenter,
        UNUserNotificationCenterDelegate,
    };

    define_class!(
        // SAFETY: NSObject has no subclassing requirements and this class has
        // no Drop impl.
        #[unsafe(super(NSObject))]
        #[name = "JunoNotificationPresenter"]
        struct Presenter;

        unsafe impl NSObjectProtocol for Presenter {}

        unsafe impl UNUserNotificationCenterDelegate for Presenter {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                completion_handler.call((super::foreground_presentation(),));
            }
        }
    );

    impl Presenter {
        fn new() -> Retained<Self> {
            let this = Self::alloc().set_ivars(());
            // SAFETY: NSObject's init on a freshly allocated instance.
            unsafe { msg_send![super(this), init] }
        }
    }

    /// Make Juno the center's delegate, once, for the life of the process.
    pub fn install() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let presenter = Presenter::new();
            UNUserNotificationCenter::currentNotificationCenter()
                .setDelegate(Some(ProtocolObject::from_ref(&*presenter)));
            // The center holds its delegate weakly. Keep this one alive.
            std::mem::forget(presenter);
        });
    }
}

/// Let notifications show as banners while Juno is frontmost.
///
/// Called once from setup. Does nothing where there is no Juno.app to post as,
/// because the notification center raises without a bundle.
pub fn install_presenter() {
    if let Some(reason) = unavailable_reason() {
        info!("Notification presenter not installed: {}", reason);
        return;
    }
    #[cfg(target_os = "macos")]
    presenter::install();
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

/// Whether to show the macOS prompt before posting.
///
/// The first time Juno has something to say and macOS has never asked is the
/// moment to ask, so a timer finishing brings up the prompt rather than
/// nothing happening at all.
fn should_ask_first(enabled: bool, authorization: NotificationAuthorization) -> bool {
    enabled && authorization == NotificationAuthorization::NotDetermined
}

/// The reason [`deliver`] withholds a notification, if it does.
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

/// Post one notification, or say why it was not posted.
///
/// The one door. Everything that notifies goes through here, so "off" is a
/// single check in a single place. Blocks while macOS answers (and, the first
/// time, while the person answers the prompt), so call it off the main thread.
pub fn deliver(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    let enabled = notifications_enabled(app);
    let mut status = current_status();
    if should_ask_first(enabled, status.authorization) {
        if let Err(reason) = request_authorization() {
            warn!("Asking for notifications failed: {}", reason);
        }
        status = current_status();
    }
    if let Err(reason) = refusal(enabled, &status) {
        info!("Withholding '{}': {}", title, reason);
        return Err(reason);
    }
    post(title, body)
}

/// Notify from anywhere, without waiting and without telling anyone if it
/// fails.
///
/// The scheduler, a new automation, a finished timer, the request for the
/// physical mouse and the menu-bar-only hint all land here. Some run on the
/// main thread, and the first notification can wait on the permission prompt,
/// so the work happens on its own thread. A failure is logged and nothing more.
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let app = app.clone();
    let title = title.to_string();
    let body = body.to_string();
    let spawned = std::thread::Builder::new()
        .name("juno-notify".into())
        .spawn(move || {
            if let Err(reason) = deliver(&app, &title, &body) {
                warn!("Notification '{}' did not go out: {}", title, reason);
            }
        });
    if let Err(e) = spawned {
        warn!("Could not start the notification thread: {}", e);
    }
}

/// [`deliver`] on a blocking thread, logging rather than returning a failure.
async fn deliver_quietly(app: AppHandle, title: String, body: String) {
    let result = tauri::async_runtime::spawn_blocking(move || {
        deliver(&app, &title, &body).map_err(|reason| (title, reason))
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err((title, reason))) => warn!("Notification '{}' did not go out: {}", title, reason),
        Err(e) => warn!("Notification thread failed: {}", e),
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

/// Send a notification. A failure is logged, never shown.
#[tauri::command]
pub async fn send_notification(
    app: AppHandle,
    _state: tauri::State<'_, AppState>,
    data: NotificationData,
) -> Result<(), String> {
    deliver_quietly(app, data.title, data.message).await;
    Ok(())
}

/// Send one notification because someone asked for one.
///
/// The banner is the answer. If it cannot go out, the reason is logged and the
/// row's status (read again by the window afterwards) says what to change.
#[tauri::command]
pub async fn test_notification(app: AppHandle) -> Result<(), String> {
    deliver_quietly(
        app,
        "Juno".to_string(),
        "This is what a notification from Juno looks like.".to_string(),
    )
    .await;
    Ok(())
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

    #[test]
    fn juno_asks_the_first_time_it_has_something_to_say() {
        use NotificationAuthorization::*;
        assert!(should_ask_first(true, NotDetermined));
        // Off in Juno: no prompt for something that will not be posted.
        assert!(!should_ask_first(false, NotDetermined));
        // Already answered, or nothing to ask: never prompt again.
        for a in [Authorized, Denied, Unavailable] {
            assert!(!should_ask_first(true, a));
        }
    }

    #[test]
    fn request_identifiers_never_collide() {
        assert_ne!(request_identifier(0, 5), request_identifier(1, 5));
        assert_eq!(request_identifier(7, 42), "juno.42.7");
        assert_ne!(next_request_identifier(), next_request_identifier());
    }

    /// The reported defect: no banner while Juno was frontmost. The delegate
    /// must ask for a banner, a Notification Center entry and the sound.
    #[cfg(target_os = "macos")]
    #[test]
    fn frontmost_notifications_present_as_banners() {
        use objc2_user_notifications::UNNotificationPresentationOptions as Options;
        let options = foreground_presentation();
        assert!(options.contains(Options::Banner));
        assert!(options.contains(Options::List));
        assert!(options.contains(Options::Sound));
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

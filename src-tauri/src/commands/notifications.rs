use crate::settings::manager::SettingsManager;
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

/// Where a click on a Juno notification goes.
///
/// macOS hands a click back the identifier of the notification it belongs to
/// and nothing else, so this enum is the whole vocabulary: one table, matched
/// in one place, rather than a callback per caller. It exists because a banner
/// that names a place to go and cannot take you there is a dead end, which
/// every notification Juno posted was until LAC-8588.
///
/// Not to be confused with macOS's `UNNotificationAction`, which is a button
/// drawn on the banner. This is what the banner itself does when clicked.
///
/// **Backend only, deliberately.** It carries no serde derives and the field
/// on [`NotificationData`] is skipped, so the webview cannot name a
/// destination through `send_notification`: where a click goes is policy the
/// backend owns, like every other decision in Juno. Keep it that way. Every
/// variant here is a fixed destination compiled into the app; a variant
/// carrying a path, a URL or a shell word would hand a chosen target to
/// whatever can reach that command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ClickAction {
    /// Put Juno's conversation in front of the person, and nothing else.
    ///
    /// The default, and the floor: a notification that names no destination,
    /// one posted by a build that knew an action this one does not, and an
    /// identifier macOS hands back that Juno did not write all land here. No
    /// click is ever answered with nothing.
    #[default]
    BringJunoForward,
    /// System Settings > Accessibility > Read & Speak, where the Mac's voice
    /// is chosen. The destination the "no voice to speak with" notice names in
    /// as many words.
    OpenReadAndSpeak,
    /// The conversation a background agent session belongs to.
    ShowSession { session_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationData {
    pub title: String,
    pub message: String,
    pub level: String, // "info", "success", "warning", "error"
    pub important: Option<bool>,
    pub timeout: Option<u32>, // Override default duration
    /// Where a click on this notification goes.
    ///
    /// Skipped on the wire, so a Rust caller sets it and a frontend payload
    /// cannot: it reads as [`ClickAction::BringJunoForward`] whatever the
    /// webview sends, including nothing, which is what it has always sent.
    #[serde(skip)]
    pub action: ClickAction,
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
pub async fn get_notification_settings(app: AppHandle) -> Result<NotificationSettings, String> {
    Ok(NotificationSettings {
        enabled: notifications_enabled(&app),
    })
}

/// Turn Juno's notifications on or off, and keep the answer.
///
/// The switch used to live only in memory, so it came back on at every launch
/// whatever the person had chosen. It is a setting like the rest now.
#[tauri::command]
pub async fn set_notifications_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let settings = app
        .try_state::<SettingsManager>()
        .ok_or_else(|| "Settings are not ready yet".to_string())?;
    settings.set_notifications_enabled(enabled).await
}

/// Is the switch on?
///
/// Read from the settings store, so it is right before any window has asked,
/// and in a process with no window at all. An unreadable store reads as on:
/// a missed notification is worse than an extra one.
fn notifications_enabled(app: &AppHandle) -> bool {
    SettingsManager::read_notifications_enabled(app)
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

/// What separates the unique part of a request identifier from the action it
/// carries.
///
/// A click hands the delegate the identifier and nothing else, so the
/// identifier is where the action has to live.
///
/// The split is unambiguous because the **unique part** cannot contain this
/// character: it is digits and dots by construction, which
/// `the_unique_part_of_an_identifier_never_contains_the_separator` asserts. The
/// slug after it may contain more of them and still parses, since the first
/// one ends the unique part and everything past it is the slug. Session ids
/// are UUIDs today, but that is convention rather than a type guarantee, so
/// the parse does not lean on it.
///
/// The unique part in front of the separator is left untouched, so two
/// notifications carrying the same action still replace nothing of each
/// other's in Notification Center.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const ACTION_SEPARATOR: char = '#';
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const READ_AND_SPEAK_SLUG: &str = "read-and-speak";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const SESSION_SLUG_PREFIX: &str = "session=";

impl ClickAction {
    /// What this action is written as on a request identifier, if anything.
    ///
    /// `None` for the default, so an identifier with no action appended reads
    /// as "bring Juno forward", which is what every notification posted before
    /// LAC-8588, and any still sitting in Notification Center from an older
    /// build, reads as too.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn slug(&self) -> Option<String> {
        match self {
            Self::BringJunoForward => None,
            Self::OpenReadAndSpeak => Some(READ_AND_SPEAK_SLUG.to_string()),
            Self::ShowSession { session_id } => {
                Some(format!("{}{}", SESSION_SLUG_PREFIX, session_id))
            }
        }
    }
}

/// The identifier a notification carrying `action` is posted under.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn identifier_with_action(unique: &str, action: &ClickAction) -> String {
    match action.slug() {
        Some(slug) => format!("{}{}{}", unique, ACTION_SEPARATOR, slug),
        None => unique.to_string(),
    }
}

/// What a click on the notification posted under `identifier` should do.
///
/// The one table, and it reads **only the slug** after the separator. The
/// unique part in front is macOS's dedup key and carries no routing
/// information, so it is deliberately not validated: tying routing to
/// `request_identifier`'s format would mean a later change to that format
/// silently stopped routing the notifications already sitting in Notification
/// Center. Nothing is risked by leaving it unchecked, because only Juno's own
/// bundle can have a response delivered to Juno's delegate, so every
/// identifier that reaches here was written by some build of Juno.
///
/// Total by construction: no separator, a separator with no slug, a slug this
/// build does not know, and an empty session id all read as
/// [`ClickAction::BringJunoForward`]. There is deliberately no path through
/// here that does nothing. A banner that named a destination and answered a
/// click with silence is the defect.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn action_from_identifier(identifier: &str) -> ClickAction {
    let Some((_unique, slug)) = identifier.split_once(ACTION_SEPARATOR) else {
        return ClickAction::BringJunoForward;
    };
    if slug == READ_AND_SPEAK_SLUG {
        return ClickAction::OpenReadAndSpeak;
    }
    match slug.strip_prefix(SESSION_SLUG_PREFIX) {
        Some(session_id) if !session_id.is_empty() => ClickAction::ShowSession {
            session_id: session_id.to_string(),
        },
        _ => ClickAction::BringJunoForward,
    }
}

/// Hand one notification to `UNUserNotificationCenter` and wait for its answer.
///
/// This is the same center the authorization comes from, so what the
/// notifications row shows and what posting does can no longer disagree.
#[cfg(target_os = "macos")]
fn post(title: &str, body: &str, action: &ClickAction) -> Result<(), String> {
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

    let identifier =
        NSString::from_str(&identifier_with_action(&next_request_identifier(), action));
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
fn post(title: &str, _body: &str, _action: &ClickAction) -> Result<(), String> {
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
        UNNotification, UNNotificationDefaultActionIdentifier, UNNotificationPresentationOptions,
        UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };
    use tauri::AppHandle;

    /// The app a click acts on.
    ///
    /// The delegate is an Objective-C object macOS owns and calls with a
    /// response and nothing else, so the handle it needs has to be waiting
    /// here for it. Set once, when the delegate is installed.
    static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

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

            // Somebody clicked a Juno banner. The only way one reaches Rust:
            // without this method macOS has nowhere to deliver a click, so
            // every banner Juno posted was a dead end (LAC-8588).
            //
            // macOS calls this for the person *opening* a notification and,
            // where a category asks for one, for dismissing it. Only the
            // default action is someone asking to go somewhere: a dismissal
            // is answered by closing the handler and nothing else. Juno
            // registers no category today, so only the default action
            // arrives, but sending a dismissal to System Settings is a real
            // bug the day one is registered.
            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive_response(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion_handler: &block2::DynBlock<dyn Fn()>,
            ) {
                // SAFETY: a framework string constant, present for as long as
                // UserNotifications is loaded, which it is inside this method.
                let opened = unsafe { UNNotificationDefaultActionIdentifier }.to_string();
                if response.actionIdentifier().to_string() == opened {
                    let identifier = response.notification().request().identifier().to_string();
                    match APP.get() {
                        Some(app) => super::handle_click(app, &identifier),
                        // Unreachable by construction, including for a click
                        // that launches Juno: `install` sets APP *before* it
                        // sets the delegate, and this object cannot be sent a
                        // message until it is the delegate. Logged rather than
                        // latched for replay, because a latch here would be
                        // code no click can reach.
                        None => log::warn!(
                            "Notification '{}' was clicked before Juno had a handle to act on",
                            identifier
                        ),
                    }
                }
                // macOS holds the next response until this is called, so it is
                // called on every path out, dismissal included.
                completion_handler.call(());
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
    pub fn install(app: &AppHandle) {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let _ = APP.set(app.clone());
            let presenter = Presenter::new();
            UNUserNotificationCenter::currentNotificationCenter()
                .setDelegate(Some(ProtocolObject::from_ref(&*presenter)));
            // The center holds its delegate weakly. Keep this one alive.
            std::mem::forget(presenter);
        });
    }
}

/// Let notifications show as banners while Juno is frontmost, and let a click
/// on one reach Rust.
///
/// Called once from setup. Does nothing where there is no Juno.app to post as,
/// because the notification center raises without a bundle. The handle is
/// what a click is carried out against, so there is no banner without it and
/// no click without the banner.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub fn install_presenter(app: &AppHandle) {
    if let Some(reason) = unavailable_reason() {
        info!("Notification presenter not installed: {}", reason);
        return;
    }
    #[cfg(target_os = "macos")]
    presenter::install(app);
}

/// Carry out a click on a Juno notification.
///
/// Reads the action off the identifier and hands it to [`run_action`] on the
/// async runtime: this is called from an Objective-C delegate method on the
/// main thread, and both opening System Settings and putting a window in
/// front of someone can wait on AppKit.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn handle_click(app: &AppHandle, identifier: &str) {
    let action = action_from_identifier(identifier);
    info!("Notification '{}' was clicked: {:?}", identifier, action);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_action(&app, action).await;
    });
}

/// Do what the click asked for.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
async fn run_action(app: &AppHandle, action: ClickAction) {
    match action {
        ClickAction::OpenReadAndSpeak => {
            // The same two URLs the Voice picker's own link opens, so the
            // banner and the picker cannot disagree about where the Mac's
            // voice is set. Juno is deliberately not brought forward: the
            // person is being sent to System Settings, and fighting it for
            // the foreground would land them back where they started.
            if let Err(reason) = crate::tts::voices::open_read_and_speak().await {
                warn!(
                    "A notification click could not open Read & Speak: {}",
                    reason
                );
            }
        }
        ClickAction::ShowSession { session_id } => {
            // Best effort, and in this order. A session that has already
            // finished is gone from the registry, so focusing it fails for the
            // most ordinary click there is: the one on "finished". The
            // conversation is still what the person asked for, so the window
            // comes forward either way.
            //
            // Focusing *before* showing the window is safe, and that is a fact
            // about the registry rather than a hope: `set_focused` has exactly
            // one caller in the app, `focus_agent_session` just below, and
            // nothing on `open_main_window`'s path (`show_bar`,
            // `create_or_show_window`, `announce_main_window`) writes focus.
            // The frontend only focuses from a click on a session row, never on
            // mount, so putting the window on screen cannot clear the selection
            // made here. If a second focus writer ever appears, this order is
            // what breaks: bring the window forward first, then focus.
            match app.try_state::<AppState>() {
                Some(state) => {
                    if let Err(reason) = crate::commands::agent_sessions::focus_agent_session(
                        app.clone(),
                        state,
                        Some(session_id.clone()),
                    )
                    .await
                    {
                        info!(
                            "Session {} was not there to focus, showing the conversation anyway: {}",
                            session_id, reason
                        );
                    }
                }
                // A click that arrives before setup finished. The window is
                // still the right answer, and this never panics for it.
                None => info!("Juno is not ready to focus session {} yet", session_id),
            }
            bring_juno_forward(app).await;
        }
        ClickAction::BringJunoForward => bring_juno_forward(app).await,
    }
}

/// Put Juno's conversation in front of the person.
///
/// Through `open_main_window`, which is the one place that decides which
/// surface holds the full chat (the window, or the Island when it owns it), so
/// a click never reaches for a window label of its own.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
async fn bring_juno_forward(app: &AppHandle) {
    if let Err(reason) = crate::window_management::open_main_window(app.clone()).await {
        warn!(
            "A notification click could not bring Juno forward: {}",
            reason
        );
    }
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
    deliver_action(app, title, body, &ClickAction::BringJunoForward)
}

/// [`deliver`], for a notification that has somewhere for a click to go.
pub fn deliver_action(
    app: &AppHandle,
    title: &str,
    body: &str,
    action: &ClickAction,
) -> Result<(), String> {
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
    post(title, body, action)
}

/// Notify from anywhere, without waiting and without telling anyone if it
/// fails.
///
/// The scheduler, a new automation, a finished timer, the request for the
/// physical mouse and the menu-bar-only hint all land here. Some run on the
/// main thread, and the first notification can wait on the permission prompt,
/// so the work happens on its own thread. A failure is logged and nothing more.
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    notify_action(app, title, body, ClickAction::BringJunoForward);
}

/// [`notify`], for a notification that names a place to go.
pub fn notify_action(app: &AppHandle, title: &str, body: &str, action: ClickAction) {
    let app = app.clone();
    let title = title.to_string();
    let body = body.to_string();
    let spawned = std::thread::Builder::new()
        .name("juno-notify".into())
        .spawn(move || {
            if let Err(reason) = deliver_action(&app, &title, &body, &action) {
                warn!("Notification '{}' did not go out: {}", title, reason);
            }
        });
    if let Err(e) = spawned {
        warn!("Could not start the notification thread: {}", e);
    }
}

/// [`deliver`] on a blocking thread, logging rather than returning a failure.
async fn deliver_quietly(app: AppHandle, title: String, body: String, action: ClickAction) {
    let result = tauri::async_runtime::spawn_blocking(move || {
        deliver_action(&app, &title, &body, &action).map_err(|reason| (title, reason))
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
    deliver_quietly(app, data.title, data.message, data.action).await;
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
        ClickAction::BringJunoForward,
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

    /// The routing table. A click hands Juno back the identifier and nothing
    /// else, so this mapping is the whole decision.
    #[test]
    fn an_identifier_says_where_its_click_goes() {
        assert_eq!(
            action_from_identifier("juno.42.7#read-and-speak"),
            ClickAction::OpenReadAndSpeak
        );
        assert_eq!(
            action_from_identifier("juno.42.7#session=9d3f7c1e-4aaa-bbbb-1234567890ab"),
            ClickAction::ShowSession {
                session_id: "9d3f7c1e-4aaa-bbbb-1234567890ab".to_string()
            }
        );
        // Nothing appended: bring Juno forward. Every notification Juno posted
        // before LAC-8588 looks exactly like this, including any still sitting
        // in Notification Center from an older build.
        assert_eq!(
            action_from_identifier(&request_identifier(7, 42)),
            ClickAction::BringJunoForward
        );
    }

    /// An action this build does not know must not crash and must not answer
    /// with silence: it brings Juno forward like any other click.
    #[test]
    fn an_unknown_action_brings_juno_forward() {
        for identifier in [
            "juno.42.7#open-the-pod-bay-doors", // a build that knew more
            "juno.42.7#",                       // a separator and no slug
            "juno.42.7#session=",               // an empty session id
            "juno.42.7#session",                // the prefix without its =
            "",                                 // not an identifier
            "ABCD-EFGH",                        // no separator, so no action
        ] {
            assert_eq!(
                action_from_identifier(identifier),
                ClickAction::BringJunoForward,
                "'{}' should bring Juno forward",
                identifier
            );
        }
    }

    /// The slug is the whole decision. The unique part in front of the
    /// separator is macOS's dedup key, carries no routing information, and is
    /// deliberately not validated, so the same slug routes the same way behind
    /// any prefix or none.
    ///
    /// Pinned because the first version of this test assumed the opposite and
    /// listed `#read-and-speak` as an unknown action. CI disagreed, and it was
    /// right: the doc comment had promised that an identifier Juno never wrote
    /// brings Juno forward, which the code did not do and should not do.
    #[test]
    fn routing_reads_the_slug_and_never_the_unique_part() {
        for identifier in [
            "juno.1763480000123.4#read-and-speak", // what Juno actually posts
            "#read-and-speak",                     // no unique part at all
            "not-an-identifier#read-and-speak",    // a prefix Juno never writes
        ] {
            assert_eq!(
                action_from_identifier(identifier),
                ClickAction::OpenReadAndSpeak,
                "'{}' names the Read & Speak action whatever sits in front of it",
                identifier
            );
        }
    }

    /// Posting and clicking are two halves of one encoding, so they are tested
    /// as a round trip: however an action is written, reading it back gives
    /// the same action.
    #[test]
    fn every_action_survives_the_round_trip() {
        for action in [
            ClickAction::BringJunoForward,
            ClickAction::OpenReadAndSpeak,
            ClickAction::ShowSession {
                session_id: "9d3f7c1e-0000-4aaa-bbbb-1234567890ab".to_string(),
            },
            // Session ids are UUIDs today, but `AgentSessionId` is a
            // `From<String>` newtype, so one carrying the separator has to
            // survive too. It does: the first separator ends the unique part
            // and everything after it is the slug, separators included.
            ClickAction::ShowSession {
                session_id: "not#a#uuid".to_string(),
            },
        ] {
            let identifier = identifier_with_action("juno.1763000000000.3", &action);
            assert_eq!(
                action_from_identifier(&identifier),
                action,
                "{:?} did not survive '{}'",
                action,
                identifier
            );
        }
    }

    /// The action rides on the identifier, so it must not cost the identifier
    /// its uniqueness: macOS replaces a delivered notification that shares
    /// one, and two sessions finishing together have to both be seen.
    #[test]
    fn an_action_does_not_make_two_notifications_collide() {
        let action = ClickAction::ShowSession {
            session_id: "same-session".to_string(),
        };
        let first = identifier_with_action(&request_identifier(0, 5), &action);
        let second = identifier_with_action(&request_identifier(1, 5), &action);
        assert_ne!(first, second);
        // The unique part is still the prefix, untouched.
        assert!(first.starts_with("juno.5.0"), "got {}", first);
    }

    /// The separator earns its keep by being impossible in the unique part of
    /// an identifier. That is the whole reason the split is unambiguous, so it
    /// is asserted rather than left to the comment that claims it.
    #[test]
    fn the_unique_part_of_an_identifier_never_contains_the_separator() {
        assert!(!next_request_identifier().contains(ACTION_SEPARATOR));
        assert!(!request_identifier(u64::MAX, u128::MAX).contains(ACTION_SEPARATOR));
    }

    /// Where a click goes is backend policy. `send_notification` takes its
    /// payload from the webview, so an action named there must not be honoured
    /// and must not be an error either: it reads as the default, exactly like
    /// the payload the frontend has always sent.
    #[test]
    fn a_payload_cannot_choose_where_a_click_goes() {
        let plain: NotificationData =
            serde_json::from_str(r#"{"title":"t","message":"m","level":"info"}"#)
                .expect("a payload with no action is still a notification");
        assert_eq!(plain.action, ClickAction::BringJunoForward);

        let asking: NotificationData = serde_json::from_str(
            r#"{"title":"t","message":"m","level":"info","action":{"kind":"open_read_and_speak"}}"#,
        )
        .expect("an action in the payload is ignored, not a parse error");
        assert_eq!(asking.action, ClickAction::BringJunoForward);
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

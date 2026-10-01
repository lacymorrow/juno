//! Asking for a macOS permission at the moment it is needed, and never before.
//!
//! Juno used to demand Accessibility and Screen Recording during onboarding,
//! before the person had seen it do anything. That is the wrong trade to put in
//! front of someone: two of the most alarming switches macOS has, in exchange
//! for a promise. So onboarding no longer blocks on them, and the ask moved
//! here, to the first moment Juno actually reaches for one.
//!
//! The rules the copy follows: lead with what it unlocks rather than what Juno
//! needs, say it is one switch, say it can be turned off again, and make "not
//! now" a real answer that costs nothing. A person who declines keeps a working
//! app, and the agent is told to carry on with whatever it can still do.
//!
//! Asking is rate limited per capability, because a tool loop can reach this in
//! a tight cycle and nothing turns a calm request into nagging faster than
//! showing it four times in a row.
//!
//! ## Automation is here for a different reason
//!
//! The other three are switches in System Settings that Juno can only point
//! at. Automation is a dialog macOS raises *itself*, the first time Juno sends
//! an AppleScript event to an app, once per target app. On a first run that is
//! several system dialogs in a row, each with no Juno around it and each
//! interrupting something the person asked for. So Automation is gated the
//! same way as the rest: Juno finds out that macOS would ask, says why in its
//! own words, and lets the system ask once on a button the person pressed.
//!
//! ## What a restart is for, and what it is not
//!
//! Some grants cannot reach a running process. That is macOS, not Juno, and no
//! amount of code gets around it. What Juno owes the person is to notice, say
//! so plainly, and offer the one button that fixes it. What it must not do is
//! ask for a restart that is not needed, which is why
//! [`restart_is_the_missing_step`] is decided per capability and is a pure
//! function with a test per case.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::native_permissions::NativePermissionChecker;
use crate::constants::events;
use crate::platform::automation::{self, AutomationConsent, AutomationTarget};

/// How long to stay quiet about a capability after asking about it once.
const ASK_AGAIN_AFTER: Duration = Duration::from_secs(300);

/// The permissions Juno reaches for mid-task.
///
/// Microphone earns its place here even though the person starts dictation
/// deliberately. The theory was that a feature you invoke on purpose can ask
/// for its own permission, but the mic path did not ask: a denial returned a
/// string that reached a log file and nothing else, so pressing the mic button
/// did nothing at all, with no explanation anywhere on screen.
///
/// Input Monitoring is still absent: it only affects whether a global shortcut
/// reaches Juno, which fails in a way the person can see and work around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Accessibility,
    ScreenRecording,
    Microphone,
    /// Permission to drive one other app with AppleScript. Held per target
    /// app, because that is how macOS holds it.
    Automation(AutomationTarget),
}

impl Capability {
    /// Matches the key used by the permissions state and the settings deep link.
    ///
    /// Automation has no row of its own in Juno's permissions state, so its
    /// keys are namespaced by target. They are what the card hands back when
    /// it asks where the person now stands.
    pub fn key(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::ScreenRecording => "screen_recording",
            Self::Microphone => "microphone",
            Self::Automation(AutomationTarget::SystemEvents) => "automation_system_events",
            Self::Automation(AutomationTarget::Safari) => "automation_safari",
        }
    }

    /// Resolve a key back into the capability it names.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "accessibility" => Some(Self::Accessibility),
            "screen_recording" => Some(Self::ScreenRecording),
            "microphone" => Some(Self::Microphone),
            "automation_system_events" => Some(Self::Automation(AutomationTarget::SystemEvents)),
            "automation_safari" => Some(Self::Automation(AutomationTarget::Safari)),
            _ => None,
        }
    }

    /// Leads with what the person gets, not with what Juno wants.
    pub fn title(self) -> String {
        match self {
            Self::Accessibility => "Juno can do that once Accessibility is on".to_string(),
            Self::ScreenRecording => {
                "Juno can see your screen once Screen Recording is on".to_string()
            }
            Self::Microphone => "Juno can listen once Microphone is on".to_string(),
            Self::Automation(target) => {
                format!("Juno can {} once you allow it", target.what_it_unlocks())
            }
        }
    }

    /// One sentence on what it buys, one on how small and reversible it is.
    pub fn detail(self) -> String {
        match self {
            Self::Accessibility => "Accessibility lets Juno click and type for you. It is one \
                 switch in System Settings, and you can turn it back off whenever you like."
                .to_string(),
            Self::ScreenRecording => "Screen Recording is how Juno sees what is in front of \
                 you. It is one switch in System Settings, and you can turn it back off \
                 whenever you like."
                .to_string(),
            Self::Microphone => "Microphone is how Juno hears you, so you can talk instead of \
                 typing. It is one switch in System Settings, and you can turn it back off \
                 whenever you like."
                .to_string(),
            // The system dialog will say "System Events", a true name that
            // explains nothing, so Juno says what it is before macOS says what
            // it is called.
            Self::Automation(AutomationTarget::SystemEvents) => {
                "macOS will ask whether Juno may control System Events, the part of your Mac \
                 that presses keys and opens menus. It asks once, and you can turn it back \
                 off in System Settings whenever you like."
                    .to_string()
            }
            Self::Automation(AutomationTarget::Safari) => {
                "macOS will ask whether Juno may control Safari, so Juno can read the page in \
                 front of you. It asks once, and you can turn it back off in System Settings \
                 whenever you like."
                    .to_string()
            }
        }
    }

    /// What the card's one button does.
    pub fn primary_action(self) -> PrimaryAction {
        match self {
            Self::Accessibility | Self::ScreenRecording | Self::Microphone => {
                PrimaryAction::OpenSettings
            }
            // Nothing in System Settings to open: until macOS has asked once,
            // Juno has no row in the Automation pane for anyone to switch on.
            Self::Automation(_) => PrimaryAction::AllowAutomation,
        }
    }

    /// The name macOS uses, so Juno and the system say the same word.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Accessibility => "Accessibility",
            Self::ScreenRecording => "Screen Recording",
            Self::Microphone => "Microphone",
            Self::Automation(AutomationTarget::SystemEvents) => "Automation (System Events)",
            Self::Automation(AutomationTarget::Safari) => "Automation (Safari)",
        }
    }

    /// What the agent is told, so it keeps working instead of reading this as a
    /// crash. It says the user has already been asked, so the model does not
    /// start improvising its own permission instructions on top of ours.
    pub fn agent_message(self, action: &str) -> String {
        format!(
            "'{}' needs {} and it has not been turned on yet. Juno has asked the user on \
             screen, so do not repeat the request or explain how to grant it. Carry on with \
             anything you can still do without it, and say you will finish this step once \
             it is on.",
            action,
            self.display_name()
        )
    }
}

/// The one thing the card's button does, decided here rather than in React.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryAction {
    /// Open the exact Privacy pane for this permission.
    OpenSettings,
    /// Let macOS raise its Automation dialog, now that Juno has explained it.
    AllowAutomation,
}

impl PrimaryAction {
    /// The words on the button.
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenSettings => "Open Settings",
            Self::AllowAutomation => "Allow",
        }
    }

    /// The key the payload carries.
    pub fn key(self) -> &'static str {
        match self {
            Self::OpenSettings => "open_settings",
            Self::AllowAutomation => "allow_automation",
        }
    }
}

/// What macOS last told *this process* about a capability.
///
/// `Refused` and `NeverAsked` are separate because only the first one sticks.
/// macOS answers a process from a decision it already gave, and only a
/// decision it already gave can be cached, so a process that has never asked
/// sees the first grant straight away. Collapsing the two is how everyone on a
/// fresh machine gets told to restart for nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessAnswer {
    /// Juno can act now.
    Granted,
    /// macOS refused this process.
    Refused,
    /// This process has never asked, so nothing is cached.
    NeverAsked,
    /// The check did not complete. Nothing is known either way.
    Unknown,
}

/// Whether starting Juno again is the step the person is missing.
///
/// Pure on purpose. This is the decision that decides whether Juno tells
/// someone to restart, and telling someone to restart when they do not have to
/// is its own small insult, so it is settled in one place with a test per case
/// rather than inferred at four call sites.
///
/// Per capability, and they genuinely differ:
///
/// * **Accessibility** is read live. `AXIsProcessTrustedWithOptions` asks TCC
///   on every call, which is the only reason the post-Accessibility auto-grant
///   can drive System Settings inside this same process. A restart is never
///   owed for it.
/// * **Screen Recording** is the opposite. The WindowServer decides what a
///   client may capture when that client connects, which is why macOS raises
///   its own "quit and reopen" sheet. A grant cannot reach a running process,
///   and no probe changes that.
/// * **Microphone** sits between them. An undetermined answer is asked fresh
///   and lands immediately, so no restart. An explicit refusal is cached for
///   the life of the process, so flipping the switch cannot reach Juno.
/// * **Automation** is checked per event by tccd, so switching Juno on in the
///   Automation pane takes effect without a restart.
pub fn restart_is_the_missing_step(capability: Capability, answer: ProcessAnswer) -> bool {
    if answer != ProcessAnswer::Refused {
        return false;
    }
    match capability {
        Capability::Accessibility | Capability::Automation(_) => false,
        Capability::ScreenRecording | Capability::Microphone => true,
    }
}

/// The sentence that goes with the restart offer, or empty when none is owed.
///
/// Says what macOS is doing and what the restart costs, and nothing else. It
/// does not claim the switch is already on: Juno cannot see that, and claiming
/// it would be a guess dressed as a fact.
pub fn restart_detail(capability: Capability) -> &'static str {
    match capability {
        Capability::ScreenRecording => {
            "macOS gives an app the screen only when the app starts, so the switch cannot \
             reach Juno while it is running. Starting again takes a few seconds. This \
             conversation is saved and waiting under History."
        }
        Capability::Microphone => {
            "macOS already answered Juno about the microphone, and it keeps that answer until \
             Juno starts again. Starting again takes a few seconds. This conversation is \
             saved and waiting under History."
        }
        Capability::Accessibility | Capability::Automation(_) => "",
    }
}

/// Read what macOS says about this process right now, uncached.
///
/// Deliberately not the short-TTL cache behind `check_permissions_status_native`.
/// That cache is right for a list of rows and wrong here: a stale answer is the
/// exact thing this call exists to catch.
pub fn process_answer(capability: Capability) -> ProcessAnswer {
    match capability {
        // A plain bool, so it cannot tell a switched-off row from one that was
        // never created. That costs nothing: Accessibility never owes a
        // restart either way.
        Capability::Accessibility => {
            match NativePermissionChecker::check_accessibility_permission() {
                Ok(true) => ProcessAnswer::Granted,
                Ok(false) => ProcessAnswer::Refused,
                Err(_) => ProcessAnswer::Unknown,
            }
        }
        // Also a plain bool, and here the two cases really are the same: the
        // WindowServer's answer is fixed for the life of the process, so any
        // non-grant is one this process cannot talk its way out of.
        Capability::ScreenRecording => {
            match NativePermissionChecker::check_screen_recording_permission() {
                Ok(true) => ProcessAnswer::Granted,
                Ok(false) => ProcessAnswer::Refused,
                Err(_) => ProcessAnswer::Unknown,
            }
        }
        // `AVCaptureDevice.authorizationStatus` reports all three states, and
        // the difference is the whole point here.
        Capability::Microphone => {
            if NativePermissionChecker::check_microphone_permission().unwrap_or(false) {
                ProcessAnswer::Granted
            } else if NativePermissionChecker::microphone_explicitly_denied() {
                ProcessAnswer::Refused
            } else {
                ProcessAnswer::NeverAsked
            }
        }
        Capability::Automation(target) => match automation::consent(target) {
            AutomationConsent::Allowed => ProcessAnswer::Granted,
            AutomationConsent::Refused => ProcessAnswer::Refused,
            AutomationConsent::WouldAsk => ProcessAnswer::NeverAsked,
            AutomationConsent::TargetNotRunning | AutomationConsent::Unknown => {
                ProcessAnswer::Unknown
            }
        },
    }
}

/// Where the person stands with one capability, right now.
///
/// One answer for the whole card: whether Juno can act, and whether a restart
/// is the missing step. The card used to read this out of the permissions
/// state blob, which had a row for Accessibility and Screen Recording and
/// nothing for Microphone, so a microphone ask could never turn into its own
/// receipt. One command per capability has no such hole.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionMoment {
    /// The key the caller asked about, echoed back.
    pub permission: String,
    /// Juno can act. The card becomes its own receipt.
    pub granted: bool,
    /// macOS is holding an answer this process cannot change, and starting
    /// Juno again is what fixes it.
    pub restart_unblocks: bool,
    /// The sentence to show with the restart offer. Empty when none is owed.
    pub restart_detail: String,
}

/// Build the moment for one capability key.
pub fn moment_for(key: &str) -> Result<PermissionMoment, String> {
    let capability =
        Capability::from_key(key).ok_or_else(|| format!("Juno does not ask for '{}'", key))?;
    let answer = process_answer(capability);
    let restart_unblocks = restart_is_the_missing_step(capability, answer);
    Ok(PermissionMoment {
        permission: capability.key().to_string(),
        granted: answer == ProcessAnswer::Granted,
        restart_unblocks,
        restart_detail: if restart_unblocks {
            restart_detail(capability).to_string()
        } else {
            String::new()
        },
    })
}

/// When each capability was last asked about, so a tool loop cannot nag.
static LAST_ASKED: LazyLock<Mutex<HashMap<Capability, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// True when enough time has passed to ask about this capability again. Records
/// the ask as a side effect, so callers cannot forget to.
fn may_ask(capability: Capability, now: Instant) -> bool {
    let Ok(mut asked) = LAST_ASKED.lock() else {
        // A poisoned lock must not cost the person the prompt they need.
        return true;
    };
    let recent = asked
        .get(&capability)
        .is_some_and(|at| now.duration_since(*at) < ASK_AGAIN_AFTER);
    if recent {
        return false;
    }
    asked.insert(capability, now);
    true
}

/// Forget what has been asked. Called when the person answers a prompt, so the
/// next genuine need asks again rather than sitting silent for five minutes.
pub fn clear_ask_history() {
    if let Ok(mut asked) = LAST_ASKED.lock() {
        asked.clear();
    }
}

/// Ask on screen for `capability`, unless we just did.
///
/// Public because the older `validate_permission` path reaches the same dead
/// end from its own checks, and an ask the person can act on beats a sentence
/// of instructions buried in a tool error.
pub fn ask(app_handle: &AppHandle, capability: Capability, action: &str) {
    ask_inner(app_handle, capability, action, false)
}

/// Ask because a person just pressed something, and say so every time.
///
/// The rate limit exists to stop a tool loop nagging. Someone pressing the mic
/// button again is not a loop, it is a person asking again because nothing
/// happened the first time, and answering that with silence is the bug this
/// whole path exists to fix.
pub fn ask_now(app_handle: &AppHandle, capability: Capability, action: &str) {
    ask_inner(app_handle, capability, action, true)
}

fn ask_inner(app_handle: &AppHandle, capability: Capability, action: &str, user_asked: bool) {
    if !user_asked && !may_ask(capability, Instant::now()) {
        tracing::debug!(
            "Not asking again for {} so soon (wanted it for '{}')",
            capability.key(),
            action
        );
        return;
    }

    let primary = capability.primary_action();
    let payload = serde_json::json!({
        "permission": capability.key(),
        "action": action,
        "title": capability.title(),
        "detail": capability.detail(),
        "primary_action": primary.key(),
        "primary_label": primary.label(),
    });

    if let Err(e) = app_handle.emit(events::permissions::NEEDED, payload) {
        tracing::error!("Failed to emit permission request: {}", e);
    }
}

/// Gate an action on a permission being granted.
///
/// Returns `Ok(())` when Juno may proceed. Otherwise it puts the ask on screen
/// and hands back a message the agent can act on, so the failure reads as "not
/// yet" rather than as an error.
pub async fn require(
    app_handle: &AppHandle,
    capability: Capability,
    action: &str,
) -> Result<(), String> {
    require_inner(app_handle, capability, action, false).await
}

/// The same gate for something a person just pressed, which always answers.
pub async fn require_for_user(
    app_handle: &AppHandle,
    capability: Capability,
    action: &str,
) -> Result<(), String> {
    require_inner(app_handle, capability, action, true).await
}

async fn require_inner(
    app_handle: &AppHandle,
    capability: Capability,
    action: &str,
    user_asked: bool,
) -> Result<(), String> {
    if is_granted(app_handle, capability).await {
        return Ok(());
    }
    ask_inner(app_handle, capability, action, user_asked);
    Err(capability.agent_message(action))
}

/// Gate an AppleScript call on Juno being allowed to drive that app.
///
/// Call this before sending any AppleScript event, in place of letting macOS
/// raise its own dialog mid-task. One line per call site:
///
/// ```ignore
/// crate::permission_gate::require_automation(
///     app_handle,
///     AutomationTarget::SystemEvents,
///     "turn on dark mode",
/// ).await?;
/// ```
///
/// The one answer that proceeds without a grant is `Unknown`: when the check
/// itself cannot reach a conclusion, Juno is no worse off letting the action
/// try than it was before this gate existed, and a false "not allowed" would
/// block someone who is fine.
pub async fn require_automation(
    app_handle: &AppHandle,
    target: AutomationTarget,
    action: &str,
) -> Result<(), String> {
    let capability = Capability::Automation(target);
    match automation::consent(target) {
        AutomationConsent::Allowed => Ok(()),
        AutomationConsent::Unknown => {
            tracing::warn!(
                "Could not read Automation consent for {}; letting '{}' try",
                target.display_name(),
                action
            );
            Ok(())
        }
        // Refused, undecided, or the target is not up yet. All three are
        // Juno's moment to explain rather than the system's moment to
        // interrupt.
        _ => {
            ask(app_handle, capability, action);
            Err(capability.agent_message(action))
        }
    }
}

/// Let macOS raise its Automation dialog, because the person pressed Allow.
///
/// Returns whether Juno ended up allowed. Blocking, because the call does not
/// return until the dialog is answered, so it runs on a blocking worker.
///
/// AppleEvents cannot decide anything about an app that is not running, and
/// System Events is launched on demand, so a faceless target is started first.
/// Nothing is brought to the front and nothing is shown.
pub async fn allow_automation(app_handle: &AppHandle, target: AutomationTarget) -> bool {
    // The floating bar sits above ordinary windows and the system's own
    // prompts do not, which is how a permission dialog ended up behind Juno
    // once already. Same treatment here.
    crate::commands::permissions::step_aside_for_prompt(app_handle);

    if automation::consent(target) == AutomationConsent::TargetNotRunning
        && target.launches_on_demand()
    {
        start_faceless_target(target).await;
    }

    let allowed = tokio::task::spawn_blocking(move || automation::request_consent(target))
        .await
        .map(|consent| consent.is_allowed())
        .unwrap_or(false);

    if allowed {
        // The answer is in, so the bar can come back without waiting out the
        // grace timer.
        crate::commands::permissions::restore_bar_after_prompt(app_handle);
        // The person just answered a prompt; the next genuine need should be
        // free to ask again rather than sitting out the rate limit.
        clear_ask_history();
    }
    allowed
}

/// Start a background-only target so AppleEvents has something to decide about.
///
/// `-g` keeps Juno in front and `-j` starts it hidden, so nothing appears on
/// screen and focus does not move.
async fn start_faceless_target(target: AutomationTarget) {
    let bundle_id = target.bundle_id();
    let launched = tokio::task::spawn_blocking(move || {
        std::process::Command::new("open")
            .args(["-g", "-j", "-b", bundle_id])
            .status()
    })
    .await;
    match launched {
        Ok(Ok(status)) if status.success() => {
            // Give launchd a moment to register it before asking tccd.
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        other => {
            tracing::warn!("Could not start {} first: {:?}", bundle_id, other);
        }
    }
}

/// Current grant state, read through the cached permissions check so a tool loop
/// does not hammer the native APIs.
async fn is_granted(app_handle: &AppHandle, capability: Capability) -> bool {
    if let Capability::Automation(target) = capability {
        // Automation has no row in the permissions state, and its own check is
        // a single cheap call.
        return automation::consent(target).is_allowed();
    }
    match crate::commands::permissions::check_permissions_status_native(app_handle.clone()).await {
        Ok(state) => match capability {
            Capability::Accessibility => state.accessibility.granted,
            Capability::ScreenRecording => state.screen_recording.granted,
            Capability::Microphone => state.microphone.granted,
            Capability::Automation(_) => true,
        },
        Err(e) => {
            // If the check itself fails, let the action try. The OS is the real
            // gate, and a false "denied" would block a person who is fine.
            tracing::warn!("Could not read permission state ({}); letting it try", e);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every capability, so a new one cannot quietly skip the copy tests.
    const ALL: [Capability; 5] = [
        Capability::Accessibility,
        Capability::ScreenRecording,
        Capability::Microphone,
        Capability::Automation(AutomationTarget::SystemEvents),
        Capability::Automation(AutomationTarget::Safari),
    ];

    const EVERY_ANSWER: [ProcessAnswer; 4] = [
        ProcessAnswer::Granted,
        ProcessAnswer::Refused,
        ProcessAnswer::NeverAsked,
        ProcessAnswer::Unknown,
    ];

    #[test]
    fn a_person_pressing_again_is_always_answered() {
        // The rate limit is for tool loops, not for someone pressing the mic a
        // second time because the first press appeared to do nothing.
        clear_ask_history();
        let now = Instant::now();
        assert!(may_ask(Capability::Microphone, now));
        assert!(!may_ask(Capability::Microphone, now));
        // ask_now skips the check entirely, which is what the bar calls.
    }

    #[test]
    fn the_same_ask_does_not_repeat_immediately() {
        clear_ask_history();
        let now = Instant::now();
        assert!(may_ask(Capability::Accessibility, now));
        assert!(!may_ask(Capability::Accessibility, now));
    }

    #[test]
    fn a_different_capability_is_its_own_question() {
        clear_ask_history();
        let now = Instant::now();
        assert!(may_ask(Capability::Accessibility, now));
        assert!(may_ask(Capability::ScreenRecording, now));
    }

    #[test]
    fn each_automation_target_is_its_own_question() {
        // macOS holds Automation per target app, so Juno asking about System
        // Events must not silence the ask about Safari.
        clear_ask_history();
        let now = Instant::now();
        assert!(may_ask(
            Capability::Automation(AutomationTarget::SystemEvents),
            now
        ));
        assert!(may_ask(
            Capability::Automation(AutomationTarget::Safari),
            now
        ));
        assert!(!may_ask(
            Capability::Automation(AutomationTarget::SystemEvents),
            now
        ));
    }

    #[test]
    fn the_ask_returns_once_enough_time_has_passed() {
        clear_ask_history();
        let now = Instant::now();
        assert!(may_ask(Capability::Accessibility, now));
        let later = now + ASK_AGAIN_AFTER + Duration::from_secs(1);
        assert!(may_ask(Capability::Accessibility, later));
    }

    #[test]
    fn answering_a_prompt_lets_the_next_need_ask_again() {
        clear_ask_history();
        let now = Instant::now();
        assert!(may_ask(Capability::ScreenRecording, now));
        clear_ask_history();
        assert!(may_ask(Capability::ScreenRecording, now));
    }

    #[test]
    fn the_agent_is_told_to_carry_on_rather_than_to_nag() {
        let message = Capability::Accessibility.agent_message("click");
        assert!(message.contains("Accessibility"));
        assert!(message.contains("do not repeat the request"));
        assert!(message.contains("Carry on"));
    }

    #[test]
    fn the_copy_offers_something_rather_than_demanding_it() {
        for capability in ALL {
            // "Juno can ..." not "Juno needs ...": the title is an offer.
            assert!(
                capability.title().starts_with("Juno can "),
                "{:?} does not lead with an offer",
                capability
            );
            // Every ask promises it is reversible, because that is what makes
            // saying yes cheap.
            assert!(
                capability.detail().contains("turn it back off"),
                "{:?} does not say it is reversible",
                capability
            );
        }
    }

    #[test]
    fn the_three_switches_say_they_are_one_switch() {
        for capability in [
            Capability::Accessibility,
            Capability::ScreenRecording,
            Capability::Microphone,
        ] {
            assert!(capability.detail().contains("one switch"));
        }
    }

    #[test]
    fn automation_says_macos_asks_once_rather_than_promising_a_switch() {
        // There is no switch to promise: until macOS has asked, Juno has no
        // row in the Automation pane at all.
        for target in [AutomationTarget::SystemEvents, AutomationTarget::Safari] {
            let detail = Capability::Automation(target).detail();
            assert!(detail.contains("asks once"), "{}", detail);
            assert!(!detail.contains("one switch"), "{}", detail);
        }
    }

    #[test]
    fn no_copy_hands_the_person_a_path_a_bundle_id_or_a_script() {
        // Lacy's bar: some people do not know what a filepath is. Nothing a
        // person reads may contain one, nor a bundle id, nor AppleScript.
        for capability in ALL {
            for text in [capability.title(), capability.detail()] {
                for forbidden in [
                    "com.apple",
                    "/Users",
                    "/Applications",
                    "tell application",
                    "~/",
                ] {
                    assert!(
                        !text.contains(forbidden),
                        "{:?} copy leaks '{}': {}",
                        capability,
                        forbidden,
                        text
                    );
                }
                assert!(!text.contains('—'), "em dash in {:?} copy", capability);
            }
        }
    }

    #[test]
    fn every_key_round_trips() {
        for capability in ALL {
            assert_eq!(Capability::from_key(capability.key()), Some(capability));
        }
        assert_eq!(Capability::from_key("input_monitoring"), None);
    }

    // ── The restart matrix ───────────────────────────────────────────────────

    #[test]
    fn accessibility_never_needs_a_restart() {
        // It is read live on every call, which is the only reason the
        // post-Accessibility auto-grant works at all inside this process.
        for answer in EVERY_ANSWER {
            assert!(
                !restart_is_the_missing_step(Capability::Accessibility, answer),
                "claimed a restart for Accessibility on {:?}",
                answer
            );
        }
    }

    #[test]
    fn automation_never_needs_a_restart() {
        // tccd is asked per event, so flipping Juno on in the Automation pane
        // reaches the running process.
        for target in [AutomationTarget::SystemEvents, AutomationTarget::Safari] {
            for answer in EVERY_ANSWER {
                assert!(
                    !restart_is_the_missing_step(Capability::Automation(target), answer),
                    "claimed a restart for Automation on {:?}",
                    answer
                );
            }
        }
    }

    #[test]
    fn screen_recording_needs_a_restart_only_once_refused() {
        assert!(restart_is_the_missing_step(
            Capability::ScreenRecording,
            ProcessAnswer::Refused
        ));
        for answer in [
            ProcessAnswer::Granted,
            ProcessAnswer::NeverAsked,
            ProcessAnswer::Unknown,
        ] {
            assert!(
                !restart_is_the_missing_step(Capability::ScreenRecording, answer),
                "claimed a restart for Screen Recording on {:?}",
                answer
            );
        }
    }

    #[test]
    fn microphone_needs_a_restart_only_once_refused() {
        // The first ask on a fresh machine lands immediately. Demanding a
        // relaunch there would be a restart nobody owed.
        assert!(restart_is_the_missing_step(
            Capability::Microphone,
            ProcessAnswer::Refused
        ));
        for answer in [
            ProcessAnswer::Granted,
            ProcessAnswer::NeverAsked,
            ProcessAnswer::Unknown,
        ] {
            assert!(
                !restart_is_the_missing_step(Capability::Microphone, answer),
                "claimed a restart for Microphone on {:?}",
                answer
            );
        }
    }

    #[test]
    fn nothing_claims_a_restart_on_an_answer_it_could_not_read() {
        // Unknown means the check failed. Turning that into "restart Juno"
        // would be a guess the person pays for.
        for capability in ALL {
            assert!(!restart_is_the_missing_step(
                capability,
                ProcessAnswer::Unknown
            ));
        }
    }

    #[test]
    fn a_restart_offer_always_comes_with_its_sentence() {
        for capability in ALL {
            let owed = restart_is_the_missing_step(capability, ProcessAnswer::Refused);
            assert_eq!(
                owed,
                !restart_detail(capability).is_empty(),
                "{:?} offers a restart with no sentence, or a sentence with no offer",
                capability
            );
        }
    }

    #[test]
    fn the_restart_sentence_says_what_happens_to_the_conversation() {
        // A restart the person agrees to should not quietly cost them what
        // they were doing, and if it does, the copy has to say so.
        for capability in [Capability::ScreenRecording, Capability::Microphone] {
            let detail = restart_detail(capability);
            assert!(detail.contains("saved"), "{}", detail);
            assert!(detail.contains("History"), "{}", detail);
            assert!(!detail.contains('—'), "em dash in restart copy");
        }
    }

    #[test]
    fn an_unknown_key_is_refused_rather_than_guessed() {
        assert!(moment_for("full_disk_access").is_err());
    }

    // ── The card's one button ────────────────────────────────────────────────

    #[test]
    fn the_three_switches_send_the_person_to_settings() {
        for capability in [
            Capability::Accessibility,
            Capability::ScreenRecording,
            Capability::Microphone,
        ] {
            assert_eq!(capability.primary_action(), PrimaryAction::OpenSettings);
        }
    }

    #[test]
    fn automation_asks_macos_rather_than_opening_a_pane() {
        // Before macOS has asked once there is no row in the Automation pane,
        // so "Open Settings" would send someone to an empty list.
        for target in [AutomationTarget::SystemEvents, AutomationTarget::Safari] {
            assert_eq!(
                Capability::Automation(target).primary_action(),
                PrimaryAction::AllowAutomation
            );
        }
    }

    #[test]
    fn every_button_label_is_two_words_or_fewer() {
        for action in [PrimaryAction::OpenSettings, PrimaryAction::AllowAutomation] {
            assert!(
                action.label().split_whitespace().count() <= 2,
                "{:?}",
                action
            );
            assert!(!action.key().is_empty());
        }
    }
}

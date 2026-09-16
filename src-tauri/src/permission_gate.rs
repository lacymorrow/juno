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

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::constants::events;

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
}

impl Capability {
    /// Matches the key used by the permissions state and the settings deep link.
    pub fn key(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::ScreenRecording => "screen_recording",
            Self::Microphone => "microphone",
        }
    }

    /// Leads with what the person gets, not with what Juno wants.
    pub fn title(self) -> &'static str {
        match self {
            Self::Accessibility => "Juno can do that once Accessibility is on",
            Self::ScreenRecording => "Juno can see your screen once Screen Recording is on",
            Self::Microphone => "Juno can listen once Microphone is on",
        }
    }

    /// One sentence on what it buys, one on how small and reversible it is.
    pub fn detail(self) -> &'static str {
        match self {
            Self::Accessibility => {
                "Accessibility lets Juno click and type for you. It is one switch in \
                 System Settings, and you can turn it back off whenever you like."
            }
            Self::ScreenRecording => {
                "Screen Recording is how Juno sees what is in front of you. It is one \
                 switch in System Settings, and you can turn it back off whenever you like."
            }
            Self::Microphone => {
                "Microphone is how Juno hears you, so you can talk instead of typing. It \
                 is one switch in System Settings, and you can turn it back off whenever \
                 you like."
            }
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
            match self {
                Self::Accessibility => "Accessibility",
                Self::ScreenRecording => "Screen Recording",
                Self::Microphone => "Microphone",
            }
        )
    }
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
    if !may_ask(capability, Instant::now()) {
        tracing::debug!(
            "Not asking again for {} so soon (wanted it for '{}')",
            capability.key(),
            action
        );
        return;
    }

    let payload = serde_json::json!({
        "permission": capability.key(),
        "action": action,
        "title": capability.title(),
        "detail": capability.detail(),
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
    if is_granted(app_handle, capability).await {
        return Ok(());
    }
    ask(app_handle, capability, action);
    Err(capability.agent_message(action))
}

/// Current grant state, read through the cached permissions check so a tool loop
/// does not hammer the native APIs.
async fn is_granted(app_handle: &AppHandle, capability: Capability) -> bool {
    match crate::commands::permissions::check_permissions_status_native(app_handle.clone()).await {
        Ok(state) => match capability {
            Capability::Accessibility => state.accessibility.granted,
            Capability::ScreenRecording => state.screen_recording.granted,
            Capability::Microphone => state.microphone.granted,
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
        for capability in [
            Capability::Accessibility,
            Capability::ScreenRecording,
            Capability::Microphone,
        ] {
            // "Juno can ..." not "Juno needs ...": the title is an offer.
            assert!(capability.title().starts_with("Juno can "));
            // Every ask promises it is reversible, because that is what makes
            // saying yes cheap.
            assert!(capability.detail().contains("turn it back off"));
            assert!(capability.detail().contains("one switch"));
        }
    }
}

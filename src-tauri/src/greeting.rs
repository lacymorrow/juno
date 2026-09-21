//! # Greeting
//!
//! Juno says one line when she starts.
//!
//! The first time, at the moment onboarding ends and she actually becomes
//! usable, she says who she is and which key summons her. Every launch after
//! that, two words. That is the whole feature: no setting of its own, no
//! rotating pool of quips, nothing to dismiss.
//!
//! Two things keep it from becoming noise. Turning TTS off silences it, like
//! every other thing Juno says, because it goes through the same
//! [`crate::tts::invoke_tts`] path. And a machine that booted a moment ago is
//! assumed to be opening Juno as a login item, where an unprompted voice is a
//! bad way to start someone's day, so she stays quiet.

use tauri::{AppHandle, Manager};
use tracing::{debug, info, warn};

use crate::triggers::{Binding, TriggerMethod, TriggerTarget};

/// How recently the machine must have booted for a launch to read as a login
/// item rather than someone opening Juno on purpose.
const LOGIN_WINDOW: std::time::Duration = std::time::Duration::from_secs(120);

/// The two words for every launch after the first.
const HELLO: &str = "Welcome back.";

/// What she says when there is no key bound to talk to her yet.
const INTRO_UNBOUND: &str = "Hi, I'm Juno. I'm in your menu bar whenever you need me.";

/// Say hello. Called on every launch where onboarding is already behind us.
pub async fn on_launch(app: &AppHandle) {
    speak(app, HELLO.to_string()).await;
}

/// Introduce herself. Called once, the moment onboarding finishes.
pub async fn on_first_run(app: &AppHandle) {
    speak(app, introduction(app)).await;
}

/// The line she opens with, naming the key that was actually bound.
fn introduction(app: &AppHandle) -> String {
    let Some(key) = summoning_key(app) else {
        return INTRO_UNBOUND.to_string();
    };
    format!("Hi, I'm Juno. Hold {key} whenever you want to talk to me.")
}

/// The binding worth naming, spoken rather than spelled.
///
/// Dictation first: it is the one people reach for without thinking about it,
/// and it is what "talk to me" describes. A mouse button is skipped, because
/// "hold Mouse Button 4" is not a sentence anybody wants read aloud.
fn summoning_key(app: &AppHandle) -> Option<String> {
    let triggers = app.state::<crate::state::AppState>().get_triggers().ok()?;

    let pick = |target: TriggerTarget| {
        triggers
            .iter()
            .filter(|t| t.enabled && t.target == target)
            .filter(|t| matches!(t.method, TriggerMethod::PushToTalk | TriggerMethod::Toggle))
            .find_map(|t| match t.binding.as_ref() {
                Some(Binding::Keyboard { shortcut }) => Some(shortcut.clone()),
                _ => None,
            })
    };

    pick(TriggerTarget::Dictation)
        .or_else(|| pick(TriggerTarget::Agent))
        .map(|combo| spoken(&combo))
}

/// Turn a combo into something a voice can read.
///
/// "Option+Space" is four words to a person and one token to a synthesizer,
/// which reads the plus sign out loud or swallows the whole thing. Splitting it
/// and spelling the modifiers the way they are spoken is the difference between
/// an instruction and a noise.
fn spoken(combo: &str) -> String {
    if let Some(key) = crate::triggers::bare_modifier(combo) {
        return match key {
            crate::triggers::ModifierKey::Fn => "the globe key".to_string(),
        };
    }

    combo
        .split('+')
        .map(|part| match part.trim().to_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" => "Command",
            "opt" | "option" | "alt" => "Option",
            "ctrl" | "control" => "Control",
            "shift" => "Shift",
            "space" => "Space",
            "comma" => "Comma",
            _ => part.trim(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Speak a line, unless this launch is one where speaking would be rude.
async fn speak(app: &AppHandle, line: String) {
    if booted_just_now() {
        debug!("[Greeting] Machine just booted; this is a login item, staying quiet");
        return;
    }

    info!("[Greeting] {}", line);

    // Through the ordinary TTS path, so the provider setting governs this the
    // same way it governs everything else she says. "Off" means off.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::state::AppState>();
        if let Err(e) = crate::tts::invoke_tts(line, state, app.clone()).await {
            warn!("[Greeting] Could not speak: {}", e);
        }
    });
}

/// Did this machine boot within the last couple of minutes?
///
/// Read from the kernel's boot time rather than tracked in Juno, because the
/// question is about the machine, not about her.
fn booted_just_now() -> bool {
    match uptime() {
        Some(up) => up < LOGIN_WINDOW,
        // Unknown means speak. A greeting that occasionally lands during login
        // beats one that silently never fires.
        None => false,
    }
}

#[cfg(target_os = "macos")]
fn uptime() -> Option<std::time::Duration> {
    // `kern.boottime` is a `struct timeval`; only the seconds are needed.
    let out = std::process::Command::new("sysctl")
        .args(["-n", "kern.boottime"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let secs: u64 = text
        .split("sec = ")
        .nth(1)?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(std::time::Duration::from_secs(now.saturating_sub(secs)))
}

#[cfg(not(target_os = "macos"))]
fn uptime() -> Option<std::time::Duration> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combo_is_spoken_not_spelled() {
        assert_eq!(spoken("Option+Space"), "Option Space");
        assert_eq!(spoken("Cmd+Shift+D"), "Command Shift D");
        assert_eq!(spoken("alt+space"), "Option Space");
    }

    #[test]
    fn the_globe_key_has_a_name_people_use() {
        // "Fn (globe)" is a label for a settings row, not something to read
        // aloud, and "Fn" on its own gets read as a word.
        assert_eq!(spoken("Fn"), "the globe key");
    }

    #[test]
    fn an_unknown_key_is_left_alone() {
        assert_eq!(spoken("F13"), "F13");
    }
}

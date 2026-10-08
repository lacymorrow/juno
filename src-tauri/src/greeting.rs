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
//!
//! The launch line is the same every time, so it is rendered while Juno
//! starts and kept on disk (`tts::prerender`), keyed by engine, voice, rate
//! and text. The reveal wakes the greeting on its beat (`intro::GREETING_AT_MS`)
//! and the file plays at once. When the render is not ready, or the voice has
//! changed since, the line is spoken the ordinary way.

use tauri::{AppHandle, Manager};
use tracing::{debug, info, warn};

use crate::triggers::TriggerTarget;

/// How recently the machine must have booted for a launch to read as a login
/// item rather than someone opening Juno on purpose.
const LOGIN_WINDOW: std::time::Duration = std::time::Duration::from_secs(120);

/// Every launch after the first. Short, but it says who is talking: a voice
/// from an empty desktop saying only "welcome back" leaves the person working
/// out which of their apps just spoke.
const HELLO: &str = "Juno here. Welcome back.";

/// What she says when there is no key bound to talk to her yet.
const INTRO_UNBOUND: &str = "Hi, I'm Juno. I'm in your menu bar whenever you need me.";

/// How long to wait for the bar before giving up and speaking anyway.
const BAR_WAIT: std::time::Duration = std::time::Duration::from_secs(8);

/// Render the launch line ahead, in the engine, voice and rate in force, so
/// it plays the moment the bar appears. Called at launch, in parallel with
/// startup, and after any change of engine, voice or rate.
pub fn refresh_cache(app: &AppHandle) {
    crate::tts::prerender::refresh_in_background(app, HELLO);
}

/// How often to look while waiting.
const BAR_POLL: std::time::Duration = std::time::Duration::from_millis(150);

/// Say hello. Called on every launch where onboarding is already behind us.
pub async fn on_launch(app: &AppHandle) {
    speak(app, HELLO.to_string(), true).await;
}

/// Introduce herself. Called once, the moment onboarding finishes.
pub async fn on_first_run(app: &AppHandle) {
    speak(app, introduction(app), false).await;
}

/// The line she opens with, naming the trigger that is actually in effect.
fn introduction(app: &AppHandle) -> String {
    match app.state::<crate::state::AppState>().get_triggers() {
        Ok(triggers) => introduction_for(&triggers),
        Err(_) => INTRO_UNBOUND.to_string(),
    }
}

/// [`introduction`] for a given trigger list, so the wording is testable.
///
/// Read from the same hint onboarding draws, so the voice and the screen name
/// the same key. Talking to Juno comes first because that is literally what
/// the line describes: her Hold trigger, or whatever other keyboard trigger
/// she has when there is no Hold. Dictation is the fallback. A mouse button
/// and a wake phrase are skipped by the hint, because "hold Mouse Button 4" is
/// not a sentence anybody wants read aloud.
fn introduction_for(triggers: &[crate::triggers::Trigger]) -> String {
    let (purpose, hint) =
        if let Some(hint) = crate::triggers::hint_for(triggers, TriggerTarget::Agent) {
            ("To talk to me", hint)
        } else if let Some(hint) = crate::triggers::hint_for(triggers, TriggerTarget::Dictation) {
            ("To dictate", hint)
        } else {
            return INTRO_UNBOUND.to_string();
        };
    format!(
        "Hi, I'm Juno. {purpose}, {} {}.",
        hint.gesture.to_lowercase(),
        spoken(&hint.shortcut)
    )
}

/// Join key names the way a person says them: "A", "A and B together",
/// "A, B and C together".
fn together(names: Vec<String>) -> String {
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => {
            format!("{} and {last} together", rest.join(", "))
        }
        _ => names.join(" "),
    }
}

/// Turn a combo into something a voice can read.
///
/// "Option+Space" is four words to a person and one token to a synthesizer,
/// which reads the plus sign out loud or swallows the whole thing. Splitting it
/// and spelling the modifiers the way they are spoken is the difference between
/// an instruction and a noise.
fn spoken(combo: &str) -> String {
    if let Some(key) = crate::triggers::bare_modifier(combo) {
        use crate::triggers::ModifierKey;
        if key == ModifierKey::FN {
            return "the globe key".to_string();
        }
        if key == ModifierKey::RIGHT_OPTION {
            return "Right Option".to_string();
        }
        // A chord: name each key, the globe key the way a person says it.
        return together(
            key.shortcut()
                .split('+')
                .map(|part| {
                    if part == "Fn" {
                        "the globe key".to_string()
                    } else {
                        part.to_string()
                    }
                })
                .collect(),
        );
    }

    together(
        combo
            .split('+')
            .map(|part| {
                match part.trim().to_lowercase().as_str() {
                    "cmd" | "command" | "meta" | "super" => "Command",
                    "opt" | "option" | "alt" => "Option",
                    "ctrl" | "control" => "Control",
                    "shift" => "Shift",
                    "space" => "Space",
                    "comma" => "Comma",
                    _ => part.trim(),
                }
                .to_string()
            })
            .collect(),
    )
}

/// Speak a line, unless this launch is one where speaking would be rude.
async fn speak(app: &AppHandle, line: String, rendered_ahead: bool) {
    if booted_just_now() {
        debug!("[Greeting] Machine just booted; this is a login item, staying quiet");
        return;
    }

    // Through the ordinary TTS path, so the provider setting governs this the
    // same way it governs everything else she says. "Off" means off.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Wait until she is actually on screen. A voice arriving before the
        // bar does is a voice from nowhere: the person hears Juno a beat
        // before they have anything to look at, and the greeting reads as a
        // glitch rather than as her saying hello.
        wait_for_the_bar(&app).await;

        info!("[Greeting] {}", line);
        // Played from the render made at launch when it matches the voice
        // in force; spoken the ordinary way otherwise.
        if rendered_ahead {
            if let Some(audio) = crate::tts::prerender::ready_for(&app, &line).await {
                let state = app.state::<crate::state::AppState>();
                match crate::tts::speak_prerendered(audio, state, app.clone()).await {
                    Ok(_) => return,
                    Err(e) => debug!("[Greeting] Rendered line did not play ({e}); speaking it"),
                }
            }
        }
        let state = app.state::<crate::state::AppState>();
        if let Err(e) = crate::tts::invoke_tts(line, state, app.clone()).await {
            warn!("[Greeting] Could not speak: {}", e);
        }
    });
}

/// Wait for the moment to speak: the reveal's greeting beat when the bar is
/// coming on with the intro, or the bar simply being on screen otherwise.
///
/// The beat is what makes the line land as the pill appears. Polling for
/// visibility found the bar up to 150 ms late and knew nothing of the smoke.
/// The poll stays for the routes that show the bar with no reveal (a bar that
/// was already visible, or a reveal that could not be arranged). Giving up
/// after [`BAR_WAIT`] and speaking anyway is deliberate: a greeting that waits
/// forever for a bar someone has switched off is a greeting that never
/// happens.
async fn wait_for_the_bar(app: &AppHandle) {
    let deadline = std::time::Instant::now() + BAR_WAIT;
    let label = crate::constants::window_labels::FLOATING_BAR;

    // A reveal is running, or is about to: its beat is the moment.
    let bar_visible = app
        .get_webview_window(label)
        .and_then(|bar| bar.is_visible().ok())
        .unwrap_or(false);
    if bar_visible && !crate::intro::reveal_in_flight() {
        return;
    }
    if crate::intro::greeting_beat(BAR_WAIT).await {
        return;
    }

    while std::time::Instant::now() < deadline {
        if let Some(bar) = app.get_webview_window(label) {
            if bar.is_visible().unwrap_or(false) {
                return;
            }
        }
        tokio::time::sleep(BAR_POLL).await;
    }

    debug!("[Greeting] Bar never appeared within the wait; greeting anyway");
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
    use crate::triggers::{default_triggers, Binding, Gesture, Trigger};

    fn agent(gesture: Gesture, shortcut: &str) -> Vec<Trigger> {
        let mut ts = default_triggers();
        ts.retain(|t| t.target != TriggerTarget::Agent);
        let mut t = default_triggers().remove(0);
        t.gesture = gesture;
        t.binding = Some(Binding::Keyboard {
            shortcut: shortcut.to_string(),
        });
        ts.push(t);
        ts
    }

    #[test]
    fn the_line_names_the_default_globe_key() {
        assert_eq!(
            introduction_for(&default_triggers()),
            "Hi, I'm Juno. To talk to me, hold the globe key."
        );
    }

    #[test]
    fn the_line_names_right_option() {
        assert_eq!(
            introduction_for(&agent(Gesture::Hold, "RightOption")),
            "Hi, I'm Juno. To talk to me, hold Right Option."
        );
    }

    #[test]
    fn the_line_names_a_modifier_chord() {
        assert_eq!(
            introduction_for(&agent(Gesture::Hold, "Control+Option")),
            "Hi, I'm Juno. To talk to me, hold Control and Option together."
        );
    }

    #[test]
    fn the_line_names_a_normal_shortcut() {
        assert_eq!(
            introduction_for(&agent(Gesture::Hold, "Alt+Space")),
            "Hi, I'm Juno. To talk to me, hold Option and Space together."
        );
    }

    #[test]
    fn with_no_hold_the_line_names_whatever_trigger_there_is() {
        assert_eq!(
            introduction_for(&agent(Gesture::Tap, "RightOption")),
            "Hi, I'm Juno. To talk to me, tap Right Option."
        );
    }

    #[test]
    fn with_no_key_at_all_the_line_says_where_she_lives() {
        assert_eq!(introduction_for(&[]), INTRO_UNBOUND);
    }

    #[test]
    fn a_combo_is_spoken_not_spelled() {
        assert_eq!(spoken("Option+Space"), "Option and Space together");
        assert_eq!(spoken("Cmd+Shift+D"), "Command, Shift and D together");
        assert_eq!(spoken("alt+space"), "Option and Space together");
    }

    #[test]
    fn the_globe_key_has_a_name_people_use() {
        assert_eq!(spoken("Fn"), "the globe key");
    }

    #[test]
    fn the_globe_control_chord_is_spoken_as_two_keys_together() {
        assert_eq!(spoken("Fn+Control"), "the globe key and Control together");
    }

    #[test]
    fn any_modifier_chord_is_spoken_as_keys_together() {
        assert_eq!(spoken("Control+Option"), "Control and Option together");
        assert_eq!(
            spoken("Control+Option+Shift"),
            "Control, Option and Shift together"
        );
        assert_eq!(spoken("Control"), "Control");
        assert_eq!(spoken("RightOption"), "Right Option");
    }

    #[test]
    fn an_unknown_key_is_left_alone() {
        assert_eq!(spoken("F13"), "F13");
    }
}

//! Local intents: requests Juno answers itself, without a model round-trip.
//!
//! # The rule: whole, unambiguous utterances only
//!
//! A local intent fires only when the *entire* utterance is one of a small set
//! of command phrasings, with nothing left over. Courtesy at the edges is
//! tolerated ("hey Juno, could you please ... thanks"), and so is trailing
//! punctuation. Anything else goes to the agent:
//!
//! - a second clause ("open Safari **and** find me flights to Denver"),
//! - an object or qualifier we do not model ("mute **Zoom**", "turn up the
//!   volume **on YouTube**", "what time is it **in Tokyo**"),
//! - a question *about* the thing rather than a command ("**how do I** mute
//!   Zoom?"),
//! - a name we cannot resolve exactly ("open the flight tracker" when no app
//!   has that name).
//!
//! Stealing a request the person meant for the agent is worse than being a
//! few seconds slower, so every grammar here is anchored at both ends and a
//! miss is always the safe outcome. Each domain's tests carry near-miss
//! negatives that must fall through; add one with every new phrasing.
//!
//! # Safety
//!
//! Nothing the person said is ever spliced into a script. Scripts are
//! constants or are built from validated integers; an app name reaches
//! AppleScript only as `argv`, and only after it matched an installed app.
//!
//! # Domains
//!
//! - [`media`]: play, pause, skip, "what's playing?" (Spotify, Apple Music)
//! - [`system`]: volume, dark mode, lock, display and Mac sleep, battery,
//!   time, date
//! - [`timer`]: start, check, cancel countdown timers (backend-owned)
//! - [`apps`]: open or quit an installed app, open a website
//! - [`settings_follow`]: "open settings" opens the settings of the app you
//!   were in, and a short window to correct it
//! - [`quit`]: "quit", "quit Juno": close Juno itself
//! - [`stop`]: a bare "stop" or "cancel" while a run is in flight
//!
//! [`try_handle_local_intent`] is the single entry point `submit_query` calls.
//! See `docs/plans/local-intents.md` for what was considered and cut.

pub mod apps;
pub mod media;
pub mod quit;
pub mod settings_follow;
pub mod stop;
pub mod system;
pub mod timer;
pub mod utterance;

use std::time::Duration;

use tauri::AppHandle;

use media::{parse_media_intent, MediaIntent};

/// How long any single native call may take before the reply gives up.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// What a locally served request shows and says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Chat content: plain text or one whitelisted card.
    pub display: String,
    /// The short line spoken aloud.
    pub spoken: String,
    /// The native call failed; the reply explains it.
    pub failed: bool,
}

impl Reply {
    /// Plain text, shown and spoken.
    pub fn text(line: impl Into<String>) -> Self {
        let line = line.into();
        Self {
            display: line.clone(),
            spoken: line,
            failed: false,
        }
    }

    /// A card from the JSX whitelist, with a spoken line.
    pub fn card(display: String, spoken: impl Into<String>) -> Self {
        Self {
            display,
            spoken: spoken.into(),
            failed: false,
        }
    }

    /// A failure, shown and spoken.
    pub fn failure(line: impl Into<String>) -> Self {
        Self {
            failed: true,
            ..Self::text(line)
        }
    }
}

/// A parsed local request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalIntent {
    Media(MediaIntent),
    System(system::SystemIntent),
    Timer(timer::TimerIntent),
    App(apps::AppIntent),
    Settings(settings_follow::SettingsIntent),
}

/// Parse everything but media, in precedence order. Apps come last because
/// "open ..." is the loosest grammar and must lose to every exact phrase.
fn parse_non_media(query: &str) -> Option<LocalIntent> {
    let utterance = utterance::normalize(query)?;
    if let Some(i) = system::parse(&utterance) {
        return Some(LocalIntent::System(i));
    }
    if let Some(i) = timer::parse(&utterance) {
        return Some(LocalIntent::Timer(i));
    }
    // Before apps: "open Music settings" would otherwise be an app name.
    if let Some(i) = settings_follow::parse(&utterance) {
        return Some(LocalIntent::Settings(i));
    }
    apps::parse(&utterance).map(LocalIntent::App)
}

/// Parse a raw query into the local intent it names, if any. Pure: no I/O.
pub fn parse_local_intent(query: &str) -> Option<LocalIntent> {
    parse_media_intent(query)
        .map(LocalIntent::Media)
        .or_else(|| parse_non_media(query))
}

/// Compile a domain's patterns once. The patterns are literals, so a failure
/// means a developer broke one; instead of panicking (no-unwrap rule) we log
/// loudly and disable that domain. Every query then falls through to the
/// agent.
fn compile<T>(domain: &str, build: fn() -> Result<T, regex::Error>) -> Option<T> {
    match build() {
        Ok(patterns) => Some(patterns),
        Err(e) => {
            tracing::error!(
                "local_intents: {} regex failed to compile ({}); {} intents disabled",
                domain,
                e,
                domain
            );
            None
        }
    }
}

/// Run a program with arguments (never through a shell) on the blocking
/// pool, bounded by [`COMMAND_TIMEOUT`]. Returns trimmed stdout.
async fn run(program: &'static str, args: Vec<String>) -> Result<String, String> {
    let task = tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new(program)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run {}: {}", program, e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    });
    match tokio::time::timeout(COMMAND_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => Err(format!("{} task failed: {}", program, e)),
        Err(_) => Err(format!("{} timed out", program)),
    }
}

/// Run a constant AppleScript, passing any variable input as `argv`.
async fn osascript(script: &str, argv: Vec<String>) -> Result<String, String> {
    let mut args = vec!["-e".to_string(), script.to_string()];
    args.extend(argv);
    run("osascript", args).await
}

/// Try to serve `query` locally. Returns `true` when it was handled and the
/// caller must not run the agent.
///
/// Emits exactly the events a normal run does (stream start, chunk with
/// spoken text, stream end, plus the floating-bar lifecycle) so every window
/// renders the reply identically to an agent reply.
pub async fn try_handle_local_intent(app_handle: &AppHandle, query: &str) -> bool {
    // First, because a bare "stop" while a run is in flight is the Escape key
    // in words and must not queue behind the run it is trying to end. It only
    // fires on a whole-utterance stop with something running, so every other
    // reading of the word (including "stop the music", which `media` owns two
    // lines down) is untouched. See `stop`.
    if stop::try_halt(app_handle, query).await {
        return true;
    }

    // Quitting Juno itself, on every input path. Whole utterance only; see `quit`.
    if quit::try_quit(app_handle, query) {
        return true;
    }

    // A correction of the settings window just opened ("no, not that",
    // "Mac settings"). Fires only while that action is under 60 s old.
    if let Some(correction) = settings_follow::correction_for(query) {
        if let Some(reply) = settings_follow::handle_correction(app_handle, correction).await {
            emit_reply(app_handle, reply).await;
            settings_follow::publish(app_handle);
            return true;
        }
    }
    // Anything else is an unrelated turn: the chance to correct has passed.
    settings_follow::forget(app_handle);

    if let Some(intent) = parse_media_intent(query) {
        if let Some(reply) = media::handle(app_handle, intent).await {
            emit_reply(app_handle, reply).await;
            return true;
        }
    }

    // Everything below shells out to macOS tools.
    if !cfg!(target_os = "macos") {
        return false;
    }
    let Some(intent) = parse_non_media(query) else {
        return false;
    };
    let reply = match intent.clone() {
        LocalIntent::System(i) => system::handle(app_handle, i).await,
        LocalIntent::Timer(i) => timer::handle(app_handle, i),
        LocalIntent::App(i) => apps::handle(app_handle, i).await,
        LocalIntent::Settings(i) => settings_follow::handle(app_handle, i).await,
        LocalIntent::Media(_) => None,
    };
    let Some(reply) = reply else {
        return false;
    };
    log::info!("Local intent {:?} served: {}", intent, reply.spoken);
    emit_reply(app_handle, reply).await;
    settings_follow::publish(app_handle);
    true
}

/// When a locally served reply last finished, for [`replied_within`].
static LAST_REPLY: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// A frontend "stop" that lands this soon after a local reply is not a
/// decision about that reply: nobody can read an answer and reach for Stop in
/// this long. The log of 2026-10-09 shows one arriving 70 ms after "Opened
/// ghostty settings." was served, which cut the reply's speech and dropped the
/// chip's.
pub const STOP_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);

/// Whether a local reply finished within `window`.
pub fn replied_within(window: std::time::Duration) -> bool {
    LAST_REPLY
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|at| at.elapsed() < window)
}

/// Show and speak `reply` as a whole turn: the bar goes working and back,
/// exactly as it does for a model's answer.
pub(crate) async fn emit_reply(app_handle: &AppHandle, reply: Reply) {
    let agent_state = if reply.failed { "Failed" } else { "Finished" };
    crate::commands::ui_commands::handle_agent_started(app_handle).await;

    let display = reply.display.clone();
    emit_reply_message(app_handle, reply, agent_state);

    crate::commands::ui_commands::handle_agent_stopped(app_handle).await;
    crate::commands::ui_commands::handle_backend_response(
        app_handle,
        Some(display),
        agent_state.to_string(),
    )
    .await;
    if let Ok(mut last) = LAST_REPLY.lock() {
        *last = Some(std::time::Instant::now());
    }
}

/// Append `reply` to the conversation as an assistant message and speak its
/// spoken line, without touching the bar's lifecycle. For a turn that is
/// already running and will close itself.
pub(crate) fn emit_reply_message(app_handle: &AppHandle, reply: Reply, agent_state: &str) {
    let message_id = uuid::Uuid::new_v4().to_string();
    crate::agent::tool_logger::emit_stream_start(app_handle, message_id.clone());
    crate::agent::tool_logger::emit_streaming_text_chunk(
        app_handle,
        reply.display.clone(),
        Some(message_id.clone()),
        Some(reply.spoken),
    );
    crate::agent::tool_logger::emit_stream_end_with_state(
        app_handle,
        message_id,
        reply.display,
        agent_state.to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_domain_is_reachable_from_the_entry_parser() {
        assert!(matches!(
            parse_local_intent("pause Spotify"),
            Some(LocalIntent::Media(_))
        ));
        assert!(matches!(
            parse_local_intent("turn the volume up"),
            Some(LocalIntent::System(system::SystemIntent::VolumeUp))
        ));
        assert!(matches!(
            parse_local_intent("set a timer for 5 minutes"),
            Some(LocalIntent::Timer(timer::TimerIntent::Start { secs: 300 }))
        ));
        assert!(matches!(
            parse_local_intent("open Spotify"),
            Some(LocalIntent::App(apps::AppIntent::Open { .. }))
        ));
    }

    #[test]
    fn exact_phrases_beat_the_open_grammar() {
        // "stop the timer" is a timer command, not media and not an app.
        assert_eq!(
            parse_local_intent("stop the timer"),
            Some(LocalIntent::Timer(timer::TimerIntent::Cancel))
        );
        // "turn off the display" is a system command, not "open ...".
        assert_eq!(
            parse_local_intent("turn off the display"),
            Some(LocalIntent::System(system::SystemIntent::SleepDisplay))
        );
    }

    #[test]
    fn requests_meant_for_the_agent_never_parse() {
        for q in [
            "open Safari and find me flights to Denver",
            "how do I mute Zoom?",
            "what time is it in Tokyo",
            "set a timer for 5 minutes and then text Sam",
            "lock the screen after the build finishes",
            "summarize my unread email",
            "what's the weather like",
            "turn off dark mode in VS Code",
            "can you check the battery on my AirPods",
        ] {
            let parsed = parse_local_intent(q);
            assert!(
                !matches!(
                    parsed,
                    Some(LocalIntent::System(_)) | Some(LocalIntent::Timer(_))
                ) && !matches!(parsed, Some(LocalIntent::Media(_))),
                "{:?} should reach the agent, got {:?}",
                q,
                parsed
            );
        }
    }

    #[test]
    fn reply_constructors() {
        let r = Reply::text("Volume 40%.");
        assert_eq!(r.display, r.spoken);
        assert!(!r.failed);
        assert!(Reply::failure("I couldn't lock the screen.").failed);
    }
}

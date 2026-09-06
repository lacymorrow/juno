//! Local intents: queries Juno answers itself, without a model round-trip.
//!
//! The first (and so far only) family is playback control. "Pause Spotify",
//! "skip this song", "what's playing?" are fully deterministic: the answer is
//! one AppleScript call plus the pre-built `<NowPlayingCard>` widget, which is
//! live-bound to the player and re-used verbatim on every response. Sending
//! those through the agent costs 5–10 s of model time to arrive at the same
//! card, so `submit_query` asks this module first and only falls through to
//! the agent when the request is not a bare transport command.
//!
//! The grammar is deliberately narrow. Anything with extra content ("play my
//! liked songs", "skip two tracks", "play the video") does not match and goes
//! to the agent, which has the tools to interpret it. Bare verbs with no
//! player named ("pause", "next") only match when a supported player is
//! actually playing, so a video in a browser is never mistaken for Spotify.

use once_cell::sync::Lazy;
use regex::Regex;
use tauri::AppHandle;

use crate::commands::media::{media_control, media_get_state, MediaState};

/// Transport action the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaAction {
    Play,
    Pause,
    Next,
    Previous,
}

impl MediaAction {
    fn as_command(self) -> &'static str {
        match self {
            MediaAction::Play => "play",
            MediaAction::Pause => "pause",
            MediaAction::Next => "next",
            MediaAction::Previous => "previous",
        }
    }
}

/// A playback request that can be served locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaIntent {
    /// Control the player. `app` is `None` when the user did not name one.
    Control {
        app: Option<&'static str>,
        action: MediaAction,
        /// The query mentioned music explicitly ("pause the music", "skip this
        /// song") rather than a bare verb ("pause", "next").
        names_music: bool,
    },
    /// "What's playing?"
    Status {
        app: Option<&'static str>,
        names_music: bool,
    },
}

/// Words that carry no meaning for playback ("hey juno, could you please…").
const FILLER_WORDS: &[&str] = &[
    "hey",
    "hi",
    "ok",
    "okay",
    "juno",
    "please",
    "can",
    "could",
    "would",
    "will",
    "you",
    "u",
    "now",
    "just",
    "go",
    "ahead",
    "and",
    "for",
    "me",
    "kindly",
    "the",
    "this",
    "that",
    "my",
    "a",
    "an",
    "current",
    "currently",
    "right",
    "up",
    "of",
    "some",
    "again",
    "quick",
    "quickly",
    "real",
    "it",
    "thanks",
    "thank",
];

static APP_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:on |in |from |with |using )?(spotify|apple music|itunes)\b").unwrap()
});
static PAUSE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:pause|stop)(?: (?:music|song|track|playback|playing|audio))?$").unwrap()
});
static PLAY_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:(?:play|unpause)(?: (?:music|song|track|playback|audio))?|(?:resume|continue|start|keep) (?:music|song|track|playback|playing|audio))$",
    )
    .unwrap()
});
static NEXT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:(?:next|skip)(?: (?:song|track|one|ahead|forward|music))?|play next(?: (?:song|track))?)$",
    )
    .unwrap()
});
static PREVIOUS_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:previous(?: (?:song|track|one))?|(?:last|prior) (?:song|track)|(?:go )?back (?:song|track)|play (?:previous|last) (?:song|track))$",
    )
    .unwrap()
});
static STATUS_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:what is(?: (?:playing|on|song|track))?|what (?:song|track|music) is(?: (?:playing|on))?|what is (?:song|track|music) playing|what am i listening to|now playing|which (?:song|track) is(?: playing)?)$",
    )
    .unwrap()
});
static MUSIC_NOUN_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:music|song|track|playback|playing|listening|audio)\b").unwrap());

/// Lower-case, strip punctuation, expand contractions, drop filler words.
fn normalize(query: &str) -> String {
    let lower = query.to_lowercase().replace('\u{2019}', "'");
    let mut spaced: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    for (from, to) in [
        ("what's", "what is"),
        ("whats", "what is"),
        ("who's", "who is"),
        ("i'm", "i am"),
        ("'", ""),
    ] {
        spaced = spaced.replace(from, to);
    }
    spaced
        .split_whitespace()
        .filter(|w| !FILLER_WORDS.contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pull the named player out of the phrase, returning the remainder.
fn extract_app(phrase: &str) -> (Option<&'static str>, String) {
    let mut app = None;
    let rest = APP_PATTERN.replace_all(phrase, |caps: &regex::Captures| {
        app = Some(match &caps[1] {
            "spotify" => "Spotify",
            _ => "Music",
        });
        " "
    });
    let rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    (app, rest)
}

/// Parse a raw user query into a playback intent, if it is one.
pub fn parse_media_intent(query: &str) -> Option<MediaIntent> {
    let normalized = normalize(query);
    if normalized.is_empty() {
        return None;
    }
    let (app, phrase) = extract_app(&normalized);
    if phrase.is_empty() {
        return None;
    }
    let names_music = MUSIC_NOUN_PATTERN.is_match(&phrase);

    let action = if PAUSE_PATTERN.is_match(&phrase) {
        Some(MediaAction::Pause)
    } else if PLAY_PATTERN.is_match(&phrase) {
        Some(MediaAction::Play)
    } else if NEXT_PATTERN.is_match(&phrase) {
        Some(MediaAction::Next)
    } else if PREVIOUS_PATTERN.is_match(&phrase) {
        Some(MediaAction::Previous)
    } else {
        None
    };
    if let Some(action) = action {
        return Some(MediaIntent::Control {
            app,
            action,
            names_music,
        });
    }

    if STATUS_PATTERN.is_match(&phrase) {
        // "what is" / "what is on" alone are only about music when a player is named
        if (phrase == "what is" || phrase == "what is on") && app.is_none() {
            return None;
        }
        return Some(MediaIntent::Status { app, names_music });
    }

    None
}

/// Which player a request without an explicit app should go to, given live
/// state. `None` means the request is too ambiguous to answer locally.
fn resolve_implicit_app(intent: &MediaIntent, states: &[MediaState]) -> Option<&'static str> {
    let playing = states.iter().find(|s| s.running && s.state == "playing");
    let paused = states.iter().find(|s| s.running && s.state == "paused");
    let running = states.iter().find(|s| s.running);
    let pick = |s: &MediaState| -> &'static str {
        if s.app == "Spotify" {
            "Spotify"
        } else {
            "Music"
        }
    };

    match intent {
        MediaIntent::Control {
            action: MediaAction::Play,
            names_music,
            ..
        } => paused.map(pick).or_else(|| {
            if *names_music {
                running.map(pick)
            } else {
                None
            }
        }),
        MediaIntent::Control { names_music, .. } | MediaIntent::Status { names_music, .. } => {
            playing.map(pick).or_else(|| {
                if *names_music {
                    running.map(pick)
                } else {
                    None
                }
            })
        }
    }
}

fn describe_track(state: &MediaState) -> Option<String> {
    let track = state.track.as_deref().filter(|t| !t.is_empty())?;
    match state.artist.as_deref().filter(|a| !a.is_empty()) {
        Some(artist) => Some(format!("{} by {}", track, artist)),
        None => Some(track.to_string()),
    }
}

/// Spoken confirmation for a completed action, from the player's real state.
fn spoken_result(intent: &MediaIntent, app: &str, state: &MediaState) -> String {
    if !state.running {
        return format!("{} isn't running.", app);
    }
    let track = describe_track(state);
    match intent {
        MediaIntent::Control { action, .. } => match action {
            MediaAction::Pause => {
                if state.state == "playing" {
                    format!("{} didn't pause.", app)
                } else {
                    "Paused.".to_string()
                }
            }
            MediaAction::Play => match (state.state.as_str(), track) {
                ("playing", Some(t)) => format!("Playing {}.", t),
                ("playing", None) => "Playing.".to_string(),
                (_, Some(t)) => format!("{} didn't resume {}.", app, t),
                (_, None) => format!("{} didn't resume.", app),
            },
            MediaAction::Next => match track {
                Some(t) => format!("Skipped. Now playing {}.", t),
                None => "Skipped.".to_string(),
            },
            MediaAction::Previous => match track {
                Some(t) => format!("Went back. Now playing {}.", t),
                None => "Went back.".to_string(),
            },
        },
        MediaIntent::Status { .. } => match (state.state.as_str(), track) {
            ("playing", Some(t)) => format!("{}.", t),
            ("paused", Some(t)) => format!("Paused on {}.", t),
            _ => format!("Nothing is playing in {}.", app),
        },
    }
}

/// Try to serve `query` locally. Returns `true` when it was handled and the
/// caller must not run the agent.
///
/// Emits exactly the events a normal run does (stream start → chunk with
/// spoken text → stream end, plus the floating-bar lifecycle) so every window
/// renders the reply identically to an agent reply.
pub async fn try_handle_media_intent(app_handle: &AppHandle, query: &str) -> bool {
    let Some(intent) = parse_media_intent(query) else {
        return false;
    };

    let explicit_app = match &intent {
        MediaIntent::Control { app, .. } | MediaIntent::Status { app, .. } => *app,
    };

    let app = match explicit_app {
        Some(app) => {
            // "Play Spotify" when it is not running means "open it and play"
            // — that needs the agent.
            if matches!(
                intent,
                MediaIntent::Control {
                    action: MediaAction::Play,
                    ..
                }
            ) {
                match media_get_state(app.to_string()).await {
                    Ok(state) if state.running => {}
                    _ => return false,
                }
            }
            app
        }
        None => {
            let mut states = Vec::new();
            for candidate in crate::commands::media::SUPPORTED_APPS {
                if let Ok(state) = media_get_state(candidate.to_string()).await {
                    states.push(state);
                }
            }
            match resolve_implicit_app(&intent, &states) {
                Some(app) => app,
                None => return false,
            }
        }
    };

    log::info!("Local media intent {:?} → {}", intent, app);
    crate::commands::ui_commands::handle_agent_started(app_handle).await;

    let result = match &intent {
        MediaIntent::Control { action, .. } => {
            media_control(app.to_string(), action.as_command().to_string()).await
        }
        MediaIntent::Status { .. } => media_get_state(app.to_string()).await,
    };

    let (spoken, agent_state) = match result {
        Ok(state) => (spoken_result(&intent, app, &state), "Finished"),
        Err(e) => {
            log::warn!("Local media intent failed: {}", e);
            (format!("I couldn't reach {}.", app), "Failed")
        }
    };

    let display = format!("<NowPlayingCard app=\"{}\" />", app);
    let message_id = uuid::Uuid::new_v4().to_string();
    crate::agent::tool_logger::emit_stream_start(app_handle, message_id.clone());
    crate::agent::tool_logger::emit_streaming_text_chunk(
        app_handle,
        display.clone(),
        Some(message_id.clone()),
        Some(spoken.clone()),
    );
    crate::agent::tool_logger::emit_stream_end_with_state(
        app_handle,
        message_id,
        display.clone(),
        agent_state.to_string(),
    );

    crate::commands::ui_commands::handle_agent_stopped(app_handle).await;
    crate::commands::ui_commands::handle_backend_response(
        app_handle,
        Some(display),
        agent_state.to_string(),
    )
    .await;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(app: Option<&'static str>, action: MediaAction, names_music: bool) -> MediaIntent {
        MediaIntent::Control {
            app,
            action,
            names_music,
        }
    }

    #[test]
    fn explicit_spotify_transport_commands() {
        assert_eq!(
            parse_media_intent("pause spotify"),
            Some(control(Some("Spotify"), MediaAction::Pause, false))
        );
        assert_eq!(
            parse_media_intent("Hey Juno, could you please pause Spotify?"),
            Some(control(Some("Spotify"), MediaAction::Pause, false))
        );
        assert_eq!(
            parse_media_intent("Spotify, next track"),
            Some(control(Some("Spotify"), MediaAction::Next, true))
        );
        assert_eq!(
            parse_media_intent("play the previous song on Spotify"),
            Some(control(Some("Spotify"), MediaAction::Previous, true))
        );
        assert_eq!(
            parse_media_intent("resume spotify"),
            None,
            "bare resume is ambiguous even with the app named; the LLM decides"
        );
        assert_eq!(
            parse_media_intent("play spotify"),
            Some(control(Some("Spotify"), MediaAction::Play, false))
        );
    }

    #[test]
    fn apple_music_aliases() {
        assert_eq!(
            parse_media_intent("pause apple music"),
            Some(control(Some("Music"), MediaAction::Pause, false))
        );
        assert_eq!(
            parse_media_intent("skip this song in iTunes"),
            Some(control(Some("Music"), MediaAction::Next, true))
        );
    }

    #[test]
    fn bare_verbs_and_music_nouns() {
        assert_eq!(
            parse_media_intent("pause"),
            Some(control(None, MediaAction::Pause, false))
        );
        assert_eq!(
            parse_media_intent("Pause the music."),
            Some(control(None, MediaAction::Pause, true))
        );
        assert_eq!(
            parse_media_intent("skip this song"),
            Some(control(None, MediaAction::Next, true))
        );
        assert_eq!(
            parse_media_intent("next"),
            Some(control(None, MediaAction::Next, false))
        );
        assert_eq!(
            parse_media_intent("go back a track"),
            Some(control(None, MediaAction::Previous, true))
        );
        assert_eq!(
            parse_media_intent("stop playing"),
            Some(control(None, MediaAction::Pause, true))
        );
        assert_eq!(
            parse_media_intent("resume playback"),
            Some(control(None, MediaAction::Play, true))
        );
        assert_eq!(
            parse_media_intent("unpause"),
            Some(control(None, MediaAction::Play, false))
        );
    }

    #[test]
    fn status_questions() {
        assert_eq!(
            parse_media_intent("what's playing?"),
            Some(MediaIntent::Status {
                app: None,
                names_music: true
            })
        );
        assert_eq!(
            parse_media_intent("What song is this?"),
            Some(MediaIntent::Status {
                app: None,
                names_music: true
            })
        );
        assert_eq!(
            parse_media_intent("what am I listening to right now"),
            Some(MediaIntent::Status {
                app: None,
                names_music: true
            })
        );
        assert_eq!(
            parse_media_intent("what's on spotify"),
            Some(MediaIntent::Status {
                app: Some("Spotify"),
                names_music: false
            })
        );
        assert_eq!(parse_media_intent("what's on"), None);
        assert_eq!(parse_media_intent("what is this?"), None);
    }

    #[test]
    fn anything_with_content_goes_to_the_agent() {
        for q in [
            "play my liked songs on Spotify",
            "play Midnight by 1991",
            "skip two tracks",
            "pause the video",
            "pause the timer",
            "resume",
            "continue",
            "back",
            "go back",
            "next page",
            "open spotify",
            "turn the volume down",
            "what's the weather",
            "",
            "   ",
        ] {
            assert_eq!(
                parse_media_intent(q),
                None,
                "{:?} should reach the agent",
                q
            );
        }
        // bare "play" parses, but is only served when a player is sitting paused
        assert_eq!(
            parse_media_intent("play"),
            Some(control(None, MediaAction::Play, false))
        );
    }

    fn state(app: &str, running: bool, playback: &str) -> MediaState {
        MediaState {
            app: app.to_string(),
            running,
            state: playback.to_string(),
            track: Some("Houdini".into()),
            artist: Some("Foster The People".into()),
            album: None,
            position_secs: None,
            duration_secs: None,
            artwork_url: None,
        }
    }

    #[test]
    fn implicit_app_follows_what_is_actually_playing() {
        let both = [
            state("Spotify", true, "paused"),
            state("Music", true, "playing"),
        ];
        let pause = control(None, MediaAction::Pause, false);
        assert_eq!(resolve_implicit_app(&pause, &both), Some("Music"));

        let play = control(None, MediaAction::Play, false);
        assert_eq!(resolve_implicit_app(&play, &both), Some("Spotify"));

        let nothing_playing = [
            state("Spotify", true, "paused"),
            state("Music", false, "not_running"),
        ];
        assert_eq!(
            resolve_implicit_app(&pause, &nothing_playing),
            None,
            "a bare 'pause' with nothing playing might be about a video"
        );
        let pause_music = control(None, MediaAction::Pause, true);
        assert_eq!(
            resolve_implicit_app(&pause_music, &nothing_playing),
            Some("Spotify"),
            "'pause the music' names music, so the running player is fine"
        );

        let none_running = [
            state("Spotify", false, "not_running"),
            state("Music", false, "not_running"),
        ];
        assert_eq!(resolve_implicit_app(&pause_music, &none_running), None);
        let status = MediaIntent::Status {
            app: None,
            names_music: true,
        };
        assert_eq!(resolve_implicit_app(&status, &none_running), None);
    }

    #[test]
    fn spoken_confirmation_reflects_real_state() {
        let paused = state("Spotify", true, "paused");
        let playing = state("Spotify", true, "playing");
        let gone = state("Spotify", false, "not_running");

        let pause = control(Some("Spotify"), MediaAction::Pause, false);
        assert_eq!(spoken_result(&pause, "Spotify", &paused), "Paused.");
        assert_eq!(
            spoken_result(&pause, "Spotify", &playing),
            "Spotify didn't pause."
        );
        assert_eq!(
            spoken_result(&pause, "Spotify", &gone),
            "Spotify isn't running."
        );

        let play = control(Some("Spotify"), MediaAction::Play, false);
        assert_eq!(
            spoken_result(&play, "Spotify", &playing),
            "Playing Houdini by Foster The People."
        );

        let next = control(Some("Spotify"), MediaAction::Next, false);
        assert_eq!(
            spoken_result(&next, "Spotify", &playing),
            "Skipped. Now playing Houdini by Foster The People."
        );

        let status = MediaIntent::Status {
            app: Some("Spotify"),
            names_music: false,
        };
        assert_eq!(
            spoken_result(&status, "Spotify", &playing),
            "Houdini by Foster The People."
        );
        assert_eq!(
            spoken_result(&status, "Spotify", &paused),
            "Paused on Houdini by Foster The People."
        );
        assert_eq!(
            spoken_result(&status, "Spotify", &state("Spotify", true, "stopped")),
            "Nothing is playing in Spotify."
        );
    }
}

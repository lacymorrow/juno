//! Live media-player state and control for agent-rendered components.
//!
//! Backs the `<NowPlayingCard>` JSX component: the frontend polls
//! [`media_get_state`] while the card is mounted and calls [`media_control`]
//! when the user presses a transport button. Both talk to the player through
//! its AppleScript dictionary, so they work with the app hidden or behind
//! other windows and never launch an app that is not already running.
//!
//! Why this exists: an agent-rendered play/pause button that only fires a new
//! agent query is fake state. The button cannot know whether playback actually
//! changed, and the round trip takes seconds. Components that show live state
//! must be fed by live state; anything else is worse than plain text.

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Players we know how to talk to. Anything else is rejected up front so an
/// agent-rendered component cannot address arbitrary applications.
pub const SUPPORTED_APPS: &[&str] = &["Spotify", "Music"];

/// Transport actions accepted by [`media_control`].
pub const SUPPORTED_ACTIONS: &[&str] = &["play", "pause", "playpause", "next", "previous"];

/// Field separator used in the AppleScript output. Track metadata can contain
/// almost anything, so use a control character rather than a newline.
const FIELD_SEP: char = '\u{1f}';

/// Snapshot of a media player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaState {
    /// Player name as passed in (`Spotify` or `Music`).
    pub app: String,
    /// Whether the application process is running at all.
    pub running: bool,
    /// `playing`, `paused`, `stopped`, or `not_running`.
    pub state: String,
    pub track: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Playback position in seconds.
    pub position_secs: Option<f64>,
    /// Track length in seconds.
    pub duration_secs: Option<f64>,
    /// Album artwork URL when the player exposes one (Spotify does).
    pub artwork_url: Option<String>,
}

impl MediaState {
    fn not_running(app: &str) -> Self {
        Self {
            app: app.to_string(),
            running: false,
            state: "not_running".to_string(),
            track: None,
            artist: None,
            album: None,
            position_secs: None,
            duration_secs: None,
            artwork_url: None,
        }
    }
}

fn canonical_app(app: &str) -> Result<&'static str, String> {
    SUPPORTED_APPS
        .iter()
        .copied()
        .find(|known| known.eq_ignore_ascii_case(app.trim()))
        .ok_or_else(|| {
            format!(
                "Unsupported media app '{}'. Supported: {}",
                app,
                SUPPORTED_APPS.join(", ")
            )
        })
}

fn canonical_action(action: &str) -> Result<&'static str, String> {
    SUPPORTED_ACTIONS
        .iter()
        .copied()
        .find(|known| known.eq_ignore_ascii_case(action.trim()))
        .ok_or_else(|| {
            format!(
                "Unsupported media action '{}'. Supported: {}",
                action,
                SUPPORTED_ACTIONS.join(", ")
            )
        })
}

/// AppleScript that reports the player state without launching the app.
///
/// Output is `state` alone for stopped/not running, otherwise
/// `state␟track␟artist␟album␟position␟duration␟artwork` separated by
/// [`FIELD_SEP`]. Spotify reports duration in milliseconds and has artwork
/// URLs; Music reports seconds and has no URL, so the script normalises both
/// to seconds and an empty artwork field.
fn state_script(app: &str) -> String {
    let (duration_expr, artwork_expr) = match app {
        "Spotify" => ("((duration of t) / 1000)", "(artwork url of t)"),
        _ => ("(duration of t)", "\"\""),
    };
    format!(
        r#"set sep to character id 31
if application "{app}" is running then
  tell application "{app}"
    set s to (player state as string)
    if s is "stopped" then return s
    try
      set t to current track
      return s & sep & (name of t) & sep & (artist of t) & sep & (album of t) & sep & (player position as string) & sep & ({duration_expr} as string) & sep & {artwork_expr}
    on error
      return s
    end try
  end tell
else
  return "not_running"
end if"#
    )
}

fn control_script(app: &str, action: &str) -> String {
    let verb = match action {
        "play" => "play",
        "pause" => "pause",
        "playpause" => "playpause",
        "next" => "next track",
        "previous" => "previous track",
        _ => unreachable!("action validated by canonical_action"),
    };
    format!(
        r#"if application "{app}" is running then
  tell application "{app}" to {verb}
  return "ok"
else
  return "not_running"
end if"#
    )
}

/// Turn the script output into a [`MediaState`]. Pure so it can be unit
/// tested without a player.
pub fn parse_state_output(app: &str, output: &str) -> MediaState {
    let output = output.trim_end_matches(['\n', '\r']);
    let mut fields = output.split(FIELD_SEP);
    let state = fields.next().unwrap_or("").trim();

    if state.is_empty() || state == "not_running" {
        return MediaState::not_running(app);
    }

    let next_text = |fields: &mut std::str::Split<'_, char>| {
        fields
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let track = next_text(&mut fields);
    let artist = next_text(&mut fields);
    let album = next_text(&mut fields);
    let position_secs = fields
        .next()
        .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok());
    let duration_secs = fields
        .next()
        .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok());
    let artwork_url = next_text(&mut fields);

    MediaState {
        app: app.to_string(),
        running: true,
        state: state.to_string(),
        track,
        artist,
        album,
        position_secs,
        duration_secs,
        artwork_url,
    }
}

#[cfg(target_os = "macos")]
async fn run_osascript(script: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    })
    .await
    .map_err(|e| format!("osascript task failed: {}", e))?
}

#[cfg(not(target_os = "macos"))]
async fn run_osascript(_script: String) -> Result<String, String> {
    Ok("not_running".to_string())
}

async fn fetch_state(app: &'static str) -> Result<MediaState, String> {
    let raw = run_osascript(state_script(app)).await?;
    debug!("[media] {} state: {:?}", app, raw.trim());
    Ok(parse_state_output(app, &raw))
}

/// Read the current state of a media player without launching it.
#[tauri::command]
pub async fn media_get_state(app: String) -> Result<MediaState, String> {
    let app = canonical_app(&app)?;
    fetch_state(app).await
}

/// Send a transport action to a running player and return the state
/// afterwards so the caller can reconcile immediately.
#[tauri::command]
pub async fn media_control(app: String, action: String) -> Result<MediaState, String> {
    let app = canonical_app(&app)?;
    let action = canonical_action(&action)?;

    let result = run_osascript(control_script(app, action)).await?;
    if result.trim() == "not_running" {
        warn!("[media] {} is not running; ignoring '{}'", app, action);
        return Err(format!("{} is not running", app));
    }

    // Players apply transport commands asynchronously; give them a beat so
    // the state we return reflects the action.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    fetch_state(app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(fields: &[&str]) -> String {
        fields.join(&FIELD_SEP.to_string())
    }

    #[test]
    fn parses_playing_spotify_output() {
        let raw = joined(&[
            "playing",
            "Houdini",
            "Foster The People",
            "Supermodel",
            "42.5",
            "203.4",
            "https://i.scdn.co/image/abc",
        ]);
        let state = parse_state_output("Spotify", &format!("{}\n", raw));
        assert!(state.running);
        assert_eq!(state.state, "playing");
        assert_eq!(state.track.as_deref(), Some("Houdini"));
        assert_eq!(state.artist.as_deref(), Some("Foster The People"));
        assert_eq!(state.album.as_deref(), Some("Supermodel"));
        assert_eq!(state.position_secs, Some(42.5));
        assert_eq!(state.duration_secs, Some(203.4));
        assert_eq!(
            state.artwork_url.as_deref(),
            Some("https://i.scdn.co/image/abc")
        );
    }

    #[test]
    fn parses_music_output_without_artwork() {
        let raw = joined(&["paused", "Track", "Artist", "Album", "1,5", "180", ""]);
        let state = parse_state_output("Music", &raw);
        assert_eq!(state.state, "paused");
        assert_eq!(state.position_secs, Some(1.5), "locale decimal comma");
        assert_eq!(state.duration_secs, Some(180.0));
        assert_eq!(state.artwork_url, None);
    }

    #[test]
    fn stopped_has_no_track() {
        let state = parse_state_output("Spotify", "stopped\n");
        assert!(state.running);
        assert_eq!(state.state, "stopped");
        assert_eq!(state.track, None);
    }

    #[test]
    fn not_running_and_empty_output() {
        assert_eq!(
            parse_state_output("Spotify", "not_running\n"),
            MediaState::not_running("Spotify")
        );
        assert_eq!(
            parse_state_output("Spotify", ""),
            MediaState::not_running("Spotify")
        );
    }

    #[test]
    fn rejects_unknown_apps_and_actions() {
        assert!(canonical_app("Finder").is_err());
        assert_eq!(canonical_app("spotify"), Ok("Spotify"));
        assert!(canonical_action("stop").is_err());
        assert_eq!(canonical_action("PlayPause"), Ok("playpause"));
    }

    #[test]
    fn scripts_never_launch_a_closed_app() {
        for app in SUPPORTED_APPS {
            assert!(state_script(app).starts_with("set sep to character id 31\nif application"));
            assert!(control_script(app, "next").starts_with("if application"));
        }
        assert!(control_script("Spotify", "previous").contains("previous track"));
        assert!(state_script("Spotify").contains("/ 1000"));
        assert!(!state_script("Music").contains("artwork url"));
    }
}

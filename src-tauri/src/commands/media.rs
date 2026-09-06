//! Live media-player state and control for agent-rendered components.
//!
//! Backs the `<NowPlayingCard>` JSX component: the frontend polls
//! [`media_get_state`] while the card is mounted and calls [`media_control`]
//! when the user presses a transport button. Both talk to the player through
//! its AppleScript dictionary, so they work with the app hidden or behind
//! other windows and never launch an app that is not already running.
//!
//! Spotify and Apple Music are at parity: Spotify hands us an artwork URL,
//! Music only exposes the artwork bytes (`raw data of artwork 1 of current
//! track`), so those are exported once per track into the app cache dir and
//! served back to the webview through Tauri's asset protocol (see
//! `app.security.assetProtocol` in `tauri.conf.json`).
//!
//! Why this exists: an agent-rendered play/pause button that only fires a new
//! agent query is fake state. The button cannot know whether playback actually
//! changed, and the round trip takes seconds. Components that show live state
//! must be fed by live state; anything else is worse than plain text.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, warn};

/// Players we know how to talk to, by AppleScript application name. Anything
/// else is rejected up front so an agent-rendered component cannot address
/// arbitrary applications.
pub const SUPPORTED_APPS: &[&str] = &["Spotify", "Music"];

/// Transport actions accepted by [`media_control`].
pub const SUPPORTED_ACTIONS: &[&str] = &["play", "pause", "playpause", "next", "previous"];

/// Sub-directory of the app cache dir where exported artwork lives. Must match
/// the `assetProtocol.scope` entry in `tauri.conf.json`.
pub const ARTWORK_CACHE_DIR: &str = "media-artwork";

/// Most artwork files kept before the oldest are pruned.
const ARTWORK_CACHE_MAX_FILES: usize = 32;

/// How long a failed export is remembered before it is retried, so a track
/// without artwork does not cost an extra AppleScript call every poll.
const ARTWORK_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Field separator used in the AppleScript output. Track metadata can contain
/// almost anything, so use a control character rather than a newline.
const FIELD_SEP: char = '\u{1f}';

/// User-facing name for a supported app. The AppleScript name of Apple Music
/// is just "Music", which reads wrong in spoken replies and on the card.
pub fn display_name(app: &str) -> &str {
    match app {
        "Music" => "Apple Music",
        other => other,
    }
}

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
    /// Album artwork the webview can load: Spotify's CDN URL, or an
    /// `asset://` URL for artwork exported from Music. `None` when the player
    /// has none for this track or the export failed.
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

/// Where a player without artwork URLs keeps the current track's artwork.
/// Music reports these alongside the track so the bytes can be exported once
/// per track instead of once per poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkHint {
    /// `persistent ID of current track` — a hex string that is stable across
    /// polls and app restarts.
    pub persistent_id: String,
    /// `format of artwork 1` as text, e.g. "JPEG picture" or "PNG picture".
    pub format: String,
}

/// What the state script reports, before artwork resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedState {
    pub state: MediaState,
    pub artwork: Option<ArtworkHint>,
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

/// Quote `s` as an AppleScript string literal.
fn applescript_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// AppleScript that reports the player state without launching the app.
///
/// Output is `state` alone for stopped/not running, otherwise
/// `state␟track␟artist␟album␟position␟duration␟artwork␟persistent_id␟format`
/// separated by [`FIELD_SEP`]. Spotify reports duration in milliseconds and
/// has artwork URLs; Music reports seconds and has no URL but does expose the
/// track's persistent ID and the artwork format, which [`fetch_state`] uses
/// to export the artwork bytes. Both are normalised to seconds and unused
/// fields are left empty.
fn state_script(app: &str) -> String {
    let (duration_expr, artwork_expr, hint_stmts) = match app {
        "Spotify" => (
            "((duration of t) / 1000)",
            "(artwork url of t)",
            "      set pid to \"\"\n      set fmt to \"\"\n",
        ),
        _ => (
            "(duration of t)",
            "\"\"",
            r#"      set pid to ""
      set fmt to ""
      try
        if (count of artworks of t) > 0 then
          set pid to (persistent ID of t)
          set fmt to ((format of artwork 1 of t) as string)
        end if
      end try
"#,
        ),
    };
    format!(
        r#"set sep to character id 31
if application "{app}" is running then
  tell application "{app}"
    set s to (player state as string)
    if s is "stopped" then return s
    try
      set t to current track
{hint_stmts}      return s & sep & (name of t) & sep & (artist of t) & sep & (album of t) & sep & (player position as string) & sep & ({duration_expr} as string) & sep & {artwork_expr} & sep & pid & sep & fmt
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

/// AppleScript that writes the current Music track's first artwork to `path`.
///
/// Returns `ok`, `none` (track has no artwork), `changed` (a different track
/// is current now, so the bytes would be filed under the wrong name) or
/// `not_running`. Never launches Music.
fn artwork_export_script(persistent_id: &str, path: &Path) -> String {
    format!(
        r#"if application "Music" is running then
  set p to POSIX file {path}
  tell application "Music"
    set t to current track
    if (persistent ID of t) is not {pid} then return "changed"
    if (count of artworks of t) is 0 then return "none"
    set d to raw data of artwork 1 of t
  end tell
  set f to open for access p with write permission
  try
    set eof f to 0
    write d to f
    close access f
  on error e
    close access f
    error e
  end try
  return "ok"
else
  return "not_running"
end if"#,
        path = applescript_string(&path.to_string_lossy()),
        pid = applescript_string(persistent_id),
    )
}

/// Turn the script output into a [`ParsedState`]. Pure so it can be unit
/// tested without a player.
pub fn parse_state_output(app: &str, output: &str) -> ParsedState {
    let output = output.trim_end_matches(['\n', '\r']);
    let mut fields = output.split(FIELD_SEP);
    let state = fields.next().unwrap_or("").trim();

    if state.is_empty() || state == "not_running" {
        return ParsedState {
            state: MediaState::not_running(app),
            artwork: None,
        };
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
    let persistent_id = next_text(&mut fields);
    let format = next_text(&mut fields);

    ParsedState {
        state: MediaState {
            app: app.to_string(),
            running: true,
            state: state.to_string(),
            track,
            artist,
            album,
            position_secs,
            duration_secs,
            artwork_url,
        },
        artwork: persistent_id.map(|persistent_id| ArtworkHint {
            persistent_id,
            format: format.unwrap_or_default(),
        }),
    }
}

/// File extension for a Music artwork `format` string.
fn artwork_extension(format: &str) -> &'static str {
    let format = format.to_ascii_lowercase();
    if format.contains("png") {
        "png"
    } else if format.contains("gif") {
        "gif"
    } else if format.contains("bmp") {
        "bmp"
    } else if format.contains("tiff") {
        "tiff"
    } else {
        "jpg"
    }
}

/// Cache file name for a track's artwork: a hash of the track identity, so
/// the same track maps to the same file on every poll and is exported once.
pub fn artwork_file_name(
    app: &str,
    hint: &ArtworkHint,
    track: Option<&str>,
    album: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        app,
        hint.persistent_id.as_str(),
        track.unwrap_or(""),
        album.unwrap_or(""),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    let digest = hasher.finalize();
    let hex: String = digest
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect();
    format!("{}.{}", hex, artwork_extension(&hint.format))
}

/// Characters `encodeURIComponent` leaves alone, so the URL is byte-for-byte
/// what Tauri's `convertFileSrc` would produce for the same path.
const ASSET_PATH_ENCODE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// Asset-protocol URL for a file the webview may load. The path must sit
/// inside the `assetProtocol.scope` or the webview gets a 403.
pub fn asset_url(path: &Path) -> String {
    let encoded = utf8_percent_encode(&path.to_string_lossy(), ASSET_PATH_ENCODE).to_string();
    if cfg!(windows) {
        format!("http://asset.localhost/{}", encoded)
    } else {
        format!("asset://localhost/{}", encoded)
    }
}

/// Delete the oldest files in `dir` beyond [`ARTWORK_CACHE_MAX_FILES`].
fn prune_artwork_cache(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((meta.modified().ok()?, entry.path()))
        })
        .collect();
    if files.len() <= ARTWORK_CACHE_MAX_FILES {
        return;
    }
    files.sort();
    for (_, path) in files.iter().take(files.len() - ARTWORK_CACHE_MAX_FILES) {
        if let Err(e) = std::fs::remove_file(path) {
            debug!("[media] could not prune {}: {}", path.display(), e);
        }
    }
}

struct ArtworkEntry {
    url: Option<String>,
    checked_at: Instant,
}

/// Artwork resolution results by cache file name. Holding this lock across an
/// export also serialises exports, so two overlapping polls cannot write the
/// same file twice.
static ARTWORK_CACHE: Lazy<TokioMutex<HashMap<String, ArtworkEntry>>> =
    Lazy::new(|| TokioMutex::new(HashMap::new()));

fn artwork_cache_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_cache_dir()
        .map(|dir| dir.join(ARTWORK_CACHE_DIR))
        .map_err(|e| format!("No app cache dir: {}", e))
}

fn file_is_nonempty(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Export the current Music track's artwork to `path` via a temp file, so a
/// half-written file is never served.
async fn export_artwork(dir: &Path, path: &Path, persistent_id: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Could not create {}: {}", dir.display(), e))?;
    let tmp = path.with_extension("tmp");
    let result = run_osascript(artwork_export_script(persistent_id, &tmp)).await;
    match result {
        Ok(output) if output.trim() == "ok" && file_is_nonempty(&tmp) => {
            std::fs::rename(&tmp, path)
                .map_err(|e| format!("Could not move artwork into place: {}", e))?;
            prune_artwork_cache(dir);
            Ok(())
        }
        Ok(other) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("artwork export returned '{}'", other.trim()))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// How long to wait after a transport command before reading state back.
/// Measured against the real apps: Spotify reflects a command within
/// ~100 ms; Music.app takes up to ~300 ms for `next track` and ~200 ms for
/// `pause`, so it gets more headroom. The card's 1 s poll reconciles any
/// straggler either way.
fn settle_delay(app: &str) -> Duration {
    match app {
        "Music" => Duration::from_millis(500),
        _ => Duration::from_millis(150),
    }
}

/// Resolve an artwork URL for a track the player only exposes bytes for.
/// Cached per track, so the per-second poll costs nothing after the first
/// export, and a track without artwork is only retried every
/// [`ARTWORK_RETRY_AFTER`].
async fn resolve_artwork(
    app_handle: &AppHandle,
    app: &str,
    state: &MediaState,
    hint: &ArtworkHint,
) -> Option<String> {
    let dir = match artwork_cache_dir(app_handle) {
        Ok(dir) => dir,
        Err(e) => {
            warn!("[media] artwork disabled: {}", e);
            return None;
        }
    };
    let file_name = artwork_file_name(app, hint, state.track.as_deref(), state.album.as_deref());
    let path = dir.join(&file_name);

    let mut cache = ARTWORK_CACHE.lock().await;
    if let Some(entry) = cache.get(&file_name) {
        match &entry.url {
            Some(url) if path.is_file() => return Some(url.clone()),
            // The file was pruned or removed; export it again below.
            Some(_) => {}
            None if entry.checked_at.elapsed() < ARTWORK_RETRY_AFTER => return None,
            None => {}
        }
    }

    let url = if file_is_nonempty(&path) {
        Some(asset_url(&path))
    } else {
        match export_artwork(&dir, &path, &hint.persistent_id).await {
            Ok(()) => {
                debug!("[media] exported {} artwork to {}", app, path.display());
                Some(asset_url(&path))
            }
            Err(e) => {
                debug!("[media] no artwork for {} track: {}", app, e);
                None
            }
        }
    };
    cache.insert(
        file_name,
        ArtworkEntry {
            url: url.clone(),
            checked_at: Instant::now(),
        },
    );
    url
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

async fn fetch_state(app_handle: &AppHandle, app: &'static str) -> Result<MediaState, String> {
    let raw = run_osascript(state_script(app)).await?;
    debug!("[media] {} state: {:?}", app, raw.trim());
    let ParsedState { mut state, artwork } = parse_state_output(app, &raw);
    if state.artwork_url.is_none() {
        if let Some(hint) = artwork {
            state.artwork_url = resolve_artwork(app_handle, app, &state, &hint).await;
        }
    }
    Ok(state)
}

/// Read the current state of a media player without launching it.
pub async fn get_state(app_handle: &AppHandle, app: &str) -> Result<MediaState, String> {
    let app = canonical_app(app)?;
    fetch_state(app_handle, app).await
}

/// Send a transport action to a running player and return the state
/// afterwards so the caller can reconcile immediately.
pub async fn control(
    app_handle: &AppHandle,
    app: &str,
    action: &str,
) -> Result<MediaState, String> {
    let app = canonical_app(app)?;
    let action = canonical_action(action)?;

    let result = run_osascript(control_script(app, action)).await?;
    if result.trim() == "not_running" {
        warn!("[media] {} is not running; ignoring '{}'", app, action);
        return Err(format!("{} is not running", display_name(app)));
    }

    // Players apply transport commands asynchronously; give them a beat so
    // the state we return reflects the action.
    tokio::time::sleep(settle_delay(app)).await;
    fetch_state(app_handle, app).await
}

/// Tauri command: read the current state of a media player without
/// launching it.
#[tauri::command]
pub async fn media_get_state(app_handle: AppHandle, app: String) -> Result<MediaState, String> {
    get_state(&app_handle, &app).await
}

/// Tauri command: send a transport action to a running player and return the
/// state afterwards.
#[tauri::command]
pub async fn media_control(
    app_handle: AppHandle,
    app: String,
    action: String,
) -> Result<MediaState, String> {
    control(&app_handle, &app, &action).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(fields: &[&str]) -> String {
        fields.join(&FIELD_SEP.to_string())
    }

    fn hint(id: &str, format: &str) -> ArtworkHint {
        ArtworkHint {
            persistent_id: id.to_string(),
            format: format.to_string(),
        }
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
            "",
            "",
        ]);
        let parsed = parse_state_output("Spotify", &format!("{}\n", raw));
        let state = parsed.state;
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
        assert_eq!(parsed.artwork, None, "Spotify never needs an export");
    }

    #[test]
    fn parses_music_output_with_artwork_hint() {
        // Verified against Music.app on macOS 26: position and duration are
        // seconds, persistent ID is 16 hex chars, format reads "JPEG picture".
        let raw = joined(&[
            "playing",
            "All I Really Want",
            "Alanis Morissette",
            "Jagged Little Pill (2015 Remaster)",
            "0.374000012875",
            "283.278991699219",
            "",
            "AD01FDA089E28107",
            "JPEG picture",
        ]);
        let parsed = parse_state_output("Music", &raw);
        assert_eq!(parsed.state.duration_secs, Some(283.278991699219));
        assert_eq!(
            parsed.state.artwork_url, None,
            "resolved later, not by the parser"
        );
        assert_eq!(
            parsed.artwork,
            Some(hint("AD01FDA089E28107", "JPEG picture"))
        );
    }

    #[test]
    fn parses_music_output_without_artwork() {
        let raw = joined(&[
            "paused", "Track", "Artist", "Album", "1,5", "180", "", "", "",
        ]);
        let parsed = parse_state_output("Music", &raw);
        assert_eq!(parsed.state.state, "paused");
        assert_eq!(
            parsed.state.position_secs,
            Some(1.5),
            "locale decimal comma"
        );
        assert_eq!(parsed.state.duration_secs, Some(180.0));
        assert_eq!(parsed.state.artwork_url, None);
        assert_eq!(parsed.artwork, None);

        // Older script output without the trailing hint fields still parses.
        let short = joined(&["paused", "Track", "Artist", "Album", "1.5", "180", ""]);
        assert_eq!(parse_state_output("Music", &short).artwork, None);
    }

    #[test]
    fn stopped_has_no_track() {
        let parsed = parse_state_output("Spotify", "stopped\n");
        assert!(parsed.state.running);
        assert_eq!(parsed.state.state, "stopped");
        assert_eq!(parsed.state.track, None);
        assert_eq!(parsed.artwork, None);
    }

    #[test]
    fn not_running_and_empty_output() {
        assert_eq!(
            parse_state_output("Spotify", "not_running\n").state,
            MediaState::not_running("Spotify")
        );
        assert_eq!(
            parse_state_output("Music", "").state,
            MediaState::not_running("Music")
        );
    }

    #[test]
    fn rejects_unknown_apps_and_actions() {
        assert!(canonical_app("Finder").is_err());
        assert_eq!(canonical_app("spotify"), Ok("Spotify"));
        assert_eq!(canonical_app("music"), Ok("Music"));
        assert!(canonical_action("stop").is_err());
        assert_eq!(canonical_action("PlayPause"), Ok("playpause"));
    }

    #[test]
    fn music_gets_a_longer_settle_than_spotify() {
        assert!(settle_delay("Music") > settle_delay("Spotify"));
        assert!(settle_delay("Music") >= Duration::from_millis(300));
        assert!(settle_delay("Spotify") < Duration::from_secs(1));
    }

    #[test]
    fn display_names_are_user_facing() {
        assert_eq!(display_name("Music"), "Apple Music");
        assert_eq!(display_name("Spotify"), "Spotify");
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
        assert!(artwork_export_script("ABC", Path::new("/tmp/x.tmp"))
            .starts_with("if application \"Music\" is running then"));
    }

    #[test]
    fn music_state_script_reports_persistent_id_and_format() {
        let script = state_script("Music");
        assert!(script.contains("persistent ID of t"));
        assert!(script.contains("format of artwork 1 of t"));
        assert!(script.contains("count of artworks of t"));
        assert!(!state_script("Spotify").contains("persistent ID"));
    }

    #[test]
    fn export_script_quotes_path_and_id() {
        let script = artwork_export_script(
            "AD01FDA089E28107",
            Path::new("/Users/me/Library/Caches/ai.junebug/media-artwork/ab\"c.tmp"),
        );
        assert!(script.contains(
            r#"set p to POSIX file "/Users/me/Library/Caches/ai.junebug/media-artwork/ab\"c.tmp""#
        ));
        assert!(script.contains(r#"is not "AD01FDA089E28107" then return "changed""#));
        assert!(script.contains("raw data of artwork 1 of t"));
        assert!(script.contains("write d to f"));
    }

    #[test]
    fn applescript_string_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(applescript_string(""), "\"\"");
    }

    #[test]
    fn artwork_file_name_is_stable_and_track_specific() {
        let jpeg = hint("AD01", "JPEG picture");
        let a = artwork_file_name("Music", &jpeg, Some("T"), Some("A"));
        let same = artwork_file_name("Music", &jpeg, Some("T"), Some("A"));
        let other_track =
            artwork_file_name("Music", &hint("AD02", "JPEG picture"), Some("T"), Some("A"));
        let other_album = artwork_file_name("Music", &jpeg, Some("T"), Some("B"));
        assert_eq!(a, same);
        assert_ne!(a, other_track);
        assert_ne!(a, other_album);
        assert!(a.ends_with(".jpg"), "{}", a);
        assert_eq!(a.len(), 16 + 4, "16 hex chars + extension");
        assert!(a
            .trim_end_matches(".jpg")
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn artwork_extension_follows_format() {
        assert!(
            artwork_file_name("Music", &hint("1", "PNG picture"), None, None).ends_with(".png")
        );
        assert!(
            artwork_file_name("Music", &hint("1", "GIF picture"), None, None).ends_with(".gif")
        );
        assert!(artwork_file_name("Music", &hint("1", ""), None, None).ends_with(".jpg"));
    }

    #[test]
    fn asset_url_matches_convert_file_src() {
        let url = asset_url(Path::new(
            "/Users/me/Library/Caches/ai.junebug/media-artwork/0123abcd.jpg",
        ));
        assert_eq!(
            url,
            "asset://localhost/%2FUsers%2Fme%2FLibrary%2FCaches%2Fai.junebug%2Fmedia-artwork%2F0123abcd.jpg"
        );
        // encodeURIComponent keeps - _ . ! ~ * ' ( ) and encodes spaces.
        assert_eq!(
            asset_url(Path::new("/a b/c-d_e.f!g~h*i'j(k)l")),
            "asset://localhost/%2Fa%20b%2Fc-d_e.f!g~h*i'j(k)l"
        );
    }

    #[test]
    fn prune_keeps_the_newest_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dir = dir.path();
        let total = ARTWORK_CACHE_MAX_FILES + 3;
        for i in 0..total {
            let path = dir.join(format!("{:03}.jpg", i));
            std::fs::write(&path, b"x").expect("write");
            let mtime = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 + i as u64);
            let file = std::fs::File::open(&path).expect("open");
            file.set_modified(mtime).expect("set mtime");
        }
        prune_artwork_cache(dir);
        let mut left: Vec<String> = std::fs::read_dir(dir)
            .expect("read_dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left.len(), ARTWORK_CACHE_MAX_FILES);
        assert_eq!(left.first().map(String::as_str), Some("003.jpg"));
        assert_eq!(
            left.last().map(String::as_str),
            Some(format!("{:03}.jpg", total - 1).as_str())
        );
    }
}

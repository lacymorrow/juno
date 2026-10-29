//! Juno's interface sounds.
//!
//! Three cues, and nothing else makes a sound: the spoken reply is how Juno
//! says a turn went well, and the bar says in words what words can say.
//!
//! | Cue | When |
//! |---|---|
//! | `Listening` | the microphone is live (press, wake phrase, mic button) |
//! | `GotIt` | a voice session is released to be sent or typed |
//! | `Failed` | something the person asked for did not happen |
//!
//! Silent on purpose: launch, the person stopping Juno (Escape, Cancel), a
//! finished answer, and a lost connection (the reply says so out loud).
//! A new sound needs a moment the person cannot see; add it here or not at all.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex as StdMutex;
use tauri::{AppHandle, Manager};
use tracing::{debug, error, info};

/// Process-wide cache of resolved sound-file paths.
///
/// Resolving a clip means probing ~10 candidate locations with `exists()` —
/// cheap once, but wasteful to repeat on every keypress for the same cue. The
/// resolved absolute path never changes for the life of the process, so cache
/// it the first time and skip the filesystem probing thereafter.
static SOUND_PATH_CACHE: once_cell::sync::Lazy<StdMutex<HashMap<String, PathBuf>>> =
    once_cell::sync::Lazy::new(|| StdMutex::new(HashMap::new()));

/// Every sound Juno makes. See the module docs for when each one plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// The microphone is live. Short, so it is over before the first word.
    Listening,
    /// A voice session was released. The falling half of `Listening`.
    GotIt,
    /// Something the person asked for did not happen.
    Failed,
}

impl Cue {
    /// The clip's resource-relative path for this platform.
    pub fn file_path(self) -> String {
        let name = match self {
            Cue::Listening => "listening",
            Cue::GotIt => "got-it",
            Cue::Failed => "failed",
        };
        if cfg!(target_os = "macos") {
            format!("sounds/caf/{}.caf", name)
        } else {
            format!("sounds/ogg/{}.ogg", name)
        }
    }
}

/// Resolve a sound file's on-disk path, caching the result.
///
/// `file_path` is the resource-relative path (e.g. `sounds/caf/...`). The first
/// call for a given clip probes the bundled-resource, development, and
/// absolute-path candidates; every later call for the same clip returns the
/// cached `PathBuf` without touching the filesystem.
fn resolve_sound_path(app: &AppHandle, file_path: &str) -> Option<PathBuf> {
    if let Ok(cache) = SOUND_PATH_CACHE.lock() {
        if let Some(path) = cache.get(file_path) {
            debug!("Sound path cache hit for {}: {:?}", file_path, path);
            return Some(path.clone());
        }
    }

    // Strategy 1: Try bundled resources (production builds)
    let mut final_path = None;

    if let Ok(resource_path) = app.path().resource_dir() {
        info!("Resource directory: {:?}", resource_path);

        // Try multiple possible paths in the bundled resources
        let possible_paths = [
            // Primary bundled path in production builds (_up_ directory)
            resource_path.join("_up_").join("public").join(file_path), // resources/_up_/public/sounds/caf/...
            resource_path.join("_up_").join(file_path), // resources/_up_/sounds/caf/...
            // Additional paths for development and production compatibility
            resource_path.join(file_path), // Direct path: resources/sounds/caf/...
            resource_path.join("sounds").join(file_path), // With sounds prefix: resources/sounds/sounds/caf/...
            if let Some(stripped) = file_path.strip_prefix("sounds/") {
                resource_path.join(stripped) // Remove "sounds/" prefix: resources/caf/...
            } else {
                resource_path.join(file_path)
            },
            // Try with different sound directory structures
            resource_path.join("public").join(file_path), // resources/public/sounds/caf/...
            resource_path.join("dist").join(file_path),   // resources/dist/sounds/caf/...
        ];

        for test_path in possible_paths.iter() {
            if test_path.exists() {
                info!("Found sound in bundled resources: {:?}", test_path);
                final_path = Some(test_path.clone());
                break;
            } else {
                info!("Checked bundled path (not found): {:?}", test_path);
            }
        }
    }

    // Strategy 2: Try development mode paths (when bundled resources not available)
    if final_path.is_none() && cfg!(debug_assertions) {
        info!("Bundled resources not found, trying development paths...");

        // Try relative to current working directory (development mode)
        if let Ok(cwd) = std::env::current_dir() {
            let mut dev_paths = vec![cwd.join("public").join(file_path), cwd.join(file_path)];

            // Try going up one directory level if we're in src-tauri
            if let Some(parent) = cwd.parent() {
                dev_paths.push(parent.join("public").join(file_path));
            }

            for dev_path in dev_paths.iter() {
                info!("Checking development path: {:?}", dev_path);
                if dev_path.exists() {
                    info!("Found sound in development path: {:?}", dev_path);
                    final_path = Some(dev_path.clone());
                    break;
                }
            }
        }
    }

    // Strategy 3: Final fallback - try absolute path
    if final_path.is_none() {
        let fallback_path = std::path::PathBuf::from(file_path);
        if fallback_path.exists() {
            info!("Found sound at absolute path: {:?}", fallback_path);
            final_path = Some(fallback_path);
        }
    }

    if let Some(ref path) = final_path {
        if let Ok(mut cache) = SOUND_PATH_CACHE.lock() {
            cache.insert(file_path.to_string(), path.clone());
        }
    }

    final_path
}

/// Play a cue with the lowest possible latency.
///
/// A plain synchronous function meant to be called directly on the key-event
/// thread. It reads the sound setting from in-memory state, resolves the clip
/// path from the process-wide cache (no per-press filesystem probing after the
/// first call), and spawns the player on a detached thread. It returns in
/// microseconds so the gesture that triggered it — a dictation start/stop,
/// say — is never held up waiting on audio init or speech-to-text
/// finalization.
pub fn play_cue(app: &AppHandle, cue: Cue) {
    // Respect the user's sound toggle — a cheap in-memory read. Default to
    // playing if state is somehow unavailable.
    let enabled = app
        .try_state::<crate::state::AppState>()
        .and_then(|s| s.get_sound_enabled().ok())
        .unwrap_or(true);
    if !enabled {
        return;
    }

    let file_path = cue.file_path();
    let Some(path) = resolve_sound_path(app, &file_path) else {
        error!("[Cue] Sound file not found: {}", file_path);
        return;
    };

    std::thread::spawn(move || {
        if let Err(e) = play_audio_file(&path) {
            error!("[Cue] Failed to play {:?} ({:?}): {}", cue, path, e);
        }
    });
}

/// The page's one sound: a typed message that did not go through.
#[tauri::command]
pub async fn play_error_sound(app: AppHandle) -> Result<(), String> {
    play_cue(&app, Cue::Failed);
    Ok(())
}

/// Command to set sound enabled/disabled
#[tauri::command]
pub async fn set_sound_enabled(
    app_handle: AppHandle,
    enabled: bool,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<(), String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.sound_enabled = enabled;

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update state for backward compatibility
    state
        .set_sound_enabled(enabled)
        .map_err(|e| format!("Failed to set sound_enabled: {}", e))?;

    info!("Sound effects set to: {}", enabled);
    Ok(())
}

/// Command to get current sound enabled setting
#[tauri::command]
pub async fn get_sound_enabled(
    app_handle: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<bool, String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    // Sync with state for backward compatibility
    state
        .set_sound_enabled(audio_settings.sound_enabled)
        .map_err(|e| format!("Failed to set sound_enabled: {}", e))?;

    info!(
        "Current sound enabled setting: {}",
        audio_settings.sound_enabled
    );
    Ok(audio_settings.sound_enabled)
}

// Platform-specific audio playback
#[cfg(target_os = "macos")]
fn play_audio_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    let output = Command::new("afplay").arg(path).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("afplay failed: {}", stderr).into());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn play_audio_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    let path_str = path.to_string_lossy();
    let output = Command::new("powershell")
        .args(&[
            "-c",
            &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", path_str),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell audio playback failed: {}", stderr).into());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn play_audio_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::process::Command;

    // Try different audio players commonly available on Linux
    let players = ["paplay", "aplay", "mpg123", "ffplay"];

    for player in &players {
        if Command::new("which").arg(player).output().is_ok() {
            let output = Command::new(player).arg(path).output()?;

            if output.status.success() {
                return Ok(());
            }
        }
    }

    Err("No suitable audio player found on Linux".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_has_a_bundled_clip() {
        let public = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|root| root.join("public"))
            .unwrap_or_default();
        for cue in [Cue::Listening, Cue::GotIt, Cue::Failed] {
            let path = public.join(cue.file_path());
            assert!(path.exists(), "{:?} has no clip at {:?}", cue, path);
        }
    }

    #[test]
    fn cue_paths_use_the_platform_format() {
        let path = Cue::GotIt.file_path();
        #[cfg(target_os = "macos")]
        assert_eq!(path, "sounds/caf/got-it.caf");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(path, "sounds/ogg/got-it.ogg");
    }
}

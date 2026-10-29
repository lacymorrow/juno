//! Juno's interface sounds.
//!
//! Three moments make a sound, and nothing else does: the spoken reply is how
//! Juno says a turn went well, and the bar says in words what words can say.
//!
//! | Cue | When |
//! |---|---|
//! | `Listening(target)` | the gesture that opens the microphone: a tap, a hold crossing its threshold, the mic button, the wake phrase |
//! | `GotIt(target)` | the gesture that ends a session to be sent or typed |
//! | `Failed` | something the person asked for did not happen |
//!
//! Dictation and Talk to Juno sound different, so the ear knows which one it
//! just started: dictation is a short tick, Juno is a two-note chime.
//!
//! A cue belongs to the gesture, not to the work behind it. It plays on the
//! key edge (or the hold threshold) and never waits on the microphone or on
//! speech-to-text, because a sound that lands late does not sound like it
//! came from the key.
//!
//! Silent on purpose: launch, the person stopping Juno (Escape, Cancel), a
//! finished answer, and a lost connection (the reply says so out loud).
//! A new sound needs a moment the person cannot see; add it here or not at all.

use crate::state::VoiceTarget;
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
    /// A voice session was started. Short, so it is over before the first word.
    Listening(VoiceTarget),
    /// A voice session was released. The falling half of `Listening`.
    GotIt(VoiceTarget),
    /// Something the person asked for did not happen.
    Failed,
}

impl Cue {
    /// Every cue, for priming and for the bundled-clip test.
    pub const ALL: [Cue; 5] = [
        Cue::Listening(VoiceTarget::Agent),
        Cue::GotIt(VoiceTarget::Agent),
        Cue::Listening(VoiceTarget::Dictation),
        Cue::GotIt(VoiceTarget::Dictation),
        Cue::Failed,
    ];

    /// The clip's resource-relative path for this platform.
    pub fn file_path(self) -> String {
        let name = match self {
            Cue::Listening(VoiceTarget::Agent) => "listening",
            Cue::GotIt(VoiceTarget::Agent) => "got-it",
            Cue::Listening(VoiceTarget::Dictation) => "dictation-start",
            Cue::GotIt(VoiceTarget::Dictation) => "dictation-end",
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

/// Play a cue now.
///
/// Returns at once, so the gesture that triggered it is never held up. On
/// macOS the clip is already decoded in memory (see [`prime`]) and plays
/// in-process on the main thread, a few milliseconds after the key. It used to
/// start an `afplay` process per cue, and the process launch alone put the
/// sound a few hundred milliseconds behind the key.
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

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = app.run_on_main_thread(move || {
            if !native::play(&path) {
                error!("[Cue] Failed to play {:?} ({:?})", cue, path);
            }
        }) {
            error!("[Cue] Could not reach the main thread for {:?}: {}", cue, e);
        }
    }

    #[cfg(not(target_os = "macos"))]
    std::thread::spawn(move || {
        if let Err(e) = play_audio_file(&path) {
            error!("[Cue] Failed to play {:?} ({:?}): {}", cue, path, e);
        }
    });
}

/// The start cue for a session that was just opened, if its gesture is one
/// that is already decided by now.
///
/// A tap, a click on the mic, or a modifier tap is a start the moment it
/// lands, and the start handler runs on that edge, before the microphone
/// opens. A held key is not: its start opens the microphone on the press, but
/// a press shorter than the hold threshold is a tap that will be thrown away,
/// so its cue waits for [`play_hold_cue`]. A wake phrase chimed when it was
/// heard.
pub fn play_start_cue(app: &AppHandle, session: crate::state::VoiceSession) {
    use crate::state::VoiceStartMethod;
    match session.method {
        VoiceStartMethod::PushToTalk | VoiceStartMethod::WakePhrase => {}
        _ => play_cue(app, Cue::Listening(session.target)),
    }
}

/// The start cue for a hold that just crossed its threshold. Only when the
/// open session is that hold's: a press whose start was refused (the
/// microphone already belongs to the other target) started nothing to hear.
pub fn play_hold_cue(app: &AppHandle, target: VoiceTarget) {
    let held = app
        .try_state::<crate::state::AppState>()
        .and_then(|state| state.current_voice_session())
        .is_some_and(|session| {
            session.target == target && session.method == crate::state::VoiceStartMethod::PushToTalk
        });
    if held {
        play_cue(app, Cue::Listening(target));
    }
}

/// Load every cue before the first one is needed, so the first press of the
/// session is as quick as the hundredth. Main thread only; called from
/// `apply_macos_setup`.
#[cfg(target_os = "macos")]
pub fn prime(app: &AppHandle) {
    for cue in Cue::ALL {
        if let Some(path) = resolve_sound_path(app, &cue.file_path()) {
            native::load(&path);
        }
    }
}

/// In-process playback through `NSSound`, one loaded instance per clip.
///
/// AppKit, so main thread only: the instances live in a thread-local that only
/// the main thread ever fills, and [`play_cue`] hops there with
/// `run_on_main_thread` (async, never `dispatch_sync`).
#[cfg(target_os = "macos")]
mod native {
    use cocoa::base::{id, nil, BOOL, YES};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    thread_local! {
        /// Retained for the life of the process: five short clips.
        static LOADED: RefCell<HashMap<PathBuf, id>> = RefCell::new(HashMap::new());
    }

    /// The loaded sound for `path`, loading it the first time. `nil` if the
    /// file could not be read as audio.
    pub fn load(path: &Path) -> id {
        LOADED.with(|loaded| {
            let mut loaded = loaded.borrow_mut();
            if let Some(&sound) = loaded.get(path) {
                return sound;
            }
            // SAFETY: plain AppKit calls on the main thread with a valid
            // NSString; the alloc/init pair is balanced by keeping the
            // instance for the life of the process.
            let sound: id = unsafe {
                let ns_path = NSString::alloc(nil).init_str(&path.to_string_lossy());
                let sound: id = msg_send![class!(NSSound), alloc];
                let sound: id = msg_send![sound, initWithContentsOfFile: ns_path byReference: YES];
                let _: () = msg_send![ns_path, release];
                sound
            };
            if sound != nil {
                loaded.insert(path.to_path_buf(), sound);
            }
            sound
        })
    }

    /// Play `path` from the start, cutting off this clip if it is still
    /// sounding from the last press. False if it could not be played.
    pub fn play(path: &Path) -> bool {
        let sound = load(path);
        if sound == nil {
            return false;
        }
        // SAFETY: `sound` is a live NSSound owned by `LOADED`.
        unsafe {
            let _: BOOL = msg_send![sound, stop];
            let started: BOOL = msg_send![sound, play];
            started == YES
        }
    }
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

// Platform-specific audio playback (macOS plays in-process, see `native`)
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
        for cue in Cue::ALL {
            let path = public.join(cue.file_path());
            assert!(path.exists(), "{:?} has no clip at {:?}", cue, path);
        }
    }

    #[test]
    fn cue_paths_use_the_platform_format() {
        let path = Cue::GotIt(VoiceTarget::Agent).file_path();
        #[cfg(target_os = "macos")]
        assert_eq!(path, "sounds/caf/got-it.caf");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(path, "sounds/ogg/got-it.ogg");
    }

    #[test]
    fn dictation_and_juno_sound_different() {
        for (a, b) in [
            (
                Cue::Listening(VoiceTarget::Agent),
                Cue::Listening(VoiceTarget::Dictation),
            ),
            (
                Cue::GotIt(VoiceTarget::Agent),
                Cue::GotIt(VoiceTarget::Dictation),
            ),
        ] {
            assert_ne!(a.file_path(), b.file_path());
        }
    }
}

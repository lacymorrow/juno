use once_cell::sync::Lazy;
use regex::Regex;
use std::path::PathBuf;
use tauri::{Manager, Runtime};

/// Compiled regex for all Whisper audio marker artifacts.
/// Matches bracket, paren, and asterisk-wrapped forms, case-insensitively.
/// Examples: [BLANK_AUDIO], [BLANK AUDIO], [SILENCE], [INAUDIBLE], (MUSIC), *BLANK_AUDIO*, blank audio
static WHISPER_ARTIFACT_RE: Lazy<Option<Regex>> = Lazy::new(|| {
    Regex::new(
        r"(?i)\[(?:BLANK[\s_]AUDIO|SILENCE|INAUDIBLE|MUSIC|NOISE|APPLAUSE|LAUGHTER)\]|\((?:BLANK[\s_]AUDIO|MUSIC|NOISE)\)|\*BLANK[\s_]AUDIO\*|blank\s+audio"
    ).map_err(|e| tracing::error!("Failed to compile WHISPER_ARTIFACT_RE: {}", e)).ok()
});

/// A whole utterance that is nothing but one bracketed span.
///
/// Whisper narrates what it hears when it hears no speech, and it writes that
/// narration as its own segment: `(upbeat music)`, `(clicking)`, `(keyboard
/// clacking)`, `[door closes]`. The wording is open ended, so the named-token
/// list above can never catch up with it, but the shape is not: an entire
/// transcription consisting of a single parenthesised or bracketed span is a
/// description of a sound, never something a person said.
///
/// Anchored to the whole string on purpose. A parenthesis inside real speech
/// ("I said (quietly) that it was fine") is speech, and eating it would be a
/// worse bug than the one this fixes.
static WHOLE_UTTERANCE_ARTIFACT_RE: Lazy<Option<Regex>> = Lazy::new(|| {
    Regex::new(r"^\s*[\(\[\*][^\)\]\*]*[\)\]\*]\s*$")
        .map_err(|e| tracing::error!("Failed to compile WHOLE_UTTERANCE_ARTIFACT_RE: {}", e))
        .ok()
});

/// "Juneau" is what Whisper reliably hears when someone says "Juno": the Alaska
/// capital is in its vocabulary, the assistant's name is not. Rewrite the whole
/// word back (case-insensitively) so addressing Juno by name works. Juno is an
/// assistant, so a genuine dictation about the city is rare enough that the
/// name recognition is the right trade.
static JUNEAU_RE: Lazy<Option<Regex>> = Lazy::new(|| {
    Regex::new(r"(?i)\bjuneau\b")
        .map_err(|e| tracing::error!("Failed to compile JUNEAU_RE: {}", e))
        .ok()
});

/// Fix known mishearings of Juno's own name in a cleaned transcript.
fn correct_name_mishearings(text: &str) -> String {
    match &*JUNEAU_RE {
        Some(re) => re.replace_all(text, "Juno").into_owned(),
        None => text.to_string(),
    }
}

/// Remove Whisper audio marker artifacts from transcription text.
///
/// Strips tokens like `[BLANK_AUDIO]`, `[BLANK AUDIO]`, `[SILENCE]`, `[INAUDIBLE]`, `[MUSIC]`,
/// `[NOISE]`, `[APPLAUSE]`, `[LAUGHTER]`, their `(...)` and `*...*` variants, and the plain-text
/// "blank audio" form.  Collapses any resulting extra whitespace.
///
/// Returns an empty string when the whole utterance was a sound description,
/// which callers treat as "nothing was said": no typing, no clipboard, no
/// query handed to the agent.
pub fn filter_transcription_text(text: &str) -> String {
    if let Some(re) = &*WHOLE_UTTERANCE_ARTIFACT_RE {
        if re.is_match(text) {
            tracing::debug!("Dropping sound-description transcription: {:?}", text);
            return String::new();
        }
    }

    let cleaned = match &*WHISPER_ARTIFACT_RE {
        Some(re) => {
            let stripped = re.replace_all(text, " ");
            stripped.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        None => text.to_string(),
    };
    correct_name_mishearings(&cleaned)
}

/// Resolve model path to an absolute path using production-ready path resolution
pub fn resolve_model_path<R: Runtime>(app: &tauri::AppHandle<R>, model_path: &str) -> String {
    tracing::debug!("Starting model path resolution for: '{}'", model_path);
    let path = PathBuf::from(model_path);

    // If it's already absolute, use as-is
    if path.is_absolute() {
        if path.exists() {
            tracing::debug!("Using absolute model path: {}", path.display());
            return model_path.to_string();
        } else {
            tracing::warn!("Absolute model path does not exist: {}", path.display());
        }
    }

    // Strategy 1: Try bundled resources (production apps)
    tracing::debug!("Strategy 1: Checking bundled resources...");

    // First try the direct resource resolution
    if let Ok(resource_path) = app
        .path()
        .resolve(model_path, tauri::path::BaseDirectory::Resource)
    {
        tracing::debug!("  Resource path resolved to: {}", resource_path.display());
        if resource_path.exists() {
            tracing::debug!(
                "Found model in bundled resources: {}",
                resource_path.display()
            );
            return resource_path.to_string_lossy().to_string();
        } else {
            tracing::debug!("  Resource path does not exist");
        }
    } else {
        tracing::debug!("  Failed to resolve resource path");
    }

    // Try the _up_ directory pattern used by other bundled resources in production
    if let Ok(resource_dir) = app.path().resource_dir() {
        tracing::debug!("  Resource directory: {:?}", resource_dir);

        // Modern bundled paths in production builds (_up_ directory)
        let bundled_paths = [
            // _up_ paths for production builds
            resource_dir.join("_up_").join("models").join(model_path),
            resource_dir.join("_up_").join(model_path),
            // Actual bundled path from our resources configuration
            resource_dir
                .join("_up_")
                .join("tauri-plugin-voice-transcription")
                .join("models")
                .join("ggml-tiny.en.bin"),
            resource_dir
                .join("_up_")
                .join("tauri-plugin-voice-transcription")
                .join(model_path),
            // Standard resource paths
            resource_dir.join("models").join(model_path),
            resource_dir.join(model_path),
            // Additional paths for development and production compatibility
            std::path::PathBuf::from("models").join(model_path),
            std::path::PathBuf::from(model_path),
        ];

        for test_path in bundled_paths.iter() {
            tracing::debug!("  Checking bundled path: {:?}", test_path);
            if test_path.exists() {
                tracing::debug!("Found model in bundled resources: {:?}", test_path);
                return test_path.to_string_lossy().to_string();
            } else {
                tracing::debug!("  Bundled path does not exist");
            }
        }
    } else {
        tracing::warn!("  Failed to get resource directory");
    }

    // Strategy 2: Try app data directory (user-installed models)
    if let Ok(app_dir) = app.path().app_data_dir() {
        let app_model_path = app_dir.join(model_path);
        if app_model_path.exists() {
            tracing::debug!("Found model in app data dir: {}", app_model_path.display());
            return app_model_path.to_string_lossy().to_string();
        }
    }

    // Strategy 3: Try app local data directory
    if let Ok(local_dir) = app.path().app_local_data_dir() {
        let local_model_path = local_dir.join(model_path);
        if local_model_path.exists() {
            tracing::debug!(
                "Found model in app local data dir: {}",
                local_model_path.display()
            );
            return local_model_path.to_string_lossy().to_string();
        }
    }

    // Strategy 4: Development mode - look for models in plugin directory structure
    if cfg!(debug_assertions) {
        tracing::debug!("Strategy 4: Development mode path checking...");
        // Try to find the plugin's models directory in development
        if let Ok(cwd) = std::env::current_dir() {
            tracing::debug!("  Current working directory: {}", cwd.display());
            // Look for tauri-plugin-voice-transcription/models and other common dev locations
            let dev_model_paths = [
                cwd.join("tauri-plugin-voice-transcription")
                    .join(model_path),
                cwd.join(model_path),
            ];

            for dev_path in &dev_model_paths {
                tracing::debug!("  Checking development path: {}", dev_path.display());
                if dev_path.exists() {
                    tracing::debug!("Found model in development path: {}", dev_path.display());
                    return dev_path
                        .canonicalize()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| dev_path.to_string_lossy().to_string());
                } else {
                    tracing::debug!("  Development path does not exist");
                }
            }
        } else {
            tracing::warn!("  Failed to get current working directory");
        }
    }

    // Strategy 5: macOS App Bundle - Check in Resources directory
    #[cfg(target_os = "macos")]
    {
        tracing::debug!("Strategy 5: Checking macOS app bundle...");
        if let Ok(exe_path) = std::env::current_exe() {
            tracing::debug!("  Current executable: {}", exe_path.display());

            // For macOS app bundles: executable is at Contents/MacOS/binary
            // Resources are at Contents/Resources/
            if let Some(macos_dir) = exe_path.parent() {
                if let Some(contents_dir) = macos_dir.parent() {
                    let resources_dir = contents_dir.join("Resources");
                    tracing::debug!(
                        "  Checking Resources directory: {}",
                        resources_dir.display()
                    );

                    let bundle_paths = vec![
                        resources_dir.join(model_path),
                        resources_dir.join("models").join(model_path),
                        resources_dir.join("_up_").join(model_path),
                        resources_dir.join("_up_").join("models").join(model_path),
                        // Add the actual bundled path we found
                        resources_dir
                            .join("_up_")
                            .join("tauri-plugin-voice-transcription")
                            .join(model_path),
                        resources_dir
                            .join("_up_")
                            .join("tauri-plugin-voice-transcription")
                            .join("models")
                            .join("ggml-tiny.en.bin"),
                    ];

                    for bundle_path in bundle_paths {
                        tracing::debug!("  Checking bundle path: {}", bundle_path.display());
                        if bundle_path.exists() {
                            tracing::debug!(
                                "Found model in macOS bundle: {}",
                                bundle_path.display()
                            );
                            return bundle_path.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }
    }

    // Strategy 6: Look relative to current working directory
    let cwd_model_path = PathBuf::from(model_path);
    if cwd_model_path.exists() {
        tracing::debug!(
            "Found model in current working directory: {}",
            cwd_model_path.display()
        );
        return cwd_model_path
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| model_path.to_string());
    }

    // Strategy 7: Final fallback - return original path (will likely fail, but preserves error handling)
    tracing::warn!(
        "Model file '{}' not found in any standard location. Locations checked:",
        model_path
    );
    tracing::warn!("  - Bundled resources: {}", model_path);
    tracing::warn!("  - Bundled resources (_up_ pattern): _up_/{}", model_path);
    tracing::warn!("  - App data directory: [app_data]/{}", model_path);
    tracing::warn!("  - App local data directory: [local_data]/{}", model_path);
    if cfg!(debug_assertions) {
        tracing::warn!(
            "  - Development paths: ./tauri-plugin-voice-transcription/{} and ./{}",
            model_path,
            model_path
        );
    }
    tracing::warn!("  - Current working directory: ./{}", model_path);

    tracing::error!(
        "Returning original model path '{}' as fallback (will likely fail)",
        model_path
    );
    model_path.to_string()
}

#[cfg(test)]
mod transcription_filter_tests {
    use super::filter_transcription_text;

    #[test]
    fn a_sound_description_is_not_something_someone_said() {
        // The reported cases, plus the shape they share. Whisper's wording here
        // is open ended, which is why this is matched by shape and not by word.
        for artifact in [
            "(upbeat music)",
            "(clicking)",
            "(keyboard clacking)",
            "[door closes]",
            "  (soft piano music)  ",
            "*rustling*",
        ] {
            assert_eq!(
                filter_transcription_text(artifact),
                "",
                "{artifact} is a description of a sound"
            );
        }
    }

    #[test]
    fn a_parenthesis_inside_real_speech_survives() {
        // Eating this would be a worse bug than the one the rule above fixes.
        assert_eq!(
            filter_transcription_text("I said (quietly) that it was fine"),
            "I said (quietly) that it was fine"
        );
    }

    #[test]
    fn juneau_is_rewritten_to_juno() {
        // Whisper hears the assistant's name as the Alaska capital.
        assert_eq!(filter_transcription_text("hey juneau"), "hey Juno");
        assert_eq!(filter_transcription_text("Juneau, what time is it"), "Juno, what time is it");
        // Whole word only: a word that merely contains the letters is untouched.
        assert_eq!(filter_transcription_text("juneaus"), "juneaus");
    }

    #[test]
    fn the_named_tokens_still_go() {
        assert_eq!(
            filter_transcription_text("hello [BLANK_AUDIO] world"),
            "hello world"
        );
    }

    #[test]
    fn ordinary_speech_is_left_exactly_alone() {
        assert_eq!(
            filter_transcription_text("move my mouse in a slow circle"),
            "move my mouse in a slow circle"
        );
    }
}

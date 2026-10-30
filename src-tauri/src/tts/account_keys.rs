//! The API keys the cloud voice engines need, from Settings.
//!
//! ElevenLabs and Replicate used to read their key from the process
//! environment and nowhere else. An app opened from the Dock or Finder has no
//! shell environment, so in the installed app neither engine could ever speak:
//! choosing ElevenLabs in Settings fell back to Kokoro on every sentence, after
//! a failed attempt and a cold model load, which is most of why speech felt
//! slow "whatever the provider". The key now lives in the audio settings, set
//! from the Voice pane. The environment variable is still read after it, so a
//! developer's shell setup keeps working.

use std::sync::OnceLock;
use tauri::AppHandle;

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Called once at startup, beside `kokoro::remember_app`.
pub fn remember_app(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// An engine that speaks with an account key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyedEngine {
    ElevenLabs,
    /// Replicate, which also runs Chatterbox.
    Replicate,
}

impl KeyedEngine {
    pub fn from_id(engine: &str) -> Option<Self> {
        match engine.to_ascii_lowercase().as_str() {
            "elevenlabs" => Some(Self::ElevenLabs),
            "replicate" | "chatterbox" => Some(Self::Replicate),
            _ => None,
        }
    }

    fn env_var(self) -> &'static str {
        match self {
            Self::ElevenLabs => "ELEVENLABS_API_KEY",
            Self::Replicate => "REPLICATE_API_KEY",
        }
    }

    /// The sentence for an engine asked to speak with no key anywhere.
    pub fn missing(self) -> String {
        let name = match self {
            Self::ElevenLabs => "ElevenLabs",
            Self::Replicate => "Replicate",
        };
        format!("No {name} API key. Add one in Settings, Voice.")
    }
}

fn stored(settings: &crate::settings::AudioSettings, engine: KeyedEngine) -> Option<String> {
    let key = match engine {
        KeyedEngine::ElevenLabs => settings.elevenlabs_api_key.as_deref(),
        KeyedEngine::Replicate => settings.replicate_api_key.as_deref(),
    };
    key.map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string)
}

/// The key for `engine`: the one saved in Settings, else the environment's,
/// else, for ElevenLabs in a demo build, the one compiled in (`demo`). The
/// person's own key always wins, so a demo they add a key to stops spending
/// the demo's characters.
pub async fn key(engine: KeyedEngine) -> Result<String, String> {
    if let Some(app) = APP.get() {
        if let Ok(manager) = crate::settings::manager::SettingsManager::new(app.clone()) {
            if let Ok(audio) = manager.get_audio_settings().await {
                if let Some(key) = stored(&audio, engine) {
                    return Ok(key);
                }
            }
        }
    }
    std::env::var(engine.env_var())
        .ok()
        .filter(|k| !k.trim().is_empty())
        .or_else(|| match engine {
            KeyedEngine::ElevenLabs => crate::demo::elevenlabs_key().map(str::to_string),
            KeyedEngine::Replicate => None,
        })
        .ok_or_else(|| engine.missing())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AudioSettings;

    #[test]
    fn chatterbox_speaks_with_the_replicate_key() {
        assert_eq!(
            KeyedEngine::from_id("chatterbox"),
            Some(KeyedEngine::Replicate)
        );
        assert_eq!(
            KeyedEngine::from_id("ElevenLabs"),
            Some(KeyedEngine::ElevenLabs)
        );
        assert_eq!(KeyedEngine::from_id("kokoro"), None);
    }

    #[test]
    fn a_blank_saved_key_is_no_key() {
        let audio = AudioSettings {
            elevenlabs_api_key: Some("  ".to_string()),
            replicate_api_key: Some(" r8_abc ".to_string()),
            ..AudioSettings::default()
        };
        assert_eq!(stored(&audio, KeyedEngine::ElevenLabs), None);
        assert_eq!(
            stored(&audio, KeyedEngine::Replicate).as_deref(),
            Some("r8_abc")
        );
    }
}

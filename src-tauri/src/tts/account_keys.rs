//! The API keys the cloud voice engines need.
//!
//! ElevenLabs and Replicate used to read their key from the process
//! environment and nowhere else. An app opened from the Dock or Finder has no
//! shell environment, so in the installed app neither engine could ever speak:
//! choosing ElevenLabs in Settings fell back to Kokoro on every sentence, after
//! a failed attempt and a cold model load, which is most of why speech felt
//! slow "whatever the provider". The key is set from the Voice pane and kept in
//! the credential store (`crate::secrets`). The environment variable is still
//! read after it, so a developer's shell setup keeps working.

use crate::secrets::Secret;

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

    /// Where the key is saved.
    pub fn secret(self) -> Secret {
        match self {
            Self::ElevenLabs => Secret::ElevenLabs,
            Self::Replicate => Secret::Replicate,
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

/// The key for `engine`: the one saved in Settings, else the environment's,
/// else, for ElevenLabs in a demo build, the one compiled in (`demo`). The
/// person's own key always wins, so a demo they add a key to stops spending
/// the demo's characters.
pub fn key(engine: KeyedEngine) -> Result<String, String> {
    crate::secrets::get(&engine.secret())
        .or_else(|| {
            std::env::var(engine.env_var())
                .ok()
                .filter(|k| !k.trim().is_empty())
        })
        .or_else(|| match engine {
            KeyedEngine::ElevenLabs => crate::demo::elevenlabs_key().map(str::to_string),
            KeyedEngine::Replicate => None,
        })
        .ok_or_else(|| engine.missing())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(KeyedEngine::Replicate.secret(), Secret::Replicate);
    }
}

use serde::{Deserialize, Serialize};

use crate::agent::core::AgentError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::types::Provider;

// Add centralized settings support
use crate::settings::{
    ProviderConfig as CentralizedProviderConfig, ProviderSettings as CentralizedProviderSettings,
};

// Configuration cache to prevent redundant loading
#[allow(clippy::type_complexity)]
static CONFIG_CACHE: std::sync::LazyLock<Arc<Mutex<HashMap<String, (ProviderConfig, Instant)>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

const CACHE_DURATION: Duration = Duration::from_secs(5); // 5 second cache

/// Agent execution mode
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentMode {
    /// Single agent handles all tasks directly
    Single,
    /// Multi-agent system with specialized agents
    #[default]
    Multi,
}

impl AgentMode {
    pub fn to_string(&self) -> &'static str {
        match self {
            AgentMode::Single => "single",
            AgentMode::Multi => "multi",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "single" => Some(AgentMode::Single),
            "multi" => Some(AgentMode::Multi),
            _ => None,
        }
    }
}

/// Configuration structure for AI providers
/// Uses centralized ProviderSettings directly instead of duplicating the structure
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    /// The active provider ID
    pub active_provider: String,
    /// Agent execution mode (single vs multi-agent)
    pub agent_mode: AgentMode,
    /// Configuration for each provider (uses centralized type)
    pub providers: Vec<CentralizedProviderConfig>,
}

/// The default AI provider. Use `.id()` when a string is needed.
pub const DEFAULT_PROVIDER: Provider = Provider::Anthropic;

/// Centralized list of default providers and models (single source of truth)
pub fn default_provider_entries() -> Vec<CentralizedProviderConfig> {
    vec![
        CentralizedProviderConfig {
            id: Provider::Anthropic.id().to_string(),
            api_key: None,
            model: Some(Provider::Anthropic.default_model().to_string()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            system_prompt: None,
        },
        CentralizedProviderConfig {
            id: Provider::OpenAI.id().to_string(),
            api_key: None,
            model: Some(Provider::OpenAI.default_model().to_string()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            system_prompt: None,
        },
        CentralizedProviderConfig {
            id: Provider::Rig.id().to_string(),
            api_key: None, // Rig uses OpenAI's API key by default
            model: Some(Provider::Rig.default_model().to_string()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            system_prompt: None,
        },
        CentralizedProviderConfig {
            id: Provider::Gemini.id().to_string(),
            api_key: None,
            model: Some(Provider::Gemini.default_model().to_string()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            system_prompt: None,
        },
        CentralizedProviderConfig {
            id: Provider::ClaudeCli.id().to_string(),
            api_key: None, // Claude CLI doesn't need an API key — uses its own auth
            model: Some(Provider::ClaudeCli.default_model().to_string()),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            system_prompt: None,
        },
    ]
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            active_provider: DEFAULT_PROVIDER.id().to_string(),
            agent_mode: AgentMode::Multi,
            providers: default_provider_entries(),
        }
    }
}

impl ProviderConfig {
    /// Load configuration from centralized settings manager with caching.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Application startup for configuration initialization.
    pub async fn load_from_centralized_settings(
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<Self, AgentError> {
        // Use a static cache key since all SettingsManager instances share the same AppHandle
        // There's only one application instance, so per-instance caching is unnecessary
        let cache_key = "provider_config";

        // Check cache first
        if let Ok(cache) = CONFIG_CACHE.lock() {
            if let Some((config, timestamp)) = cache.get(cache_key) {
                if timestamp.elapsed() < CACHE_DURATION {
                    debug!(
                        "Using cached provider configuration (age: {:?})",
                        timestamp.elapsed()
                    );
                    return Ok(config.clone());
                }
            }
        }

        let provider_settings = settings_manager
            .get_provider_settings()
            .await
            .map_err(|e| {
                AgentError::ConfigurationError(format!("Failed to load provider settings: {}", e))
            })?;

        let config = Self::from_centralized_settings(&provider_settings)?;

        // Update cache
        if let Ok(mut cache) = CONFIG_CACHE.lock() {
            cache.insert(cache_key.to_string(), (config.clone(), Instant::now()));
        }

        info!("Loaded provider configuration from centralized settings");
        Ok(config)
    }

    /// Save configuration to centralized settings manager and invalidate cache.
    /// NEW: Uses centralized settings instead of direct JSON store access.
    /// Used by: Settings UI and provider configuration updates.
    pub async fn save_to_centralized_settings(
        &self,
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<(), AgentError> {
        let provider_settings = self.to_centralized_settings()?;
        settings_manager
            .set_provider_settings(&provider_settings)
            .await
            .map_err(|e| {
                AgentError::ConfigurationError(format!("Failed to save provider settings: {}", e))
            })?;

        // Invalidate cache after saving
        if let Ok(mut cache) = CONFIG_CACHE.lock() {
            cache.clear();
            debug!("Cleared provider configuration cache after save");
        }

        info!("Saved provider configuration to centralized settings");
        Ok(())
    }

    /// Convert from centralized ProviderSettings to ProviderConfig.
    /// Handles schema differences between the two formats.
    pub(crate) fn from_centralized_settings(
        settings: &CentralizedProviderSettings,
    ) -> Result<Self, AgentError> {
        let providers = settings.providers.clone();

        // Ensure all default providers are present for backwards compatibility
        let default_config = Self::default();
        let mut final_providers = providers.clone();
        for default_provider in &default_config.providers {
            if !final_providers.iter().any(|p| p.id == default_provider.id) {
                info!(
                    "Adding missing provider from defaults: {}",
                    default_provider.id
                );
                final_providers.push(default_provider.clone());
            }
        }

        // Use default agent mode since it's now managed in AgentSettings
        let agent_mode = AgentMode::default();

        Ok(Self {
            active_provider: settings.active_provider.clone(),
            agent_mode,
            providers: final_providers,
        })
    }

    /// Convert from ProviderConfig to centralized ProviderSettings.
    /// Handles schema differences between the two formats.
    fn to_centralized_settings(&self) -> Result<CentralizedProviderSettings, AgentError> {
        let providers = self.providers.clone();

        Ok(CentralizedProviderSettings {
            active_provider: self.active_provider.clone(),
            providers,
        })
    }

    /// Update provider API key
    pub fn update_api_key(&mut self, provider_id: &str, api_key: String) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.api_key = Some(api_key);
            Ok(())
        } else {
            Err(AgentError::ConfigurationError(format!(
                "Provider '{}' not found",
                provider_id
            )))
        }
    }

    /// Update provider model
    pub fn update_model(&mut self, provider_id: &str, model: String) -> Result<(), AgentError> {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.model = Some(model);
            Ok(())
        } else {
            Err(AgentError::ConfigurationError(format!(
                "Provider '{}' not found",
                provider_id
            )))
        }
    }

    /// Set active provider
    pub fn set_active_provider(&mut self, provider_id: String) -> Result<(), AgentError> {
        // Verify the provider exists
        if !self.providers.iter().any(|p| p.id == provider_id) {
            return Err(AgentError::ConfigurationError(format!(
                "Provider '{}' not found",
                provider_id
            )));
        }
        self.active_provider = provider_id;
        Ok(())
    }

    /// Set agent mode
    pub fn set_agent_mode(&mut self, mode: AgentMode) -> Result<(), AgentError> {
        self.agent_mode = mode;
        Ok(())
    }

    /// Get agent mode
    pub fn get_agent_mode(&self) -> &AgentMode {
        &self.agent_mode
    }

    /// Get settings for the active provider
    pub fn get_active_provider_settings(&self) -> Option<&CentralizedProviderConfig> {
        self.providers.iter().find(|p| p.id == self.active_provider)
    }

    /// Get settings for a specific provider
    pub fn get_provider_settings(&self, provider_id: &str) -> Option<&CentralizedProviderConfig> {
        self.providers.iter().find(|p| p.id == provider_id)
    }

    /// Get resolved config for a specific provider.
    /// Returns a cloned config with special-case fallbacks applied:
    /// - Rig provider falls back to OpenAI's API key if its own is missing.
    pub fn resolve_provider(&self, provider: Provider) -> Option<CentralizedProviderConfig> {
        let provider_id = provider.id();
        let mut config = self.providers.iter().find(|p| p.id == provider_id)?.clone();
        // Rig special case: fall back to OpenAI's API key
        if provider == Provider::Rig && config.api_key.is_none() {
            if let Some(openai) = self
                .providers
                .iter()
                .find(|p| p.id == Provider::OpenAI.id())
            {
                config.api_key = openai.api_key.clone();
            }
        }
        Some(config)
    }
}

/// Load provider configuration from Tauri Store (or defaults if no AppHandle).
/// This is the primary entry point for getting provider configuration.
/// Replaces the old apply_provider_settings() / load_config_from_store() flow
/// that used unsafe env::set_var() as an intermediary.
pub fn load_provider_config(app_handle: Option<&tauri::AppHandle>) -> ProviderConfig {
    match app_handle {
        Some(handle) => match load_config_from_store_internal(handle) {
            Ok(config) => {
                info!("Loaded provider configuration from Tauri Store");
                config
            }
            Err(e) => {
                warn!(
                    "Failed to load provider settings from store: {}. Using defaults.",
                    e
                );
                ProviderConfig::default()
            }
        },
        None => ProviderConfig::default(),
    }
}

/// Load ProviderConfig directly from Tauri Store (sync, internal helper).
fn load_config_from_store_internal(
    app_handle: &tauri::AppHandle,
) -> Result<ProviderConfig, AgentError> {
    use crate::constants::settings::{store_keys, SETTINGS_STORE_FILE};
    use tauri_plugin_store::StoreExt;

    let store = app_handle.store(SETTINGS_STORE_FILE).map_err(|e| {
        AgentError::ConfigurationError(format!("Failed to access settings store: {}", e))
    })?;

    let provider_settings: CentralizedProviderSettings = store
        .get(store_keys::PROVIDERS)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    ProviderConfig::from_centralized_settings(&provider_settings)
}

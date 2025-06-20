//! # Centralized Settings Manager
//!
//! Single source of truth for all application settings with reactive updates.
//! Replaces scattered store operations throughout the codebase.

use std::sync::Arc;
use serde_json::Value;
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_store::StoreExt;
use tokio::sync::RwLock;

use crate::constants::settings::{
    SETTINGS_STORE_FILE,
    store_keys,
    events,
    validation,
};
use crate::settings::{
    AppSettings,
    KeyboardShortcuts,
    FloatingBarSettings,
    AgentSettings,
    ProviderSettings,
    CloudSettings,
    AudioSettings,
    ToolSettings,
    OnboardingSettings
};

/// Centralized settings manager with reactive updates
/// This replaces all individual store operations throughout the codebase
#[derive(Clone)]
pub struct SettingsManager {
    app_handle: AppHandle,
}

impl SettingsManager {
    /// Initialize the settings manager with the app handle
    pub fn new(app_handle: AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = Self { app_handle };

        // Initialize with defaults if empty
        tauri::async_runtime::spawn({
            let manager = manager.clone();
            async move {
                if let Err(e) = manager.initialize_defaults().await {
                    eprintln!("Failed to initialize default settings: {}", e);
                }
            }
        });

        Ok(manager)
    }

    /// Initialize default settings if they don't exist
    async fn initialize_defaults(&self) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;

        // Check if settings exist, if not, create defaults
        if store.get(store_keys::KEYBOARD_SHORTCUTS).is_none() {
            let defaults = AppSettings::default();
            self.save_all_settings(&defaults).await?;
        }

        Ok(())
    }

    /// Get complete application settings
    pub async fn get_all_settings(&self) -> Result<AppSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;

        let settings = AppSettings {
            keyboard_shortcuts: self.get_keyboard_shortcuts_from_store(&store)?,
            floating_bar: self.get_floating_bar_settings_from_store(&store)?,
            agent: self.get_agent_settings_from_store(&store)?,
            providers: self.get_provider_settings_from_store(&store)?,
            cloud: self.get_cloud_settings_from_store(&store)?,
            audio: self.get_audio_settings_from_store(&store)?,
            tools: self.get_tool_settings_from_store(&store)?,
            onboarding: self.get_onboarding_settings_from_store(&store)?,
            autostart_enabled: store
                .get(store_keys::AUTOSTART_ENABLED)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        Ok(settings)
    }

    /// Save complete application settings and emit change events
    pub async fn save_all_settings(&self, settings: &AppSettings) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;

        // Save each section
        store.set(store_keys::KEYBOARD_SHORTCUTS, serde_json::to_value(&settings.keyboard_shortcuts)?);
        store.set(store_keys::FLOATING_BAR, serde_json::to_value(&settings.floating_bar)?);
        store.set(store_keys::AGENT, serde_json::to_value(&settings.agent)?);
        store.set(store_keys::PROVIDERS, serde_json::to_value(&settings.providers)?);
        store.set(store_keys::CLOUD, serde_json::to_value(&settings.cloud)?);
        store.set(store_keys::AUDIO, serde_json::to_value(&settings.audio)?);
        store.set(store_keys::TOOLS, serde_json::to_value(&settings.tools)?);
        store.set(store_keys::ONBOARDING, serde_json::to_value(&settings.onboarding)?);
        store.set(store_keys::AUTOSTART_ENABLED, Value::Bool(settings.autostart_enabled));

        store.save()?;

        // Emit change events for reactivity
        self.emit_settings_changed().await;

        Ok(())
    }

    // Individual getters
    pub async fn get_keyboard_shortcuts(&self) -> Result<KeyboardShortcuts, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_keyboard_shortcuts_from_store(&store)
    }

    pub async fn get_floating_bar_settings(&self) -> Result<FloatingBarSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_floating_bar_settings_from_store(&store)
    }

    pub async fn get_agent_settings(&self) -> Result<AgentSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_agent_settings_from_store(&store)
    }

    pub async fn get_provider_settings(&self) -> Result<ProviderSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_provider_settings_from_store(&store)
    }

    pub async fn get_cloud_settings(&self) -> Result<CloudSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_cloud_settings_from_store(&store)
    }

    pub async fn get_audio_settings(&self) -> Result<AudioSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_audio_settings_from_store(&store)
    }

    pub async fn get_tool_settings(&self) -> Result<ToolSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_tool_settings_from_store(&store)
    }

    pub async fn get_onboarding_settings(&self) -> Result<OnboardingSettings, Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        self.get_onboarding_settings_from_store(&store)
    }

    // Individual setters with validation and events
    pub async fn set_keyboard_shortcuts(&self, shortcuts: &KeyboardShortcuts) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::KEYBOARD_SHORTCUTS, serde_json::to_value(shortcuts)?);
        store.save()?;

        self.app_handle.emit(events::KEYBOARD_SHORTCUTS_CHANGED, shortcuts)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_floating_bar_settings(&self, settings: &FloatingBarSettings) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::FLOATING_BAR, serde_json::to_value(settings)?);
        store.save()?;

        self.app_handle.emit(events::FLOATING_BAR_SETTINGS_CHANGED, settings)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_agent_settings(&self, settings: &AgentSettings) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::AGENT, serde_json::to_value(settings)?);
        store.save()?;

        self.app_handle.emit(events::AGENT_SETTINGS_CHANGED, settings)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_provider_settings(&self, settings: &ProviderSettings) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::PROVIDERS, serde_json::to_value(settings)?);
        store.save()?;

        self.app_handle.emit(events::PROVIDER_SETTINGS_CHANGED, settings)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_cloud_settings(&self, settings: &CloudSettings) -> Result<(), Box<dyn std::error::Error>> {
        // Validate cloud settings
        if settings.heartbeat_interval < validation::MIN_HEARTBEAT_INTERVAL ||
           settings.heartbeat_interval > validation::MAX_HEARTBEAT_INTERVAL {
            return Err("Invalid heartbeat interval".into());
        }

        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::CLOUD, serde_json::to_value(settings)?);
        store.save()?;

        self.app_handle.emit(events::CLOUD_SETTINGS_CHANGED, settings)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_audio_settings(&self, settings: &AudioSettings) -> Result<(), Box<dyn std::error::Error>> {
        // Validate audio settings
        if settings.always_listening_sensitivity < validation::MIN_SENSITIVITY ||
           settings.always_listening_sensitivity > validation::MAX_SENSITIVITY {
            return Err("Invalid sensitivity value".into());
        }

        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::AUDIO, serde_json::to_value(settings)?);
        store.save()?;

        self.app_handle.emit(events::AUDIO_SETTINGS_CHANGED, settings)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_tool_settings(&self, settings: &ToolSettings) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::TOOLS, serde_json::to_value(settings)?);
        store.save()?;

        self.app_handle.emit(events::TOOL_SETTINGS_CHANGED, settings)?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_onboarding_settings(&self, settings: &OnboardingSettings) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::ONBOARDING, serde_json::to_value(settings)?);
        store.save()?;

        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_autostart_enabled(&self, enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
        let store = self.app_handle.store(SETTINGS_STORE_FILE)?;
        store.set(store_keys::AUTOSTART_ENABLED, Value::Bool(enabled));
        store.save()?;

        self.emit_settings_changed().await;
        Ok(())
    }

    // Internal helpers
    fn get_keyboard_shortcuts_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<KeyboardShortcuts, Box<dyn std::error::Error>> {
        match store.get(store_keys::KEYBOARD_SHORTCUTS)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(shortcuts) => Ok(shortcuts),
            None => Ok(KeyboardShortcuts::default())
        }
    }

    fn get_floating_bar_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<FloatingBarSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::FLOATING_BAR)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(FloatingBarSettings::default())
        }
    }

    fn get_agent_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<AgentSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::AGENT)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(AgentSettings::default())
        }
    }

    fn get_provider_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<ProviderSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::PROVIDERS)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(ProviderSettings::default())
        }
    }

    fn get_cloud_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<CloudSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::CLOUD)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(CloudSettings::default())
        }
    }

    fn get_audio_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<AudioSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::AUDIO)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(AudioSettings::default())
        }
    }

    fn get_tool_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<ToolSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::TOOLS)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(ToolSettings::default())
        }
    }

    fn get_onboarding_settings_from_store(&self, store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<OnboardingSettings, Box<dyn std::error::Error>> {
        match store.get(store_keys::ONBOARDING)
            .and_then(|v| serde_json::from_value(v.clone()).ok()) {
            Some(settings) => Ok(settings),
            None => Ok(OnboardingSettings::default())
        }
    }

    /// Emit general settings changed event for full reactivity
    async fn emit_settings_changed(&self) {
        if let Ok(settings) = self.get_all_settings().await {
            if let Err(e) = self.app_handle.emit(events::SETTINGS_CHANGED, &settings) {
                eprintln!("Failed to emit settings changed event: {}", e);
            }
        }
    }

    /// Migration helper: Import settings from legacy store files
    pub async fn migrate_from_legacy_stores(&self) -> Result<(), Box<dyn std::error::Error>> {
        // This will be implemented when we start migrating individual files
        // For now, just ensure defaults are set
        self.initialize_defaults().await
    }
}

//! # Centralized Settings Manager
//!
//! Clean, simple settings management for the Juno app.

use super::schema::{AppSettings, SettingsUpdateEvent};
use tauri::{AppHandle, Runtime, Emitter};
use tauri_plugin_store::StoreExt;
use tracing::{info, warn, error};
use std::sync::{Arc, RwLock};
use serde_json::Value;

/// Thread-safe settings manager
pub struct SettingsManager<R: Runtime = tauri::Wry> {
    app_handle: AppHandle<R>,
    settings: Arc<RwLock<AppSettings>>,
}

impl<R: Runtime> Clone for SettingsManager<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            settings: Arc::clone(&self.settings),
        }
    }
}

impl<R: Runtime> SettingsManager<R> {
    /// Create a new settings manager instance
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self {
            app_handle,
            settings: Arc::new(RwLock::new(AppSettings::default())),
        }
    }

    /// Initialize settings from store
    pub async fn initialize(&self) -> Result<(), String> {
        info!("🔧 Initializing SettingsManager...");

        match self.load_from_store().await {
            Ok(loaded_settings) => {
                *self.settings.write().map_err(|e| format!("Failed to write settings: {}", e))? = loaded_settings;
                info!("✅ Settings loaded from store");
            }
            Err(_) => {
                info!("📝 Creating new settings with defaults");
                self.save_to_store().await?;
            }
        }

        Ok(())
    }

    /// Get current settings (synchronous)
    pub fn get_settings(&self) -> AppSettings {
        match self.settings.read() {
            Ok(settings) => settings.clone(),
            Err(_) => {
                warn!("Settings lock poisoned, returning defaults");
                AppSettings::default()
            }
        }
    }

    /// Get all settings (alias for compatibility)
    pub async fn get_all(&self) -> Result<AppSettings, String> {
        Ok(self.get_settings())
    }

    /// Get a section by path
    pub async fn get_section(&self, path: &str) -> Result<Value, String> {
        let settings = self.get_settings();
        let settings_value = serde_json::to_value(&settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        self.get_json_path(&settings_value, path)
    }

    /// Update a section by path (e.g., "keyboard_shortcuts", "cloud.enabled")
    pub async fn update_section(&self, path: &str, value: Value) -> Result<(), String> {
        {
            let mut settings = self.settings.write()
                .map_err(|e| format!("Failed to write settings: {}", e))?;

            let settings_value = serde_json::to_value(&*settings)
                .map_err(|e| format!("Failed to serialize settings: {}", e))?;

            let updated_value = self.update_json_path(settings_value, path, value)?;

            *settings = serde_json::from_value(updated_value)
                .map_err(|e| format!("Failed to deserialize updated settings: {}", e))?;
        }

        self.save_to_store().await?;
        self.emit_settings_updated(path).await;
        Ok(())
    }

    /// Update multiple sections at once
    pub async fn update_multiple(&self, updates: Vec<(String, Value)>) -> Result<(), String> {
        {
            let mut settings = self.settings.write()
                .map_err(|e| format!("Failed to write settings: {}", e))?;

            let mut settings_value = serde_json::to_value(&*settings)
                .map_err(|e| format!("Failed to serialize settings: {}", e))?;

            for (path, value) in updates {
                settings_value = self.update_json_path(settings_value, &path, value)?;
            }

            *settings = serde_json::from_value(settings_value)
                .map_err(|e| format!("Failed to deserialize updated settings: {}", e))?;
        }

        self.save_to_store().await?;
        self.emit_settings_updated("multiple").await;
        Ok(())
    }

    /// Reset a section to defaults
    pub async fn reset_section(&self, path: &str) -> Result<(), String> {
        let default_settings = AppSettings::default();
        let default_value = serde_json::to_value(&default_settings)
            .map_err(|e| format!("Failed to serialize default settings: {}", e))?;

        let section_value = self.get_json_path(&default_value, path)?;
        self.update_section(path, section_value).await
    }

    /// Reset all settings to defaults
    pub async fn reset_all(&self) -> Result<(), String> {
        {
            let mut settings = self.settings.write()
                .map_err(|e| format!("Failed to write settings: {}", e))?;
            *settings = AppSettings::default();
        }

        self.save_to_store().await?;
        self.emit_settings_updated("all").await;
        Ok(())
    }

    /// Migrate from legacy stores (no-op for new app, kept for compatibility)
    pub async fn migrate_from_legacy_stores(&self) -> Result<(), String> {
        info!("🔄 Migration from legacy stores (skipped - new app)");
        Ok(())
    }

    /// Update floating bar configuration
    pub async fn update_floating_bar(&self, config: serde_json::Value) -> Result<(), String> {
        self.update_section("floating_bar", config).await
    }

    /// Update onboarding configuration
    pub async fn update_onboarding(&self, config: serde_json::Value) -> Result<(), String> {
        self.update_section("onboarding", config).await
    }

    /// Save settings directly (used by cloud config update)
    pub async fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        {
            let mut current_settings = self.settings.write()
                .map_err(|e| format!("Failed to write settings: {}", e))?;
            *current_settings = settings.clone();
        }

        self.save_to_store().await?;
        self.emit_settings_updated("all").await;
        Ok(())
    }

    /// Helper to update JSON by path
    fn update_json_path(&self, mut json: Value, path: &str, value: Value) -> Result<Value, String> {
        let path_parts: Vec<&str> = path.split('.').collect();

        let mut current = &mut json;
        for (i, part) in path_parts.iter().enumerate() {
            if i == path_parts.len() - 1 {
                // Set the final value
                if let Some(obj) = current.as_object_mut() {
                    obj.insert(part.to_string(), value.clone());
                } else {
                    return Err(format!("Cannot set value at path '{}': not an object", path));
                }
            } else {
                // Navigate deeper
                current = current.get_mut(part)
                    .ok_or_else(|| format!("Path '{}' not found", path))?;
            }
        }

        Ok(json)
    }

    /// Helper to get JSON value by path
    fn get_json_path(&self, json: &Value, path: &str) -> Result<Value, String> {
        let path_parts: Vec<&str> = path.split('.').collect();

        let mut current = json;
        for part in path_parts {
            current = current.get(part)
                .ok_or_else(|| format!("Path '{}' not found", path))?;
        }

        Ok(current.clone())
    }

    /// Load settings from store
    async fn load_from_store(&self) -> Result<AppSettings, String> {
        let store = self.app_handle.store("app_settings.json")
            .map_err(|e| format!("Failed to access settings store: {}", e))?;

        let Some(Value::Object(settings_map)) = store.get("settings") else {
            return Err("No settings found in store".to_string());
        };

        let settings_value = Value::Object(settings_map);
        serde_json::from_value(settings_value)
            .map_err(|e| format!("Failed to deserialize settings: {}", e))
    }

    /// Save settings to store
    async fn save_to_store(&self) -> Result<(), String> {
        let settings = self.settings.read()
            .map_err(|e| format!("Failed to read settings: {}", e))?;

        let settings_value = serde_json::to_value(&*settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        let store = self.app_handle.store("app_settings.json")
            .map_err(|e| format!("Failed to access settings store: {}", e))?;

        store.set("settings", settings_value);
        store.save()
            .map_err(|e| format!("Failed to save settings store: {}", e))?;

        Ok(())
    }

    /// Emit settings updated event
    async fn emit_settings_updated(&self, section: &str) {
        let event = SettingsUpdateEvent {
            section: section.to_string(),
            settings: self.get_settings(),
        };

        if let Err(e) = self.app_handle.emit("settings-updated", &event) {
            error!("Failed to emit settings update event: {}", e);
        }
    }
}

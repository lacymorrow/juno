//! # Centralized Settings Manager
//!
//! Single source of truth for all application settings with reactive updates.
//! Replaces scattered store operations throughout the codebase.

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Wry};
use tauri_plugin_store::{Store, StoreExt};
use tracing::warn;

use crate::constants::settings::{defaults, events, store_keys, validation, SETTINGS_STORE_FILE};
use crate::settings::persist;
use crate::settings::{
    AgentSettings, AppSettings, AudioSettings, CLISettings, CloudSettings, FloatingBarSettings,
    KeyboardShortcuts, OnboardingSettings, PromptSettings, ProviderSettings, ToolSettings,
    UpdateSettings, VoiceTranscriptionSettings,
};

/// Centralized settings manager with reactive updates
/// This replaces all individual store operations throughout the codebase
#[derive(Clone)]
pub struct SettingsManager {
    app_handle: AppHandle,
}

impl SettingsManager {
    /// Initialize the settings manager with the app handle
    pub fn new(app_handle: AppHandle) -> Result<Self, String> {
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

    /// The settings store. The file under it was checked before anything
    /// opened it (see [`persist::guard_plugin`]), and every save of it is an
    /// atomic replace (see the vendored `tauri-plugin-store`).
    fn store(&self) -> Result<Arc<Store<Wry>>, String> {
        self.app_handle
            .store(SETTINGS_STORE_FILE)
            .map_err(|e| format!("Failed to access settings store: {}", e))
    }

    /// Put `entries` in the store and write it to disk, as one turn.
    ///
    /// Every write in this manager comes through here and takes
    /// [`persist::one_writer`]'s turn, so two saves never interleave: a
    /// whole-settings write cannot have another save land between two of
    /// its sections, and nothing set by one writer is lost to another.
    async fn write_entries(&self, entries: Vec<(&'static str, Value)>) -> Result<(), String> {
        let store = self.store()?;
        persist::one_writer(|| {
            for (key, value) in entries {
                store.set(key, value);
            }
            store
                .save()
                .map_err(|e| format!("Failed to save settings store: {}", e))
        })
        .await
    }

    /// Get the app handle for this settings manager instance
    /// Used internally for creating unique cache keys
    pub fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }

    /// Initialize default settings if they don't exist
    async fn initialize_defaults(&self) -> Result<(), String> {
        let store = self.store()?;

        // Check if settings exist, if not, create defaults
        if store.get(store_keys::KEYBOARD_SHORTCUTS).is_none() {
            let defaults = AppSettings::default();
            self.save_all_settings(&defaults).await?;
        }

        Ok(())
    }

    /// Get complete application settings
    pub async fn get_all_settings(&self) -> Result<AppSettings, String> {
        let store = self.store()?;

        // Legacy sections are read first so the triggers list can be migrated
        // from them when a store predates the unified activation model.
        let keyboard_shortcuts = self.get_keyboard_shortcuts_from_store(&store)?;
        let agent = self.get_agent_settings_from_store(&store)?;
        let audio = self.get_audio_settings_from_store(&store)?;
        let triggers = self.get_triggers_from_store(&store, &keyboard_shortcuts, &agent, &audio);

        let settings = AppSettings {
            keyboard_shortcuts,
            floating_bar: self.get_floating_bar_settings_from_store(&store)?,
            agent,
            providers: self.get_provider_settings_from_store(&store)?,
            cloud: self.get_cloud_settings_from_store(&store)?,
            audio,
            tools: self.get_tool_settings_from_store(&store)?,
            prompts: self.get_prompt_settings_from_store(&store)?,
            onboarding: self.get_onboarding_settings_from_store(&store)?,
            autostart_enabled: store
                .get(store_keys::AUTOSTART_ENABLED)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            advanced_settings_enabled: store
                .get(store_keys::ADVANCED_SETTINGS_ENABLED)
                .and_then(|v| v.as_bool())
                .unwrap_or(defaults::ADVANCED_SETTINGS_ENABLED),
            cli: self.get_cli_settings_from_store(&store)?,
            voice_transcription: self.get_voice_transcription_settings_from_store(&store)?,
            triggers,
            updates: read_section(&store, store_keys::UPDATES),
        };

        Ok(settings)
    }

    /// Save complete application settings and emit change events
    pub async fn save_all_settings(&self, settings: &AppSettings) -> Result<(), String> {
        // Every section goes to the store and to disk as one write, so no
        // other save can land between two of its sections.
        self.write_entries(vec![
            (
                store_keys::KEYBOARD_SHORTCUTS,
                to_json("keyboard shortcuts", &settings.keyboard_shortcuts)?,
            ),
            (
                store_keys::FLOATING_BAR,
                to_json("floating bar settings", &settings.floating_bar)?,
            ),
            (
                store_keys::AGENT,
                to_json("agent settings", &settings.agent)?,
            ),
            (
                store_keys::PROVIDERS,
                to_json("provider settings", &settings.providers)?,
            ),
            (
                store_keys::CLOUD,
                to_json("cloud settings", &settings.cloud)?,
            ),
            (
                store_keys::AUDIO,
                to_json("audio settings", &settings.audio)?,
            ),
            (
                store_keys::TOOLS,
                to_json("tool settings", &settings.tools)?,
            ),
            (
                store_keys::PROMPTS,
                to_json("prompt settings", &settings.prompts)?,
            ),
            (
                store_keys::ONBOARDING,
                to_json("onboarding settings", &settings.onboarding)?,
            ),
            (
                store_keys::AUTOSTART_ENABLED,
                Value::Bool(settings.autostart_enabled),
            ),
            (
                store_keys::ADVANCED_SETTINGS_ENABLED,
                Value::Bool(settings.advanced_settings_enabled),
            ),
            (store_keys::CLI, to_json("CLI settings", &settings.cli)?),
            (
                store_keys::VOICE_TRANSCRIPTION,
                to_json(
                    "voice transcription settings",
                    &settings.voice_transcription,
                )?,
            ),
            (
                store_keys::TRIGGERS,
                to_json("triggers", &settings.triggers)?,
            ),
            (
                store_keys::UPDATES,
                to_json("update settings", &settings.updates)?,
            ),
        ])
        .await?;

        // A whole-settings write changes every section, so it says so about
        // every section. See `emit_every_section_changed`.
        self.emit_every_section_changed(settings).await;

        Ok(())
    }

    // Individual getters
    pub async fn get_keyboard_shortcuts(&self) -> Result<KeyboardShortcuts, String> {
        let store = self.store()?;
        self.get_keyboard_shortcuts_from_store(&store)
    }

    pub async fn get_floating_bar_settings(&self) -> Result<FloatingBarSettings, String> {
        let store = self.store()?;
        self.get_floating_bar_settings_from_store(&store)
    }

    /// The same answer without an `.await`, for launch code that runs before
    /// any task could await it (the bar's config is written into its page as
    /// the window is built). The read is synchronous underneath anyway.
    pub fn floating_bar_settings_now(&self) -> Result<FloatingBarSettings, String> {
        let store = self.store()?;
        self.get_floating_bar_settings_from_store(&store)
    }

    pub async fn get_agent_settings(&self) -> Result<AgentSettings, String> {
        let store = self.store()?;
        self.get_agent_settings_from_store(&store)
    }

    pub async fn get_provider_settings(&self) -> Result<ProviderSettings, String> {
        let store = self.store()?;
        self.get_provider_settings_from_store(&store)
    }

    pub async fn get_cloud_settings(&self) -> Result<CloudSettings, String> {
        let store = self.store()?;
        self.get_cloud_settings_from_store(&store)
    }

    pub async fn get_audio_settings(&self) -> Result<AudioSettings, String> {
        let store = self.store()?;
        self.get_audio_settings_from_store(&store)
    }

    pub async fn get_tool_settings(&self) -> Result<ToolSettings, String> {
        let store = self.store()?;
        self.get_tool_settings_from_store(&store)
    }

    pub async fn get_prompt_settings(&self) -> Result<PromptSettings, String> {
        let store = self.store()?;
        self.get_prompt_settings_from_store(&store)
    }

    pub async fn get_onboarding_settings(&self) -> Result<OnboardingSettings, String> {
        let store = self.store()?;
        self.get_onboarding_settings_from_store(&store)
    }

    pub async fn get_autostart_enabled(&self) -> Result<bool, String> {
        let store = self.store()?;
        Ok(store
            .get(store_keys::AUTOSTART_ENABLED)
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Whether the settings window shows the full (advanced) set of settings.
    pub async fn get_advanced_settings_enabled(&self) -> Result<bool, String> {
        let store = self.store()?;
        Ok(store
            .get(store_keys::ADVANCED_SETTINGS_ENABLED)
            .and_then(|v| v.as_bool())
            .unwrap_or(defaults::ADVANCED_SETTINGS_ENABLED))
    }

    /// Auto-update behaviour. A store written before this shipped has no
    /// `updates` key and gets the defaults, which is what starts an old
    /// install updating rather than leaving it stranded.
    pub async fn get_update_settings(&self) -> Result<UpdateSettings, String> {
        let store = self.store()?;
        Ok(read_section(&store, store_keys::UPDATES))
    }

    pub async fn set_update_settings(&self, settings: &UpdateSettings) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::UPDATES,
            to_json("update settings", settings)?,
        )])
        .await?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn get_cli_settings(&self) -> Result<CLISettings, String> {
        let store = self.store()?;
        self.get_cli_settings_from_store(&store)
    }

    pub async fn get_voice_transcription_settings(
        &self,
    ) -> Result<VoiceTranscriptionSettings, String> {
        let store = self.store()?;
        self.get_voice_transcription_settings_from_store(&store)
    }

    // Individual setters with validation and events
    pub async fn set_keyboard_shortcuts(
        &self,
        shortcuts: &KeyboardShortcuts,
    ) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::KEYBOARD_SHORTCUTS,
            to_json("keyboard shortcuts", shortcuts)?,
        )])
        .await?;

        self.app_handle
            .emit(events::KEYBOARD_SHORTCUTS_CHANGED, shortcuts)
            .map_err(|e| format!("Failed to emit keyboard shortcuts changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_floating_bar_settings(
        &self,
        settings: &FloatingBarSettings,
    ) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::FLOATING_BAR,
            to_json("floating bar settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::FLOATING_BAR_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit floating bar settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_agent_settings(&self, settings: &AgentSettings) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::AGENT,
            to_json("agent settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::AGENT_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit agent settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_provider_settings(&self, settings: &ProviderSettings) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::PROVIDERS,
            to_json("provider settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::PROVIDER_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit provider settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_cloud_settings(&self, settings: &CloudSettings) -> Result<(), String> {
        // Validate cloud settings
        if settings.heartbeat_interval < validation::MIN_HEARTBEAT_INTERVAL
            || settings.heartbeat_interval > validation::MAX_HEARTBEAT_INTERVAL
        {
            return Err("Invalid heartbeat interval".to_string());
        }

        self.write_entries(vec![(
            store_keys::CLOUD,
            to_json("cloud settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::CLOUD_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit cloud settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_audio_settings(&self, settings: &AudioSettings) -> Result<(), String> {
        // Validate audio settings
        if settings.always_listening_sensitivity < validation::MIN_SENSITIVITY
            || settings.always_listening_sensitivity > validation::MAX_SENSITIVITY
        {
            return Err("Invalid sensitivity value".to_string());
        }

        self.write_entries(vec![(
            store_keys::AUDIO,
            to_json("audio settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::AUDIO_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit audio settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_tool_settings(&self, settings: &ToolSettings) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::TOOLS,
            to_json("tool settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::TOOL_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit tool settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_prompt_settings(&self, settings: &PromptSettings) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::PROMPTS,
            to_json("prompt settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::PROMPT_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit prompt settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_onboarding_settings(
        &self,
        settings: &OnboardingSettings,
    ) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::ONBOARDING,
            to_json("onboarding settings", settings)?,
        )])
        .await?;

        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_autostart_enabled(&self, enabled: bool) -> Result<(), String> {
        self.write_entries(vec![(store_keys::AUTOSTART_ENABLED, Value::Bool(enabled))])
            .await?;

        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_advanced_settings_enabled(&self, enabled: bool) -> Result<(), String> {
        self.write_entries(vec![(
            store_keys::ADVANCED_SETTINGS_ENABLED,
            Value::Bool(enabled),
        )])
        .await?;

        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_cli_settings(&self, settings: &CLISettings) -> Result<(), String> {
        // Validate CLI settings
        if settings.command_timeout == 0 || settings.command_timeout > 3600 {
            return Err("Command timeout must be between 1 and 3600 seconds".to_string());
        }

        self.write_entries(vec![(store_keys::CLI, to_json("CLI settings", settings)?)])
            .await?;

        self.app_handle
            .emit(events::CLI_SETTINGS_CHANGED, settings)
            .map_err(|e| format!("Failed to emit CLI settings changed event: {}", e))?;
        self.emit_settings_changed().await;
        Ok(())
    }

    pub async fn set_voice_transcription_settings(
        &self,
        settings: &VoiceTranscriptionSettings,
    ) -> Result<(), String> {
        // Validate voice transcription settings
        if settings.sample_rate == 0 || settings.sample_rate > 96000 {
            return Err("Sample rate must be between 1 and 96000 Hz".to_string());
        }
        if settings.channels == 0 || settings.channels > 8 {
            return Err("Channels must be between 1 and 8".to_string());
        }
        if settings.buffer_duration_ms == 0 || settings.buffer_duration_ms > 10000 {
            return Err("Buffer duration must be between 1 and 10000 ms".to_string());
        }

        self.write_entries(vec![(
            store_keys::VOICE_TRANSCRIPTION,
            to_json("voice transcription settings", settings)?,
        )])
        .await?;

        self.app_handle
            .emit(events::VOICE_TRANSCRIPTION_SETTINGS_CHANGED, settings)
            .map_err(|e| {
                format!(
                    "Failed to emit voice transcription settings changed event: {}",
                    e
                )
            })?;
        self.emit_settings_changed().await;
        Ok(())
    }

    // Internal helpers
    fn get_keyboard_shortcuts_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<KeyboardShortcuts, String> {
        let mut shortcuts: KeyboardShortcuts = read_section(store, store_keys::KEYBOARD_SHORTCUTS);
        // Escape and Cmd+Comma stopped being settings. Normalizing them on
        // read, rather than trusting the store, is what stops a custom value
        // written by an older build from outliving the decision, and the next
        // save writes the normalized pair back.
        shortcuts.stop_current_task = defaults::STOP_CURRENT_TASK.to_string();
        shortcuts.open_settings = defaults::OPEN_SETTINGS.to_string();
        Ok(shortcuts)
    }

    fn get_floating_bar_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<FloatingBarSettings, String> {
        let mut bar: FloatingBarSettings = read_section(store, store_keys::FLOATING_BAR);
        // A look that is hidden from the picker shows as the default, silently.
        if crate::constants::ui::bar_appearances::is_hidden(&bar.bar_appearance) {
            bar.bar_appearance = crate::constants::ui::bar_appearances::DEFAULT.to_string();
        }
        Ok(bar)
    }

    fn get_agent_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<AgentSettings, String> {
        Ok(read_section(store, store_keys::AGENT))
    }

    fn get_provider_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<ProviderSettings, String> {
        Ok(read_section(store, store_keys::PROVIDERS))
    }

    fn get_cloud_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<CloudSettings, String> {
        Ok(read_section(store, store_keys::CLOUD))
    }

    fn get_audio_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<AudioSettings, String> {
        Ok(read_section(store, store_keys::AUDIO))
    }

    /// Read the unified triggers list. When the store predates the model (key
    /// missing or empty array), synthesize it from the legacy shortcut fields
    /// so an upgrading user keeps their setup; when it predates gestures,
    /// [`crate::triggers::load_stored`] brings it forward. The migrated
    /// list is not written back here; it persists on the next
    /// `save_all_settings`.
    fn get_triggers_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
        keyboard_shortcuts: &KeyboardShortcuts,
        agent: &AgentSettings,
        audio: &AudioSettings,
    ) -> Vec<crate::triggers::Trigger> {
        let stored: Option<Vec<crate::triggers::Trigger>> =
            store.get(store_keys::TRIGGERS).and_then(|v| {
                // The one door a stored list comes through: old method names
                // and retired gestures are settled here, on every load.
                match crate::triggers::load_stored(&v) {
                    Ok(triggers) => Some(triggers),
                    Err(e) => {
                        // One unreadable trigger discards the whole saved list
                        // and rebuilds a keyboard-only one from the legacy
                        // fields, so a binding the legacy fields cannot express
                        // (Fn, a mouse button) just disappears. Silently, until
                        // now: say so, because "my key stopped working after an
                        // update" is otherwise unanswerable.
                        warn!(
                            "[Settings] Stored triggers could not be read ({}); falling back to the legacy fields, which will drop any non-keyboard binding",
                            e
                        );
                        None
                    }
                }
            });

        match stored {
            Some(triggers) if !triggers.is_empty() => triggers,
            _ => crate::triggers::migrate_from_legacy(
                &keyboard_shortcuts.agent_mode,
                &agent.trigger_mode,
                &keyboard_shortcuts.dictation_input,
                &audio.dictation_trigger_mode,
                audio.always_listening_active,
                &audio.always_listening_wake_words,
            ),
        }
    }

    fn get_tool_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<ToolSettings, String> {
        Ok(read_section(store, store_keys::TOOLS))
    }

    fn get_prompt_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<PromptSettings, String> {
        Ok(read_section(store, store_keys::PROMPTS))
    }

    fn get_onboarding_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<OnboardingSettings, String> {
        Ok(read_section(store, store_keys::ONBOARDING))
    }

    fn get_cli_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<CLISettings, String> {
        Ok(read_section(store, store_keys::CLI))
    }

    fn get_voice_transcription_settings_from_store(
        &self,
        store: &tauri_plugin_store::Store<tauri::Wry>,
    ) -> Result<VoiceTranscriptionSettings, String> {
        Ok(read_section(store, store_keys::VOICE_TRANSCRIPTION))
    }

    /// Emit general settings changed event for full reactivity
    async fn emit_settings_changed(&self) {
        if let Ok(settings) = self.get_all_settings().await {
            if let Err(e) = self.app_handle.emit(events::SETTINGS_CHANGED, &settings) {
                eprintln!("Failed to emit settings changed event: {}", e);
            }
        }
    }

    /// Emit one section's event, logging rather than failing.
    ///
    /// A single setter returns the emit error, because the caller asked for
    /// that one change and deserves to hear that the announcement failed.
    /// A whole-settings write is announcing ten things, and giving up on the
    /// remaining nine because the first one failed would leave more of the UI
    /// stale, not less.
    fn emit_section_changed<T: serde::Serialize>(&self, event: &str, payload: &T) {
        if let Err(e) = self.app_handle.emit(event, payload) {
            warn!("Failed to emit {} event: {}", event, e);
        }
    }

    /// Announce a whole-settings write the way ten individual writes would.
    ///
    /// `save_all_settings` used to emit only `settings_changed`. Every pane
    /// follows its own section event instead, so a write that skipped those
    /// was invisible: "Reset all settings" changed the file and nothing on
    /// screen moved. Emitting per section here means a pane is subscribed to
    /// a reset by virtue of being subscribed to its own settings, so a pane
    /// added later is covered without anyone remembering to add it to a list.
    async fn emit_every_section_changed(&self, settings: &AppSettings) {
        self.emit_section_changed(
            events::KEYBOARD_SHORTCUTS_CHANGED,
            &settings.keyboard_shortcuts,
        );
        self.emit_section_changed(
            events::FLOATING_BAR_SETTINGS_CHANGED,
            &settings.floating_bar,
        );
        self.emit_section_changed(events::AGENT_SETTINGS_CHANGED, &settings.agent);
        self.emit_section_changed(events::PROVIDER_SETTINGS_CHANGED, &settings.providers);
        self.emit_section_changed(events::CLOUD_SETTINGS_CHANGED, &settings.cloud);
        self.emit_section_changed(events::AUDIO_SETTINGS_CHANGED, &settings.audio);
        self.emit_section_changed(events::TOOL_SETTINGS_CHANGED, &settings.tools);
        self.emit_section_changed(events::PROMPT_SETTINGS_CHANGED, &settings.prompts);
        self.emit_section_changed(events::CLI_SETTINGS_CHANGED, &settings.cli);
        self.emit_section_changed(
            events::VOICE_TRANSCRIPTION_SETTINGS_CHANGED,
            &settings.voice_transcription,
        );
        self.emit_settings_changed().await;
    }
}

fn to_json<T: Serialize>(what: &str, value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("Failed to serialize {}: {}", what, e))
}

/// Read one section, field by field: a missing section or field takes its
/// default, an unknown field is ignored, and one field of the wrong type
/// falls back on its own instead of taking the whole section with it.
fn read_section<T>(store: &Store<Wry>, key: &str) -> T
where
    T: DeserializeOwned + Serialize + Default,
{
    match store.get(key) {
        Some(value) => persist::lenient(key, &value),
        None => T::default(),
    }
}

//! # Trigger commands
//!
//! The frontend Triggers screen reads and writes the unified activation model
//! through these two commands. `triggers` is the source of truth; the legacy
//! shortcut / trigger-mode / wake-word fields are derived from it so existing
//! runtime consumers keep working (see [`crate::triggers::derive_legacy`]).

use tauri::{AppHandle, State};
use tracing::{error, info};

use crate::settings::manager::SettingsManager;
use crate::state::{self, AppState};
use crate::triggers::{self, Trigger};

/// Return the current activation triggers.
#[tauri::command]
pub async fn get_triggers(app_state: State<'_, AppState>) -> Result<Vec<Trigger>, String> {
    app_state.get_triggers()
}

/// Replace the activation triggers: validate, store in memory, persist, and
/// re-register global shortcuts / voice. Returns the normalized list, or an
/// error string the UI shows inline on the offending row.
#[tauri::command]
pub async fn set_triggers(
    app: AppHandle,
    triggers: Vec<Trigger>,
    app_state: State<'_, AppState>,
) -> Result<Vec<Trigger>, String> {
    let normalized = triggers::dedupe_by_key(triggers);

    // Bindings owned by the non-activation utility shortcuts are off-limits.
    let ks = app_state.get_keyboard_shortcuts()?;
    let reserved = vec![
        ks.stop_current_task.clone(),
        ks.open_settings.clone(),
        ks.voice_activation.clone(),
    ];
    triggers::validate(&normalized, &reserved)?;

    // 1. In-memory source of truth.
    app_state.set_triggers(normalized.clone())?;

    // 2. Keep the legacy in-memory fields coherent for peripheral consumers.
    sync_state_legacy(&app_state, &normalized, &ks);

    // 3. Persist to the store (triggers + derived legacy fields).
    persist(&app, &normalized).await?;

    // 4. Re-register global shortcuts from the new triggers.
    if let Err(e) = crate::commands::shortcuts::update_global_shortcuts(&app, &app_state).await {
        error!("[Triggers] Failed to re-register shortcuts: {}", e);
    }

    // 5. Drive the always-listening engine from the voice triggers.
    sync_voice_listener(&app, &app_state, &normalized).await;

    info!("[Triggers] Saved {} trigger(s)", normalized.len());
    Ok(normalized)
}

/// Start or stop the always-listening engine to match the voice triggers, and
/// push their phrases as wake words. Started first so the running worker
/// receives the wake-word update.
async fn sync_voice_listener(
    app: &AppHandle,
    app_state: &State<'_, AppState>,
    triggers: &[Trigger],
) {
    let voice_phrases: Vec<String> = triggers
        .iter()
        .filter(|t| t.enabled && t.is_voice())
        .flat_map(|t| t.voice_phrases())
        .collect();

    if voice_phrases.is_empty() {
        if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(
            app.clone(),
            app_state.clone(),
        )
        .await
        {
            // "already inactive" is returned as Ok; a real error is worth noting.
            error!("[Triggers] Failed to stop always-listening: {}", e);
        }
        return;
    }

    if let Err(e) = crate::commands::always_listening::start_always_listening_mode(
        app.clone(),
        app_state.clone(),
    )
    .await
    {
        error!("[Triggers] Failed to start always-listening: {}", e);
        return;
    }
    if let Err(e) = crate::commands::always_listening::set_always_listening_wake_words(
        voice_phrases,
        app.clone(),
        app_state.clone(),
    )
    .await
    {
        error!("[Triggers] Failed to set wake words: {}", e);
    }
}

fn agent_mode_enum(mode: &str) -> state::AgentTriggerMode {
    if mode.eq_ignore_ascii_case("hold") {
        state::AgentTriggerMode::Hold
    } else {
        state::AgentTriggerMode::Tap
    }
}

fn dictation_mode_enum(mode: &str) -> state::DictationTriggerMode {
    if mode.eq_ignore_ascii_case("tap") {
        state::DictationTriggerMode::Tap
    } else {
        state::DictationTriggerMode::Hold
    }
}

/// Mirror the derived legacy values into `AppState` so the dispatch path and
/// tray/onboarding reads stay in sync without a restart.
fn sync_state_legacy(app_state: &AppState, triggers: &[Trigger], prev: &state::KeyboardShortcuts) {
    let (agent_combo, agent_mode, dictation_combo, dictation_mode, _al, _words) =
        triggers::derive_legacy(triggers, &prev.agent_mode, &prev.dictation_input);

    let mut ks = prev.clone();
    ks.agent_mode = agent_combo;
    ks.dictation_input = dictation_combo;
    if let Err(e) = app_state.set_keyboard_shortcuts(ks) {
        error!(
            "[Triggers] Failed to sync keyboard shortcuts to state: {}",
            e
        );
    }
    if let Err(e) = app_state.set_agent_trigger_mode(agent_mode_enum(&agent_mode)) {
        error!(
            "[Triggers] Failed to sync agent trigger mode to state: {}",
            e
        );
    }
    if let Err(e) = app_state.set_dictation_trigger_mode(dictation_mode_enum(&dictation_mode)) {
        error!(
            "[Triggers] Failed to sync dictation trigger mode to state: {}",
            e
        );
    }
}

/// Write the triggers and their derived legacy fields to the centralized store.
async fn persist(app: &AppHandle, triggers: &[Trigger]) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let mut all = settings_manager.get_all_settings().await?;

    let (agent_combo, agent_mode, dictation_combo, dictation_mode, always_listening, wake_words) =
        triggers::derive_legacy(
            triggers,
            &all.keyboard_shortcuts.agent_mode,
            &all.keyboard_shortcuts.dictation_input,
        );
    all.keyboard_shortcuts.agent_mode = agent_combo;
    all.keyboard_shortcuts.dictation_input = dictation_combo;
    all.agent.trigger_mode = agent_mode;
    all.audio.dictation_trigger_mode = dictation_mode;
    all.audio.always_listening_active = always_listening;
    all.audio.always_listening_wake_words = wake_words;
    all.triggers = triggers.to_vec();

    settings_manager.save_all_settings(&all).await
}

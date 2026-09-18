//! # Trigger commands
//!
//! The frontend Triggers screen reads and writes the unified activation model
//! through these two commands. `triggers` is the source of truth; the legacy
//! shortcut / trigger-mode / wake-word fields are derived from it so existing
//! runtime consumers keep working (see [`crate::triggers::derive_legacy`]).

use tauri::{AppHandle, Manager, State};
use tracing::{error, info, warn};

use crate::settings::manager::SettingsManager;
use crate::state::{self, AppState};
use crate::triggers::{self, Trigger};

/// Return the current activation triggers.
#[tauri::command]
pub async fn get_triggers(app_state: State<'_, AppState>) -> Result<Vec<Trigger>, String> {
    app_state.get_triggers()
}

/// Replace the activation triggers: validate, store in memory, persist, and
/// Listen for a bare modifier key so setup can ask someone to press theirs.
///
/// While this is on, pressing Fn reports the key rather than starting
/// dictation: the person is choosing a binding, not using one. Setup asks
/// instead of interrogating the hardware, because "does this machine have an
/// Fn key" has no single answer once a second keyboard is plugged in, and the
/// press proves the key actually reaches Juno, which no capability check can.
#[tauri::command]
pub async fn set_trigger_capture(app: tauri::AppHandle, active: bool) -> Result<(), String> {
    crate::platform::modifier_key_monitor::set_capture(&app, active)
}

/// re-register global shortcuts / voice. Returns the normalized list, or an
/// error string the UI shows inline on the offending row.
#[tauri::command]
pub async fn set_triggers(
    app: AppHandle,
    triggers: Vec<Trigger>,
    app_state: State<'_, AppState>,
) -> Result<Vec<Trigger>, String> {
    // Validate what was actually sent, before deduping. Deduping first meant a
    // trigger that collided on (method, target) was dropped on the floor and
    // the command still returned Ok, so the row the person had just added
    // simply vanished with nothing said about it.
    let dropped = triggers.len() - triggers::dedupe_by_key(triggers.clone()).len();
    if dropped > 0 {
        return Err(
            "That combination of trigger and action already exists. Edit the existing one instead."
                .to_string(),
        );
    }
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
    apply_voice_triggers(&app, &normalized).await;

    info!("[Triggers] Saved {} trigger(s)", normalized.len());
    Ok(normalized)
}

/// Turn the voice triggers on or off to match `triggers`.
///
/// Called on save and again at startup. A keyboard trigger is re-registered
/// every launch by `update_global_shortcuts`; voice had no equivalent, so an
/// enabled wake phrase worked until the app was quit and then silently never
/// listened again. Voice is a trigger method like any other and is activated
/// on the same schedule as the rest.
pub async fn apply_voice_triggers(app: &AppHandle, triggers: &[Trigger]) {
    let app_state = app.state::<AppState>();
    sync_voice_listener(app, &app_state, triggers).await;
}

/// Activate the voice triggers already in state. Startup entry point.
pub async fn apply_stored_voice_triggers(app: &AppHandle) {
    // Nothing is listening in a process that has just started, whatever the
    // last run left on disk. Saying so first matters: the start path treats a
    // true flag as "already running" and returns without starting anything, so
    // a stale true from a previous session would keep the engine off forever.
    if let Err(e) = clear_stale_listening_flag(app).await {
        warn!("[Triggers] Could not clear the stale listening flag: {}", e);
    }

    let triggers = match app.state::<AppState>().get_triggers() {
        Ok(t) => t,
        Err(e) => {
            error!("[Triggers] Could not read triggers to start voice: {}", e);
            return;
        }
    };
    apply_voice_triggers(app, &triggers).await;
}

async fn clear_stale_listening_flag(app: &AppHandle) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    let mut audio = settings_manager.get_audio_settings().await?;
    if !audio.always_listening_active {
        return Ok(());
    }
    audio.always_listening_active = false;
    settings_manager.set_audio_settings(&audio).await?;
    let _ = app.state::<AppState>().set_always_listening_active(false);
    Ok(())
}

/// Start or stop the always-listening engine to match the voice triggers, and
/// push their phrases as wake words. Started first so the running worker
/// receives the wake-word update.
async fn sync_voice_listener(
    app: &AppHandle,
    app_state: &State<'_, AppState>,
    triggers: &[Trigger],
) {
    let voice_phrases = triggers::voice_phrases_for(triggers);

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
    // `always_listening_active` is owned by the engine, not derived here.
    // Writing the desired value first meant `start_always_listening_mode` read
    // its own "already active" early-return and returned success without ever
    // starting the controller, so a saved voice trigger never listened.
    let _ = always_listening;
    all.audio.always_listening_wake_words = wake_words;
    all.triggers = triggers.to_vec();

    settings_manager.save_all_settings(&all).await
}

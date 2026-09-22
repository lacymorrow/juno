use crate::constants::settings::dictation_insertion_modes;
use crate::settings::manager::SettingsManager;
use crate::state::AppState;
use tauri::{AppHandle, Manager, State};
use tracing::{error, info, warn};

// Command to set the dictation copy-to-clipboard toggle. The name predates
// the insertion-mode split: this toggle only controls whether the transcript
// is copied to the clipboard after a successful insert.
#[tauri::command]
pub async fn set_dictation_clipboard_enabled(
    app_handle: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.set_dictation_copy_to_clipboard(enabled);

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update state for backward compatibility
    state
        .set_dictation_clipboard_enabled(enabled)
        .map_err(|e| format!("Failed to set dictation_clipboard_enabled: {}", e))?;

    info!("Dictation copy-to-clipboard set to: {}", enabled);
    Ok(())
}

// Command to get the current dictation copy-to-clipboard setting
#[tauri::command]
pub async fn get_dictation_clipboard_enabled(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    let enabled = audio_settings.dictation_copy_to_clipboard();

    // Sync with state for backward compatibility
    state
        .set_dictation_clipboard_enabled(enabled)
        .map_err(|e| format!("Failed to set dictation_clipboard_enabled: {}", e))?;

    tracing::debug!("Current dictation copy-to-clipboard setting: {}", enabled);
    Ok(enabled)
}

/// Get the dictation text-insertion mode ("paste" or "clipboard_free").
#[tauri::command]
pub async fn get_dictation_insertion_mode(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    state
        .set_dictation_insertion_mode(audio_settings.dictation_insertion_mode.clone())
        .map_err(|e| format!("Failed to sync dictation insertion mode: {}", e))?;

    Ok(audio_settings.dictation_insertion_mode)
}

/// Set the dictation text-insertion mode ("paste" or "clipboard_free").
#[tauri::command]
pub async fn set_dictation_insertion_mode(
    app_handle: AppHandle,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if mode != dictation_insertion_modes::PASTE && mode != dictation_insertion_modes::CLIPBOARD_FREE
    {
        return Err(format!(
            "Invalid dictation insertion mode '{}'; expected '{}' or '{}'",
            mode,
            dictation_insertion_modes::PASTE,
            dictation_insertion_modes::CLIPBOARD_FREE
        ));
    }

    let settings_manager = SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.dictation_insertion_mode = mode.clone();

    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    state
        .set_dictation_insertion_mode(mode.clone())
        .map_err(|e| format!("Failed to sync dictation insertion mode: {}", e))?;

    info!("Dictation insertion mode set to: {}", mode);
    Ok(())
}

/// How one dictation insert behaves, decided from the two settings. The pure
/// decision is separated from the I/O so the whole matrix is unit-testable:
///
/// | insertion_mode | copy_to_clipboard | behaviour |
/// |---|---|---|
/// | paste | on  | write transcript, Cmd+V, no restore — transcript stays |
/// | paste | off | write transcript, Cmd+V, restore old clipboard (changeCount-guarded) |
/// | clipboard_free | on  | unicode CGEvent insert, then one clipboard write |
/// | clipboard_free | off | unicode CGEvent insert, clipboard untouched |
#[derive(Debug, PartialEq, Eq)]
struct InsertionPlan {
    /// Insert via unicode CGEvents instead of Cmd+V paste.
    clipboard_free: bool,
    /// Paste mode only: leave the transcript on the clipboard (no snapshot,
    /// no transient markers, no delayed restore).
    paste_retains_clipboard: bool,
    /// Write the transcript to the clipboard after a successful insert. Only
    /// the clipboard-free path needs an explicit copy; in paste mode the
    /// paste's own write already retains it.
    copy_after_insert: bool,
}

fn plan_insertion(insertion_mode: &str, copy_to_clipboard: bool) -> InsertionPlan {
    let clipboard_free = insertion_mode == dictation_insertion_modes::CLIPBOARD_FREE;
    InsertionPlan {
        clipboard_free,
        paste_retains_clipboard: !clipboard_free && copy_to_clipboard,
        copy_after_insert: clipboard_free && copy_to_clipboard,
    }
}

/// Insert a dictation transcript into the focused app using the configured
/// insertion mode and copy-to-clipboard toggle (see [`InsertionPlan`]).
///
/// The two settings are independent: an explicit copy happens after a
/// successful insert, never as a side effect of the insert path. Settings are
/// read from the store (not the state mirror) so the behaviour is correct
/// even when no frontend has run yet.
pub async fn insert_dictation_text(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    let from_store = match SettingsManager::new(app_handle.clone()) {
        Ok(manager) => manager.get_audio_settings().await.map(|audio| {
            (
                audio.dictation_insertion_mode.clone(),
                audio.dictation_copy_to_clipboard(),
            )
        }),
        Err(e) => Err(e),
    };
    let (insertion_mode, copy_to_clipboard) = from_store.unwrap_or_else(|e| {
        warn!(
            "[Dictation] Falling back to state mirror for insertion settings: {}",
            e
        );
        (
            app_state
                .get_dictation_insertion_mode()
                .unwrap_or_else(|_| dictation_insertion_modes::PASTE.to_string()),
            app_state.get_dictation_clipboard_enabled().unwrap_or(true),
        )
    });

    let plan = plan_insertion(&insertion_mode, copy_to_clipboard);

    let started = std::time::Instant::now();
    if plan.clipboard_free {
        let owned = text.to_string();
        let method = tokio::task::spawn_blocking(move || {
            computer_use_ai_sdk::insert_text_clipboard_free(&owned)
        })
        .await
        .map_err(|e| format!("Clipboard-free insertion task panicked: {}", e))?
        .map_err(|e| format!("Clipboard-free insertion failed: {}", e))?;
        info!(
            "[Dictation] Inserted {} chars in {} ms (mode=clipboard_free, path={})",
            text.chars().count(),
            started.elapsed().as_millis(),
            method
        );
    } else {
        let owned = text.to_string();
        let retain = plan.paste_retains_clipboard;
        tokio::task::spawn_blocking(move || computer_use_ai_sdk::paste_text_global(&owned, retain))
            .await
            .map_err(|e| format!("Paste insertion task panicked: {}", e))?
            .map_err(|e| format!("Paste insertion failed: {}", e))?;
        info!(
            "[Dictation] Inserted {} chars in {} ms (mode=paste, retain_clipboard={})",
            text.chars().count(),
            started.elapsed().as_millis(),
            retain
        );
    }

    // Mirror global_type_text's key-press visualization so dictation looks
    // the same in the UI regardless of insertion path.
    if let Err(e) = tauri::Emitter::emit(
        app_handle,
        crate::constants::events::ui::KEY_PRESS_VISUALIZATION,
        serde_json::json!({
            "key": format!(
                "Global: {}",
                text.chars()
                    .take(crate::constants::ui::text_display::MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH)
                    .collect::<String>()
            ),
            "modifier": null
        }),
    ) {
        warn!("[Dictation] Failed to emit key press visualization: {}", e);
    }

    if plan.copy_after_insert {
        if let Err(e) = crate::commands::core::set_clipboard(
            text.to_string(),
            app_handle.clone(),
            app_state.clone(),
        )
        .await
        {
            // The insert itself succeeded; a failed copy should not read as a
            // failed dictation.
            error!(
                "[Dictation] Inserted, but failed to copy transcript to clipboard: {}",
                e
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::settings::dictation_insertion_modes::{CLIPBOARD_FREE, PASTE};

    // One case per row of the behaviour matrix in the issue.

    #[test]
    fn paste_with_copy_on_retains_the_clipboard() {
        // SuperWhisper default: transcript lives on the clipboard afterwards.
        assert_eq!(
            plan_insertion(PASTE, true),
            InsertionPlan {
                clipboard_free: false,
                paste_retains_clipboard: true,
                copy_after_insert: false,
            }
        );
    }

    #[test]
    fn paste_with_copy_off_restores_the_old_clipboard() {
        assert_eq!(
            plan_insertion(PASTE, false),
            InsertionPlan {
                clipboard_free: false,
                paste_retains_clipboard: false,
                copy_after_insert: false,
            }
        );
    }

    #[test]
    fn clipboard_free_with_copy_on_writes_the_clipboard_once_after_insert() {
        assert_eq!(
            plan_insertion(CLIPBOARD_FREE, true),
            InsertionPlan {
                clipboard_free: true,
                paste_retains_clipboard: false,
                copy_after_insert: true,
            }
        );
    }

    #[test]
    fn clipboard_free_with_copy_off_never_touches_the_clipboard() {
        assert_eq!(
            plan_insertion(CLIPBOARD_FREE, false),
            InsertionPlan {
                clipboard_free: true,
                paste_retains_clipboard: false,
                copy_after_insert: false,
            }
        );
    }
}

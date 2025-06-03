use tauri::{State, Manager, AppHandle, Emitter};
use std::sync::{Arc, Mutex};
use crate::controller::VoiceController;
use crate::error::Error;
use crate::config::VoiceTranscriptionConfig;
use tracing::{info, error};

#[tauri::command]
pub async fn start_dictation<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<(), Error> {
    info!("[Plugin] start_dictation command called");

    let mut voice_controller = controller.lock()
        .map_err(|e| Error::LockError(format!("Failed to lock VoiceController: {}", e)))?;

    voice_controller.start_dictation(&app)?;

    // Emit started event through the plugin system
    app.emit("plugin:voice-transcription:dictation-started", ())
        .map_err(|e| Error::EventError(format!("Failed to emit dictation-started event: {}", e)))?;

    info!("[Plugin] Dictation started successfully");
    Ok(())
}

#[tauri::command]
pub async fn stop_dictation<R: tauri::Runtime>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] stop_dictation command called");

    let mut voice_controller = controller.lock()
        .map_err(|e| Error::LockError(format!("Failed to lock VoiceController: {}", e)))?;

    let result = voice_controller.stop_dictation()?;

    if result {
        // Emit stopped event through the plugin system
        app.emit("plugin:voice-transcription:dictation-stopped", ())
            .map_err(|e| Error::EventError(format!("Failed to emit dictation-stopped event: {}", e)))?;
    }

    info!("[Plugin] Dictation stopped: {}", result);
    Ok(result)
}

#[tauri::command]
pub async fn toggle_dictation<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] toggle_dictation command called");

    let mut voice_controller = controller.lock()
        .map_err(|e| Error::LockError(format!("Failed to lock VoiceController: {}", e)))?;

    let was_dictating = voice_controller.is_dictating();

    if was_dictating {
        voice_controller.stop_dictation()?;
        app.emit("plugin:voice-transcription:dictation-stopped", ())
            .map_err(|e| Error::EventError(format!("Failed to emit dictation-stopped event: {}", e)))?;
        Ok(false)
    } else {
        voice_controller.start_dictation(&app)?;
        app.emit("plugin:voice-transcription:dictation-started", ())
            .map_err(|e| Error::EventError(format!("Failed to emit dictation-started event: {}", e)))?;
        Ok(true)
    }
}

#[tauri::command]
pub async fn get_dictation_status(
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<bool, Error> {
    info!("[Plugin] get_dictation_status command called");

    let voice_controller = controller.lock()
        .map_err(|e| Error::LockError(format!("Failed to lock VoiceController: {}", e)))?;

    Ok(voice_controller.is_dictating())
}

#[tauri::command]
pub async fn transcribe_file(
    path: String,
    controller: State<'_, Arc<Mutex<VoiceController>>>,
) -> Result<String, Error> {
    info!("[Plugin] transcribe_file command called for path: {}", path);

    let voice_controller = controller.lock()
        .map_err(|e| Error::LockError(format!("Failed to lock VoiceController: {}", e)))?;

    voice_controller.transcribe_audio_file(&path)
        .map_err(|e| Error::TranscriptionError(e))
}

#[tauri::command]
pub async fn set_model_path<R: tauri::Runtime>(
    path: String,
    app: AppHandle<R>,
) -> Result<(), Error> {
    info!("[Plugin] set_model_path command called with path: {}", path);

    // Update config
    let mut config = VoiceTranscriptionConfig::default();
    config.model_path = path.clone();
    config.save()?;

    // Reinitialize the voice controller with the new model
    match VoiceController::new(&path) {
        Ok(controller) => {
            app.manage(Arc::new(Mutex::new(controller)));
            info!("[Plugin] Voice controller reinitialized with new model: {}", path);
            Ok(())
        }
        Err(e) => {
            error!("[Plugin] Failed to reinitialize voice controller: {}", e);
            Err(Error::ModelError(format!("Failed to load model: {}", e)))
        }
    }
}

#[tauri::command]
pub async fn get_model_path() -> Result<String, Error> {
    let config = VoiceTranscriptionConfig::default();
    Ok(config.model_path)
}

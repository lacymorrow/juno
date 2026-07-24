use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

const HUGGINGFACE_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

// (id, filename, display_name, size_mb, is_default)
const MODEL_DEFS: &[(&str, &str, &str, u32, bool)] = &[
    (
        "tiny-en",
        "ggml-tiny.en.bin",
        "Tiny — English only (~75MB, fastest)",
        75,
        false,
    ),
    (
        "small-en",
        "ggml-small.en.bin",
        "Small — English only (~466MB, balanced)",
        466,
        false,
    ),
    (
        "large-v3-turbo",
        "ggml-large-v3-turbo-q5_0.bin",
        "Large v3 Turbo — Multilingual (~600MB, best quality)",
        600,
        true,
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModelInfo {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub size_mb: u32,
    pub downloaded: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperDownloadProgress {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

#[derive(Default)]
pub struct WhisperDownloadState {
    pub active_model_id: Option<String>,
}

impl WhisperDownloadState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn get_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;
    Ok(data_dir.join("models"))
}

fn find_model_def(
    model_id: &str,
) -> Option<&'static (&'static str, &'static str, &'static str, u32, bool)> {
    MODEL_DEFS.iter().find(|m| m.0 == model_id)
}

#[tauri::command]
pub async fn get_whisper_models(app: AppHandle) -> Result<Vec<WhisperModelInfo>, String> {
    let models_dir = get_models_dir(&app)?;

    let models = MODEL_DEFS
        .iter()
        .map(|&(id, filename, display_name, size_mb, is_default)| {
            let downloaded = models_dir.join(filename).exists();
            WhisperModelInfo {
                id: id.to_string(),
                filename: filename.to_string(),
                display_name: display_name.to_string(),
                size_mb,
                downloaded,
                is_default,
            }
        })
        .collect();

    Ok(models)
}

#[tauri::command]
pub async fn get_current_whisper_model(app: AppHandle) -> Result<String, String> {
    use crate::settings::manager::SettingsManager;

    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let voice_settings = settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))?;

    let filename = std::path::Path::new(&voice_settings.model_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&voice_settings.model_path)
        .to_string();

    let model_id = MODEL_DEFS
        .iter()
        .find(|&&(_, fname, _, _, _)| fname == filename)
        .map(|&(id, _, _, _, _)| id.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(model_id)
}

#[tauri::command]
pub async fn get_whisper_download_status(
    download_state: tauri::State<'_, Arc<Mutex<WhisperDownloadState>>>,
) -> Result<Option<String>, String> {
    let state = download_state
        .lock()
        .map_err(|e| format!("Download state lock poisoned: {}", e))?;
    Ok(state.active_model_id.clone())
}

#[tauri::command]
pub async fn download_whisper_model(
    model_id: String,
    app: AppHandle,
    download_state: tauri::State<'_, Arc<Mutex<WhisperDownloadState>>>,
) -> Result<(), String> {
    let def = find_model_def(&model_id).ok_or_else(|| format!("Unknown model id: {}", model_id))?;

    let (_, filename, _, _, _) = *def;
    let models_dir = get_models_dir(&app)?;
    let model_path = models_dir.join(filename);

    if model_path.exists() {
        info!(
            "[WhisperModel] Model {} already downloaded at {:?}",
            model_id, model_path
        );
        let _ = app.emit(
            "whisper-download-complete",
            serde_json::json!({ "model_id": model_id }),
        );
        return Ok(());
    }

    {
        let mut state = download_state
            .lock()
            .map_err(|e| format!("Download state lock poisoned: {}", e))?;
        if state.active_model_id.is_some() {
            return Err("Another model download is already in progress".to_string());
        }
        state.active_model_id = Some(model_id.clone());
    }

    let url = format!("{}/{}", HUGGINGFACE_BASE_URL, filename);
    let model_id_bg = model_id.clone();
    let app_bg = app.clone();
    let download_state_bg = download_state.inner().clone();
    let filename_bg = filename.to_string();

    tauri::async_runtime::spawn(async move {
        info!(
            "[WhisperModel] Starting download of {} from {}",
            model_id_bg, url
        );

        match download_to_disk(&app_bg, &model_id_bg, &url, &models_dir, &filename_bg).await {
            Ok(()) => {
                info!("[WhisperModel] Download complete: {}", model_id_bg);
                let _ = app_bg.emit(
                    "whisper-download-complete",
                    serde_json::json!({ "model_id": model_id_bg }),
                );
            }
            Err(e) => {
                error!("[WhisperModel] Download failed for {}: {}", model_id_bg, e);
                let _ = app_bg.emit(
                    "whisper-download-error",
                    serde_json::json!({ "model_id": model_id_bg, "error": e }),
                );
            }
        }

        if let Ok(mut state) = download_state_bg.lock() {
            state.active_model_id = None;
        }
    });

    Ok(())
}

async fn download_to_disk(
    app: &AppHandle,
    model_id: &str,
    url: &str,
    models_dir: &PathBuf,
    filename: &str,
) -> Result<(), String> {
    // Guard against directory traversal (e.g. a crafted filename containing "..")
    if models_dir
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err("Invalid models directory: path traversal not allowed".to_string());
    }
    tokio::fs::create_dir_all(models_dir)
        .await
        .map_err(|e| format!("Failed to create models directory: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download from {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Server returned HTTP {}: {}",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("Unknown")
        ));
    }

    let total_bytes = response.content_length().unwrap_or(0);
    let tmp_path = models_dir.join(format!("{}.tmp", filename));
    let dest_path = models_dir.join(filename);

    info!(
        "[WhisperModel] Downloading {} bytes to {:?}",
        total_bytes, dest_path
    );

    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut bytes_downloaded: u64 = 0;
    let mut last_reported_percent: f32 = -1.0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Download stream error: {}", e))?;

        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write chunk: {}", e))?;

        bytes_downloaded += chunk.len() as u64;

        let percent = if total_bytes > 0 {
            (bytes_downloaded as f32 / total_bytes as f32) * 100.0
        } else {
            0.0
        };

        if percent - last_reported_percent >= 1.0 {
            let _ = app.emit(
                "whisper-download-progress",
                WhisperDownloadProgress {
                    model_id: model_id.to_string(),
                    bytes_downloaded,
                    total_bytes,
                    percent,
                },
            );
            last_reported_percent = percent;
        }
    }

    use tokio::io::AsyncWriteExt;
    file.flush()
        .await
        .map_err(|e| format!("Failed to flush file: {}", e))?;
    drop(file);

    tokio::fs::rename(&tmp_path, &dest_path)
        .await
        .map_err(|e| format!("Failed to move downloaded file: {}", e))?;

    info!("[WhisperModel] Download complete: {:?}", dest_path);
    Ok(())
}

#[tauri::command]
pub async fn set_whisper_model(model_id: String, app: AppHandle) -> Result<(), String> {
    let def = find_model_def(&model_id).ok_or_else(|| format!("Unknown model id: {}", model_id))?;

    let (_, filename, _, _, _) = *def;
    let models_dir = get_models_dir(&app)?;
    let model_path = models_dir.join(filename);

    if !model_path.exists() {
        return Err(format!(
            "Model {} is not downloaded yet. Use download_whisper_model first.",
            model_id
        ));
    }

    let path_str = model_path.to_string_lossy().to_string();

    use tauri_plugin_voice_transcription::SharedWhisperManager;

    info!(
        "[WhisperModel] Switching to model: {} at {}",
        model_id, path_str
    );
    let shared_context = SharedWhisperManager::reinitialize(&path_str)
        .map_err(|e| format!("Failed to load model: {}", e))?;

    if let Some(vc_state) =
        app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::VoiceController>>>()
    {
        match vc_state.try_lock() {
            Ok(mut vc) => {
                if let Err(e) = vc.update_shared_context(&path_str, shared_context.clone()) {
                    warn!("[WhisperModel] Failed to update VoiceController: {}", e);
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[WhisperModel] VoiceController busy — will use new model on next use");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[WhisperModel] VoiceController mutex poisoned: {}", e);
            }
        }
    }

    if let Some(al_state) =
        app.try_state::<Arc<Mutex<tauri_plugin_voice_transcription::AlwaysListeningController>>>()
    {
        match al_state.try_lock() {
            Ok(mut al) => {
                if let Err(e) = al.update_shared_context(&path_str, shared_context.clone()) {
                    warn!(
                        "[WhisperModel] Failed to update AlwaysListeningController: {}",
                        e
                    );
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[WhisperModel] AlwaysListeningController busy — will use new model on next use");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!(
                    "[WhisperModel] AlwaysListeningController mutex poisoned: {}",
                    e
                );
            }
        }
    }

    use crate::settings::manager::SettingsManager;
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut voice_settings = settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))?;

    voice_settings.model_path = format!("models/{}", filename);

    settings_manager
        .set_voice_transcription_settings(&voice_settings)
        .await
        .map_err(|e| format!("Failed to save voice settings: {}", e))?;

    let _ = app.emit(
        "whisper-model-switched",
        serde_json::json!({ "model_id": model_id }),
    );

    info!("[WhisperModel] Switched to model: {}", model_id);
    Ok(())
}

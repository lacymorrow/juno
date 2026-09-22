//! Speech-to-text models: the catalog, what is on disk, what the engine is
//! really running, and the four things a person can do about it (download,
//! cancel, use, delete). The Settings > Models pane and the onboarding offer
//! are display-only over these commands: one status call answers everything,
//! and events carry download progress and switches. The UI never does
//! anything engine-specific; every command takes a catalog id and dispatches
//! by engine internally.
//!
//! Two engines, one list. Whisper models are single ggml files; Parakeet is a
//! directory of ONNX files (manifest in the voice plugin, next to its loader).
//! Every download streams into `<app data>/models/.partial/<id>/` and is moved
//! into place only once every byte is verified, so a loader never finds a
//! half file.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

use tauri_plugin_voice_transcription::{
    downloaded_models_dir, parakeet_file_url, parakeet_total_bytes, resolve_model_path,
    resolve_parakeet_model_dir, AlwaysListeningController, EngineManager, SharedWhisperManager,
    SttProvider, TranscriptionEngine, VoiceController, VoiceTranscriptionConfig,
    PARAKEET_MODEL_FILES,
};

use crate::constants::events::stt_models as events;
use crate::settings::manager::SettingsManager;
use crate::settings::VoiceTranscriptionSettings;

const WHISPER_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Catalog id of the Parakeet model (the only non-Whisper entry).
pub const PARAKEET_ID: &str = "parakeet-ctc";
/// Catalog id of the model that ships inside the app bundle.
pub const BUNDLED_ID: &str = "tiny-en";

/// Staging directory for in-flight downloads, inside the models dir.
const PARTIAL_DIR: &str = ".partial";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Whisper,
    Parakeet,
}

impl Engine {
    fn as_str(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper",
            Engine::Parakeet => "parakeet",
        }
    }
}

/// One catalog entry. Speed and accuracy are 1..=5 pips, set from published
/// numbers, not vibes:
/// - Word error rate (Open ASR leaderboard style average over the eight
///   standard English sets): large-v3 7.4, Parakeet CTC 0.6B 7.7 (its model
///   card, greedy decoding), large-v3-turbo 7.8, small.en 8.6, tiny.en 12.8.
/// - Speed: relative decode cost on Apple Silicon. tiny.en is about a tenth of
///   turbo's compute; Parakeet is non-autoregressive (one encoder pass per
///   utterance, 4000+ RTFx on GPU per its card) so it lands with the small
///   models despite its size; large-v3 is roughly five times turbo.
#[derive(Debug, Clone, Copy)]
pub struct ModelDef {
    pub id: &'static str,
    pub engine: Engine,
    pub name: &'static str,
    /// ggml filename for Whisper models; unused for Parakeet.
    pub filename: &'static str,
    pub size_mb: u32,
    pub bundled: bool,
    /// Parakeet only builds on Apple Silicon (parakeet-rs / ort).
    pub arm64_only: bool,
    pub speed: u8,
    pub accuracy: u8,
}

const CATALOG: &[ModelDef] = &[
    ModelDef {
        id: BUNDLED_ID,
        engine: Engine::Whisper,
        name: "Whisper tiny.en",
        filename: "ggml-tiny.en.bin",
        size_mb: 75,
        bundled: true,
        arm64_only: false,
        speed: 5,
        accuracy: 2,
    },
    ModelDef {
        id: PARAKEET_ID,
        engine: Engine::Parakeet,
        name: "Parakeet CTC 0.6B",
        filename: "",
        size_mb: 612,
        bundled: false,
        arm64_only: true,
        speed: 4,
        accuracy: 4,
    },
    ModelDef {
        id: "large-v3-turbo",
        engine: Engine::Whisper,
        name: "Whisper large-v3-turbo",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 600,
        bundled: false,
        arm64_only: false,
        speed: 3,
        accuracy: 4,
    },
    ModelDef {
        id: "large-v3",
        engine: Engine::Whisper,
        name: "Whisper large-v3",
        filename: "ggml-large-v3.bin",
        size_mb: 3100,
        bundled: false,
        arm64_only: false,
        speed: 1,
        accuracy: 5,
    },
    ModelDef {
        id: "small-en",
        engine: Engine::Whisper,
        name: "Whisper small.en",
        filename: "ggml-small.en.bin",
        size_mb: 466,
        bundled: false,
        arm64_only: false,
        speed: 4,
        accuracy: 3,
    },
];

/// The three plain-language rows everyone sees. Advanced settings reveal the
/// rest of the catalog in the same list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tier {
    pub key: &'static str,
    pub name: &'static str,
    pub recommended: bool,
}

const FAST: Tier = Tier {
    key: "fast",
    name: "Fast",
    recommended: false,
};
const BALANCED: Tier = Tier {
    key: "balanced",
    name: "Balanced",
    recommended: true,
};
const ACCURATE: Tier = Tier {
    key: "accurate",
    name: "Most accurate",
    recommended: false,
};

pub fn is_arm64(arch: &str) -> bool {
    arch == "aarch64" || arch == "arm64"
}

/// The CPU architecture the catalog is built for. Debug builds honour
/// `JUNO_DICTATION_ARCH=x86_64` so the Intel row set can be checked on an
/// Apple Silicon machine (the download and engine code paths are unaffected).
pub fn current_arch() -> String {
    #[cfg(debug_assertions)]
    if let Ok(forced) = std::env::var("JUNO_DICTATION_ARCH") {
        if !forced.is_empty() {
            return forced;
        }
    }
    std::env::consts::ARCH.to_string()
}

/// Which tier a model fills on this architecture, if any. Balanced is the
/// recommended default: Parakeet on Apple Silicon, large-v3-turbo on Intel.
pub fn tier_for(arch: &str, id: &str) -> Option<Tier> {
    let balanced_id = if is_arm64(arch) {
        PARAKEET_ID
    } else {
        "large-v3-turbo"
    };
    if id == BUNDLED_ID {
        Some(FAST)
    } else if id == balanced_id {
        Some(BALANCED)
    } else if id == "large-v3" {
        Some(ACCURATE)
    } else {
        None
    }
}

/// The catalog for an architecture: tier rows first (Fast, Balanced, Most
/// accurate), then the rest; Parakeet is absent on Intel.
pub fn catalog_for_arch(arch: &str) -> Vec<&'static ModelDef> {
    let mut models: Vec<&ModelDef> = CATALOG
        .iter()
        .filter(|m| !m.arm64_only || is_arm64(arch))
        .collect();
    let rank = |m: &ModelDef| match tier_for(arch, m.id).map(|t| t.key) {
        Some("fast") => 0,
        Some("balanced") => 1,
        Some("accurate") => 2,
        _ => 3,
    };
    models.sort_by_key(|m| rank(m));
    models
}

pub fn find_def(id: &str) -> Option<&'static ModelDef> {
    CATALOG.iter().find(|m| m.id == id)
}

/// The recommended (Balanced) model id for an architecture.
pub fn recommended_id(arch: &str) -> &'static str {
    if is_arm64(arch) {
        PARAKEET_ID
    } else {
        "large-v3-turbo"
    }
}

// ---------------------------------------------------------------------------
// Serialized shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttModelInfo {
    pub id: String,
    /// "whisper" | "parakeet"
    pub engine: String,
    pub name: String,
    pub size_mb: u32,
    pub downloaded: bool,
    /// Ships with the app; always present, never deletable.
    pub bundled: bool,
    /// The engine is running this model right now. Exactly one row is active.
    pub active: bool,
    /// "fast" | "balanced" | "accurate" for the three headline rows.
    pub tier: Option<String>,
    pub tier_name: Option<String>,
    pub recommended: bool,
    pub speed: u8,
    pub accuracy: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: f32,
    /// The caller asked for the model to become active when it lands (the
    /// onboarding offer does; the Models pane waits for a tap on Use).
    pub activate_when_done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttModelsStatus {
    pub arch: String,
    pub models: Vec<SttModelInfo>,
    pub active_id: String,
    /// The one download in flight, if any.
    pub download: Option<DownloadProgress>,
    /// Onboarding should offer the recommended download: Apple Silicon,
    /// Parakeet not on disk, and the person has not declined or chosen a
    /// model by hand.
    pub offer_recommended: bool,
}

// ---------------------------------------------------------------------------
// Download state (managed by Tauri)
// ---------------------------------------------------------------------------

struct ActiveDownload {
    model_id: String,
    activate_when_done: bool,
    cancel: Arc<AtomicBool>,
    last_progress: Option<DownloadProgress>,
}

#[derive(Default)]
pub struct SttDownloadState {
    active: Option<ActiveDownload>,
}

pub type SharedDownloadState = Arc<Mutex<SttDownloadState>>;

impl SttDownloadState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn download_state(app: &AppHandle) -> Result<SharedDownloadState, String> {
    app.try_state::<SharedDownloadState>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| "Download state not initialized".to_string())
}

fn active_download_id(state: &SharedDownloadState) -> Option<String> {
    state
        .lock()
        .ok()
        .and_then(|s| s.active.as_ref().map(|a| a.model_id.clone()))
}

// ---------------------------------------------------------------------------
// Paths and on-disk state
// ---------------------------------------------------------------------------

fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    downloaded_models_dir(app).ok_or_else(|| "Failed to resolve app data dir".to_string())
}

fn parakeet_dir(app: &AppHandle) -> String {
    let config = VoiceTranscriptionConfig::default();
    resolve_parakeet_model_dir(app, &config.parakeet_model_dir)
}

/// Absolute path of a Whisper model if it is on disk (bundled or downloaded).
fn whisper_path_on_disk(app: &AppHandle, def: &ModelDef) -> Option<PathBuf> {
    if def.bundled {
        let resolved = resolve_model_path(app, &format!("models/{}", def.filename));
        let path = PathBuf::from(resolved);
        if path.is_file() {
            return Some(path);
        }
    }
    let path = models_dir(app).ok()?.join(def.filename);
    path.is_file().then_some(path)
}

fn is_downloaded(app: &AppHandle, def: &ModelDef) -> bool {
    match def.engine {
        Engine::Whisper => whisper_path_on_disk(app, def).is_some(),
        Engine::Parakeet => {
            tauri_plugin_voice_transcription::missing_parakeet_files(Path::new(&parakeet_dir(app)))
                .is_empty()
        }
    }
}

fn whisper_id_for_path(path: &str) -> Option<&'static str> {
    let filename = Path::new(path).file_name()?.to_str()?;
    CATALOG
        .iter()
        .find(|m| m.engine == Engine::Whisper && m.filename == filename)
        .map(|m| m.id)
}

/// The model the engine is really running. Asks the engine first; before it
/// has finished booting, falls back to what settings ask for, provided that
/// model is on disk (otherwise the plugin will have fallen back to tiny.en).
fn active_model_id(app: &AppHandle, settings: &VoiceTranscriptionSettings) -> String {
    match EngineManager::current_provider_name() {
        "parakeet" => return PARAKEET_ID.to_string(),
        "whisper" => {
            if let Some(id) = SharedWhisperManager::current_model_path()
                .as_deref()
                .and_then(whisper_id_for_path)
            {
                return id.to_string();
            }
        }
        _ => {}
    }

    if settings.stt_provider.eq_ignore_ascii_case("parakeet") {
        if let Some(def) = find_def(PARAKEET_ID) {
            if is_downloaded(app, def) {
                return PARAKEET_ID.to_string();
            }
        }
    }
    if let Some(id) = whisper_id_for_path(&settings.model_path) {
        if find_def(id).is_some_and(|d| is_downloaded(app, d)) {
            return id.to_string();
        }
    }
    BUNDLED_ID.to_string()
}

async fn load_voice_settings(app: &AppHandle) -> Result<VoiceTranscriptionSettings, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    settings_manager
        .get_voice_transcription_settings()
        .await
        .map_err(|e| format!("Failed to get voice settings: {}", e))
}

async fn save_voice_settings(
    app: &AppHandle,
    settings: &VoiceTranscriptionSettings,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;
    settings_manager
        .set_voice_transcription_settings(settings)
        .await
        .map_err(|e| format!("Failed to save voice settings: {}", e))
}

/// Build the whole status in one pass; the UI never polls per model.
pub async fn stt_models_status(app: &AppHandle) -> Result<SttModelsStatus, String> {
    let settings = load_voice_settings(app).await?;
    let arch = current_arch();
    let active_id = active_model_id(app, &settings);

    let download = download_state(app).ok().and_then(|state| {
        state.lock().ok().and_then(|s| {
            s.active.as_ref().map(|a| {
                a.last_progress.clone().unwrap_or(DownloadProgress {
                    model_id: a.model_id.clone(),
                    bytes_downloaded: 0,
                    total_bytes: expected_total_bytes(&a.model_id),
                    percent: 0.0,
                    activate_when_done: a.activate_when_done,
                })
            })
        })
    });

    let models = catalog_for_arch(&arch)
        .into_iter()
        .map(|def| {
            let tier = tier_for(&arch, def.id);
            SttModelInfo {
                id: def.id.to_string(),
                engine: def.engine.as_str().to_string(),
                name: def.name.to_string(),
                size_mb: def.size_mb,
                downloaded: is_downloaded(app, def),
                bundled: def.bundled,
                active: def.id == active_id,
                tier: tier.map(|t| t.key.to_string()),
                tier_name: tier.map(|t| t.name.to_string()),
                recommended: tier.is_some_and(|t| t.recommended),
                speed: def.speed,
                accuracy: def.accuracy,
            }
        })
        .collect();

    let offer_recommended = is_arm64(&arch)
        && !settings.parakeet_download_declined
        && !settings.stt_provider.eq_ignore_ascii_case("parakeet")
        && !find_def(PARAKEET_ID).is_some_and(|d| is_downloaded(app, d));

    Ok(SttModelsStatus {
        arch,
        models,
        active_id,
        download,
        offer_recommended,
    })
}

#[tauri::command]
pub async fn get_stt_models_status(app: AppHandle) -> Result<SttModelsStatus, String> {
    stt_models_status(&app).await
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

struct FetchFile {
    url: String,
    name: String,
    /// Exact size when the manifest knows it; the server's Content-Length
    /// otherwise.
    expected_bytes: Option<u64>,
}

fn fetch_plan(def: &ModelDef) -> Vec<FetchFile> {
    match def.engine {
        Engine::Whisper => vec![FetchFile {
            url: format!("{}/{}", WHISPER_BASE_URL, def.filename),
            name: def.filename.to_string(),
            expected_bytes: None,
        }],
        Engine::Parakeet => PARAKEET_MODEL_FILES
            .iter()
            .map(|f| FetchFile {
                url: parakeet_file_url(f),
                name: f.name.to_string(),
                expected_bytes: Some(f.bytes),
            })
            .collect(),
    }
}

fn expected_total_bytes(model_id: &str) -> u64 {
    match find_def(model_id) {
        Some(def) if def.engine == Engine::Parakeet => parakeet_total_bytes(),
        Some(def) => u64::from(def.size_mb) * 1024 * 1024,
        None => 0,
    }
}

enum DownloadOutcome {
    Done,
    Cancelled,
    Failed(String),
}

/// Start a download in the background. Returns as soon as it is queued; progress
/// and completion arrive as events. With `activate_when_done` the engine
/// switches to the model as soon as it lands (the onboarding offer); the
/// Models pane leaves that to a tap on Use.
pub fn start_download(
    app: &AppHandle,
    model_id: &str,
    activate_when_done: bool,
) -> Result<(), String> {
    let def = find_def(model_id).ok_or_else(|| format!("Unknown dictation model: {}", model_id))?;
    if def.arm64_only && !is_arm64(&current_arch()) {
        return Err(format!("{} is only available on Apple Silicon.", def.name));
    }
    if is_downloaded(app, def) {
        let _ = app.emit(
            events::DOWNLOAD_COMPLETE,
            serde_json::json!({ "model_id": model_id, "activated": false }),
        );
        return Ok(());
    }

    let state = download_state(app)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state
            .lock()
            .map_err(|e| format!("Download state lock poisoned: {}", e))?;
        if let Some(active) = &guard.active {
            return Err(format!(
                "{} is still downloading. Wait for it or cancel it first.",
                find_def(&active.model_id)
                    .map(|d| d.name)
                    .unwrap_or("Another model")
            ));
        }
        guard.active = Some(ActiveDownload {
            model_id: model_id.to_string(),
            activate_when_done,
            cancel: cancel.clone(),
            last_progress: None,
        });
    }

    let app = app.clone();
    let model_id = model_id.to_string();
    tauri::async_runtime::spawn(async move {
        info!(
            "[SttModels] Downloading {} ({}, {} MB){}",
            model_id,
            def.name,
            def.size_mb,
            if activate_when_done {
                " [activate when done]"
            } else {
                ""
            }
        );
        let outcome = match run_download(&app, def, &state, &cancel, activate_when_done).await {
            Ok(()) => DownloadOutcome::Done,
            Err(e) if cancel.load(Ordering::SeqCst) => {
                info!("[SttModels] Download of {} cancelled ({})", model_id, e);
                DownloadOutcome::Cancelled
            }
            Err(e) => {
                error!("[SttModels] Download of {} failed: {}", model_id, e);
                DownloadOutcome::Failed(e)
            }
        };

        if let Ok(mut guard) = state.lock() {
            guard.active = None;
        }

        match outcome {
            DownloadOutcome::Done => {
                let activated = if activate_when_done {
                    activate_after_download(&app, &model_id).await
                } else {
                    false
                };
                let _ = app.emit(
                    events::DOWNLOAD_COMPLETE,
                    serde_json::json!({ "model_id": model_id, "activated": activated }),
                );
            }
            DownloadOutcome::Cancelled => {
                let _ = app.emit(
                    events::DOWNLOAD_ERROR,
                    serde_json::json!({ "model_id": model_id, "error": "Cancelled", "cancelled": true }),
                );
            }
            DownloadOutcome::Failed(e) => {
                let _ = app.emit(
                    events::DOWNLOAD_ERROR,
                    serde_json::json!({ "model_id": model_id, "error": e, "cancelled": false }),
                );
            }
        }
    });

    Ok(())
}

async fn run_download(
    app: &AppHandle,
    def: &ModelDef,
    state: &SharedDownloadState,
    cancel: &AtomicBool,
    activate_when_done: bool,
) -> Result<(), String> {
    let models_dir = models_dir(app)?;
    if models_dir
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err("Invalid models directory: path traversal not allowed".to_string());
    }
    let staging = models_dir.join(PARTIAL_DIR).join(def.id);
    let _ = tokio::fs::remove_dir_all(&staging).await;
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|e| format!("Failed to create download directory: {}", e))?;

    let result = fetch_all(app, def, state, cancel, activate_when_done, &staging).await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(e);
    }

    // Every file is complete and the right size: move into place in one step.
    match def.engine {
        Engine::Whisper => {
            let dest = models_dir.join(def.filename);
            tokio::fs::rename(staging.join(def.filename), &dest)
                .await
                .map_err(|e| format!("Failed to move downloaded file into place: {}", e))?;
            let _ = tokio::fs::remove_dir_all(&staging).await;
        }
        Engine::Parakeet => {
            let dest = PathBuf::from(parakeet_dir(app));
            if dest.exists() {
                tokio::fs::remove_dir_all(&dest)
                    .await
                    .map_err(|e| format!("Failed to replace old Parakeet files: {}", e))?;
            }
            if let Some(parent) = dest.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            tokio::fs::rename(&staging, &dest)
                .await
                .map_err(|e| format!("Failed to move downloaded files into place: {}", e))?;
        }
    }
    info!("[SttModels] {} is on disk", def.id);
    Ok(())
}

async fn fetch_all(
    app: &AppHandle,
    def: &ModelDef,
    state: &SharedDownloadState,
    cancel: &AtomicBool,
    activate_when_done: bool,
    staging: &Path,
) -> Result<(), String> {
    let files = fetch_plan(def);
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .read_timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Manifest sizes are exact; a Whisper file's size is learned from the
    // server's Content-Length when its response arrives.
    let mut total_bytes: u64 = files.iter().filter_map(|f| f.expected_bytes).sum();
    let mut bytes_downloaded: u64 = 0;
    let mut last_reported_percent: f32 = -1.0;
    let mut last_report = std::time::Instant::now();

    let mut report = |bytes_downloaded: u64, total_bytes: u64, force: bool| {
        let percent = if total_bytes > 0 {
            (bytes_downloaded as f32 / total_bytes as f32) * 100.0
        } else {
            0.0
        };
        let due = percent - last_reported_percent >= 1.0
            || last_report.elapsed() >= std::time::Duration::from_millis(500);
        if !(force || due) {
            return;
        }
        let progress = DownloadProgress {
            model_id: def.id.to_string(),
            bytes_downloaded,
            total_bytes,
            percent,
            activate_when_done,
        };
        if let Ok(mut guard) = state.lock() {
            if let Some(active) = guard.active.as_mut() {
                active.last_progress = Some(progress.clone());
            }
        }
        let _ = app.emit(events::DOWNLOAD_PROGRESS, progress);
        last_reported_percent = percent;
        last_report = std::time::Instant::now();
    };

    report(0, total_bytes, true);

    for file in &files {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".to_string());
        }
        let response = client
            .get(&file.url)
            .send()
            .await
            .map_err(|e| format!("Could not reach {}: {}", file.url, e))?;
        if !response.status().is_success() {
            return Err(format!(
                "Server returned HTTP {} for {}",
                response.status().as_u16(),
                file.name
            ));
        }
        let content_length = response.content_length().unwrap_or(0);
        let expected = file
            .expected_bytes
            .or((content_length > 0).then_some(content_length));
        if file.expected_bytes.is_none() && content_length > 0 {
            total_bytes += content_length;
        }
        if let (Some(exp), true) = (file.expected_bytes, content_length > 0) {
            if exp != content_length {
                return Err(format!(
                    "{} changed upstream (expected {} bytes, server offers {})",
                    file.name, exp, content_length
                ));
            }
        }

        let tmp_path = staging.join(&file.name);
        let mut out = tokio::fs::File::create(&tmp_path)
            .await
            .map_err(|e| format!("Failed to create {}: {}", tmp_path.display(), e))?;
        let mut stream = response.bytes_stream();
        let mut file_bytes: u64 = 0;

        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Err("cancelled".to_string());
            }
            let chunk = chunk.map_err(|e| format!("Download interrupted: {}", e))?;
            use tokio::io::AsyncWriteExt;
            out.write_all(&chunk)
                .await
                .map_err(|e| format!("Failed to write {}: {}", file.name, e))?;
            file_bytes += chunk.len() as u64;
            bytes_downloaded += chunk.len() as u64;
            report(bytes_downloaded, total_bytes, false);
        }

        use tokio::io::AsyncWriteExt;
        out.flush()
            .await
            .map_err(|e| format!("Failed to flush {}: {}", file.name, e))?;
        drop(out);

        if let Some(exp) = expected {
            if file_bytes != exp {
                return Err(format!(
                    "{} is incomplete ({} of {} bytes)",
                    file.name, file_bytes, exp
                ));
            }
        }
    }

    report(bytes_downloaded, total_bytes, true);
    Ok(())
}

/// Download a model. `activate` makes it the active model as soon as it lands
/// (the onboarding offer); the Models pane passes false and waits for Use.
#[tauri::command]
pub async fn download_stt_model(
    model_id: String,
    activate: Option<bool>,
    app: AppHandle,
) -> Result<(), String> {
    start_download(&app, &model_id, activate.unwrap_or(false))
}

/// The person said "Not now" to the Parakeet offer. Remembered so onboarding
/// never asks again; the Models pane still offers Download.
#[tauri::command]
pub async fn decline_stt_model_offer(app: AppHandle) -> Result<(), String> {
    let mut settings = load_voice_settings(&app).await?;
    settings.parakeet_download_declined = true;
    save_voice_settings(&app, &settings).await
}

#[tauri::command]
pub async fn cancel_stt_model_download(app: AppHandle) -> Result<(), String> {
    let state = download_state(&app)?;
    let guard = state
        .lock()
        .map_err(|e| format!("Download state lock poisoned: {}", e))?;
    match &guard.active {
        Some(active) => {
            active.cancel.store(true, Ordering::SeqCst);
            Ok(())
        }
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

fn apply_engine_to_controllers(app: &AppHandle, engine: Arc<dyn TranscriptionEngine>) {
    if let Some(vc_state) = app.try_state::<Arc<Mutex<VoiceController>>>() {
        match vc_state.try_lock() {
            Ok(mut vc) => {
                if let Err(e) = vc.update_engine(engine.clone()) {
                    warn!("[SttModels] VoiceController engine swap failed: {}", e);
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[SttModels] VoiceController busy; new engine applies on next recording");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!("[SttModels] VoiceController mutex poisoned: {}", e);
            }
        }
    }
    if let Some(al_state) = app.try_state::<Arc<Mutex<AlwaysListeningController>>>() {
        match al_state.try_lock() {
            Ok(mut al) => {
                if let Err(e) = al.update_engine(engine) {
                    warn!(
                        "[SttModels] AlwaysListeningController engine swap failed: {}",
                        e
                    );
                }
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                warn!("[SttModels] AlwaysListeningController busy; new engine applies on next session");
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                error!(
                    "[SttModels] AlwaysListeningController mutex poisoned: {}",
                    e
                );
            }
        }
    }
}

/// Load `model_id` into the engine (blocking work runs off the async runtime)
/// and hand it to the controllers. Errors name the model and the reason.
async fn switch_engine_to(app: &AppHandle, model_id: &str) -> Result<(), String> {
    let def = find_def(model_id).ok_or_else(|| format!("Unknown dictation model: {}", model_id))?;
    let engine = match def.engine {
        Engine::Whisper => {
            let path = whisper_path_on_disk(app, def)
                .ok_or_else(|| format!("{} is not downloaded yet.", def.name))?;
            let path_str = path.to_string_lossy().to_string();
            tokio::task::spawn_blocking(move || EngineManager::reload_whisper(&path_str))
                .await
                .map_err(|e| format!("Model load task failed: {}", e))?
                .map_err(|e| format!("{} could not be loaded: {}", def.name, e))?
        }
        Engine::Parakeet => {
            let dir = parakeet_dir(app);
            let whisper_path =
                resolve_model_path(app, &VoiceTranscriptionConfig::default().model_path);
            tokio::task::spawn_blocking(move || {
                EngineManager::switch(SttProvider::Parakeet, &whisper_path, Some(&dir))
            })
            .await
            .map_err(|e| format!("Model load task failed: {}", e))?
            .map_err(|e| format!("{} could not be loaded: {}", def.name, e))?
        }
    };
    apply_engine_to_controllers(app, engine);
    Ok(())
}

async fn persist_choice(app: &AppHandle, def: &ModelDef, settle_offer: bool) -> Result<(), String> {
    let mut settings = load_voice_settings(app).await?;
    match def.engine {
        Engine::Whisper => {
            settings.stt_provider = "whisper".to_string();
            settings.model_path = format!("models/{}", def.filename);
        }
        Engine::Parakeet => {
            settings.stt_provider = "parakeet".to_string();
        }
    }
    if settle_offer {
        settings.parakeet_download_declined = true;
    }
    save_voice_settings(app, &settings).await
}

/// Make `model_id` the active dictation model: load it, hand it to the
/// controllers, persist the choice. Choosing a model by hand also settles the
/// Parakeet offer, so onboarding never asks again.
#[tauri::command]
pub async fn use_stt_model(model_id: String, app: AppHandle) -> Result<(), String> {
    let def =
        find_def(&model_id).ok_or_else(|| format!("Unknown dictation model: {}", model_id))?;
    if !is_downloaded(&app, def) {
        return Err(format!("{} is not downloaded yet.", def.name));
    }
    switch_engine_to(&app, &model_id).await?;
    persist_choice(&app, def, true).await?;
    let _ = app.emit(
        events::CHANGED,
        serde_json::json!({ "model_id": model_id, "automatic": false }),
    );
    info!("[SttModels] Active model is now {}", model_id);
    Ok(())
}

/// A download the caller asked to activate has landed: switch to it, persist
/// the choice. Returns whether the switch happened; a failure leaves the row
/// at Use with the reason in the log, never a dialog.
async fn activate_after_download(app: &AppHandle, model_id: &str) -> bool {
    let def = match find_def(model_id) {
        Some(d) => d,
        None => return false,
    };
    match switch_engine_to(app, model_id).await {
        Ok(()) => {
            if let Err(e) = persist_choice(app, def, false).await {
                warn!("[SttModels] Could not persist model choice: {}", e);
            }
            let _ = app.emit(
                events::CHANGED,
                serde_json::json!({ "model_id": model_id, "automatic": true }),
            );
            info!("[SttModels] {} downloaded and now active", model_id);
            true
        }
        Err(e) => {
            warn!(
                "[SttModels] Switch to {} after download failed: {}",
                model_id, e
            );
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

/// Remove a downloaded model from disk. Refuses the bundled model, the active
/// model, and a model that is still downloading.
#[tauri::command]
pub async fn delete_stt_model(model_id: String, app: AppHandle) -> Result<(), String> {
    let def =
        find_def(&model_id).ok_or_else(|| format!("Unknown dictation model: {}", model_id))?;
    if def.bundled {
        return Err(format!(
            "{} ships with Juno and can't be removed.",
            def.name
        ));
    }
    let settings = load_voice_settings(&app).await?;
    if active_model_id(&app, &settings) == model_id {
        return Err(format!(
            "{} is the active model. Switch to another model first.",
            def.name
        ));
    }
    if download_state(&app)
        .ok()
        .and_then(|s| active_download_id(&s))
        .as_deref()
        == Some(model_id.as_str())
    {
        return Err(format!(
            "{} is still downloading. Cancel it first.",
            def.name
        ));
    }

    match def.engine {
        Engine::Whisper => {
            let path = models_dir(&app)?.join(def.filename);
            if path.is_file() {
                tokio::fs::remove_file(&path)
                    .await
                    .map_err(|e| format!("Could not remove {}: {}", def.name, e))?;
            }
        }
        Engine::Parakeet => {
            let dir = PathBuf::from(parakeet_dir(&app));
            if dir.is_dir() {
                tokio::fs::remove_dir_all(&dir)
                    .await
                    .map_err(|e| format!("Could not remove {}: {}", def.name, e))?;
            }
        }
    }
    info!("[SttModels] Removed {}", model_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

/// Honour the saved model at startup. The plugin boots Whisper with its own
/// default path; if settings name a different model that is on disk, load
/// that one. Nothing is downloaded here: the Parakeet offer is an onboarding
/// step, and the Models pane is the only other place a download starts.
pub async fn apply_persisted_stt_model(app: &AppHandle) {
    let settings = match load_voice_settings(app).await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "[SttModels] Could not read voice settings at startup: {}",
                e
            );
            return;
        }
    };

    let wanted = if settings.stt_provider.eq_ignore_ascii_case("parakeet") {
        Some(PARAKEET_ID)
    } else {
        whisper_id_for_path(&settings.model_path)
    };
    let Some(id) = wanted else {
        return;
    };
    let running = active_model_id(app, &settings);
    if id == running {
        return;
    }
    if !find_def(id).is_some_and(|d| is_downloaded(app, d)) {
        warn!(
            "[SttModels] Saved model '{}' is not on disk; staying on {}",
            id, running
        );
        return;
    }
    match switch_engine_to(app, id).await {
        Ok(()) => info!("[SttModels] Honouring saved model '{}' at startup", id),
        Err(e) => warn!(
            "[SttModels] Saved model '{}' unavailable at startup ({}); staying on {}",
            id, e, running
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(arch: &str) -> Vec<&'static str> {
        catalog_for_arch(arch).into_iter().map(|m| m.id).collect()
    }

    #[test]
    fn apple_silicon_gets_parakeet_as_the_recommended_balanced_row() {
        assert_eq!(
            ids("aarch64"),
            [
                "tiny-en",
                "parakeet-ctc",
                "large-v3",
                "large-v3-turbo",
                "small-en"
            ]
        );
        assert_eq!(tier_for("aarch64", "tiny-en"), Some(FAST));
        assert_eq!(tier_for("aarch64", "parakeet-ctc"), Some(BALANCED));
        assert_eq!(tier_for("aarch64", "large-v3"), Some(ACCURATE));
        assert_eq!(tier_for("aarch64", "large-v3-turbo"), None);
        assert_eq!(recommended_id("aarch64"), PARAKEET_ID);
    }

    #[test]
    fn intel_never_sees_parakeet_and_recommends_turbo() {
        assert_eq!(
            ids("x86_64"),
            ["tiny-en", "large-v3-turbo", "large-v3", "small-en"]
        );
        assert_eq!(tier_for("x86_64", "large-v3-turbo"), Some(BALANCED));
        assert_eq!(tier_for("x86_64", "parakeet-ctc"), None);
        assert_eq!(recommended_id("x86_64"), "large-v3-turbo");
    }

    #[test]
    fn exactly_one_recommended_tier_per_arch() {
        for arch in ["aarch64", "x86_64"] {
            let recommended = catalog_for_arch(arch)
                .iter()
                .filter(|m| tier_for(arch, m.id).is_some_and(|t| t.recommended))
                .count();
            assert_eq!(recommended, 1, "{arch}");
            let tiers = catalog_for_arch(arch)
                .iter()
                .filter(|m| tier_for(arch, m.id).is_some())
                .count();
            assert_eq!(tiers, 3, "{arch}");
        }
    }

    #[test]
    fn only_tiny_en_is_bundled_and_it_is_whisper() {
        let bundled: Vec<_> = CATALOG.iter().filter(|m| m.bundled).collect();
        assert_eq!(bundled.len(), 1);
        assert_eq!(bundled[0].id, BUNDLED_ID);
        assert_eq!(bundled[0].engine, Engine::Whisper);
    }

    #[test]
    fn parakeet_fetch_plan_is_the_pinned_manifest_with_exact_sizes() {
        let def = find_def(PARAKEET_ID).unwrap();
        let plan = fetch_plan(def);
        assert_eq!(plan.len(), 3);
        assert!(plan.iter().all(|f| f.expected_bytes.is_some()));
        assert!(plan.iter().all(|f| f.url.contains("/resolve/7df2cab7")));
        assert_eq!(expected_total_bytes(PARAKEET_ID), 612_689_838);
        // The pane shows the same number the download verifies.
        assert_eq!(def.size_mb, 612);
    }

    #[test]
    fn whisper_fetch_plan_learns_its_size_from_the_server() {
        let def = find_def("large-v3").unwrap();
        let plan = fetch_plan(def);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].name, "ggml-large-v3.bin");
        assert!(plan[0].expected_bytes.is_none());
    }

    #[test]
    fn whisper_ids_resolve_from_any_path_shape() {
        assert_eq!(
            whisper_id_for_path("models/ggml-tiny.en.bin"),
            Some("tiny-en")
        );
        assert_eq!(
            whisper_id_for_path(
                "/Users/x/Library/Application Support/juno/models/ggml-large-v3.bin"
            ),
            Some("large-v3")
        );
        assert_eq!(whisper_id_for_path("models/unknown.bin"), None);
    }

    #[test]
    fn pips_are_ordered_by_the_published_numbers() {
        let acc = |id: &str| find_def(id).unwrap().accuracy;
        let spd = |id: &str| find_def(id).unwrap().speed;
        // WER: large-v3 < parakeet ~ turbo < small.en < tiny.en
        assert!(acc("large-v3") > acc("parakeet-ctc"));
        assert_eq!(acc("parakeet-ctc"), acc("large-v3-turbo"));
        assert!(acc("large-v3-turbo") > acc("small-en"));
        assert!(acc("small-en") > acc("tiny-en"));
        // Speed: tiny.en fastest, large-v3 slowest, Parakeet with the small models.
        assert!(spd("tiny-en") > spd("parakeet-ctc"));
        assert!(spd("parakeet-ctc") > spd("large-v3-turbo"));
        assert!(spd("large-v3-turbo") > spd("large-v3"));
    }
}

use reqwest::Client;
use std::env;
use tauri::State;
use serde::Serialize;
use serde_json::json;
use crate::state::AppState;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use tracing::{error, info};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};

// --- ElevenLabs API Structures ---
#[derive(Serialize)]
pub(crate) struct ElevenLabsVoiceSettings {
    stability: f32,
    similarity_boost: f32,
}

#[derive(Serialize)]
pub(crate) struct ElevenLabsRequest {
    text: String,
    model_id: String,
    voice_settings: ElevenLabsVoiceSettings,
}
// --- End ElevenLabs API Structures ---

// --- ElevenLabs TTS Command ---
#[tauri::command]
pub async fn invoke_elevenlabs_tts(
    text: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Invoking ElevenLabs TTS for text: \"{}\"", text);

    let api_key = match env::var("ELEVENLABS_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err("ELEVENLABS_API_KEY not configured.".to_string()),
    };
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string()); // Default voice ID

    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", voice_id);

    let request_body = json!({
        "text": text,
        "model_id": "eleven_multilingual_v2",
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.75
        }
    });

    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("audio/mpeg"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("xi-api-key", HeaderValue::from_str(&api_key).map_err(|e| {
        let err_msg = format!("Invalid ElevenLabs API key format: {}", e);
        error!("{}", err_msg);
        err_msg
    })?);

    let response = client
        .post(&url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("ElevenLabs request failed: {}", e))?;

    if response.status().is_success() {
        let bytes = response.bytes().await.map_err(|e| format!("Failed to read ElevenLabs response body: {}", e))?;
        let base64_audio = BASE64_STANDARD.encode(&bytes);
        info!("Successfully received and encoded ElevenLabs audio.");
        Ok(base64_audio)
    } else {
        let error_body = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        let err_msg = format!("ElevenLabs API error: {}", error_body);
        error!("{}", err_msg);
        Err(err_msg)
    }
}
// --- End ElevenLabs TTS Command ---


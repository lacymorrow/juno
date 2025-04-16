use serde::Serialize;
use reqwest::{Client, header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE}};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use tracing::{debug, error, info, warn};

// Bring AppState and Desktop into scope from the crate root
use crate::{AppState, Desktop}; // Assuming AppState is made pub(crate) or pub in lib.rs
use std::sync::Arc; // Required for Desktop Arc

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
pub async fn invoke_elevenlabs_tts(text_to_speak: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let desktop_arc: Arc<Desktop> = state.desktop.clone(); // Explicitly type desktop_arc
    info!("Invoking ElevenLabs TTS for text: \"{}\"", text_to_speak);

    let api_key = std::env::var("ELEVENLABS_API_KEY")
        .map_err(|_| "ELEVENLABS_API_KEY not configured.".to_string())?;
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string()); // Default voice ID

    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", voice_id);

    let request_body = ElevenLabsRequest {
        text: text_to_speak,
        model_id: "eleven_monolingual_v1".to_string(),
        voice_settings: ElevenLabsVoiceSettings {
            stability: 0.5,
            similarity_boost: 0.5,
        },
    };

    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("audio/mpeg"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("xi-api-key", HeaderValue::from_str(&api_key).map_err(|e| {
        let err_msg = format!("Invalid ElevenLabs API key format: {}", e);
        error!("{}", err_msg);
        err_msg
    })?);

    let response = match client
        .post(&url)
        .headers(headers)
        .json(&request_body)
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) => {
            let err_msg = format!("HTTP request to ElevenLabs failed: {}", e);
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_body = match response.text().await {
            Ok(body) => body,
            Err(_) => "Failed to read error body".to_string(),
        };
        let err_msg = format!("ElevenLabs API error: {} - {}", status, error_body);
        error!("{}", err_msg);
        return Err(err_msg);
    }

    // Read the response body as bytes
    match response.bytes().await {
        Ok(audio_bytes) => {
            // Encode the audio bytes to base64
            let base64_audio = BASE64_STANDARD.encode(&audio_bytes);
            info!("Successfully received and encoded ElevenLabs audio.");
            Ok(base64_audio)
        }
        Err(e) => {
            let err_msg = format!("Failed to read ElevenLabs audio bytes: {}", e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}
// --- End ElevenLabs TTS Command ---


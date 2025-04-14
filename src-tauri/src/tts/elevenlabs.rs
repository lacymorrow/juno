use serde::Serialize;
use reqwest::{Client, header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE}};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

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
    desktop_arc.log("info", format!("Invoking ElevenLabs TTS for text: \"{}\"", text_to_speak));

    let api_key = match std::env::var("ELEVENLABS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            let err_msg = "ELEVENLABS_API_KEY not found in environment variables.".to_string();
            desktop_arc.log("error", err_msg.clone());
            return Err(err_msg);
        }
    };

    let voice_id = "21m00Tcm4TlvDq8ikWAM"; // Example Voice ID (Rachel)
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
        desktop_arc.log("error", err_msg.clone());
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
            let err_msg = format!("Request to ElevenLabs API failed: {}", e);
            desktop_arc.log("error", err_msg.clone());
            return Err(err_msg);
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error body".to_string());
        let err_msg = format!(
            "ElevenLabs API request failed with status {}: {}",
            status,
            error_text
        );
        desktop_arc.log("error", err_msg.clone());
        return Err(err_msg);
    }

    // Read the response body as bytes
    let audio_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            let err_msg = format!("Failed to read ElevenLabs audio bytes: {}", e);
            desktop_arc.log("error", err_msg.clone());
            return Err(err_msg);
        }
    };

    // Encode bytes to base64
    let base64_audio = BASE64_STANDARD.encode(&audio_bytes);
    desktop_arc.log("info", "Successfully received and encoded ElevenLabs audio.".to_string());

    Ok(base64_audio)
}
// --- End ElevenLabs TTS Command ---


use crate::constants::http_headers;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};

// --- ElevenLabs API Structures ---
#[derive(Deserialize, Debug)]
struct ElevenLabsErrorResponse {
    detail: Option<ElevenLabsErrorDetail>,
}

#[derive(Deserialize, Debug)]
struct ElevenLabsErrorDetail {
    message: Option<String>,
    // Add other fields if necessary based on API documentation
}

#[derive(Serialize)]
struct ElevenLabsVoiceSettings {
    stability: f32,
    similarity_boost: f32,
}

#[derive(Serialize)]
struct ElevenLabsPayload {
    text: String,
    voice_settings: ElevenLabsVoiceSettings,
}
// --- End ElevenLabs API Structures ---

// --- ElevenLabs TTS Command ---
#[tauri::command]
pub async fn invoke_elevenlabs_tts(text: String) -> Result<String, String> {
    info!("Invoking ElevenLabs TTS for text: {}", text);

    // Check if stop was requested before starting
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before starting ElevenLabs TTS, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let api_key = env::var("ELEVENLABS_API_KEY")
        .map_err(|_| "ELEVENLABS_API_KEY environment variable not set".to_string())?;
    let voice_id =
        env::var("ELEVENLABS_VOICE_ID").unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string());
    info!("Using ElevenLabs Voice ID: {}", voice_id);

    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", voice_id);

    let client = Client::new();
    let payload = ElevenLabsPayload {
        text: text.clone(),
        voice_settings: ElevenLabsVoiceSettings {
            stability: 0.5,
            similarity_boost: 0.75,
        },
    };

    info!(
        "Sending request to ElevenLabs: URL={}, Payload snippet: {{ text: '{}...', ... }}",
        url,
        text.chars().take(20).collect::<String>()
    );

    // Check if stop was requested before sending the request
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before sending ElevenLabs request, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let response = client
        .post(&url)
        .header(http_headers::CONTENT_TYPE, http_headers::APPLICATION_JSON)
        .header("xi-api-key", api_key)
        .json(&payload)
        .send()
        .await;

    // Check if stop was requested after receiving the response
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested after receiving ElevenLabs response, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    match response {
        Ok(res) => {
            info!(
                "Received response from ElevenLabs with status: {}",
                res.status()
            );
            if res.status().is_success() {
                // Check if stop was requested before processing the audio
                if crate::tts::is_tts_stop_requested() {
                    info!("TTS stop was requested before processing ElevenLabs audio, aborting");
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                match res.bytes().await {
                    Ok(audio_bytes) => {
                        // Final check before encoding
                        if crate::tts::is_tts_stop_requested() {
                            info!(
                                "TTS stop was requested before encoding ElevenLabs audio, aborting"
                            );
                            return Ok("TTS_STOPPED_BY_USER".to_string());
                        }

                        let base64_audio = BASE64_STANDARD.encode(&audio_bytes);
                        info!(
                            "Successfully received and encoded ElevenLabs audio ({} bytes).",
                            audio_bytes.len()
                        );
                        Ok(base64_audio)
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to read ElevenLabs response body: {}", e);
                        error!("{}", err_msg);
                        Err(err_msg)
                    }
                }
            } else {
                let status = res.status();
                let error_body = res
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read error body".to_string());
                // Try to parse the specific error structure
                let detailed_error =
                    match serde_json::from_str::<ElevenLabsErrorResponse>(&error_body) {
                        Ok(parsed_error) => format!(
                            "{} - {}",
                            status,
                            parsed_error
                                .detail
                                .and_then(|d| d.message)
                                .unwrap_or_else(|| error_body.clone())
                        ),
                        Err(_) => format!("{} - {}", status, error_body), // Fallback to raw body
                    };
                error!("ElevenLabs API request failed: {}", detailed_error);
                Err(format!("ElevenLabs API Error: {}", detailed_error))
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to send request to ElevenLabs: {}", e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}
// --- End ElevenLabs TTS Command ---

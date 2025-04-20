use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use tokio; // Ensure tokio is available for sleep
use tracing::{error, info, warn}; // Import tracing macros
use std::env;
use base64::Engine;

// --- Replicate API Structures ---
#[derive(Serialize)]
pub(crate) struct ReplicateInput { // Make pub(crate) if only used within the crate
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")] // Omit if None
    speaker_wav: Option<String>, // Optional: URL to a speaker reference audio
}

#[derive(Serialize)]
pub(crate) struct ReplicateRequest {
    version: String,
    input: ReplicateInput,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateInitialResponse {
    id: String,
    _status: String,
    urls: ReplicateUrls,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateUrls {
    get: String,
    #[allow(dead_code)] // Allow dead code, might be useful later
    cancel: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateStatusResponse {
    status: String,
    output: Option<String>, // URL to the generated audio
    error: Option<String>, // Capture error messages
}
// --- End Replicate API Structures ---

const _REPLICATE_API_BASE: &str = "https://api.replicate.com/v1";

// Command to invoke Replicate TTS
#[tauri::command]
pub async fn invoke_replicate_tts(
    text: String,
) -> Result<String, String> {
    info!("Invoking Replicate TTS for text: {}", text);
    let api_key = env::var("REPLICATE_API_KEY")
        .map_err(|_| "REPLICATE_API_KEY environment variable not set".to_string())?;

    // Restore default model version logic
    let model_version = env::var("REPLICATE_MODEL_VERSION")
        .unwrap_or_else(|_| "3e59b10a9894c54ae5f2fc0347e3a2f5c82f0574407e53a7d9f76ec7c502ad03".to_string());
    info!("Using Replicate Model Version: {}", model_version);

    let speaker_wav_url = env::var("REPLICATE_SPEAKER_WAV_URL").ok(); // Optional
    if speaker_wav_url.is_some() {
        info!("Using speaker WAV URL: {:?}", speaker_wav_url);
    }

    let client = Client::new();
    let start_url = "https://api.replicate.com/v1/predictions";

    let request_payload = ReplicateRequest {
        version: model_version,
        input: ReplicateInput {
            text: text.clone(), // Clone text for logging
            speaker_wav: speaker_wav_url,
        },
    };

    // Debug log the actual JSON being sent
    match serde_json::to_string(&request_payload) {
        Ok(json_string) => info!("Sending Replicate payload: {}", json_string),
        Err(e) => warn!("Failed to serialize Replicate payload for logging: {}", e),
    }

    // 1. Start the prediction
    let initial_response = client
        .post(start_url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_payload)
        .send()
        .await;

    let initial_res = match initial_response {
        Ok(res) => res,
        Err(e) => {
            let err_msg = format!("Failed to send initial request to Replicate: {}", e);
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    if !initial_res.status().is_success() {
        let status = initial_res.status();
        let error_body = initial_res.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        let err_msg = format!("Replicate initial API request failed: {} - {}", status, error_body);
        error!("{}", err_msg);
        return Err(err_msg);
    }

    let initial_data = match initial_res.json::<ReplicateInitialResponse>().await {
        Ok(data) => data,
        Err(e) => {
            let err_msg = format!("Failed to parse Replicate initial response: {}", e);
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    let get_url = initial_data.urls.get;
    info!("Replicate prediction started (ID: {}). Polling status at: {}", initial_data.id, get_url);

    // 2. Poll for the result
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await; // Wait before polling

        let status_response = client
            .get(&get_url)
            .header("Authorization", format!("Token {}", api_key))
            .send()
            .await;

        let status_res = match status_response {
            Ok(res) => res,
            Err(e) => {
                warn!("Failed to poll Replicate status (URL: {}): {}. Retrying...", get_url, e);
                continue; // Retry polling
            }
        };

        if !status_res.status().is_success() {
            let status = status_res.status();
            let error_body = status_res.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
            warn!("Replicate status polling failed: {} - {}. Retrying...", status, error_body);
            continue; // Retry polling
        }

        let status_data = match status_res.json::<ReplicateStatusResponse>().await {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to parse Replicate status response: {}. Retrying...", e);
                continue; // Retry polling
            }
        };

        info!("Current Replicate prediction status: {:?}", status_data.status);

        match status_data.status.as_str() {
            "succeeded" => {
                if let Some(output_url) = status_data.output {
                    info!("Replicate prediction succeeded. Downloading audio from: {}", output_url);
                    // 3. Download the audio file
                    match client.get(&output_url).send().await {
                        Ok(audio_res) => {
                            if audio_res.status().is_success() {
                                match audio_res.bytes().await {
                                    Ok(audio_bytes) => {
                                        let base64_audio = base64::engine::general_purpose::STANDARD.encode(&audio_bytes);
                                        info!("Successfully downloaded and encoded Replicate audio ({} bytes).", audio_bytes.len());
                                        return Ok(base64_audio);
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Failed to read Replicate audio bytes: {}", e);
                                        error!("{}", err_msg);
                                        return Err(err_msg);
                                    }
                                }
                            } else {
                                let err_msg = format!("Failed to download Replicate audio file (status: {}). URL: {}", audio_res.status(), output_url);
                                error!("{}", err_msg);
                                return Err(err_msg);
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to send request to download Replicate audio: {}", e);
                            error!("{}", err_msg);
                            return Err(err_msg);
                        }
                    }
                } else {
                    let err_msg = "Replicate prediction succeeded but no output URL was provided.".to_string();
                    error!("{}", err_msg);
                    return Err(err_msg);
                }
            }
            "failed" | "canceled" => {
                let error_message = status_data.error.unwrap_or_else(|| "Unknown error".to_string());
                let err_msg = format!("Replicate prediction {}: {}", status_data.status, error_message);
                error!("{}", err_msg);
                return Err(err_msg);
            }
            "processing" | "starting" => { // Continue polling
                continue;
            }
            _ => { // Unknown status
                let err_msg = format!("Unknown Replicate prediction status: {}", status_data.status);
                error!("{}", err_msg);
                return Err(err_msg);
            }
        }
    }
}

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use tokio; // Ensure tokio is available for sleep
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use crate::{AppState, Desktop}; // Assuming AppState is made pub(crate) or pub in lib.rs
use std::sync::Arc; // Required for Desktop Arc
use tokio::time::sleep;
use tracing::{debug, error, info, warn}; // Import tracing macros

// --- Replicate API Structures ---
#[derive(Serialize)]
pub(crate) struct ReplicateInput { // Make pub(crate) if only used within the crate
    text: String,
    speaker: u32,
    max_audio_length_ms: u32,
}

#[derive(Serialize)]
pub(crate) struct ReplicateRequest {
    version: String,
    input: ReplicateInput,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateInitialResponse {
    id: String,
    status: String,
    urls: Option<ReplicateUrls>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateUrls {
    get: String,
    cancel: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicatePollingResponse {
    id: String,
    status: String,
    output: Option<String>,
    error: Option<String>,
    logs: Option<String>,
}
// --- End Replicate API Structures ---

// Command to invoke Replicate TTS
#[tauri::command]
pub async fn invoke_replicate_tts(text: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let desktop_arc: Arc<Desktop> = state.desktop.clone(); // Explicitly type desktop_arc
    info!("invoke_replicate_tts called with text: {}", text);

    let replicate_api_key = std::env::var("REPLICATE_API_KEY")
        .map_err(|_| "REPLICATE_API_KEY not configured.".to_string())?;

    let client = Client::new();
    let replicate_model_version = "3e59b10a9894c54ae5f2fc0347e3a2f5c82f0574407e53a7d9f76ec7c502ad03";
    let replicate_api_url = "https://api.replicate.com/v1/predictions";

    let request_payload = ReplicateRequest {
        version: replicate_model_version.to_string(),
        input: ReplicateInput {
            text,
            speaker: 0,
            max_audio_length_ms: 30000,
        },
    };

    debug!("Sending Replicate TTS request: {:?}", serde_json::to_string(&request_payload).unwrap_or_default());

    // 1. Start the prediction job
    let initial_response = client
        .post(replicate_api_url)
        .header("Authorization", format!("Bearer {}", replicate_api_key))
        .header("Content-Type", "application/json")
        .json(&request_payload)
        .send()
        .await;

    let initial_response = match initial_response {
        Ok(res) => res,
        Err(e) => {
            let err_msg = format!("HTTP request to start Replicate job failed: {}", e);
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    if !initial_response.status().is_success() {
        let status = initial_response.status();
        let body = initial_response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        let err_msg = format!("Replicate API error starting job: {} - {}", status, body);
        error!("{}", err_msg);
        return Err(err_msg);
    }

    let initial_data: ReplicateInitialResponse = match initial_response.json().await {
        Ok(data) => data,
        Err(e) => {
            let err_msg = format!("Failed to parse Replicate start response JSON: {}", e);
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    info!("Replicate job started: ID={}, Status={}", initial_data.id, initial_data.status);

    let get_url = match initial_data.urls {
        Some(urls) => urls.get,
        None => {
            let err_msg = "Missing 'get' URL in Replicate response".to_string();
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    // 2. Poll for the result
    let max_polls = 30;
    let poll_interval = Duration::from_secs(2);

    for _ in 0..max_polls {
        sleep(poll_interval).await;
        debug!("Polling Replicate job status: {}", get_url);

        let poll_response = client
            .get(&get_url)
            .header("Authorization", format!("Bearer {}", replicate_api_key))
            .send()
            .await;

        let poll_response = match poll_response {
            Ok(res) => res,
            Err(e) => {
                warn!("Replicate polling request failed: {}", e);
                continue;
            }
        };

        if !poll_response.status().is_success() {
            let status = poll_response.status();
            let body = poll_response.text().await.unwrap_or_else(|_| "Failed to read polling error body".to_string());
            warn!("Replicate polling error response: {} - {}", status, body);
            continue;
        }

        let poll_data: ReplicatePollingResponse = match poll_response.json().await {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to parse Replicate polling JSON response: {}", e);
                continue;
            }
        };

        debug!("Replicate poll status: {}", poll_data.status);

        match poll_data.status.as_str() {
            "succeeded" => {
                if let Some(audio_url) = poll_data.output {
                    info!("Replicate job succeeded. Audio URL: {}", audio_url);
                    return Ok(audio_url);
                } else {
                    let err_msg = "Replicate job succeeded but no output URL found".to_string();
                    error!("{}", err_msg);
                    return Err(err_msg);
                }
            }
            "failed" | "canceled" => {
                let error_detail = poll_data.error.unwrap_or_else(|| "No error details provided".to_string());
                let err_msg = format!("Replicate job {}: {}", poll_data.status, error_detail);
                error!("{}", err_msg);
                return Err(err_msg);
            }
            "starting" | "processing" => continue,
            _ => {
                warn!("Unexpected Replicate job status: {}", poll_data.status);
                continue;
            }
        }
    }

    let err_msg = "Replicate job timed out after polling.".to_string();
    error!("{}", err_msg);
    Err(err_msg)
}

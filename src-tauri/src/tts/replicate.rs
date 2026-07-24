use crate::constants::timeouts;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio; // Ensure tokio is available for sleep
use tracing::{error, info, warn}; // Import tracing macros

// Maximum time to wait for Replicate prediction to complete (5 minutes)
// const REPLICATE_TIMEOUT_SECONDS: u64 = 300;

// --- Replicate API Structures ---
#[derive(Serialize)]
pub(crate) struct ReplicateInput {
    // Make pub(crate) if only used within the crate
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
    #[serde(rename = "status")] // Map JSON field "status" to this Rust field
    _status: String,
    urls: ReplicateUrls,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateUrls {
    get: String,
    // cancel field removed - unused for performance
}

#[derive(Deserialize, Debug)]
pub(crate) struct ReplicateStatusResponse {
    status: String,
    output: Option<String>, // URL to the generated audio
    error: Option<String>,  // Capture error messages
}
// --- End Replicate API Structures ---

const _REPLICATE_API_BASE: &str = "https://api.replicate.com/v1";

// --- Chatterbox API Structures (model-based endpoint, no version pin) ---
#[derive(Serialize)]
pub(crate) struct ChatterboxInput {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_url: Option<String>, // Optional URL to reference audio for voice cloning
    exaggeration: Option<f64>, // Emotion exaggeration level 0.0-2.0
}

#[derive(Serialize)]
pub(crate) struct ChatterboxRequest {
    input: ChatterboxInput,
}
// --- End Chatterbox API Structures ---
// Command to invoke Replicate TTS
#[tauri::command]
pub async fn invoke_replicate_tts(text: String) -> Result<String, String> {
    info!("Invoking Replicate TTS for text: {}", text);

    // Check if stop was requested before starting
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before starting Replicate TTS, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let api_key = env::var("REPLICATE_API_KEY")
        .map_err(|_| "REPLICATE_API_KEY environment variable not set".to_string())?;

    // Restore default model version logic
    let model_version = env::var("REPLICATE_MODEL_VERSION").unwrap_or_else(|_| {
        "3e59b10a9894c54ae5f2fc0347e3a2f5c82f0574407e53a7d9f76ec7c502ad03".to_string()
    });
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

    // Check if stop was requested before sending the initial request
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested before sending Replicate initial request, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
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

    // Check if stop was requested after receiving initial response
    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop was requested after receiving Replicate initial response, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    if !initial_res.status().is_success() {
        let status = initial_res.status();
        let error_body = initial_res
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error body".to_string());
        let err_msg = format!(
            "Replicate initial API request failed: {} - {}",
            status, error_body
        );
        error!("{}", err_msg);
        return Err(err_msg);
    }

    // Read the response body as text first for logging and more robust parsing
    let initial_response_text = match initial_res.text().await {
        Ok(text) => text,
        Err(e) => {
            let err_msg = format!(
                "Failed to read Replicate initial response body as text: {}",
                e
            );
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    info!(
        "Received Replicate initial response body: {}",
        initial_response_text
    );

    let initial_data =
        match serde_json::from_str::<ReplicateInitialResponse>(&initial_response_text) {
            Ok(data) => data,
            Err(e) => {
                let err_msg = format!(
                    "Failed to parse Replicate initial response from text: {}. Body: {}",
                    e, initial_response_text
                );
                error!("{}", err_msg);
                return Err(err_msg);
            }
        };

    let get_url = initial_data.urls.get;
    info!(
        "Replicate prediction started (ID: {}). Polling status at: {}",
        initial_data.id, get_url
    );

    // 2. Poll for the result with timeout
    let start_time = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(timeouts::REPLICATE_TIMEOUT_SECONDS);

    loop {
        // Check if stop was requested before each polling iteration
        if crate::tts::is_tts_stop_requested() {
            info!("TTS stop was requested during Replicate polling, aborting");
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        // Check for timeout
        if start_time.elapsed() > timeout_duration {
            let err_msg = format!(
                "Replicate prediction timed out after {} seconds (prediction ID: {})",
                timeouts::REPLICATE_TIMEOUT_SECONDS,
                initial_data.id
            );
            error!("{}", err_msg);
            return Err(err_msg);
        }

        tokio::time::sleep(Duration::from_secs(1)).await; // Wait before polling

        let status_response = client
            .get(&get_url)
            .header("Authorization", format!("Token {}", api_key))
            .send()
            .await;

        let status_res = match status_response {
            Ok(res) => res,
            Err(e) => {
                warn!(
                    "Failed to poll Replicate status (URL: {}): {}. Retrying...",
                    get_url, e
                );
                continue; // Retry polling
            }
        };

        if !status_res.status().is_success() {
            let status = status_res.status();
            let error_body = status_res
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            warn!(
                "Replicate status polling failed: {} - {}. Retrying...",
                status, error_body
            );
            continue; // Retry polling
        }

        let status_data = match status_res.json::<ReplicateStatusResponse>().await {
            Ok(data) => data,
            Err(e) => {
                warn!(
                    "Failed to parse Replicate status response: {}. Retrying...",
                    e
                );
                continue; // Retry polling
            }
        };

        info!(
            "Current Replicate prediction status: {:?} (elapsed: {:.1}s)",
            status_data.status,
            start_time.elapsed().as_secs_f32()
        );

        match status_data.status.as_str() {
            "succeeded" => {
                // Check if stop was requested before processing the successful result
                if crate::tts::is_tts_stop_requested() {
                    info!("TTS stop was requested after Replicate prediction succeeded, aborting");
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                if let Some(output_url) = status_data.output {
                    info!(
                        "Replicate prediction succeeded. Downloading audio from: {}",
                        output_url
                    );
                    // 3. Download the audio file
                    match client.get(&output_url).send().await {
                        Ok(audio_res) => {
                            // Check if stop was requested before processing audio
                            if crate::tts::is_tts_stop_requested() {
                                info!("TTS stop was requested before processing Replicate audio, aborting");
                                return Ok("TTS_STOPPED_BY_USER".to_string());
                            }

                            if audio_res.status().is_success() {
                                match audio_res.bytes().await {
                                    Ok(audio_bytes) => {
                                        // Final check before encoding
                                        if crate::tts::is_tts_stop_requested() {
                                            info!("TTS stop was requested before encoding Replicate audio, aborting");
                                            return Ok("TTS_STOPPED_BY_USER".to_string());
                                        }

                                        let base64_audio =
                                            base64::engine::general_purpose::STANDARD
                                                .encode(&audio_bytes);
                                        info!("Successfully downloaded and encoded Replicate audio ({} bytes).", audio_bytes.len());
                                        return Ok(base64_audio);
                                    }
                                    Err(e) => {
                                        let err_msg =
                                            format!("Failed to read Replicate audio bytes: {}", e);
                                        error!("{}", err_msg);
                                        return Err(err_msg);
                                    }
                                }
                            } else {
                                let err_msg = format!(
                                    "Failed to download Replicate audio file (status: {}). URL: {}",
                                    audio_res.status(),
                                    output_url
                                );
                                error!("{}", err_msg);
                                return Err(err_msg);
                            }
                        }
                        Err(e) => {
                            let err_msg = format!(
                                "Failed to send request to download Replicate audio: {}",
                                e
                            );
                            error!("{}", err_msg);
                            return Err(err_msg);
                        }
                    }
                } else {
                    let err_msg = "Replicate prediction succeeded but no output URL was provided."
                        .to_string();
                    error!("{}", err_msg);
                    return Err(err_msg);
                }
            }
            "failed" | "canceled" => {
                let error_message = status_data
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string());
                let err_msg = format!(
                    "Replicate prediction {}: {}",
                    status_data.status, error_message
                );
                error!("{}", err_msg);
                return Err(err_msg);
            }
            "processing" | "starting" => {
                // Continue polling
                continue;
            }
            _ => {
                // Unknown status
                let err_msg = format!(
                    "Unknown Replicate prediction status: {}",
                    status_data.status
                );
                error!("{}", err_msg);
                return Err(err_msg);
            }
        }
    }
}

/// Invoke Chatterbox TTS via the Replicate model-based endpoint.
/// Uses resemble-ai/chatterbox (or chatterbox-hd) with optional voice cloning.
pub async fn invoke_chatterbox_tts(
    text: String,
    reference_audio_url: Option<String>,
    exaggeration: f32,
    use_hd: bool,
) -> Result<String, String> {
    info!(
        "Invoking Chatterbox TTS (hd={}, exaggeration={:.2})",
        use_hd, exaggeration
    );

    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop requested before Chatterbox TTS, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let api_key = env::var("REPLICATE_API_KEY")
        .map_err(|_| "REPLICATE_API_KEY environment variable not set".to_string())?;

    let model_name = if use_hd {
        "chatterbox-hd"
    } else {
        "chatterbox"
    };
    let start_url = format!(
        "https://api.replicate.com/v1/models/resemble-ai/{}/predictions",
        model_name
    );
    info!("Using Chatterbox model endpoint: {}", start_url);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to create Chatterbox HTTP client: {}", e))?;

    let request_payload = ChatterboxRequest {
        input: ChatterboxInput {
            text: text.clone(),
            audio_url: reference_audio_url,
            exaggeration: Some(exaggeration as f64),
        },
    };

    match serde_json::to_string(&request_payload) {
        Ok(json_string) => info!("Sending Chatterbox payload: {}", json_string),
        Err(e) => warn!("Failed to serialize Chatterbox payload for logging: {}", e),
    }

    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop requested before Chatterbox initial request, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let initial_res = client
        .post(&start_url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_payload)
        .send()
        .await
        .map_err(|e| format!("Failed to send initial request to Chatterbox: {}", e))?;

    if crate::tts::is_tts_stop_requested() {
        info!("TTS stop requested after Chatterbox initial response, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    if !initial_res.status().is_success() {
        let status = initial_res.status();
        let error_body = initial_res
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error body".to_string());
        let err_msg = format!(
            "Chatterbox initial API request failed: {} - {}",
            status, error_body
        );
        error!("{}", err_msg);
        return Err(err_msg);
    }

    let initial_response_text = initial_res
        .text()
        .await
        .map_err(|e| format!("Failed to read Chatterbox initial response body: {}", e))?;
    info!("Chatterbox initial response: {}", initial_response_text);

    let initial_data = serde_json::from_str::<ReplicateInitialResponse>(&initial_response_text)
        .map_err(|e| {
            format!(
                "Failed to parse Chatterbox initial response: {}. Body: {}",
                e, initial_response_text
            )
        })?;

    let get_url = initial_data.urls.get;
    info!(
        "Chatterbox prediction started (ID: {}). Polling at: {}",
        initial_data.id, get_url
    );

    // Poll for result with timeout
    let start_time = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(timeouts::REPLICATE_TIMEOUT_SECONDS);

    loop {
        if crate::tts::is_tts_stop_requested() {
            info!("TTS stop requested during Chatterbox polling, aborting");
            return Ok("TTS_STOPPED_BY_USER".to_string());
        }

        if start_time.elapsed() > timeout_duration {
            return Err(format!(
                "Chatterbox prediction timed out after {} seconds (ID: {})",
                timeouts::REPLICATE_TIMEOUT_SECONDS,
                initial_data.id
            ));
        }

        tokio::time::sleep(Duration::from_secs(1)).await;

        let status_res = match client
            .get(&get_url)
            .header("Authorization", format!("Token {}", api_key))
            .send()
            .await
        {
            Ok(res) => res,
            Err(e) => {
                warn!("Failed to poll Chatterbox status: {}. Retrying...", e);
                continue;
            }
        };

        if !status_res.status().is_success() {
            warn!(
                "Chatterbox status poll failed: {}. Retrying...",
                status_res.status()
            );
            continue;
        }

        let status_data = match status_res.json::<ReplicateStatusResponse>().await {
            Ok(data) => data,
            Err(e) => {
                warn!(
                    "Failed to parse Chatterbox status response: {}. Retrying...",
                    e
                );
                continue;
            }
        };

        info!(
            "Chatterbox prediction status: {:?} (elapsed: {:.1}s)",
            status_data.status,
            start_time.elapsed().as_secs_f32()
        );

        match status_data.status.as_str() {
            "succeeded" => {
                if crate::tts::is_tts_stop_requested() {
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                if let Some(output_url) = status_data.output {
                    info!(
                        "Chatterbox prediction succeeded. Downloading audio from: {}",
                        output_url
                    );
                    match client.get(&output_url).send().await {
                        Ok(audio_res) => {
                            if crate::tts::is_tts_stop_requested() {
                                return Ok("TTS_STOPPED_BY_USER".to_string());
                            }
                            if audio_res.status().is_success() {
                                match audio_res.bytes().await {
                                    Ok(audio_bytes) => {
                                        if crate::tts::is_tts_stop_requested() {
                                            return Ok("TTS_STOPPED_BY_USER".to_string());
                                        }
                                        let base64_audio =
                                            base64::engine::general_purpose::STANDARD
                                                .encode(&audio_bytes);
                                        info!(
                                            "Chatterbox audio downloaded successfully ({} bytes).",
                                            audio_bytes.len()
                                        );
                                        return Ok(base64_audio);
                                    }
                                    Err(e) => {
                                        return Err(format!(
                                            "Failed to read Chatterbox audio bytes: {}",
                                            e
                                        ))
                                    }
                                }
                            } else {
                                return Err(format!(
                                    "Failed to download Chatterbox audio (status: {})",
                                    audio_res.status()
                                ));
                            }
                        }
                        Err(e) => {
                            return Err(format!("Failed to download Chatterbox audio: {}", e))
                        }
                    }
                } else {
                    return Err(
                        "Chatterbox prediction succeeded but no output URL provided.".to_string(),
                    );
                }
            }
            "failed" | "canceled" => {
                let error_message = status_data
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string());
                return Err(format!(
                    "Chatterbox prediction {}: {}",
                    status_data.status, error_message
                ));
            }
            "processing" | "starting" => continue,
            _ => {
                return Err(format!(
                    "Unknown Chatterbox prediction status: {}",
                    status_data.status
                ));
            }
        }
    }
}

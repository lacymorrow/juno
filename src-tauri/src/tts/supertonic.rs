use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::Client;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{error, info, warn};

pub const DEFAULT_SERVER_URL: &str = "http://localhost:8000";
pub const DEFAULT_VOICE: &str = "M1";
pub const DEFAULT_SPEED: f64 = 1.05;

const REQUEST_TIMEOUT_SECS: u64 = 60;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn get_http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

#[derive(Serialize)]
struct SupertonicRequest {
    model: String,
    input: String,
    voice: String,
    response_format: String,
    speed: f64,
}

/// Invoke Supertonic TTS via its local OpenAI-compatible HTTP server.
///
/// Requires `supertonic serve` running locally. Hits `/v1/audio/speech`.
/// Returns base64-encoded WAV audio on success.
pub async fn invoke_supertonic_tts(
    text: String,
    server_url: String,
    voice: String,
    speed: f64,
) -> Result<String, String> {
    info!(
        "[Supertonic] TTS requested: {} chars, voice: {}, speed: {:.2}, server: {}",
        text.chars().count(),
        voice,
        speed,
        server_url
    );

    if crate::tts::is_tts_stop_requested() {
        info!("[Supertonic] Stop requested before start, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let base_url = if server_url.trim().is_empty() {
        DEFAULT_SERVER_URL.to_string()
    } else {
        server_url
    };
    let url = format!("{}/v1/audio/speech", base_url.trim_end_matches('/'));

    let client = get_http_client();

    let payload = SupertonicRequest {
        model: "supertonic".to_string(),
        input: text,
        voice,
        response_format: "wav".to_string(),
        speed,
    };

    if crate::tts::is_tts_stop_requested() {
        info!("[Supertonic] Stop requested before sending request, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    if crate::tts::is_tts_stop_requested() {
        info!("[Supertonic] Stop requested after response, aborting");
        return Ok("TTS_STOPPED_BY_USER".to_string());
    }

    match response {
        Ok(res) => {
            if res.status().is_success() {
                if crate::tts::is_tts_stop_requested() {
                    return Ok("TTS_STOPPED_BY_USER".to_string());
                }

                match res.bytes().await {
                    Ok(audio_bytes) => {
                        if crate::tts::is_tts_stop_requested() {
                            return Ok("TTS_STOPPED_BY_USER".to_string());
                        }

                        let base64_audio = BASE64_STANDARD.encode(&audio_bytes);
                        info!(
                            "[Supertonic] Audio generated successfully ({} bytes)",
                            audio_bytes.len()
                        );
                        Ok(base64_audio)
                    }
                    Err(e) => {
                        let err_msg = format!("[Supertonic] Failed to read response body: {}", e);
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
                let err_msg =
                    format!("[Supertonic] Server returned {}: {}", status, error_body);
                error!("{}", err_msg);
                Err(err_msg)
            }
        }
        Err(e) => {
            let is_connect = e.is_connect() || e.is_timeout();
            if is_connect {
                warn!(
                    "[Supertonic] Cannot reach server at {}. Is `supertonic serve` running?",
                    url
                );
                Err(format!(
                    "Supertonic server not reachable at {}. Start it with: pip install supertonic && supertonic serve",
                    base_url
                ))
            } else {
                let err_msg = format!("[Supertonic] Request failed: {}", e);
                error!("{}", err_msg);
                Err(err_msg)
            }
        }
    }
}

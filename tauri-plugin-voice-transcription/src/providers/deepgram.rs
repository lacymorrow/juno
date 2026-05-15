/// Deepgram real-time streaming transcription provider.
///
/// Protocol:
///   Connect: wss://api.deepgram.com/v1/listen?encoding=linear16&sample_rate=16000&channels=1&punctuate=true&interim_results=true
///   Auth:    Authorization: Token <api_key> header
///   Send:    binary PCM16 little-endian frames at 16 kHz mono
///   Receive: JSON — Metadata, Results (is_final: bool), SpeechStarted, UtteranceEnd
///   Stop:    send {"type":"CloseStream"}, server closes connection

use futures_util::{SinkExt, StreamExt};
use http::Request;
use tauri::{AppHandle, Emitter, Runtime};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};

use crate::constants;

const WS_URL: &str = "wss://api.deepgram.com/v1/listen\
    ?encoding=linear16\
    &sample_rate=16000\
    &channels=1\
    &punctuate=true\
    &interim_results=true";

pub async fn streaming_task<R: Runtime>(
    api_key: String,
    mut audio_rx: tokio::sync::mpsc::UnboundedReceiver<Option<Vec<i16>>>,
    app_handle: AppHandle<R>,
) {
    info!("[Deepgram] Starting streaming transcription session");

    let request = match Request::builder()
        .uri(WS_URL)
        .header("Authorization", format!("Token {}", api_key))
        .header("Host", "api.deepgram.com")
        .body(())
    {
        Ok(r) => r,
        Err(e) => {
            error!("[Deepgram] Failed to build WebSocket request: {}", e);
            emit_error(&app_handle, format!("Failed to build request: {}", e));
            return;
        }
    };

    let ws_stream = match connect_async(request).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            error!("[Deepgram] WebSocket connection failed: {}", e);
            emit_error(&app_handle, format!("Connection failed: {}", e));
            return;
        }
    };

    info!("[Deepgram] WebSocket connected");

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let app_for_reader = app_handle.clone();

    // Spawn reader task — accumulates final-segment transcripts across utterances.
    let reader_handle = tauri::async_runtime::spawn(async move {
        let mut accumulated_final = String::new();

        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    handle_message(&app_for_reader, &text, &mut accumulated_final);
                }
                Ok(Message::Close(_)) => {
                    info!("[Deepgram] Connection closed by server");
                    // Emit the accumulated final transcript as the authoritative result.
                    if !accumulated_final.is_empty() {
                        let _ = app_for_reader.emit(
                            constants::voice_transcription::FINAL_RESULT,
                            serde_json::json!({ "text": accumulated_final }),
                        );
                    }
                    let _ = app_for_reader
                        .emit(constants::voice_transcription::DICTATION_STOPPED, ());
                    break;
                }
                Err(e) => {
                    error!("[Deepgram] WebSocket read error: {}", e);
                    emit_error(&app_for_reader, format!("Stream error: {}", e));
                    break;
                }
                _ => {}
            }
        }
    });

    // Writer loop: forward PCM16 chunks; None signals session close.
    loop {
        match audio_rx.recv().await {
            Some(Some(pcm16)) => {
                let bytes: Vec<u8> = pcm16.iter().flat_map(|s| s.to_le_bytes()).collect();
                if let Err(e) = ws_write.send(Message::Binary(bytes)).await {
                    error!("[Deepgram] Failed to send audio chunk: {}", e);
                    break;
                }
            }
            Some(None) | None => {
                info!("[Deepgram] Sending CloseStream");
                let _ = ws_write
                    .send(Message::Text(r#"{"type":"CloseStream"}"#.to_string()))
                    .await;
                let _ = ws_write.close().await;
                break;
            }
        }
    }

    let _ = reader_handle.await;
    info!("[Deepgram] Streaming session ended");
}

fn handle_message<R: Runtime>(
    app: &AppHandle<R>,
    text: &str,
    accumulated_final: &mut String,
) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            error!("[Deepgram] Failed to parse message: {} — raw: {}", e, text);
            return;
        }
    };

    match v["type"].as_str() {
        Some("Results") => {
            let transcript = v["channel"]["alternatives"][0]["transcript"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let is_final = v["is_final"].as_bool().unwrap_or(false);

            if transcript.is_empty() {
                return;
            }

            if is_final {
                if !accumulated_final.is_empty() {
                    accumulated_final.push(' ');
                }
                accumulated_final.push_str(&transcript);
                info!(
                    "[Deepgram] Final segment: '{}' (running total: '{}')",
                    transcript, accumulated_final
                );
                let _ = app.emit(
                    constants::voice_transcription::FINAL_RESULT,
                    serde_json::json!({ "text": accumulated_final.clone() }),
                );
            } else {
                let _ = app.emit(
                    constants::voice_transcription::PARTIAL_RESULT,
                    serde_json::json!({ "text": transcript }),
                );
            }
        }
        Some("Metadata") => {
            info!("[Deepgram] Metadata received");
        }
        Some("SpeechStarted") => {
            info!("[Deepgram] Speech started");
        }
        Some("UtteranceEnd") => {
            info!("[Deepgram] Utterance ended");
        }
        Some(other) => {
            info!("[Deepgram] Unhandled message type: {}", other);
        }
        None => {}
    }
}

fn emit_error<R: Runtime>(app: &AppHandle<R>, message: String) {
    let _ = app.emit(
        constants::voice_transcription::ERROR,
        serde_json::json!({
            "type": "provider_error",
            "provider": "deepgram",
            "message": message,
        }),
    );
}

/// AssemblyAI real-time streaming transcription provider.
///
/// Protocol:
///   Connect: wss://api.assemblyai.com/v2/realtime/ws?sample_rate=16000
///   Auth:    Authorization: <api_key> header
///   Send:    binary PCM16 little-endian frames at 16 kHz mono
///   Receive: JSON — SessionBegins, PartialTranscript, FinalTranscript, SessionTerminated
///   Stop:    send {"message_type":"TerminateSession"}, wait for FinalTranscript

use futures_util::{SinkExt, StreamExt};
use http::Request;
use tauri::{AppHandle, Emitter, Runtime};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};

use crate::constants;

const WS_URL: &str = "wss://api.assemblyai.com/v2/realtime/ws?sample_rate=16000";

pub async fn streaming_task<R: Runtime>(
    api_key: String,
    mut audio_rx: tokio::sync::mpsc::UnboundedReceiver<Option<Vec<i16>>>,
    app_handle: AppHandle<R>,
) {
    info!("[AssemblyAI] Starting streaming transcription session");

    let request = match Request::builder()
        .uri(WS_URL)
        .header("Authorization", &api_key)
        .header("Host", "api.assemblyai.com")
        .body(())
    {
        Ok(r) => r,
        Err(e) => {
            error!("[AssemblyAI] Failed to build WebSocket request: {}", e);
            emit_error(&app_handle, format!("Failed to build request: {}", e));
            return;
        }
    };

    let ws_stream = match connect_async(request).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            error!("[AssemblyAI] WebSocket connection failed: {}", e);
            emit_error(&app_handle, format!("Connection failed: {}", e));
            return;
        }
    };

    info!("[AssemblyAI] WebSocket connected");

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let app_for_reader = app_handle.clone();

    // Spawn reader task — decoupled from writer so network reads don't block audio sends.
    let reader_handle = tauri::async_runtime::spawn(async move {
        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    handle_message(&app_for_reader, &text);
                }
                Ok(Message::Close(frame)) => {
                    info!("[AssemblyAI] Connection closed: {:?}", frame);
                    let _ = app_for_reader
                        .emit(constants::voice_transcription::DICTATION_STOPPED, ());
                    break;
                }
                Err(e) => {
                    error!("[AssemblyAI] WebSocket read error: {}", e);
                    emit_error(&app_for_reader, format!("Stream error: {}", e));
                    break;
                }
                _ => {}
            }
        }
    });

    // Writer loop: forward PCM16 chunks; None signals session terminate.
    loop {
        match audio_rx.recv().await {
            Some(Some(pcm16)) => {
                let bytes: Vec<u8> = pcm16.iter().flat_map(|s| s.to_le_bytes()).collect();
                if let Err(e) = ws_write.send(Message::Binary(bytes)).await {
                    error!("[AssemblyAI] Failed to send audio chunk: {}", e);
                    break;
                }
            }
            Some(None) | None => {
                info!("[AssemblyAI] Sending TerminateSession");
                let _ = ws_write
                    .send(Message::Text(
                        r#"{"message_type":"TerminateSession"}"#.to_string(),
                    ))
                    .await;
                break;
            }
        }
    }

    // Wait for reader to emit the final transcript before returning.
    let _ = reader_handle.await;
    info!("[AssemblyAI] Streaming session ended");
}

fn handle_message<R: Runtime>(app: &AppHandle<R>, text: &str) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            error!("[AssemblyAI] Failed to parse message: {} — raw: {}", e, text);
            return;
        }
    };

    match v["message_type"].as_str() {
        Some("SessionBegins") => {
            info!(
                "[AssemblyAI] Session started — id={}",
                v["session_id"].as_str().unwrap_or("?")
            );
        }
        Some("PartialTranscript") => {
            let transcript = v["text"].as_str().unwrap_or("").to_string();
            if !transcript.is_empty() {
                let _ = app.emit(
                    constants::voice_transcription::PARTIAL_RESULT,
                    serde_json::json!({ "text": transcript }),
                );
            }
        }
        Some("FinalTranscript") => {
            let transcript = v["text"].as_str().unwrap_or("").to_string();
            info!("[AssemblyAI] Final transcript: '{}'", transcript);
            let _ = app.emit(
                constants::voice_transcription::FINAL_RESULT,
                serde_json::json!({ "text": transcript }),
            );
            let _ = app.emit(constants::voice_transcription::DICTATION_STOPPED, ());
        }
        Some("SessionTerminated") => {
            info!("[AssemblyAI] Session terminated by server");
        }
        Some(other) => {
            info!("[AssemblyAI] Unhandled message_type: {}", other);
        }
        None => {}
    }
}

fn emit_error<R: Runtime>(app: &AppHandle<R>, message: String) {
    let _ = app.emit(
        constants::voice_transcription::ERROR,
        serde_json::json!({
            "type": "provider_error",
            "provider": "assemblyai",
            "message": message,
        }),
    );
}

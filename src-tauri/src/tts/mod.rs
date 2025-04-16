pub mod elevenlabs;
pub mod replicate;

// Placeholder command to satisfy the handler
#[tauri::command]
pub fn stop_speech() {
    println!("[TTS] stop_speech command invoked (placeholder).");
    // TODO: Implement actual speech stopping logic here
}

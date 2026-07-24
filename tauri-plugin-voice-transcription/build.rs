const COMMANDS: &[&str] = &[
    "start_dictation",
    "stop_dictation",
    "toggle_dictation",
    "get_dictation_status",
    "get_initialization_status",
    "transcribe_file",
    "set_model_path",
    "get_model_path",
    "start_always_listening",
    "stop_always_listening",
    "toggle_always_listening",
    "get_always_listening_status",
    "set_always_listening_sensitivity",
    "get_always_listening_sensitivity",
    "set_always_listening_wake_words",
    "get_always_listening_wake_words",
    "set_transcription_debugging",
    "set_audio_level_monitoring",
    "test_whisper_model",
    "force_transcription_test",
    "check_microphone_permission",
    "request_microphone_permission",
    "ensure_microphone_ready",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}

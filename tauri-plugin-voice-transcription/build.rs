const COMMANDS: &[&str] = &[
    "start_dictation",
    "stop_dictation",
    "toggle_dictation",
    "get_dictation_status",
    "transcribe_file",
    "set_model_path",
    "get_model_path",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .build();
}

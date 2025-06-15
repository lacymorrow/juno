#[tauri::command]
async fn init_app(app_handle: AppHandle) -> Result<(), String> {
    // let current_dir = env::current_dir()
    //     .map_err(|e| format!("Failed to get current directory: {}", e.to_string()))?;
    // log::info!("Current working directory: {:?}", current_dir);

    let _app_state_clone = app_handle.state::<state::AppState>();
    // ... existing code ...
}

// Listen for global escape key press to cancel current agent task
app_handle_for_listener.global_shortcut_manager().register("escape", move || {
    log::warn!("Escape key pressed, attempting to cancel current task...");
    let _state_clone = app_handle_for_listener.state::<crate::state::AppState>();
    // ... existing code ...
})

// Listen for agent errors
app_handle_for_error_listener.listen_global("agent-error", move |event| {
    log::error!("Agent Error Event: {:?}", event);
    let _state_clone = app_handle_for_error_listener.state::<crate::state::AppState>();
    // ... existing code ...
})

// Dictation start/stop listener
app_handle_for_dictation_start.listen_global("dictation-status-change", move |event| {
    let _app_state_clone = app_handle_for_dictation_start.state::<state::AppState>();
    // ... existing code ...
})

#[allow(dead_code)]
fn try_get_app_handle() -> Option<AppHandle> {
    // ... existing code ...
}

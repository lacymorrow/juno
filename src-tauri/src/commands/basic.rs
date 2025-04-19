use crate::AppState;
use tauri;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub fn check_server_status(state: tauri::State<'_, AppState>) -> bool {
    // For now, just confirm the state exists.
    // We could add more checks here later if needed.
    let _ = state.desktop; // Access it to ensure it's valid
    true // Assume connected if we reached here
}

#[tauri::command]
pub async fn list_apps(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    match state.desktop.applications() {
        Ok(apps) => {
            let app_names = apps
                .into_iter()
                .map(|app| {
                    // Use attributes().label instead of title()
                    app.attributes()
                        .label
                        .unwrap_or_else(|| "Unknown Label".to_string())
                })
                .collect();
            Ok(app_names)
        }
        Err(e) => Err(format!("Failed to get applications: {}", e)),
    }
}

#[tauri::command]
pub async fn get_logs() -> Vec<String> {
    // This is a stub - the original function was not shown in the provided code
    // Return an empty vector as a placeholder
    Vec::new()
}
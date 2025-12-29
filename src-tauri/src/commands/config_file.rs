use tauri::{AppHandle, Manager};
use tracing::info;

/// Open the configuration directory in the system file manager
#[tauri::command]
pub async fn open_config_directory(app_handle: AppHandle) -> Result<(), String> {
    info!("Opening configuration directory in file manager");

    #[cfg(target_os = "macos")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    #[cfg(target_os = "linux")]
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get app config directory: {}", e))?;

    #[cfg(target_os = "windows")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    info!("Configuration directory path: {:?}", config_dir);

    // Open the directory in the system file manager
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try common file managers
        let file_managers = ["xdg-open", "nautilus", "dolphin", "thunar", "pcmanfm"];
        let mut opened = false;
        
        for fm in &file_managers {
            if std::process::Command::new(fm)
                .arg(&config_dir)
                .spawn()
                .is_ok()
            {
                opened = true;
                break;
            }
        }
        
        if !opened {
            return Err("Failed to open directory: No suitable file manager found".to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    Ok(())
}

/// Open a specific configuration file in the system default editor
#[tauri::command]
pub async fn open_config_file(app_handle: AppHandle, file_name: String) -> Result<(), String> {
    info!("Opening configuration file: {}", file_name);

    // Validate file name to prevent path traversal
    if file_name.contains("..") || file_name.contains("/") || file_name.contains("\\") {
        return Err("Invalid file name".to_string());
    }

    #[cfg(target_os = "macos")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    #[cfg(target_os = "linux")]
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get app config directory: {}", e))?;

    #[cfg(target_os = "windows")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    let file_path = config_dir.join(&file_name);
    
    if !file_path.exists() {
        return Err(format!("Configuration file '{}' not found", file_name));
    }

    info!("Opening file: {:?}", file_path);

    // Open the file in the default editor
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/C", "start", "", file_path.to_str().unwrap()])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }

    Ok(())
}

/// Get the configuration directory path
#[tauri::command]
pub async fn get_config_directory_path(app_handle: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    #[cfg(target_os = "linux")]
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get app config directory: {}", e))?;

    #[cfg(target_os = "windows")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    Ok(config_dir.to_string_lossy().to_string())
}
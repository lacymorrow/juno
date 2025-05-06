use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tracing::info;

use crate::commands::send_dev_tool_notification;
use crate::state::AppState;

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_dir: bool,
    // Consider adding other fields like size, modified_date if needed in the future
}

#[tauri::command]
pub(crate) async fn dev_list_files(
    app: AppHandle,
    _state: State<'_, AppState>, // _state might be needed later for config or context
    path_str: String, // Renamed from path to path_str to avoid conflict with std::path
) -> Result<String, String> {
    info!("[DEV_TOOL] Listing files for input path: {}", path_str);

    let expanded_path = if path_str.starts_with("~") {
        match dirs::home_dir() {
            Some(home) => {
                if path_str == "~" {
                    home
                } else if path_str.starts_with("~/" ){
                    home.join(&path_str[2..])
                } else {
                    PathBuf::from(path_str.clone())
                }
            }
            None => {
                let err_msg = "Failed to resolve home directory for path starting with ~".to_string();
                info!("[DEV_TOOL] Error: {}", err_msg);
                send_dev_tool_notification(
                    &app,
                    "List Files Error",
                    &err_msg,
                )?;
                return Err(err_msg);
            }
        }
    } else {
        PathBuf::from(path_str.clone())
    };

    info!("[DEV_TOOL] Expanded path to list: {:?}", expanded_path);
    let path_to_list = expanded_path.as_path();

    if !path_to_list.exists() {
        let err_msg = format!("Path does not exist: {:?}", path_to_list);
        info!("[DEV_TOOL] Error: {}", err_msg);
        send_dev_tool_notification(
            &app,
            "List Files Error",
            &format!("Path not found: {:?}", path_to_list),
        )?;
        return Err(err_msg);
    }

    if !path_to_list.is_dir() {
        let err_msg = format!("Path is not a directory: {:?}", path_to_list);
        info!("[DEV_TOOL] Error: {}", err_msg);
        send_dev_tool_notification(
            &app,
            "List Files Error",
            &format!("Not a directory: {:?}", path_to_list),
        )?;
        return Err(err_msg);
    }

    match fs::read_dir(path_to_list) {
        Ok(entries) => {
            let mut file_entries: Vec<FileEntry> = Vec::new();
            for entry_result in entries {
                match entry_result {
                    Ok(entry) => {
                        let file_name = entry
                            .file_name()
                            .into_string()
                            .unwrap_or_else(|_| "Invalid UTF-8 name".to_string());
                        let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
                        file_entries.push(FileEntry {
                            name: file_name,
                            is_dir,
                        });
                    }
                    Err(e) => {
                        info!("[DEV_TOOL] Error reading directory entry in {:?}: {}", path_to_list, e);
                    }
                }
            }

            match serde_json::to_string_pretty(&file_entries) {
                Ok(json_string) => {
                    send_dev_tool_notification(
                        &app,
                        "List Files",
                        &format!("Listed {} items in {:?}", file_entries.len(), path_to_list),
                    )?;
                    Ok(json_string)
                }
                Err(e) => {
                    let err_msg = format!("Failed to serialize file list for {:?}: {}", path_to_list, e);
                    info!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to read directory '{:?}': {}", path_to_list, e);
            info!("[DEV_TOOL] Error: {}", err_msg);
            send_dev_tool_notification(
                &app,
                "List Files Error",
                &format!("Failed to read dir {:?}: {}", path_to_list, e),
            )?;
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_get_file_content(
    app: AppHandle,
    _state: State<'_, AppState>,
    path_str: String, // Use path_str for consistency
) -> Result<String, String> {
    info!("[DEV_TOOL] Getting content for file: {}", path_str);

    let expanded_path = if path_str.starts_with("~") {
        match dirs::home_dir() {
            Some(home) => {
                if path_str == "~" {
                    // Technically, listing home dir as file content is an error, but path expansion is generic.
                    // The function should ideally check if it's a file before reading.
                    // For now, let it proceed and fail at read_to_string if it's a directory.
                    home
                } else if path_str.starts_with("~/" ){
                    home.join(&path_str[2..])
                } else {
                    PathBuf::from(path_str.clone())
                }
            }
            None => {
                let err_msg = "Failed to resolve home directory for path starting with ~".to_string();
                info!("[DEV_TOOL] Error: {}", err_msg);
                send_dev_tool_notification(&app, "Get Content Error", &err_msg)?;
                return Err(err_msg);
            }
        }
    } else {
        PathBuf::from(path_str.clone())
    };

    info!("[DEV_TOOL] Expanded path to read: {:?}", expanded_path);
    let file_path = expanded_path.as_path();

    if !file_path.exists() {
        let err_msg = format!("File does not exist: {:?}", file_path);
        info!("[DEV_TOOL] Error: {}", err_msg);
        send_dev_tool_notification(&app, "Get Content Error", &err_msg)?;
        return Err(err_msg);
    }

    if file_path.is_dir() {
        let err_msg = format!("Path is a directory, not a file: {:?}", file_path);
        info!("[DEV_TOOL] Error: {}", err_msg);
        send_dev_tool_notification(&app, "Get Content Error", &err_msg)?;
        return Err(err_msg);
    }

    match fs::read_to_string(file_path) {
        Ok(content) => {
            send_dev_tool_notification(
                &app,
                "Get File Content",
                &format!("Read content from {:?}", file_path),
            )?;
            Ok(content)
        }
        Err(e) => {
            let err_msg = format!("Failed to read file '{:?}': {}", file_path, e);
            info!("[DEV_TOOL] Error: {}", err_msg);
            send_dev_tool_notification(&app, "Get Content Error", &err_msg)?;
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_set_file_content(
    app: AppHandle,
    _state: State<'_, AppState>,
    path_str: String, // Use path_str for consistency
    content: String,
) -> Result<(), String> {
    info!("[DEV_TOOL] Setting content for file: {}", path_str);

    let expanded_path = if path_str.starts_with("~") {
        match dirs::home_dir() {
            Some(home) => {
                if path_str == "~" {
                     // Cannot write content to home directory directly like this
                    let err_msg = "Cannot set content for home directory '~' as if it were a file.".to_string();
                    info!("[DEV_TOOL] Error: {}", err_msg);
                    send_dev_tool_notification(&app, "Set Content Error", &err_msg)?;
                    return Err(err_msg);
                } else if path_str.starts_with("~/" ){
                    home.join(&path_str[2..])
                } else {
                    PathBuf::from(path_str.clone())
                }
            }
            None => {
                let err_msg = "Failed to resolve home directory for path starting with ~".to_string();
                info!("[DEV_TOOL] Error: {}", err_msg);
                send_dev_tool_notification(&app, "Set Content Error", &err_msg)?;
                return Err(err_msg);
            }
        }
    } else {
        PathBuf::from(path_str.clone())
    };

    info!("[DEV_TOOL] Expanded path to write: {:?}", expanded_path);
    let file_path = expanded_path.as_path();

    // Optional: Create parent directories if they don't exist
    if let Some(parent_dir) = file_path.parent() {
        if !parent_dir.exists() {
            if let Err(e) = fs::create_dir_all(parent_dir) {
                let err_msg = format!("Failed to create parent directories for '{:?}': {}", file_path, e);
                info!("[DEV_TOOL] Error: {}", err_msg);
                send_dev_tool_notification(&app, "Set Content Error", &err_msg)?;
                return Err(err_msg);
            }
            info!("[DEV_TOOL] Created parent directories for {:?}", file_path);
        }
    }

    // If it's a directory, we shouldn't write to it as if it's a file.
    if file_path.is_dir() {
        let err_msg = format!("Path is a directory, cannot write file content: {:?}", file_path);
        info!("[DEV_TOOL] Error: {}", err_msg);
        send_dev_tool_notification(&app, "Set Content Error", &err_msg)?;
        return Err(err_msg);
    }

    match fs::write(file_path, content) {
        Ok(_) => {
            send_dev_tool_notification(
                &app,
                "Set File Content",
                &format!("Wrote content to {:?}", file_path),
            )?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to write file '{:?}': {}", file_path, e);
            info!("[DEV_TOOL] Error: {}", err_msg);
            send_dev_tool_notification(&app, "Set Content Error", &err_msg)?;
            Err(err_msg)
        }
    }
}

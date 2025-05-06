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

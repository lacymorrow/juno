use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tracing::info;
use html_escape;
use chrono;

use crate::commands::send_dev_tool_notification;
use crate::state::AppState;

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_dir: bool,
    // Consider adding other fields like size, modified_date if needed in the future
}

#[tauri::command]
pub async fn dev_list_files(
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
pub async fn dev_get_file_content(
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
pub async fn dev_set_file_content(
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

#[tauri::command]
pub async fn save_agent_response(
    app: AppHandle,
    _state: State<'_, AppState>,
    content: String,
    format: String, // "html" or "markdown"
    suggested_filename: Option<String>,
) -> Result<String, String> {
    info!("[SAVE_RESPONSE] Saving agent response in {} format", format);

    // Create a safe filename based on timestamp and content preview
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let content_preview = content
        .chars()
        .take(30)
        .collect::<String>()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "_");
    
    let default_filename = if let Some(suggested) = suggested_filename {
        suggested
    } else {
        format!("agent_response_{}_{}", timestamp, content_preview)
    };

    let file_extension = match format.as_str() {
        "html" => "html",
        "markdown" => "md",
        _ => return Err("Invalid format. Use 'html' or 'markdown'".to_string()),
    };

    // Get the user's Downloads directory
    let downloads_dir = dirs::download_dir()
        .ok_or_else(|| "Could not find Downloads directory".to_string())?;

    let file_path = downloads_dir.join(format!("{}.{}", default_filename, file_extension));

    // Generate the content based on format
    let file_content = match format.as_str() {
        "html" => generate_html_content(&content, &timestamp.to_string()),
        "markdown" => generate_markdown_content(&content, &timestamp.to_string()),
        _ => return Err("Invalid format".to_string()),
    };

    // Write the file
    match fs::write(&file_path, file_content) {
        Ok(_) => {
            let success_msg = format!("Saved agent response to: {:?}", file_path);
            info!("[SAVE_RESPONSE] {}", success_msg);
            send_dev_tool_notification(
                &app,
                "Save Agent Response",
                &success_msg,
            )?;
            Ok(file_path.to_string_lossy().to_string())
        }
        Err(e) => {
            let err_msg = format!("Failed to save file '{:?}': {}", file_path, e);
            info!("[SAVE_RESPONSE] Error: {}", err_msg);
            send_dev_tool_notification(&app, "Save Response Error", &err_msg)?;
            Err(err_msg)
        }
    }
}

fn generate_html_content(content: &str, timestamp: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Juno AI Agent Response</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            line-height: 1.6;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f9f9f9;
        }}
        .container {{
            background: white;
            border-radius: 8px;
            padding: 30px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}
        .header {{
            border-bottom: 2px solid #e2e8f0;
            padding-bottom: 15px;
            margin-bottom: 25px;
        }}
        .title {{
            color: #2563eb;
            margin: 0;
            font-size: 24px;
        }}
        .timestamp {{
            color: #64748b;
            font-size: 14px;
            margin-top: 5px;
        }}
        .content {{
            color: #334155;
            white-space: pre-wrap;
            word-wrap: break-word;
        }}
        .footer {{
            margin-top: 30px;
            padding-top: 15px;
            border-top: 1px solid #e2e8f0;
            text-align: center;
            color: #64748b;
            font-size: 12px;
        }}
        code {{
            background-color: #f1f5f9;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
        }}
        pre {{
            background-color: #f1f5f9;
            padding: 15px;
            border-radius: 6px;
            overflow-x: auto;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1 class="title">🐕 Juno AI Agent Response</h1>
            <div class="timestamp">Generated on: {}</div>
        </div>
        <div class="content">{}</div>
        <div class="footer">
            Generated by Juno AI Assistant
        </div>
    </div>
</body>
</html>"#,
        timestamp,
        html_escape::encode_text(content)
    )
}

fn generate_markdown_content(content: &str, timestamp: &str) -> String {
    format!(
        r#"# 🐕 Juno AI Agent Response

**Generated on:** {}

---

{}

---

*Generated by Juno AI Assistant*
"#,
        timestamp, content
    )
}

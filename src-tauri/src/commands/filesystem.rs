//! Filesystem operations - file listing, reading, and writing capabilities
//! Consolidated to production functions with conditional debug features

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

// ============================================================================
// CONSOLIDATED: dev_ functions removed - use production functions instead
// ============================================================================

// CONSOLIDATED: dev_list_files removed - use list_files production function
// Located in this same file with debug capabilities enabled via debug_mode parameter
// CONSOLIDATED: dev_get_file_content removed - use get_file_content production function
// Located in this same file with debug capabilities enabled via debug_mode parameter
// CONSOLIDATED: dev_set_file_content removed - use set_file_content production function
// Located in this same file with debug capabilities enabled via debug_mode parameter

// These functions replace the dev_ prefixed functions by incorporating debug features conditionally

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

// ============================================================================
// PRODUCTION FILESYSTEM FUNCTIONS WITH UNIFIED DEBUG SYSTEM
// ============================================================================

/// Production function to list files in a directory with optional debug features
#[tauri::command]
pub async fn list_files(
    app: AppHandle,
    state: State<'_, AppState>,
    path_str: String,
    debug_mode: Option<bool>,
) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, DebugOperation, should_enable_debug, validators::valid_file_path, send_debug_notification};
    use std::path::Path;
    use std::fs;
    use tracing::{info, error, warn};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("list_files", debug_config.clone());

        // Debug validation
    if debug_config.validate_inputs {
        if let Err(e) = valid_file_path(&path_str) {
            let err_msg = format!("Invalid path: {}", e);
            if debug_config.send_notifications {
                send_debug_notification(&app, "List Files Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }
    }

    if debug_config.log_operations {
        info!("[FILESYSTEM] Listing files in: {}", path_str);
    }

    let path = Path::new(&path_str);

        if !path.exists() {
        let err_msg = format!("Path does not exist: {:?}", path);
        if debug_config.log_operations {
            error!("[FILESYSTEM] Error: {}", err_msg);
        }
        if debug_config.send_notifications {
            send_debug_notification(&app, "List Files Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
    }

    if !path.is_dir() {
        let err_msg = format!("Path is not a directory: {:?}", path);
        if debug_config.log_operations {
            error!("[FILESYSTEM] Error: {}", err_msg);
        }
        if debug_config.send_notifications {
            send_debug_notification(&app, "List Files Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            let err_msg = format!("Failed to read directory '{:?}': {}", path, e);
            if debug_config.log_operations {
                error!("[FILESYSTEM] Error: {}", err_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app, "List Files Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }
    };

    let mut file_entries = Vec::new();

    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("<invalid>")
                    .to_string();

                let is_dir = path.is_dir();
                file_entries.push(FileEntry { name, is_dir });
            }
                        Err(e) => {
                if debug_config.log_operations {
                    warn!("[FILESYSTEM] Skipping invalid entry: {}", e);
                }
            }
        }
    }

    // Sort entries: directories first, then files, both alphabetically
    file_entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    let result = match serde_json::to_string(&file_entries) {
        Ok(json) => json,
        Err(e) => {
            let err_msg = format!("Failed to serialize file entries: {}", e);
            if debug_config.log_operations {
                error!("[FILESYSTEM] Error: {}", err_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app, "List Files Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }
    };

    if debug_config.send_notifications {
        send_debug_notification(
            &app,
            "List Files",
            &format!("Listed {} items in {:?}", file_entries.len(), path),
        )?;
    }

    debug_op.complete(Some(&app), true);
    Ok(result)
}

/// Production function to get file content with optional debug features
#[tauri::command]
pub async fn get_file_content(
    app: AppHandle,
    state: State<'_, AppState>,
    path_str: String,
    debug_mode: Option<bool>,
) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, DebugOperation, should_enable_debug, validators::valid_file_path, send_debug_notification};
    use std::path::Path;
    use std::fs;
    use tracing::{info, error};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("get_file_content", debug_config.clone());

    // Debug validation
    if debug_config.validate_inputs {
        if let Err(e) = valid_file_path(&path_str) {
            let err_msg = format!("Invalid path: {}", e);
            if debug_config.send_notifications {
                send_debug_notification(&app, "Get File Content Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }
    }

    if debug_config.log_operations {
        info!("[FILESYSTEM] Reading file: {}", path_str);
    }

    let file_path = Path::new(&path_str);

    if !file_path.exists() {
        let err_msg = format!("File does not exist: {:?}", file_path);
        if debug_config.log_operations {
            error!("[FILESYSTEM] Error: {}", err_msg);
        }
        if debug_config.send_notifications {
            send_debug_notification(&app, "Get File Content Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
    }

    if file_path.is_dir() {
        let err_msg = format!("Path is a directory, not a file: {:?}", file_path);
        if debug_config.log_operations {
            error!("[FILESYSTEM] Error: {}", err_msg);
        }
        if debug_config.send_notifications {
            send_debug_notification(&app, "Get File Content Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
    }

    let content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            let err_msg = format!("Failed to read file '{:?}': {}", file_path, e);
            if debug_config.log_operations {
                error!("[FILESYSTEM] Error: {}", err_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app, "Get File Content Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }
    };

    if debug_config.send_notifications {
        let preview = if content.len() > 100 {
            format!("{}... ({} chars)", &content[..100], content.len())
        } else {
            content.clone()
        };
        send_debug_notification(
            &app,
            "Get File Content",
            &format!("Read file {:?}: {}", file_path, preview),
        )?;
    }

    debug_op.complete(Some(&app), true);
    Ok(content)
}

/// Production function to set file content with optional debug features
#[tauri::command]
pub async fn set_file_content(
    app: AppHandle,
    state: State<'_, AppState>,
    path_str: String,
    content: String,
    debug_mode: Option<bool>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, DebugOperation, should_enable_debug, validators::valid_file_path, send_debug_notification};
    use std::path::Path;
    use std::fs;
    use tracing::{info, error};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("set_file_content", debug_config.clone());

        // Debug validation
    if debug_config.validate_inputs {
        if let Err(e) = valid_file_path(&path_str) {
            let err_msg = format!("Invalid path: {}", e);
            if debug_config.send_notifications {
                send_debug_notification(&app, "Set File Content Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            return Err(err_msg);
        }
    }

    if debug_config.log_operations {
        info!("[FILESYSTEM] Writing to file: {} ({} chars)", path_str, content.len());
    }

    let file_path = Path::new(&path_str);

    // Create parent directories if they don't exist
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                let err_msg = format!("Failed to create parent directories for '{:?}': {}", file_path, e);
                if debug_config.log_operations {
                    error!("[FILESYSTEM] Error: {}", err_msg);
                }
                if debug_config.send_notifications {
                    send_debug_notification(&app, "Set File Content Error", &err_msg)?;
                }
                debug_op.complete(Some(&app), false);
                return Err(err_msg);
            }
            if debug_config.log_operations {
                info!("[FILESYSTEM] Created parent directories for {:?}", file_path);
            }
        }
    }

    // If it's a directory, we shouldn't write to it as if it's a file
    if file_path.is_dir() {
        let err_msg = format!("Path is a directory, cannot write file content: {:?}", file_path);
        if debug_config.log_operations {
            error!("[FILESYSTEM] Error: {}", err_msg);
        }
        if debug_config.send_notifications {
            send_debug_notification(&app, "Set File Content Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
    }

    match fs::write(file_path, &content) {
        Ok(_) => {
            if debug_config.send_notifications {
                let preview = if content.len() > 100 {
                    format!("{}... ({} chars)", &content[..100], content.len())
                } else {
                    content.clone()
                };
                send_debug_notification(
                    &app,
                    "Set File Content",
                    &format!("Wrote to {:?}: {}", file_path, preview),
                )?;
            }

            debug_op.complete(Some(&app), true);
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to write file '{:?}': {}", file_path, e);
            if debug_config.log_operations {
                error!("[FILESYSTEM] Error: {}", err_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app, "Set File Content Error", &err_msg)?;
            }
            debug_op.complete(Some(&app), false);
            Err(err_msg)
        }
    }
}

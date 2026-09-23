//! Filesystem operations - file listing, reading, and writing capabilities
//! Consolidated to production functions with conditional debug features

use serde::Serialize;

use tauri::{AppHandle, State};

use crate::export::{ResponseExport, ResponseExportInput};
use crate::platform::share_sheet::ShareAnchor;
use crate::state::AppState;

#[derive(Serialize)]
struct FileEntry {
    name: String,
    is_dir: bool,
    // Consider adding other fields like size, modified_date if needed in the future
}

/// Put an agent response on the system clipboard, as the person read it. The
/// web view's own clipboard API is refused in the floating bar (the window is
/// never key, so WebKit sees no user activation), so the button goes through
/// the native pasteboard.
#[tauri::command]
pub async fn copy_agent_response(response: ResponseExportInput) -> Result<(), String> {
    let text = ResponseExport::from_input(response).plain_text();
    tokio::task::spawn_blocking(move || {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text))
            .map_err(|e| format!("Couldn't write to the clipboard: {e}"))
    })
    .await
    .map_err(|e| format!("Clipboard task failed: {e}"))?
}

/// Open the native share sheet for an agent response, anchored to the Share
/// button that was clicked. The sheet carries the reply as text plus Juno's
/// "Save as Markdown…" / "Save as HTML…" services (see `platform::share_sheet`).
#[tauri::command]
pub async fn share_agent_response(
    window: tauri::WebviewWindow,
    response: ResponseExportInput,
    anchor: ShareAnchor,
) -> Result<(), String> {
    crate::platform::share_sheet::present(&window, ResponseExport::from_input(response), anchor)
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
    use crate::commands::debug_utils::{
        send_debug_notification, should_enable_debug, validators::valid_file_path, DebugConfig,
        DebugOperation,
    };
    use std::fs;
    use std::path::Path;
    use tracing::{error, info, warn};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("list_files", debug_config.clone());

    // Unconditional path check (LAC-4013). Interim until this command is
    // wired to path_security::resolve_within_default_roots (Fix B).
    if let Err(e) = valid_file_path(&path_str) {
        let err_msg = format!("Invalid path: {}", e);
        if debug_config.send_notifications {
            send_debug_notification(&app, "List Files Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
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
                let name = path
                    .file_name()
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
    file_entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
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
    use crate::commands::debug_utils::{
        send_debug_notification, should_enable_debug, validators::valid_file_path, DebugConfig,
        DebugOperation,
    };
    use std::fs;
    use std::path::Path;
    use tracing::{error, info};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("get_file_content", debug_config.clone());

    // Unconditional path check (LAC-4013). Interim until this command is
    // wired to path_security::resolve_within_default_roots (Fix B).
    if let Err(e) = valid_file_path(&path_str) {
        let err_msg = format!("Invalid path: {}", e);
        if debug_config.send_notifications {
            send_debug_notification(&app, "Get File Content Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
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
            format!(
                "{}... ({} chars)",
                crate::utils::strings::truncate_chars(&content, 100),
                content.len()
            )
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
    use crate::commands::debug_utils::{
        send_debug_notification, should_enable_debug, validators::valid_file_path, DebugConfig,
        DebugOperation,
    };
    use std::fs;
    use std::path::Path;
    use tracing::{error, info};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("set_file_content", debug_config.clone());

    // Unconditional path check (LAC-4013). Interim until this command is
    // wired to path_security::resolve_within_default_roots (Fix B).
    if let Err(e) = valid_file_path(&path_str) {
        let err_msg = format!("Invalid path: {}", e);
        if debug_config.send_notifications {
            send_debug_notification(&app, "Set File Content Error", &err_msg)?;
        }
        debug_op.complete(Some(&app), false);
        return Err(err_msg);
    }

    if debug_config.log_operations {
        info!(
            "[FILESYSTEM] Writing to file: {} ({} chars)",
            path_str,
            content.len()
        );
    }

    let file_path = Path::new(&path_str);

    // Create parent directories if they don't exist
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                let err_msg = format!(
                    "Failed to create parent directories for '{:?}': {}",
                    file_path, e
                );
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
                info!(
                    "[FILESYSTEM] Created parent directories for {:?}",
                    file_path
                );
            }
        }
    }

    // If it's a directory, we shouldn't write to it as if it's a file
    if file_path.is_dir() {
        let err_msg = format!(
            "Path is a directory, cannot write file content: {:?}",
            file_path
        );
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
                    format!(
                        "{}... ({} chars)",
                        crate::utils::strings::truncate_chars(&content, 100),
                        content.len()
                    )
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

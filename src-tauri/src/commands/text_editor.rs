// Commands related to text editor operations (view, create, replace, insert, undo)

use crate::state::AppState;
use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};
use tauri::{AppHandle, State};
use std::fs;
use std::io;
use std::path::{PathBuf, Path};
use tracing::{info, warn, error};
use super::send_dev_tool_notification; // Use helper from parent module

// Helper moved here as it's only used by text editor commands
fn update_undo_state(state: &State<AppState>, path: String, previous_content: Option<String>) -> Result<(), String> {
    let mut last_edited = state.last_edited_file.lock()
        .map_err(|e| format!("Failed to acquire last_edited_file lock: {}", e))?;
    *last_edited = Some(path.into()); // Convert String to PathBuf
    let mut prev_content = state.previous_content.lock()
        .map_err(|e| format!("Failed to acquire previous_content lock: {}", e))?;
    *prev_content = Some(previous_content); // Wrap Option<String> in Option
    Ok(())
}

// =============================================================================
// CONSOLIDATED PRODUCTION COMMANDS WITH DEBUG FEATURES
// =============================================================================

#[tauri::command]
pub(crate) async fn text_editor_view(
    path: String,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<String, String> {
    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_debug_operation("text_editor_view", &format!("Reading file content: {}", Path::new(&path).display()));
    }

    let start_time = std::time::Instant::now();

    match fs::read_to_string(&path) {
        Ok(content) => {
            if debug {
                time_operation(start_time);
                log_debug_operation("text_editor_view", &format!("Successfully read {} bytes", content.len()));
            }
            Ok(content)
        }
        Err(e) => {
            let err_msg = format!("Failed to read file '{}': {}", path, e);
            if debug {
                log_debug_operation("text_editor_view", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_create(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    content: String,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(debug_mode, &state);
    let path_buf: PathBuf = path.into();

    if debug {
        log_debug_operation("text_editor_create", &format!("Creating/overwriting file: {}", path_buf.display()));
    }

    let start_time = std::time::Instant::now();

    // Store previous state for undo
    let previous_content = fs::read_to_string(&path_buf).ok();
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), previous_content) {
        if debug {
            log_debug_operation("text_editor_create", &format!("Warning: Failed to update undo state: {}", e));
        }
    }

    match fs::write(&path_buf, &content) {
        Ok(_) => {
            if debug {
                time_operation(start_time);
                send_debug_notification(&app, "File Operation", &format!("File '{}' created/updated.", path_buf.display()))?;
            }
            Ok(())
        },
        Err(e) => {
            let err_msg = format!("Failed to write file '{}': {}", path_buf.display(), e);
            if debug {
                log_debug_operation("text_editor_create", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_str_replace(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    find: String,
    replace: String,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(debug_mode, &state);
    let path_buf: PathBuf = path.into();

    if debug {
        log_debug_operation("text_editor_str_replace", &format!("Replacing '{}' with '{}' in: {}", find, replace, path_buf.display()));
    }

    let start_time = std::time::Instant::now();

    let original_content = fs::read_to_string(&path_buf).map_err(|e| {
        format!("Failed to read file for replace '{}': {}", path_buf.display(), e)
    })?;

    // Store previous state for undo
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone())) {
        if debug {
            log_debug_operation("text_editor_str_replace", &format!("Warning: Failed to update undo state: {}", e));
        }
    }

    let modified_content = original_content.replace(&find, &replace);

    match fs::write(&path_buf, modified_content) {
        Ok(_) => {
            if debug {
                time_operation(start_time);
                send_debug_notification(&app, "File Operation", &format!("String replaced in '{}'.", path_buf.display()))?;
            }
            Ok(())
        },
        Err(e) => {
            let err_msg = format!("Failed to write replaced content to '{}': {}", path_buf.display(), e);
            if debug {
                log_debug_operation("text_editor_str_replace", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_insert(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    line_number: usize, // 1-based line number for insertion
    text: String,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(debug_mode, &state);
    let path_buf: PathBuf = path.into();

    if debug {
        log_debug_operation("text_editor_insert", &format!("Inserting text at line {} in: {}", line_number, path_buf.display()));
    }

    let start_time = std::time::Instant::now();

    let original_content = match fs::read_to_string(&path_buf) {
        Ok(content) => content,
        // If the file doesn't exist and we're inserting at line 1, treat it as creation
        Err(e) if e.kind() == io::ErrorKind::NotFound && line_number == 1 => String::new(),
        Err(e) => {
            return Err(format!("Failed to read file for insert '{}': {}", path_buf.display(), e));
        }
    };

    // Store previous state for undo
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone())) {
        if debug {
            log_debug_operation("text_editor_insert", &format!("Warning: Failed to update undo state: {}", e));
        }
    }

    let mut lines: Vec<String> = original_content.lines().map(String::from).collect();

    // Adjust line number to be 0-based index
    let index = if line_number == 0 { 0 } else { line_number.saturating_sub(1) };

    if index > lines.len() {
        let err_msg = format!("Line number {} is out of bounds for file '{}' ({} lines)", line_number, path_buf.display(), lines.len());
        if debug {
            log_debug_operation("text_editor_insert", &format!("Error: {}", err_msg));
        }
        return Err(err_msg);
    }

    // Insert the new text line by line
    for (i, line_to_insert) in text.lines().enumerate() {
       lines.insert(index + i, line_to_insert.to_string());
    }

    let modified_content = lines.join("\n");

    match fs::write(&path_buf, modified_content) {
        Ok(_) => {
            if debug {
                time_operation(start_time);
                send_debug_notification(&app, "File Operation", &format!("Text inserted into '{}' at line {}.", path_buf.display(), line_number))?;
            }
            Ok(())
        },
        Err(e) => {
            let err_msg = format!("Failed to write inserted content to '{}': {}", path_buf.display(), e);
            if debug {
                log_debug_operation("text_editor_insert", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_undo_edit(
    state: State<'_, AppState>,
    app: AppHandle,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_debug_operation("text_editor_undo_edit", "Undoing last text editor operation");
    }

    let start_time = std::time::Instant::now();

    let mut last_file_lock = state.last_edited_file.lock()
        .map_err(|e| format!("Failed to acquire last_edited_file lock: {}", e))?;
    let mut prev_content_lock = state.previous_content.lock()
        .map_err(|e| format!("Failed to acquire previous_content lock: {}", e))?;

    if let Some(path) = last_file_lock.take() {
        let prev_content_option = prev_content_lock.take();

        if let Some(prev_content) = prev_content_option {
            if let Some(content_to_restore) = prev_content {
                // Had previous content, so restore it
                match fs::write(&path, &content_to_restore) {
                    Ok(_) => {
                        if debug {
                            time_operation(start_time);
                            send_debug_notification(&app, "File Operation", &format!("Undo: Restored '{}'.", path.display()))?;
                        }
                        Ok(())
                    },
                    Err(e) => {
                        let err_msg = format!("Undo failed: Could not restore file '{}': {}", path.display(), e);
                        // Put the state back if write failed
                        *last_file_lock = Some(path);
                        *prev_content_lock = Some(Some(content_to_restore));
                        if debug {
                            log_debug_operation("text_editor_undo_edit", &format!("Error: {}", err_msg));
                        }
                        Err(err_msg)
                    }
                }
            } else {
                // No previous content, meaning the last operation was create, so delete the file
                match fs::remove_file(&path) {
                    Ok(_) => {
                        if debug {
                            time_operation(start_time);
                            send_debug_notification(&app, "File Operation", &format!("Undo: Deleted '{}'.", path.display()))?;
                        }
                        Ok(())
                    },
                    // If the file doesn't exist, that's okay for undoing a create
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        if debug {
                            log_debug_operation("text_editor_undo_edit", &format!("File '{}' was already deleted", path.display()));
                        }
                        Ok(())
                    },
                    Err(e) => {
                        let err_msg = format!("Undo failed: Could not delete file '{}': {}", path.display(), e);
                        // Put the state back if delete failed
                        *last_file_lock = Some(path);
                        *prev_content_lock = Some(None);
                        if debug {
                            log_debug_operation("text_editor_undo_edit", &format!("Error: {}", err_msg));
                        }
                        Err(err_msg)
                    }
                }
            }
        } else {
            let err_msg = format!("Undo failed: Inconsistent state for path '{}'", path.display());
            *last_file_lock = Some(path);
            if debug {
                log_debug_operation("text_editor_undo_edit", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    } else {
        let err_msg = "No text editor operation to undo.".to_string();
        if debug {
            log_debug_operation("text_editor_undo_edit", &err_msg);
        }
        Err(err_msg)
    }
}

// =============================================================================
// BACKWARD COMPATIBILITY WRAPPERS
// =============================================================================

#[tauri::command]
pub(crate) async fn dev_text_editor_view(path: String, state: State<'_, AppState>) -> Result<String, String> {
    text_editor_view(path, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_text_editor_create(state: State<'_, AppState>, app: AppHandle, path: String, content: String) -> Result<(), String> {
    text_editor_create(state, app, path, content, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_text_editor_str_replace(state: State<'_, AppState>, app: AppHandle, path: String, find: String, replace: String) -> Result<(), String> {
    text_editor_str_replace(state, app, path, find, replace, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_text_editor_insert(state: State<'_, AppState>, app: AppHandle, path: String, line_number: usize, text: String) -> Result<(), String> {
    text_editor_insert(state, app, path, line_number, text, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_text_editor_undo_edit(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    text_editor_undo_edit(state, app, Some(true)).await
}

// Commands related to text editor operations (view, create, replace, insert, undo)

use crate::state::AppState;
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


#[tauri::command]
pub(crate) async fn dev_text_editor_view(path: String) -> Result<String, String> {
    info!(path = %Path::new(&path).display(), "[DEV_TOOL] Reading file content");
    match fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(e) => {
            let err_msg = format!("Failed to read file '{}': {}", path, e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_text_editor_create(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    content: String,
) -> Result<(), String> {
    let path_buf: PathBuf = path.into();
    info!(path = %path_buf.display(), "[DEV_TOOL] Creating/overwriting file");

    // Store previous state for undo
    let previous_content = fs::read_to_string(&path_buf).ok();
    // Use original String for state update, convert PathBuf back for notification
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), previous_content) {
        warn!("Failed to update undo state: {}", e);
    }

    match fs::write(&path_buf, content) {
        Ok(_) => {
            send_dev_tool_notification(&app, "File Operation", &format!("File '{}' created/updated.", path_buf.display()))?;
            Ok(())
        },
        Err(e) => {
            let err_msg = format!("Failed to write file '{}': {}", path_buf.display(), e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}


#[tauri::command]
pub(crate) async fn dev_text_editor_str_replace(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    find: String,
    replace: String,
) -> Result<(), String> {
    let path_buf: PathBuf = path.into();
    info!(path = %path_buf.display(), find, replace, "[DEV_TOOL] Replacing string in file");

    let original_content = match fs::read_to_string(&path_buf) {
        Ok(content) => content,
        Err(e) => {
            let err_msg = format!("Failed to read file for replace '{}': {}", path_buf.display(), e);
            error!("{}", err_msg);
            return Err(err_msg);
        }
    };

    // Store previous state for undo
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone())) {
        warn!("Failed to update undo state: {}", e);
    }

    let modified_content = original_content.replace(&find, &replace);

    match fs::write(&path_buf, modified_content) {
        Ok(_) => {
            send_dev_tool_notification(&app, "File Operation", &format!("String replaced in '{}'.", path_buf.display()))?;
            Ok(())
        },
        Err(e) => {
            let err_msg = format!("Failed to write replaced content to '{}': {}", path_buf.display(), e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_text_editor_insert(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    line_number: usize, // 1-based line number for insertion
    text: String,
) -> Result<(), String> {
    let path_buf: PathBuf = path.into();
    info!(path = %path_buf.display(), line_number, "[DEV_TOOL] Inserting text into file");

    let original_content = match fs::read_to_string(&path_buf) {
         Ok(content) => content,
         // If the file doesn't exist and we're inserting at line 1, treat it as creation
         Err(e) if e.kind() == io::ErrorKind::NotFound && line_number == 1 => String::new(),
         Err(e) => {
             let err_msg = format!("Failed to read file for insert '{}': {}", path_buf.display(), e);
             error!("{}", err_msg);
             return Err(err_msg);
         }
     };

    // Store previous state for undo
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone())) {
        warn!("Failed to update undo state: {}", e);
    }

    let mut lines: Vec<String> = original_content.lines().map(String::from).collect();

    // Adjust line number to be 0-based index
    let index = if line_number == 0 { 0 } else { line_number.saturating_sub(1) };

    if index > lines.len() {
        let err_msg = format!("Line number {} is out of bounds for file '{}' ({} lines)", line_number, path_buf.display(), lines.len());
        error!("{}", err_msg);
        return Err(err_msg);
    }

    // Insert the new text line by line
    for (i, line_to_insert) in text.lines().enumerate() {
       lines.insert(index + i, line_to_insert.to_string());
    }


    let modified_content = lines.join("\n");

    match fs::write(&path_buf, modified_content) {
        Ok(_) => {
            send_dev_tool_notification(&app, "File Operation", &format!("Text inserted into '{}' at line {}.", path_buf.display(), line_number))?;
            Ok(())
        },
        Err(e) => {
            let err_msg = format!("Failed to write inserted content to '{}': {}", path_buf.display(), e);
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
}


#[tauri::command]
pub(crate) async fn dev_text_editor_undo_edit(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    info!("[DEV_TOOL] Undoing last text editor operation");

    let mut last_file_lock = state.last_edited_file.lock()
        .map_err(|e| format!("Failed to acquire last_edited_file lock: {}", e))?;
    let mut prev_content_lock = state.previous_content.lock()
        .map_err(|e| format!("Failed to acquire previous_content lock: {}", e))?;

    if let Some(path) = last_file_lock.take() {
        let prev_content_option = prev_content_lock.take();

        if let Some(prev_content) = prev_content_option {
            // This was Some(Option<String>), so we attempt restore/delete based on inner Option
            if let Some(content_to_restore) = prev_content {
                // Had previous content (Some(Some(String))), so restore it
                info!(path = %path.display(), "[DEV_TOOL] Restoring previous content");
                match fs::write(&path, &content_to_restore) { // Write the inner String
                    Ok(_) => {
                         send_dev_tool_notification(&app, "File Operation", &format!("Undo: Restored '{}'.", path.display()))?;
                         // Locks are automatically released here as path and prev_content_option go out of scope
                         Ok(())
                    },
                    Err(e) => {
                        let err_msg = format!("Undo failed: Could not restore file '{}': {}", path.display(), e);
                        error!("{}", err_msg);
                        // Put the state back if write failed
                        *last_file_lock = Some(path);
                        *prev_content_lock = Some(Some(content_to_restore)); // Put the state back correctly
                        Err(err_msg)
                    }
                }
            } else {
                // No previous content (Some(None)), meaning the last operation was create, so delete the file
                info!(path = %path.display(), "[DEV_TOOL] Deleting file created by last operation");
                match fs::remove_file(&path) {
                    Ok(_) => {
                        send_dev_tool_notification(&app, "File Operation", &format!("Undo: Deleted '{}'.", path.display()))?;
                        Ok(())
                    },
                    // If the file doesn't exist, that's okay for undoing a create
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                         warn!(path = %path.display(), "Undo: File was already deleted.");
                         Ok(())
                     },
                    Err(e) => {
                        let err_msg = format!("Undo failed: Could not delete file '{}': {}", path.display(), e);
                        error!("{}", err_msg);
                        // Put the state back if delete failed
                        *last_file_lock = Some(path);
                        *prev_content_lock = Some(None); // Put the state back correctly (Some(None))
                        Err(err_msg)
                    }
                }
            }
        } else {
            // This case implies prev_content_lock was None initially, which shouldn't happen if last_file_lock was Some.
            let err_msg = format!("Undo failed: Inconsistent state for path '{}', expected previous content state.", path.display());
            error!("{}", err_msg);
            // Put the path back
            *last_file_lock = Some(path);
            // prev_content_lock remains None, which is consistent with the error state
             Err(err_msg)
        }
    } else {
        // last_file_lock was None initially
        let err_msg = "No text editor operation to undo.".to_string();
        warn!("{}", err_msg);
        Err(err_msg)
    }
}

// --- PRODUCTION TEXT EDITOR FUNCTIONS WITH DEBUG CAPABILITIES ---
// These functions replace the dev_ prefixed functions by incorporating debug features conditionally

#[tauri::command]
pub(crate) async fn text_editor_view(path: String) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, validators};

    // Note: For text editor operations, we can't easily access AppState for debug settings,
    // so we'll use a simplified debug approach based on cfg!(debug_assertions)
    let debug_enabled = cfg!(debug_assertions);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&path)?;
        validators::valid_file_path(&path)?;
    }

    log_debug_operation("text_editor_view", &format!("Reading file content: {}", path), &debug_config);
    info!(path = %Path::new(&path).display(), "Executing text_editor_view");

    match fs::read_to_string(&path) {
        Ok(content) => {
            info!("Successfully read file content (length: {})", content.len());
            Ok(content)
        }
        Err(e) => {
            let error_msg = format!("Failed to read file '{}': {}", path, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_create(
    path: String,
    content: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&path)?;
        validators::valid_file_path(&path)?;
    }

    let path_buf: PathBuf = path.into();
    log_debug_operation("text_editor_create", &format!("Creating/overwriting file: {}", path_buf.display()), &debug_config);
    info!(path = %path_buf.display(), "Executing text_editor_create");

    // Store previous state for undo
    let previous_content = fs::read_to_string(&path_buf).ok();
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), previous_content) {
        warn!("Failed to update undo state: {}", e);
    }

    match fs::write(&path_buf, content) {
        Ok(_) => {
            info!("Successfully created/updated file: {}", path_buf.display());

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "File Operation", &format!("File '{}' created/updated", path_buf.display()));
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to write file '{}': {}", path_buf.display(), e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_str_replace(
    path: String,
    find: String,
    replace: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&path)?;
        validators::valid_file_path(&path)?;
        validators::non_empty_text(&find)?;
    }

    let path_buf: PathBuf = path.into();
    log_debug_operation("text_editor_str_replace", &format!("Replacing string in file: {} (find: '{}', replace: '{}')", path_buf.display(), find, replace), &debug_config);
    info!(path = %path_buf.display(), find, replace, "Executing text_editor_str_replace");

    let original_content = match fs::read_to_string(&path_buf) {
        Ok(content) => content,
        Err(e) => {
            let error_msg = format!("Failed to read file for replace '{}': {}", path_buf.display(), e);
            error!("{}", error_msg);
            return Err(error_msg);
        }
    };

    // Store previous state for undo
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone())) {
        warn!("Failed to update undo state: {}", e);
    }

    let modified_content = original_content.replace(&find, &replace);

    match fs::write(&path_buf, modified_content) {
        Ok(_) => {
            info!("Successfully replaced string in file: {}", path_buf.display());

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "File Operation", &format!("String replaced in '{}'", path_buf.display()));
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to write replaced content to '{}': {}", path_buf.display(), e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_insert(
    path: String,
    line_number: usize,
    text: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&path)?;
        validators::valid_file_path(&path)?;
        if line_number == 0 {
            return Err("Line number must be greater than 0".to_string());
        }
    }

    let path_buf: PathBuf = path.into();
    log_debug_operation("text_editor_insert", &format!("Inserting text into file: {} at line {}", path_buf.display(), line_number), &debug_config);
    info!(path = %path_buf.display(), line_number, "Executing text_editor_insert");

    let original_content = match fs::read_to_string(&path_buf) {
        Ok(content) => content,
        // If the file doesn't exist and we're inserting at line 1, treat it as creation
        Err(e) if e.kind() == io::ErrorKind::NotFound && line_number == 1 => String::new(),
        Err(e) => {
            let error_msg = format!("Failed to read file for insert '{}': {}", path_buf.display(), e);
            error!("{}", error_msg);
            return Err(error_msg);
        }
    };

    // Store previous state for undo
    if let Err(e) = update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone())) {
        warn!("Failed to update undo state: {}", e);
    }

    let mut lines: Vec<String> = original_content.lines().map(String::from).collect();

    // Adjust line number to be 0-based index
    let index = if line_number == 0 { 0 } else { line_number.saturating_sub(1) };

    if index > lines.len() {
        let error_msg = format!("Line number {} is out of bounds for file '{}' ({} lines)", line_number, path_buf.display(), lines.len());
        error!("{}", error_msg);
        return Err(error_msg);
    }

    // Insert the new text line by line
    for (i, line_to_insert) in text.lines().enumerate() {
        lines.insert(index + i, line_to_insert.to_string());
    }

    let modified_content = lines.join("\n");

    match fs::write(&path_buf, modified_content) {
        Ok(_) => {
            info!("Successfully inserted text into file: {} at line {}", path_buf.display(), line_number);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "File Operation", &format!("Text inserted into '{}' at line {}", path_buf.display(), line_number));
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to write inserted content to '{}': {}", path_buf.display(), e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn text_editor_undo_edit(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("text_editor_undo_edit", "Undoing last text editor operation", &debug_config);
    info!("Executing text_editor_undo_edit");

    let mut last_file_lock = state.last_edited_file.lock()
        .map_err(|e| format!("Failed to acquire last_edited_file lock: {}", e))?;
    let mut prev_content_lock = state.previous_content.lock()
        .map_err(|e| format!("Failed to acquire previous_content lock: {}", e))?;

    if let Some(path) = last_file_lock.take() {
        let prev_content_option = prev_content_lock.take();

        if let Some(prev_content) = prev_content_option {
            if let Some(content_to_restore) = prev_content {
                // Had previous content, so restore it
                info!(path = %path.display(), "Restoring previous content");
                match fs::write(&path, &content_to_restore) {
                    Ok(_) => {
                        info!("Successfully restored file: {}", path.display());

                        // Send debug notification if enabled
                        if debug_config.send_notifications {
                            let _ = send_debug_notification(&app, "File Operation", &format!("Undo: Restored '{}'", path.display()));
                        }

                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!("Undo failed: Could not restore file '{}': {}", path.display(), e);
                        error!("{}", error_msg);
                        // Put the state back if write failed
                        *last_file_lock = Some(path);
                        *prev_content_lock = Some(Some(content_to_restore));
                        Err(error_msg)
                    }
                }
            } else {
                // No previous content, meaning the last operation was create, so delete the file
                info!(path = %path.display(), "Deleting file created by last operation");
                match fs::remove_file(&path) {
                    Ok(_) => {
                        info!("Successfully deleted file: {}", path.display());

                        // Send debug notification if enabled
                        if debug_config.send_notifications {
                            let _ = send_debug_notification(&app, "File Operation", &format!("Undo: Deleted '{}'", path.display()));
                        }

                        Ok(())
                    }
                    // If the file doesn't exist, that's okay for undoing a create
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        warn!(path = %path.display(), "Undo: File was already deleted");
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!("Undo failed: Could not delete file '{}': {}", path.display(), e);
                        error!("{}", error_msg);
                        // Put the state back if delete failed
                        *last_file_lock = Some(path);
                        *prev_content_lock = Some(None);
                        Err(error_msg)
                    }
                }
            }
        } else {
            let error_msg = format!("Undo failed: Inconsistent state for path '{}', expected previous content state", path.display());
            error!("{}", error_msg);
            // Put the path back
            *last_file_lock = Some(path);
            Err(error_msg)
        }
    } else {
        let error_msg = "No text editor operation to undo".to_string();
        warn!("{}", error_msg);
        Err(error_msg)
    }
}

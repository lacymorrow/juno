// Commands related to text editor operations (view, create, replace, insert, undo)

use crate::state::AppState;
use tauri::{AppHandle, State};
use std::fs;
use std::io;
use std::path::{PathBuf, Path};
use tracing::{info, warn, error};
use super::send_dev_tool_notification; // Use helper from parent module

// Helper moved here as it's only used by text editor commands
fn update_undo_state(state: &State<AppState>, path: String, previous_content: Option<String>) {
    let mut last_edited = state.last_edited_file.lock().unwrap();
    *last_edited = Some(path.into()); // Convert String to PathBuf
    let mut prev_content = state.previous_content.lock().unwrap();
    *prev_content = Some(previous_content); // Wrap Option<String> in Option
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
    update_undo_state(&state, path_buf.to_string_lossy().to_string(), previous_content);

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
    update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone()));

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
    update_undo_state(&state, path_buf.to_string_lossy().to_string(), Some(original_content.clone()));

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

    let mut last_file_lock = state.last_edited_file.lock().unwrap();
    let mut prev_content_lock = state.previous_content.lock().unwrap();

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

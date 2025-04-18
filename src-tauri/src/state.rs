use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use computer_use_ai_sdk::Desktop;


// Application state structure
#[allow(dead_code)] // Allow dead code for potentially unused fields
pub(crate) struct AppState {
    pub(crate) desktop: Arc<Desktop>,
    // State for text_editor_undo_edit
    pub(crate) last_edited_file: Mutex<Option<PathBuf>>,
    pub(crate) previous_content: Mutex<Option<Option<String>>>, // Option<Option<String>>: None=no undo, Some(None)=last was create, Some(Some(content))=last was edit
}

// Helper function to update undo state
#[allow(dead_code)] // Keep allowing dead code as it might be conditionally used
pub(crate) fn update_undo_state(state: &AppState, file_path: PathBuf, previous_content: Option<String>) {
    *state.last_edited_file.lock().unwrap() = Some(file_path);
    *state.previous_content.lock().unwrap() = Some(previous_content);
}

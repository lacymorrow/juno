use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::agent::implementations::persistent_memory::{
    MemoryCategory, MemoryEntry, PersistentMemoryManager,
};

/// Request body for adding a new persistent memory entry
#[derive(Debug, Serialize, Deserialize)]
pub struct AddMemoryRequest {
    pub category: String,
    pub content: String,
    #[serde(default = "default_relevance")]
    pub relevance_score: f64,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_relevance() -> f64 {
    0.7
}

/// Get all persistent memory entries sorted by decayed relevance score
#[tauri::command]
pub async fn get_persistent_memory_entries(
    app_handle: AppHandle,
) -> Result<Vec<MemoryEntry>, String> {
    let manager = PersistentMemoryManager::new(app_handle);
    manager.load_entries()
}

/// Add a new persistent memory entry
#[tauri::command]
pub async fn add_persistent_memory_entry(
    app_handle: AppHandle,
    request: AddMemoryRequest,
) -> Result<MemoryEntry, String> {
    let manager = PersistentMemoryManager::new(app_handle);
    let category = MemoryCategory::from_str(&request.category);
    manager.add_entry(
        category,
        request.content,
        request.relevance_score,
        request.tags,
    )
}

/// Update content or relevance score of an existing entry
#[tauri::command]
pub async fn update_persistent_memory_entry(
    app_handle: AppHandle,
    id: String,
    content: Option<String>,
    relevance_score: Option<f64>,
) -> Result<MemoryEntry, String> {
    let manager = PersistentMemoryManager::new(app_handle);
    manager.update_entry(&id, content, relevance_score)
}

/// Delete a single persistent memory entry by ID
#[tauri::command]
pub async fn delete_persistent_memory_entry(
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let manager = PersistentMemoryManager::new(app_handle);
    manager.delete_entry(&id)
}

/// Delete all persistent memory entries
#[tauri::command]
pub async fn clear_all_persistent_memory(app_handle: AppHandle) -> Result<(), String> {
    let manager = PersistentMemoryManager::new(app_handle);
    manager.clear_all()
}

/// Remove stale/low-relevance entries; returns count of removed entries
#[tauri::command]
pub async fn prune_persistent_memory(app_handle: AppHandle) -> Result<usize, String> {
    let manager = PersistentMemoryManager::new(app_handle);
    manager.prune()
}

/// Get the formatted memory block as it will appear in the agent system prompt
#[tauri::command]
pub async fn get_persistent_memory_prompt_preview(
    app_handle: AppHandle,
) -> Result<String, String> {
    let manager = PersistentMemoryManager::new(app_handle);
    Ok(manager.format_for_prompt())
}

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::persistent_memory::{MemoryCategory, PersistentMemoryEntry, PersistentMemoryStore};

/// Request body for adding a new memory entry
#[derive(Debug, Deserialize)]
pub struct AddMemoryRequest {
    pub category: String,
    pub content: String,
    pub relevance_score: Option<f32>,
}

/// Response shape returned to the frontend
#[derive(Debug, Serialize)]
pub struct MemoryEntryDto {
    pub id: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub category: String,
    pub content: String,
    pub relevance_score: f32,
    pub access_count: u32,
}

impl From<PersistentMemoryEntry> for MemoryEntryDto {
    fn from(e: PersistentMemoryEntry) -> Self {
        Self {
            id: e.id,
            created_at: e.created_at,
            updated_at: e.updated_at,
            category: e.category.as_str().to_string(),
            content: e.content,
            relevance_score: e.relevance_score,
            access_count: e.access_count,
        }
    }
}

fn parse_category(s: &str) -> Result<MemoryCategory, String> {
    match s {
        "preference" => Ok(MemoryCategory::Preference),
        "correction" => Ok(MemoryCategory::Correction),
        "fact" => Ok(MemoryCategory::Fact),
        "shortcut" => Ok(MemoryCategory::Shortcut),
        "context" => Ok(MemoryCategory::Context),
        other => Err(format!(
            "Unknown memory category '{}'. Valid values: preference, correction, fact, shortcut, context",
            other
        )),
    }
}

/// List all persistent memory entries
#[tauri::command]
pub async fn get_persistent_memory(app_handle: AppHandle) -> Result<Vec<MemoryEntryDto>, String> {
    let store = PersistentMemoryStore::new(app_handle);
    let entries = store.load_entries()?;
    Ok(entries.into_iter().map(MemoryEntryDto::from).collect())
}

/// Add a new persistent memory entry
#[tauri::command]
pub async fn add_persistent_memory(
    app_handle: AppHandle,
    category: String,
    content: String,
    relevance_score: Option<f32>,
) -> Result<MemoryEntryDto, String> {
    if content.trim().is_empty() {
        return Err("Memory content cannot be empty".to_string());
    }
    let cat = parse_category(&category)?;
    let score = relevance_score.unwrap_or(0.7).clamp(0.0, 1.0);
    let store = PersistentMemoryStore::new(app_handle);
    let entry = store.add_entry(cat, content.trim().to_string(), score)?;
    Ok(MemoryEntryDto::from(entry))
}

/// Update an existing memory entry's content and/or relevance score
#[tauri::command]
pub async fn update_persistent_memory(
    app_handle: AppHandle,
    id: String,
    content: Option<String>,
    relevance_score: Option<f32>,
) -> Result<MemoryEntryDto, String> {
    if let Some(ref c) = content {
        if c.trim().is_empty() {
            return Err("Memory content cannot be empty".to_string());
        }
    }
    let store = PersistentMemoryStore::new(app_handle);
    let entry = store.update_entry(&id, content.map(|c| c.trim().to_string()), relevance_score)?;
    Ok(MemoryEntryDto::from(entry))
}

/// Delete a specific memory entry by ID
#[tauri::command]
pub async fn delete_persistent_memory(app_handle: AppHandle, id: String) -> Result<(), String> {
    let store = PersistentMemoryStore::new(app_handle);
    store.delete_entry(&id)
}

/// Clear all persistent memory entries
#[tauri::command]
pub async fn clear_persistent_memory(app_handle: AppHandle) -> Result<(), String> {
    let store = PersistentMemoryStore::new(app_handle);
    store.clear_all()
}

/// Preview what memory block will be injected into the next system prompt
#[tauri::command]
pub async fn preview_memory_injection(app_handle: AppHandle) -> Result<Option<String>, String> {
    let store = PersistentMemoryStore::new(app_handle);
    match store.build_injection_block()? {
        Some((block, _)) => Ok(Some(block)),
        None => Ok(None),
    }
}

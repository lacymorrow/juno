use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

const MEMORY_STORE_FILE: &str = "memory.json";
const MEMORY_STORE_KEY: &str = "persistent_memory_entries";
const MAX_ENTRIES: usize = 100;
const MAX_INJECTION_ENTRIES: usize = 20;

/// Category of a persistent memory entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    /// User preferences (e.g. "User prefers concise responses")
    Preference,
    /// Corrections the user gave the agent (e.g. "Use 'bun' not 'npm' in this project")
    Correction,
    /// Gathered facts about the user's environment or projects
    Fact,
    /// Frequently used shortcuts or commands
    Shortcut,
    /// Ongoing project context
    Context,
}

impl MemoryCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "preference",
            MemoryCategory::Correction => "correction",
            MemoryCategory::Fact => "fact",
            MemoryCategory::Shortcut => "shortcut",
            MemoryCategory::Context => "context",
        }
    }
}

/// A single persistent memory entry that survives across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentMemoryEntry {
    pub id: String,
    /// Unix timestamp (seconds) when this entry was created
    pub created_at: u64,
    /// Unix timestamp (seconds) of last access/update
    pub updated_at: u64,
    pub category: MemoryCategory,
    pub content: String,
    /// 0.0–1.0; higher = injected into prompt more often
    pub relevance_score: f32,
    /// How many agent sessions have used this entry
    pub access_count: u32,
}

impl PersistentMemoryEntry {
    pub fn new(category: MemoryCategory, content: String, relevance_score: f32) -> Self {
        let now = now_secs();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            category,
            content,
            relevance_score: relevance_score.clamp(0.0, 1.0),
            access_count: 0,
        }
    }

    /// Combined priority score for injection ordering
    fn priority(&self) -> f64 {
        let recency_weight = 0.3;
        let now = now_secs() as f64;
        let age_days = (now - self.created_at as f64) / 86400.0;
        let recency = 1.0 / (1.0 + age_days * 0.1); // decays slowly over weeks

        let access_weight = 0.2;
        let access_score = (self.access_count as f64 / 10.0).min(1.0);

        let relevance_weight = 0.5;

        (self.relevance_score as f64 * relevance_weight)
            + (recency * recency_weight)
            + (access_score * access_weight)
    }
}

/// Persistent cross-session memory store backed by Tauri Store
pub struct PersistentMemoryStore {
    app_handle: AppHandle,
}

impl PersistentMemoryStore {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Load all entries from disk
    pub fn load_entries(&self) -> Result<Vec<PersistentMemoryEntry>, String> {
        let store = self
            .app_handle
            .store(MEMORY_STORE_FILE)
            .map_err(|e| format!("Failed to open memory store: {}", e))?;

        match store.get(MEMORY_STORE_KEY) {
            Some(value) => serde_json::from_value(value)
                .map_err(|e| format!("Failed to parse memory entries: {}", e)),
            None => Ok(Vec::new()),
        }
    }

    /// Persist all entries to disk
    fn save_entries(&self, entries: &[PersistentMemoryEntry]) -> Result<(), String> {
        let store = self
            .app_handle
            .store(MEMORY_STORE_FILE)
            .map_err(|e| format!("Failed to open memory store: {}", e))?;

        let value = serde_json::to_value(entries)
            .map_err(|e| format!("Failed to serialize memory entries: {}", e))?;

        store.set(MEMORY_STORE_KEY, value);
        store
            .save()
            .map_err(|e| format!("Failed to save memory store: {}", e))?;

        Ok(())
    }

    /// Add a new entry, pruning oldest low-relevance entries when over limit
    pub fn add_entry(
        &self,
        category: MemoryCategory,
        content: String,
        relevance_score: f32,
    ) -> Result<PersistentMemoryEntry, String> {
        let mut entries = self.load_entries()?;
        let entry = PersistentMemoryEntry::new(category, content, relevance_score);
        entries.push(entry.clone());

        if entries.len() > MAX_ENTRIES {
            // Sort ascending by priority so we remove the least valuable entries
            entries.sort_by(|a, b| a.priority().partial_cmp(&b.priority()).unwrap_or(std::cmp::Ordering::Equal));
            entries.truncate(MAX_ENTRIES);
        }

        self.save_entries(&entries)?;
        log::info!("Persistent memory: added entry '{}' ({})", entry.id, entry.category.as_str());
        Ok(entry)
    }

    /// Update an existing entry's content, relevance, and timestamp
    pub fn update_entry(
        &self,
        id: &str,
        content: Option<String>,
        relevance_score: Option<f32>,
    ) -> Result<PersistentMemoryEntry, String> {
        let mut entries = self.load_entries()?;
        let entry = entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("Memory entry not found: {}", id))?;

        if let Some(c) = content {
            entry.content = c;
        }
        if let Some(r) = relevance_score {
            entry.relevance_score = r.clamp(0.0, 1.0);
        }
        entry.updated_at = now_secs();

        let result = entry.clone();
        self.save_entries(&entries)?;
        Ok(result)
    }

    /// Delete an entry by ID
    pub fn delete_entry(&self, id: &str) -> Result<(), String> {
        let mut entries = self.load_entries()?;
        let before = entries.len();
        entries.retain(|e| e.id != id);
        if entries.len() == before {
            return Err(format!("Memory entry not found: {}", id));
        }
        self.save_entries(&entries)?;
        log::info!("Persistent memory: deleted entry '{}'", id);
        Ok(())
    }

    /// Clear all entries
    pub fn clear_all(&self) -> Result<(), String> {
        self.save_entries(&[])?;
        log::info!("Persistent memory: all entries cleared");
        Ok(())
    }

    /// Increment access count for a set of entry IDs (called after injection)
    pub fn record_access(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut entries = self.load_entries()?;
        let now = now_secs();
        for entry in entries.iter_mut() {
            if ids.contains(&entry.id) {
                entry.access_count += 1;
                entry.updated_at = now;
            }
        }
        self.save_entries(&entries)
    }

    /// Build the memory block to inject into the system prompt.
    /// Returns the formatted string and the IDs of injected entries.
    pub fn build_injection_block(&self) -> Result<Option<(String, Vec<String>)>, String> {
        let mut entries = self.load_entries()?;
        if entries.is_empty() {
            return Ok(None);
        }

        // Sort descending by priority
        entries.sort_by(|a, b| b.priority().partial_cmp(&a.priority()).unwrap_or(std::cmp::Ordering::Equal));
        entries.truncate(MAX_INJECTION_ENTRIES);

        let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();

        let mut block = String::from("\n\n## Persistent User Memory\n\nThe following facts about the user and their environment have been learned across previous sessions. Respect these as established context:\n\n");

        for entry in &entries {
            block.push_str(&format!(
                "- [{}] {}\n",
                entry.category.as_str(),
                entry.content
            ));
        }

        Ok(Some((block, ids)))
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

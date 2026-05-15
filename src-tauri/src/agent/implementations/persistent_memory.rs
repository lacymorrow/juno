use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

use crate::agent::core::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;

const MEMORY_STORE_FILE: &str = "memory.json";
const ENTRIES_KEY: &str = "entries";
const MAX_ENTRIES: usize = 100;
const PRUNE_THRESHOLD: f64 = 0.05;
const DECAY_HALF_LIFE_DAYS: f64 = 30.0;
/// Entries injected into the system prompt each session
const PROMPT_INJECTION_LIMIT: usize = 30;

/// Category for a persistent memory entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Preference,
    Correction,
    Fact,
    Context,
}

impl MemoryCategory {
    pub fn as_label(&self) -> &'static str {
        match self {
            MemoryCategory::Preference => "Preference",
            MemoryCategory::Correction => "Correction",
            MemoryCategory::Fact => "Fact",
            MemoryCategory::Context => "Context",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "preference" => MemoryCategory::Preference,
            "correction" => MemoryCategory::Correction,
            "fact" => MemoryCategory::Fact,
            _ => MemoryCategory::Context,
        }
    }
}

/// A single persistent memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    /// Unix timestamp (seconds) when entry was created
    pub timestamp: u64,
    pub category: MemoryCategory,
    pub content: String,
    /// Importance weight 0.0..1.0 (higher = more important)
    pub relevance_score: f64,
    pub access_count: u32,
    pub last_accessed: u64,
    pub tags: Vec<String>,
}

impl MemoryEntry {
    pub fn new(
        category: MemoryCategory,
        content: String,
        relevance_score: f64,
        tags: Vec<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: now,
            category,
            content,
            relevance_score: relevance_score.clamp(0.0, 1.0),
            access_count: 0,
            last_accessed: now,
            tags,
        }
    }

    /// Time-decayed score used for ranking and pruning.
    /// Uses exponential decay with a 30-day half-life, boosted by access frequency.
    pub fn decayed_score(&self) -> f64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let age_secs = now.saturating_sub(self.last_accessed);
        let age_days = age_secs as f64 / 86400.0;
        let decay = (-age_days / DECAY_HALF_LIFE_DAYS * std::f64::consts::LN_2).exp();
        // Small logarithmic boost for frequently-accessed entries
        let access_boost = 1.0 + (self.access_count as f64).ln().max(0.0) * 0.1;
        self.relevance_score * decay * access_boost
    }
}

/// Lightweight handle for reading/writing persistent memory to `memory.json` via Tauri Store.
/// Created on-demand from an `AppHandle` — no long-lived state needed.
pub struct PersistentMemoryManager {
    app_handle: tauri::AppHandle,
}

impl PersistentMemoryManager {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn load_entries(&self) -> Result<Vec<MemoryEntry>, String> {
        let store = self
            .app_handle
            .store(MEMORY_STORE_FILE)
            .map_err(|e| format!("Failed to open memory store: {}", e))?;
        match store.get(ENTRIES_KEY) {
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to parse memory entries: {}", e)),
            None => Ok(Vec::new()),
        }
    }

    fn save_entries(&self, entries: &[MemoryEntry]) -> Result<(), String> {
        let store = self
            .app_handle
            .store(MEMORY_STORE_FILE)
            .map_err(|e| format!("Failed to open memory store: {}", e))?;
        let value = serde_json::to_value(entries)
            .map_err(|e| format!("Failed to serialize memory entries: {}", e))?;
        store.set(ENTRIES_KEY, value);
        store
            .save()
            .map_err(|e| format!("Failed to save memory store: {}", e))
    }

    /// Add a new entry, pruning when capacity is reached.
    pub fn add_entry(
        &self,
        category: MemoryCategory,
        content: String,
        relevance_score: f64,
        tags: Vec<String>,
    ) -> Result<MemoryEntry, String> {
        let mut entries = self.load_entries()?;
        let entry = MemoryEntry::new(category, content, relevance_score, tags);
        entries.push(entry.clone());
        if entries.len() > MAX_ENTRIES {
            entries = Self::prune_entries(entries);
        }
        self.save_entries(&entries)?;
        Ok(entry)
    }

    /// Update content or relevance of an existing entry by ID.
    pub fn update_entry(
        &self,
        id: &str,
        content: Option<String>,
        relevance_score: Option<f64>,
    ) -> Result<MemoryEntry, String> {
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        entry.last_accessed = now;
        entry.access_count += 1;
        let result = entry.clone();
        self.save_entries(&entries)?;
        Ok(result)
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), String> {
        let mut entries = self.load_entries()?;
        let before = entries.len();
        entries.retain(|e| e.id != id);
        if entries.len() == before {
            return Err(format!("Memory entry not found: {}", id));
        }
        self.save_entries(&entries)
    }

    pub fn clear_all(&self) -> Result<(), String> {
        self.save_entries(&[])
    }

    /// Return top-N entries by decayed relevance score, updating access counts.
    pub fn get_relevant_entries(&self, limit: usize) -> Result<Vec<MemoryEntry>, String> {
        let mut entries = self.load_entries()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for entry in &mut entries {
            entry.access_count += 1;
            entry.last_accessed = now;
        }
        entries.sort_by(|a, b| {
            b.decayed_score()
                .partial_cmp(&a.decayed_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Persist updated access counts (non-critical, ignore errors)
        let _ = self.save_entries(&entries);
        entries.truncate(limit);
        Ok(entries)
    }

    /// Remove stale/low-relevance entries. Returns count removed.
    pub fn prune(&self) -> Result<usize, String> {
        let entries = self.load_entries()?;
        let before = entries.len();
        let pruned = Self::prune_entries(entries);
        let after = pruned.len();
        self.save_entries(&pruned)?;
        Ok(before - after)
    }

    fn prune_entries(mut entries: Vec<MemoryEntry>) -> Vec<MemoryEntry> {
        entries.retain(|e| e.decayed_score() >= PRUNE_THRESHOLD);
        if entries.len() > MAX_ENTRIES {
            entries.sort_by(|a, b| {
                b.decayed_score()
                    .partial_cmp(&a.decayed_score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            entries.truncate(MAX_ENTRIES);
        }
        entries
    }

    /// Format the top relevant entries as a system-prompt section.
    /// Returns an empty string when no entries exist.
    pub fn format_for_prompt(&self) -> String {
        let entries = match self.get_relevant_entries(PROMPT_INJECTION_LIMIT) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Failed to load persistent memory for prompt: {}", e);
                return String::new();
            }
        };
        if entries.is_empty() {
            return String::new();
        }
        Self::format_entries_for_prompt(&entries)
    }

    /// Pure function for formatting — useful in tests.
    pub fn format_entries_for_prompt(entries: &[MemoryEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }
        let mut lines = vec![
            "## Persistent Memory".to_string(),
            "You have stored knowledge about this user across previous sessions — use it to personalise your responses and avoid repeating mistakes:".to_string(),
            String::new(),
        ];
        let category_order = [
            MemoryCategory::Correction,
            MemoryCategory::Preference,
            MemoryCategory::Fact,
            MemoryCategory::Context,
        ];
        for category in &category_order {
            let cat_entries: Vec<&MemoryEntry> =
                entries.iter().filter(|e| &e.category == category).collect();
            if !cat_entries.is_empty() {
                lines.push(format!("**{}s:**", category.as_label()));
                for entry in cat_entries {
                    lines.push(format!("- {}", entry.content));
                }
                lines.push(String::new());
            }
        }
        lines.push(
            "When you discover new preferences, corrections, or important facts about this user, \
             call the `remember_fact` tool to persist them for future sessions."
                .to_string(),
        );
        lines.join("\n")
    }
}

/// Register the `remember_fact` tool on a tool provider.
/// The agent calls this tool to persist preferences, corrections, and facts across sessions.
pub async fn register_remember_fact_tool(
    tool_provider: &LocalToolProvider,
    app_handle: tauri::AppHandle,
) {
    let definition = ToolDefinition {
        name: "remember_fact".to_string(),
        description: "Save an important fact, user preference, or correction for future sessions. \
            Call this whenever you learn something that will help you work better with this user — \
            for example when they correct a mistake, express a preference, or share context that \
            should persist beyond this conversation."
            .to_string(),
        api_type: None,
        beta_flag: None,
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The fact, preference, or correction to remember (be specific and concise)"
                },
                "category": {
                    "type": "string",
                    "enum": ["preference", "correction", "fact", "context"],
                    "description": "Category: 'preference' (user likes/dislikes), 'correction' (user corrected a mistake you made), 'fact' (objective info about the user), 'context' (situational context)"
                },
                "relevance_score": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Importance 0.0-1.0. Use 0.9+ for corrections, 0.7-0.9 for strong preferences, 0.5-0.7 for useful facts."
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional searchable tags"
                }
            },
            "required": ["content", "category"]
        }),
    };

    let executor = move |input: serde_json::Value| {
        let app_handle_cap = app_handle.clone();
        async move {
            let category_str = input["category"].as_str().unwrap_or("context");
            let content = input["content"]
                .as_str()
                .ok_or_else(|| "Missing required parameter: content".to_string())?
                .to_string();
            let relevance_score = input["relevance_score"].as_f64().unwrap_or(0.7);
            let tags: Vec<String> = input["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            let manager = PersistentMemoryManager::new(app_handle_cap);
            let entry = manager.add_entry(
                MemoryCategory::from_str(category_str),
                content,
                relevance_score,
                tags,
            )?;

            log::info!(
                "Agent saved persistent memory entry [{}]: {}",
                entry.category.as_label(),
                entry.id
            );

            Ok(serde_json::json!({
                "success": true,
                "id": entry.id,
                "message": format!("Remembered: {}", entry.content)
            }))
        }
    };

    tool_provider.register_async_tool(definition, executor).await;
    log::info!("Registered remember_fact tool for persistent cross-session memory");
}

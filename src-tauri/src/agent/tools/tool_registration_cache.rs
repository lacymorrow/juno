use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, debug, warn};
use once_cell::sync::Lazy;

/// Tool registration cache to prevent redundant registrations
pub struct ToolRegistrationCache {
    /// Track which tool categories have been registered
    registered_categories: Arc<RwLock<HashMap<String, bool>>>,
    /// Track if basic tools are registered
    basic_tools_registered: Arc<AtomicBool>,
    /// Track if desktop tools are registered
    desktop_tools_registered: Arc<AtomicBool>,
    /// Track if browser tools are registered
    browser_tools_registered: Arc<AtomicBool>,
    /// Track if MCP tools are registered
    mcp_tools_registered: Arc<AtomicBool>,
}

impl ToolRegistrationCache {
    pub fn new() -> Self {
        Self {
            registered_categories: Arc::new(RwLock::new(HashMap::new())),
            basic_tools_registered: Arc::new(AtomicBool::new(false)),
            desktop_tools_registered: Arc::new(AtomicBool::new(false)),
            browser_tools_registered: Arc::new(AtomicBool::new(false)),
            mcp_tools_registered: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if basic tools are already registered
    pub fn are_basic_tools_registered(&self) -> bool {
        self.basic_tools_registered.load(Ordering::SeqCst)
    }

    /// Mark basic tools as registered
    pub fn mark_basic_tools_registered(&self) {
        self.basic_tools_registered.store(true, Ordering::SeqCst);
        debug!("✅ Basic tools marked as registered in cache");
    }

    /// Check if desktop tools are already registered
    pub fn are_desktop_tools_registered(&self) -> bool {
        self.desktop_tools_registered.load(Ordering::SeqCst)
    }

    /// Mark desktop tools as registered
    pub fn mark_desktop_tools_registered(&self) {
        self.desktop_tools_registered.store(true, Ordering::SeqCst);
        debug!("✅ Desktop tools marked as registered in cache");
    }

    /// Check if browser tools are already registered
    pub fn are_browser_tools_registered(&self) -> bool {
        self.browser_tools_registered.load(Ordering::SeqCst)
    }

    /// Mark browser tools as registered
    pub fn mark_browser_tools_registered(&self) {
        self.browser_tools_registered.store(true, Ordering::SeqCst);
        debug!("✅ Browser tools marked as registered in cache");
    }

    /// Check if MCP tools are already registered
    pub fn are_mcp_tools_registered(&self) -> bool {
        self.mcp_tools_registered.load(Ordering::SeqCst)
    }

    /// Mark MCP tools as registered
    pub fn mark_mcp_tools_registered(&self) {
        self.mcp_tools_registered.store(true, Ordering::SeqCst);
        debug!("✅ MCP tools marked as registered in cache");
    }

    /// Check if a specific category is registered
    pub async fn is_category_registered(&self, category: &str) -> bool {
        let categories = self.registered_categories.read().await;
        categories.get(category).copied().unwrap_or(false)
    }

    /// Mark a category as registered
    pub async fn mark_category_registered(&self, category: &str) {
        let mut categories = self.registered_categories.write().await;
        categories.insert(category.to_string(), true);
        debug!("✅ Category '{}' marked as registered in cache", category);
    }

    /// Reset the cache (useful for testing or configuration changes)
    pub async fn reset_cache(&self) {
        self.basic_tools_registered.store(false, Ordering::SeqCst);
        self.desktop_tools_registered.store(false, Ordering::SeqCst);
        self.browser_tools_registered.store(false, Ordering::SeqCst);
        self.mcp_tools_registered.store(false, Ordering::SeqCst);

        let mut categories = self.registered_categories.write().await;
        categories.clear();

        info!("🔄 Tool registration cache reset");
    }

    /// Get cache status for debugging
    pub async fn get_cache_status(&self) -> HashMap<String, bool> {
        let mut status = HashMap::new();
        status.insert("basic_tools".to_string(), self.are_basic_tools_registered());
        status.insert("desktop_tools".to_string(), self.are_desktop_tools_registered());
        status.insert("browser_tools".to_string(), self.are_browser_tools_registered());
        status.insert("mcp_tools".to_string(), self.are_mcp_tools_registered());

        let categories = self.registered_categories.read().await;
        for (category, registered) in categories.iter() {
            status.insert(format!("category_{}", category), *registered);
        }

        status
    }
}

// Global cache instance
static TOOL_REGISTRATION_CACHE: Lazy<ToolRegistrationCache> =
    Lazy::new(|| ToolRegistrationCache::new());

/// Get the global tool registration cache
pub fn get_tool_registration_cache() -> &'static ToolRegistrationCache {
    &TOOL_REGISTRATION_CACHE
}

/// Tauri command to get cache status for debugging
#[tauri::command]
pub async fn get_tool_registration_cache_status() -> Result<HashMap<String, bool>, String> {
    Ok(get_tool_registration_cache().get_cache_status().await)
}

/// Tauri command to reset cache for testing
#[tauri::command]
pub async fn reset_tool_registration_cache() -> Result<String, String> {
    get_tool_registration_cache().reset_cache().await;
    Ok("Tool registration cache reset successfully".to_string())
}

//! Tool Coordinator
//! 
//! Coordinates tool execution in the event-driven architecture.
//! Receives ToolCall events and orchestrates their execution through ToolExecutor.
//! Enhanced for Phase 1.7 to work seamlessly with EventDrivenToolExecutor.
//! 
//! TARS Integration Phase 1.7: Tool System Refactor

use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use tracing::{error, info, debug, warn};

use crate::agent::events::{EventHandler, JunoAgentEvent, now};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::traits::ToolProvider;

/// Coordinates tool execution in the event-driven system
pub struct ToolCoordinator {
    /// Registry of available tool providers by domain
    tool_providers: HashMap<String, Arc<LocalToolProvider>>,
    /// App handle for accessing shared resources
    app_handle: tauri::AppHandle,
}

impl ToolCoordinator {
    /// Create a new tool coordinator
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            tool_providers: HashMap::new(),
            app_handle,
        }
    }
    
    /// Register a tool provider for a specific domain
    pub fn register_provider(&mut self, domain: String, provider: Arc<LocalToolProvider>) {
        info!("Registering tool provider for domain: {}", domain);
        self.tool_providers.insert(domain, provider);
    }
    
    /// Register the default tool provider (handles most tools)
    pub fn register_default_provider(&mut self, provider: Arc<LocalToolProvider>) {
        self.register_provider("default".to_string(), provider);
    }
    
    /// Find the appropriate tool provider for a given tool
    fn find_provider_for_tool(&self, tool_name: &str) -> Option<&Arc<LocalToolProvider>> {
        // For now, use simple domain mapping
        let domain = match tool_name {
            name if name.starts_with("browser_") => "browser",
            name if name.starts_with("file_") => "file", 
            name if name.starts_with("desktop_") => "desktop",
            _ => "default",
        };
        
        // Try domain-specific provider first, fallback to default
        self.tool_providers.get(domain)
            .or_else(|| self.tool_providers.get("default"))
    }
    
    /// Coordinate tool execution by emitting coordination events
    /// In Phase 1.7, this delegates actual execution to EventDrivenToolExecutor
    async fn coordinate_tool_call(
        &self, 
        tool_name: &str, 
        args: &serde_json::Value, 
        tool_call_id: &str,
        session_id: Option<&str>
    ) -> Result<Vec<JunoAgentEvent>, String> {
        info!("Coordinating tool call: {} with ID: {}", tool_name, tool_call_id);
        debug!("Tool coordination args: {}", args);
        
        // Validate tool availability
        if !self.is_tool_available(tool_name) {
            let error_msg = format!("Tool '{}' is not available in any registered provider", tool_name);
            error!("{}", error_msg);
            
            return Ok(vec![
                JunoAgentEvent::ErrorOccurred {
                    error_type: "tool_not_available".to_string(),
                    message: error_msg.clone(),
                    recoverable: false,
                    timestamp: now(),
                    context: Some(serde_json::json!({
                        "tool_name": tool_name,
                        "tool_call_id": tool_call_id,
                        "session_id": session_id,
                        "available_tools": self.list_available_tools().await
                    })),
                }
            ]);
        }
        
        // Emit coordination start event for monitoring
        let coordination_start = JunoAgentEvent::ToolCoordinationStart {
            tool_name: tool_name.to_string(),
            tool_call_id: tool_call_id.to_string(),
            timestamp: now(),
        };
        
        // Note: ToolExecutionStart events removed as they were redundant
        // The ToolCall events contain all necessary information for execution
        // The EventDrivenToolExecutor handles ToolCall events directly
        Ok(vec![coordination_start])
    }
    
    /// Check if a tool is available in any registered provider
    fn is_tool_available(&self, tool_name: &str) -> bool {
        if let Some(provider) = self.find_provider_for_tool(tool_name) {
            // We could check the provider's tool list here, but for now assume availability
            // This could be enhanced to actually query the provider's available tools
            true
        } else {
            false
        }
    }
    
    /// List all available tools across all providers (for debugging/monitoring)
    async fn list_available_tools(&self) -> Vec<String> {
        let mut all_tools = Vec::new();
        
        for (domain, provider) in &self.tool_providers {
            match provider.list_tools().await {
                Ok(tools) => {
                    for tool in tools {
                        all_tools.push(format!("{}:{}", domain, tool.name));
                    }
                }
                Err(e) => {
                    warn!("Failed to list tools for domain {}: {}", domain, e);
                }
            }
        }
        
        all_tools
    }
    
    /// Get coordination statistics for monitoring
    pub fn get_coordination_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "registered_domains": self.tool_providers.keys().collect::<Vec<_>>(),
            "provider_count": self.tool_providers.len(),
            "coordination_mode": "event_driven_phase_1.7"
        })
    }
}

#[async_trait]
impl EventHandler for ToolCoordinator {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        match event {
            JunoAgentEvent::ToolCall { tool_name, args, id, session_id, .. } => {
                info!("Tool coordinator handling tool call: {} (ID: {})", tool_name, id);
                
                self.coordinate_tool_call(
                    tool_name, 
                    args, 
                    id, 
                    session_id.as_deref()
                ).await
            }
            
            _ => {
                // This handler only processes ToolCall events
                Ok(vec![])
            }
        }
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec!["tool_call"]
    }
    
    fn name(&self) -> &'static str {
        "ToolCoordinator"
    }
    
    fn priority(&self) -> u8 {
        70 // Medium-high priority for tool execution
    }
}
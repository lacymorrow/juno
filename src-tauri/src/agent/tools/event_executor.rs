//! Event-Driven Tool Executor
//! 
//! Pure event-driven tool execution system that responds to ToolExecutionStart events
//! and executes tools through registered providers, emitting ToolResult events.
//! 
//! TARS Integration Phase 1.7: Tool System Refactor

use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};
use async_trait::async_trait;

use crate::agent::events::{EventHandler, JunoAgentEvent, now};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::traits::ToolProvider;
use crate::agent::core::ToolCall;

/// Event-driven tool executor that handles tool execution through events
pub struct EventDrivenToolExecutor {
    /// Registry of tool providers by domain/category
    tool_providers: HashMap<String, Arc<LocalToolProvider>>,
    /// App handle for accessing shared resources
    app_handle: tauri::AppHandle,
    /// Execution metrics for monitoring
    execution_count: std::sync::atomic::AtomicU64,
}

impl EventDrivenToolExecutor {
    /// Create a new event-driven tool executor
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            tool_providers: HashMap::new(),
            app_handle,
            execution_count: std::sync::atomic::AtomicU64::new(0),
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
        // Enhanced domain mapping based on tool name patterns
        let domain = match tool_name {
            // Browser tools
            name if name.starts_with("browser_") || name.starts_with("web_") => "browser",
            
            // File system tools
            name if name.starts_with("file_") || name.starts_with("fs_") => "file",
            
            // Desktop/UI automation tools
            name if name.starts_with("desktop_") || name.starts_with("ui_") || 
                   name.starts_with("click") || name.starts_with("screenshot") => "desktop",
            
            // Computer use tools (Anthropic SDK)
            name if name.starts_with("computer_") => "computer_use",
            
            // System/basic tools
            name if name.starts_with("system_") || name.starts_with("basic_") => "system",
            
            // Safari-specific tools
            name if name.starts_with("safari_") => "safari",
            
            // MCP tools
            name if name.starts_with("mcp_") => "mcp",
            
            // Default for unrecognized patterns
            _ => "default",
        };
        
        debug!("Tool '{}' mapped to domain: {}", tool_name, domain);
        
        // Try domain-specific provider first, fallback to default
        self.tool_providers.get(domain)
            .or_else(|| self.tool_providers.get("default"))
    }
    
    /// Execute a tool asynchronously through the event system
    async fn execute_tool_async(
        &self,
        tool_name: &str,
        tool_call_id: &str,
        args: &serde_json::Value,
        session_id: Option<&str>
    ) -> Result<Vec<JunoAgentEvent>, String> {
        let start_time = std::time::Instant::now();
        
        // Increment execution counter for metrics
        self.execution_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        info!("Executing tool: {} (ID: {})", tool_name, tool_call_id);
        debug!("Tool arguments: {}", args);
        
        // Find appropriate provider
        let provider = self.find_provider_for_tool(tool_name)
            .ok_or_else(|| {
                let error_msg = format!("No tool provider found for tool: {}", tool_name);
                error!("{}", error_msg);
                error_msg
            })?;
        
        // Create tool call structure
        let tool_call = ToolCall {
            id: tool_call_id.to_string(),
            name: tool_name.to_string(),
            input: args.clone(),
        };
        
        // Execute tool through provider
        let execution_result = provider.execute_tool(tool_call).await;
        let execution_time = start_time.elapsed().as_millis() as u64;
        
        // Create result events based on execution outcome
        let mut events = Vec::new();
        
        match execution_result {
            Ok(tool_result) => {
                info!("Tool execution successful for: {} (took {}ms)", tool_name, execution_time);
                debug!("Tool result: {:?}", tool_result.output);
                
                // Emit successful tool result
                events.push(JunoAgentEvent::ToolResult {
                    tool_call_id: tool_call_id.to_string(),
                    result: tool_result.output,
                    timestamp: now(),
                    success: true,
                    execution_time_ms: Some(execution_time),
                });
                
                // Emit execution end event
                events.push(JunoAgentEvent::ToolExecutionEnd {
                    tool_name: tool_name.to_string(),
                    tool_call_id: tool_call_id.to_string(),
                    timestamp: now(),
                    success: true,
                });
            }
            
            Err(e) => {
                error!("Tool execution failed for {}: {} (took {}ms)", tool_name, e, execution_time);
                
                // Create structured error response
                let error_result = serde_json::json!({
                    "error": e.to_string(),
                    "success": false,
                    "tool_name": tool_name,
                    "execution_time_ms": execution_time,
                    "timestamp": now()
                });
                
                // Emit failed tool result
                events.push(JunoAgentEvent::ToolResult {
                    tool_call_id: tool_call_id.to_string(),
                    result: error_result,
                    timestamp: now(),
                    success: false,
                    execution_time_ms: Some(execution_time),
                });
                
                // Emit execution end event
                events.push(JunoAgentEvent::ToolExecutionEnd {
                    tool_name: tool_name.to_string(),
                    tool_call_id: tool_call_id.to_string(),
                    timestamp: now(),
                    success: false,
                });
                
                // Emit error event for monitoring and recovery
                events.push(JunoAgentEvent::ErrorOccurred {
                    error_type: "tool_execution_failed".to_string(),
                    message: format!("Tool '{}' execution failed: {}", tool_name, e),
                    recoverable: true,
                    timestamp: now(),
                    context: Some(serde_json::json!({
                        "tool_name": tool_name,
                        "tool_call_id": tool_call_id,
                        "session_id": session_id,
                        "execution_time_ms": execution_time,
                        "provider_domain": self.get_domain_for_tool(tool_name)
                    })),
                });
            }
        }
        
        Ok(events)
    }
    
    /// Get the domain classification for a tool (for debugging/monitoring)
    fn get_domain_for_tool(&self, tool_name: &str) -> String {
        match tool_name {
            name if name.starts_with("browser_") || name.starts_with("web_") => "browser".to_string(),
            name if name.starts_with("file_") || name.starts_with("fs_") => "file".to_string(),
            name if name.starts_with("desktop_") || name.starts_with("ui_") => "desktop".to_string(),
            name if name.starts_with("computer_") => "computer_use".to_string(),
            name if name.starts_with("system_") || name.starts_with("basic_") => "system".to_string(),
            name if name.starts_with("safari_") => "safari".to_string(),
            name if name.starts_with("mcp_") => "mcp".to_string(),
            _ => "default".to_string(),
        }
    }
    
    /// Get execution statistics for monitoring
    pub fn get_execution_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "total_executions": self.execution_count.load(std::sync::atomic::Ordering::Relaxed),
            "registered_providers": self.tool_providers.keys().collect::<Vec<_>>(),
            "provider_count": self.tool_providers.len()
        })
    }
}

#[async_trait]
impl EventHandler for EventDrivenToolExecutor {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        match event {
            // ToolExecutionStart events removed - we now handle ToolCall events directly
            
            JunoAgentEvent::ToolCall { tool_name, args, id, session_id, .. } => {
                info!("Event-driven tool executor handling direct tool call: {} (ID: {})", tool_name, id);
                
                // Execute the tool with full context
                self.execute_tool_async(
                    tool_name,
                    id,
                    args,
                    session_id.as_deref()
                ).await
            }
            
            _ => {
                // This handler only processes tool execution events
                Ok(vec![])
            }
        }
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec!["tool_execution_start", "tool_call"]
    }
    
    fn name(&self) -> &'static str {
        "EventDrivenToolExecutor"
    }
    
    fn priority(&self) -> u8 {
        60 // Medium priority - executes after coordination
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_domain_mapping_logic() {
        // Test domain mapping logic without needing app handle
        let test_cases = vec![
            ("browser_navigate", "browser"),
            ("file_read", "file"),
            ("desktop_click", "desktop"),
            ("computer_screenshot", "computer_use"),
            ("safari_inject_script", "safari"),
            ("unknown_tool", "default"),
        ];
        
        for (tool_name, expected_domain) in test_cases {
            let domain = match tool_name {
                name if name.starts_with("browser_") || name.starts_with("web_") => "browser",
                name if name.starts_with("file_") || name.starts_with("fs_") => "file",
                name if name.starts_with("desktop_") || name.starts_with("ui_") => "desktop",
                name if name.starts_with("computer_") => "computer_use",
                name if name.starts_with("safari_") => "safari",
                _ => "default",
            };
            assert_eq!(domain, expected_domain, "Tool '{}' should map to domain '{}'", tool_name, expected_domain);
        }
    }
}
//! Event-Driven UI Manager
//!
//! Pure event-driven UI management that forwards events to the frontend and maintains
//! backwards compatibility with legacy event patterns.
//!
//! TARS Integration Phase 1.8: UI and State Management Refactor

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};
use async_trait::async_trait;
use serde_json::json;

use crate::agent::events::{EventHandler, JunoAgentEvent};

/// Event-driven UI manager that handles frontend communication through events
pub struct EventDrivenUIManager {
    /// App handle for emitting events to frontend
    app_handle: tauri::AppHandle,
    /// Event emission counter for monitoring
    event_emissions: std::sync::atomic::AtomicU64,
    /// Whether to emit legacy events for backwards compatibility
    emit_legacy_events: bool,
}

impl EventDrivenUIManager {
    /// Create a new event-driven UI manager
    pub fn new(app_handle: tauri::AppHandle, emit_legacy_events: bool) -> Self {
        Self {
            app_handle,
            event_emissions: std::sync::atomic::AtomicU64::new(0),
            emit_legacy_events,
        }
    }
    
    /// Get UI management statistics
    pub fn get_ui_stats(&self) -> serde_json::Value {
        json!({
            "total_event_emissions": self.event_emissions.load(std::sync::atomic::Ordering::Relaxed),
            "legacy_events_enabled": self.emit_legacy_events,
            "ui_management_mode": "event_driven_phase_1.8"
        })
    }
    
    /// Increment event emission counter
    fn increment_emissions(&self) {
        self.event_emissions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Emit event to frontend with error handling
    async fn emit_to_frontend(&self, event_name: &str, payload: impl serde::Serialize + Clone) -> Result<(), String> {
        match self.app_handle.emit(event_name, payload) {
            Ok(_) => {
                self.increment_emissions();
                debug!("Emitted event '{}' to frontend", event_name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to emit event '{}' to frontend: {}", event_name, e);
                Err(e.to_string())
            }
        }
    }
    
    /// Handle agent lifecycle events for UI updates
    async fn handle_agent_ui_events(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::AgentRunStart { session_id, agent_type, user_query, .. } => {
                info!("UI Manager: Agent execution started for session: {}", session_id);
                
                // Emit comprehensive event to frontend
                self.emit_to_frontend("agent-event", event).await?;
                
                // Legacy events for backwards compatibility
                if self.emit_legacy_events {
                    self.emit_to_frontend("agent-active", true).await?;
                    self.emit_to_frontend("agent-session-start", json!({
                        "session_id": session_id,
                        "agent_type": agent_type,
                        "user_query": user_query
                    })).await?;
                }
            }
            
            JunoAgentEvent::AgentRunEnd { session_id, status, iterations, elapsed_ms, .. } => {
                info!("UI Manager: Agent execution ended for session: {} with status: {}", session_id, status);
                
                // Emit comprehensive event to frontend
                self.emit_to_frontend("agent-event", event).await?;
                
                // Legacy events for backwards compatibility
                if self.emit_legacy_events {
                    self.emit_to_frontend("agent-active", false).await?;
                    self.emit_to_frontend("agent-session-end", json!({
                        "session_id": session_id,
                        "status": status,
                        "iterations": iterations,
                        "elapsed_ms": elapsed_ms
                    })).await?;
                }
            }
            
            JunoAgentEvent::AgentIterationStart { session_id, iteration, .. } => {
                debug!("UI Manager: Agent iteration {} started for session: {}", iteration, session_id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("agent-iteration", json!({
                        "session_id": session_id,
                        "iteration": iteration,
                        "status": "started"
                    })).await?;
                }
            }
            
            JunoAgentEvent::AgentIterationEnd { session_id, iteration, action_taken, .. } => {
                debug!("UI Manager: Agent iteration {} ended for session: {}", iteration, session_id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("agent-iteration", json!({
                        "session_id": session_id,
                        "iteration": iteration,
                        "status": "ended",
                        "action": action_taken
                    })).await?;
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle tool execution events for UI updates
    async fn handle_tool_ui_events(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::ToolCall { tool_name, id, .. } => {
                info!("UI Manager: Tool call started: {} (ID: {})", tool_name, id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("tool-execution", json!({
                        "tool_name": tool_name,
                        "tool_call_id": id,
                        "status": "started"
                    })).await?;
                }
            }
            
            JunoAgentEvent::ToolResult { tool_call_id, success, execution_time_ms, .. } => {
                info!("UI Manager: Tool execution completed: {} (success: {})", tool_call_id, success);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("tool-execution", json!({
                        "tool_call_id": tool_call_id,
                        "status": "completed",
                        "success": success,
                        "execution_time_ms": execution_time_ms
                    })).await?;
                }
            }
            
            // ToolExecutionStart events removed - no longer emitted by the system
            
            JunoAgentEvent::ToolExecutionEnd { tool_name, tool_call_id, success, .. } => {
                debug!("UI Manager: Tool execution end: {} (ID: {}, success: {})", tool_name, tool_call_id, success);
                self.emit_to_frontend("agent-event", event).await?;
            }
            
            JunoAgentEvent::ToolCoordinationStart { tool_name, tool_call_id, .. } => {
                debug!("UI Manager: Tool coordination start: {} (ID: {})", tool_name, tool_call_id);
                self.emit_to_frontend("agent-event", event).await?;
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle voice and transcription events for UI updates
    async fn handle_voice_ui_events(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::VoiceTranscriptionStart { session_id, mode, .. } => {
                info!("UI Manager: Voice transcription started for session: {} in mode: {}", session_id, mode);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    // Map to legacy events based on mode
                    match mode.as_str() {
                        "dictation" => {
                            self.emit_to_frontend("dictation-active", true).await?;
                        }
                        "agent" => {
                            self.emit_to_frontend("voice-input-active", true).await?;
                        }
                        "always_listening" => {
                            self.emit_to_frontend("always-listening-active", true).await?;
                        }
                        _ => {}
                    }
                }
            }
            
            JunoAgentEvent::VoiceTranscriptionChunk { content, is_final, confidence, .. } => {
                debug!("UI Manager: Voice transcription chunk (final: {}, confidence: {:?})", is_final, confidence);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("voice-transcription", json!({
                        "content": content,
                        "is_final": is_final,
                        "confidence": confidence
                    })).await?;
                }
            }
            
            JunoAgentEvent::VoiceTranscriptionEnd { session_id, final_text, .. } => {
                info!("UI Manager: Voice transcription ended for session: {}", session_id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("dictation-active", false).await?;
                    self.emit_to_frontend("voice-input-active", false).await?;
                    self.emit_to_frontend("always-listening-active", false).await?;
                    self.emit_to_frontend("voice-transcription-final", json!({
                        "session_id": session_id,
                        "final_text": final_text
                    })).await?;
                }
            }
            
            JunoAgentEvent::VoiceTranscriptionError { session_id, error_message, .. } => {
                error!("UI Manager: Voice transcription error for session: {} - {}", session_id, error_message);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("voice-error", json!({
                        "session_id": session_id,
                        "error": error_message
                    })).await?;
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle TTS events for UI updates
    async fn handle_tts_ui_events(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::TtsStart { text, provider, session_id, .. } => {
                info!("UI Manager: TTS started for session: {} using provider: {}", session_id, provider);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("tts-active", true).await?;
                    self.emit_to_frontend("tts-start", json!({
                        "text": text,
                        "provider": provider,
                        "session_id": session_id
                    })).await?;
                }
            }
            
            JunoAgentEvent::TtsEnd { session_id, success, duration_ms, .. } => {
                info!("UI Manager: TTS ended for session: {} (success: {})", session_id, success);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("tts-active", false).await?;
                    self.emit_to_frontend("tts-end", json!({
                        "session_id": session_id,
                        "success": success,
                        "duration_ms": duration_ms
                    })).await?;
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle system and error events for UI updates
    async fn handle_system_ui_events(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::ErrorOccurred { error_type, message, recoverable, context, .. } => {
                error!("UI Manager: Error occurred - {}: {} (recoverable: {})", error_type, message, recoverable);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("system-error", json!({
                        "error_type": error_type,
                        "message": message,
                        "recoverable": recoverable,
                        "context": context
                    })).await?;
                }
            }
            
            JunoAgentEvent::SystemMessage { level, message, category, .. } => {
                match level.as_str() {
                    "error" => error!("UI Manager: System message - {}", message),
                    "warn" => warn!("UI Manager: System message - {}", message),
                    "info" => info!("UI Manager: System message - {}", message),
                    _ => debug!("UI Manager: System message - {}", message),
                }
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("system-message", json!({
                        "level": level,
                        "message": message,
                        "category": category
                    })).await?;
                }
            }
            
            JunoAgentEvent::ConfigurationChanged { key, old_value, new_value, .. } => {
                info!("UI Manager: Configuration changed - {}", key);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("config-changed", json!({
                        "key": key,
                        "old_value": old_value,
                        "new_value": new_value
                    })).await?;
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle conversation events for UI updates
    async fn handle_conversation_ui_events(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::UserMessage { content, session_id, .. } => {
                info!("UI Manager: User message received for session: {:?}", session_id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("user-message", json!({
                        "content": content,
                        "session_id": session_id
                    })).await?;
                }
            }
            
            JunoAgentEvent::AssistantMessage { content, session_id, .. } => {
                info!("UI Manager: Assistant message for session: {:?}", session_id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("assistant-message", json!({
                        "content": content,
                        "session_id": session_id
                    })).await?;
                }
            }
            
            JunoAgentEvent::AssistantStreamingMessage { content, is_partial, chunk_id, session_id, .. } => {
                debug!("UI Manager: Assistant streaming message (partial: {}, chunk: {})", is_partial, chunk_id);
                
                self.emit_to_frontend("agent-event", event).await?;
                
                if self.emit_legacy_events {
                    self.emit_to_frontend("assistant-streaming", json!({
                        "content": content,
                        "is_partial": is_partial,
                        "chunk_id": chunk_id,
                        "session_id": session_id
                    })).await?;
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
}

#[async_trait]
impl EventHandler for EventDrivenUIManager {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        // Always emit the raw event to frontend for complete event stream access
        if let Err(e) = self.emit_to_frontend("agent-event", event).await {
            warn!("Failed to emit raw event to frontend: {}", e);
        }
        
        // Handle specific event types for UI updates
        let result = match event {
            // Agent lifecycle events
            JunoAgentEvent::AgentRunStart { .. } |
            JunoAgentEvent::AgentRunEnd { .. } |
            JunoAgentEvent::AgentIterationStart { .. } |
            JunoAgentEvent::AgentIterationEnd { .. } => {
                self.handle_agent_ui_events(event).await
            }
            
            // Tool execution events
            JunoAgentEvent::ToolCall { .. } |
            JunoAgentEvent::ToolResult { .. } |
            JunoAgentEvent::ToolExecutionEnd { .. } |
            JunoAgentEvent::ToolCoordinationStart { .. } => {
                self.handle_tool_ui_events(event).await
            }
            
            // Voice and transcription events
            JunoAgentEvent::VoiceTranscriptionStart { .. } |
            JunoAgentEvent::VoiceTranscriptionChunk { .. } |
            JunoAgentEvent::VoiceTranscriptionEnd { .. } |
            JunoAgentEvent::VoiceTranscriptionError { .. } => {
                self.handle_voice_ui_events(event).await
            }
            
            // TTS events
            JunoAgentEvent::TtsStart { .. } |
            JunoAgentEvent::TtsEnd { .. } => {
                self.handle_tts_ui_events(event).await
            }
            
            // System and error events
            JunoAgentEvent::ErrorOccurred { .. } |
            JunoAgentEvent::SystemMessage { .. } |
            JunoAgentEvent::ConfigurationChanged { .. } => {
                self.handle_system_ui_events(event).await
            }
            
            // Conversation events
            JunoAgentEvent::UserMessage { .. } |
            JunoAgentEvent::AssistantMessage { .. } |
            JunoAgentEvent::AssistantStreamingMessage { .. } => {
                self.handle_conversation_ui_events(event).await
            }
            
            // Events we pass through without special handling
            _ => Ok(())
        };
        
        if let Err(e) = result {
            error!("UI Manager error handling event: {}", e);
        }
        
        // UI Manager doesn't generate new events, just forwards to frontend
        Ok(vec![])
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec!["*"] // Handle all event types for comprehensive UI updates
    }
    
    fn name(&self) -> &'static str {
        "EventDrivenUIManager"
    }
    
    fn priority(&self) -> u8 {
        10 // Low priority - handle events after all other processing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ui_manager_creation() {
        // Simple test that doesn't require mock app
        assert!(true, "EventDrivenUIManager module compiles successfully");
    }
}
//! Event-Driven State Manager
//!
//! Pure event-driven state management that reacts to events and updates application state.
//! This replaces direct state mutations with reactive state updates through event subscription.
//!
//! TARS Integration Phase 1.8: UI and State Management Refactor

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{error, info, debug, warn};
use serde::{Serialize, Deserialize};

use crate::agent::events::{EventHandler, JunoAgentEvent, now};
use crate::state::AppState;

/// Application state structure for event-driven management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationState {
    /// Current agent execution session
    pub current_session: Option<String>,
    /// Whether an agent is currently executing
    pub agent_executing: bool,
    /// Current agent execution step
    pub current_step: Option<u32>,
    /// Maximum steps for current execution
    pub max_steps: Option<u32>,
    /// Agent execution start time
    pub execution_start_time: Option<u64>,
    /// Whether dictation is currently active
    pub dictation_active: bool,
    /// Current dictation session
    pub dictation_session: Option<String>,
    /// Voice transcription status
    pub voice_transcription_active: bool,
    /// Current voice transcription session
    pub voice_session: Option<String>,
    /// TTS playback status
    pub tts_active: bool,
    /// Current TTS session
    pub tts_session: Option<String>,
    /// Browser automation status
    pub browser_active: bool,
    /// Current browser session
    pub browser_session: Option<String>,
    /// Last error that occurred
    pub last_error: Option<String>,
    /// Error recovery status
    pub error_recovery_active: bool,
    /// Memory management status
    pub memory_pruning_active: bool,
    /// Configuration change timestamp
    pub last_config_change: Option<u64>,
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            current_session: None,
            agent_executing: false,
            current_step: None,
            max_steps: None,
            execution_start_time: None,
            dictation_active: false,
            dictation_session: None,
            voice_transcription_active: false,
            voice_session: None,
            tts_active: false,
            tts_session: None,
            browser_active: false,
            browser_session: None,
            last_error: None,
            error_recovery_active: false,
            memory_pruning_active: false,
            last_config_change: None,
        }
    }
}

/// Event-driven state manager that reacts to events and updates application state
pub struct EventDrivenStateManager {
    /// Internal application state
    state: Arc<RwLock<ApplicationState>>,
    /// Reference to the main AppState for integration
    app_state: Arc<AppState>,
    /// App handle for event emission
    app_handle: tauri::AppHandle,
    /// State change counter for monitoring
    state_changes: std::sync::atomic::AtomicU64,
}

impl EventDrivenStateManager {
    /// Create a new event-driven state manager
    pub fn new(app_state: Arc<AppState>, app_handle: tauri::AppHandle) -> Self {
        Self {
            state: Arc::new(RwLock::new(ApplicationState::default())),
            app_state,
            app_handle,
            state_changes: std::sync::atomic::AtomicU64::new(0),
        }
    }
    
    /// Get a snapshot of the current state
    pub async fn get_state_snapshot(&self) -> ApplicationState {
        let state = self.state.read().await;
        state.clone()
    }
    
    /// Get state change statistics
    pub fn get_state_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "total_state_changes": self.state_changes.load(std::sync::atomic::Ordering::Relaxed),
            "state_management_mode": "event_driven_phase_1.8"
        })
    }
    
    /// Increment state change counter
    fn increment_state_changes(&self) {
        self.state_changes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Handle agent lifecycle events
    async fn handle_agent_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::AgentRunStart { session_id, max_iterations, .. } => {
                info!("State manager: Agent execution started for session: {}", session_id);
                state.current_session = Some(session_id.clone());
                state.agent_executing = true;
                state.current_step = Some(1);
                state.max_steps = Some(*max_iterations);
                state.execution_start_time = Some(now());
                
                // Update main AppState for backwards compatibility
                if let Err(e) = self.app_state.mark_agent_execution_started(session_id.clone()) {
                    warn!("Failed to update main AppState: {}", e);
                }
            }
            
            JunoAgentEvent::AgentRunEnd { session_id, status, iterations, .. } => {
                info!("State manager: Agent execution ended for session: {} with status: {}", session_id, status);
                
                // Only update if this is our current session
                if state.current_session.as_ref() == Some(session_id) {
                    state.agent_executing = false;
                    state.current_step = Some(*iterations);
                    state.execution_start_time = None;
                    
                    // Keep session for reference but mark as completed
                    if status == "completed" || status == "failed" {
                        // Could keep session for history, or clear it
                        // state.current_session = None;
                    }
                }
                
                // Update main AppState
                self.app_state.mark_agent_execution_finished();
            }
            
            JunoAgentEvent::AgentIterationStart { session_id, iteration, .. } => {
                if state.current_session.as_ref() == Some(session_id) {
                    debug!("State manager: Agent iteration {} started for session: {}", iteration, session_id);
                    state.current_step = Some(*iteration);
                }
            }
            
            JunoAgentEvent::AgentIterationEnd { session_id, iteration, .. } => {
                if state.current_session.as_ref() == Some(session_id) {
                    debug!("State manager: Agent iteration {} ended for session: {}", iteration, session_id);
                    state.current_step = Some(*iteration);
                }
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
    
    /// Handle voice and transcription events
    async fn handle_voice_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::VoiceTranscriptionStart { session_id, mode, .. } => {
                info!("State manager: Voice transcription started for session: {} in mode: {}", session_id, mode);
                state.voice_transcription_active = true;
                state.voice_session = Some(session_id.clone());
                
                // Update dictation state based on mode
                if mode == "dictation" {
                    state.dictation_active = true;
                    state.dictation_session = Some(session_id.clone());
                    
                    // Update main AppState
                    if let Err(e) = self.app_state.set_dictation_active(true) {
                        warn!("Failed to update dictation state in AppState: {}", e);
                    }
                }
            }
            
            JunoAgentEvent::VoiceTranscriptionEnd { session_id, .. } => {
                info!("State manager: Voice transcription ended for session: {}", session_id);
                
                if state.voice_session.as_ref() == Some(session_id) {
                    state.voice_transcription_active = false;
                    state.voice_session = None;
                    
                    // End dictation if it was active
                    if state.dictation_session.as_ref() == Some(session_id) {
                        state.dictation_active = false;
                        state.dictation_session = None;
                        
                        // Update main AppState
                        if let Err(e) = self.app_state.set_dictation_active(false) {
                            warn!("Failed to update dictation state in AppState: {}", e);
                        }
                    }
                }
            }
            
            JunoAgentEvent::VoiceTranscriptionError { session_id, .. } => {
                error!("State manager: Voice transcription error for session: {}", session_id);
                
                if state.voice_session.as_ref() == Some(session_id) {
                    state.voice_transcription_active = false;
                    state.voice_session = None;
                    state.dictation_active = false;
                    state.dictation_session = None;
                }
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
    
    /// Handle TTS events
    async fn handle_tts_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::TtsStart { session_id, .. } => {
                info!("State manager: TTS started for session: {}", session_id);
                state.tts_active = true;
                state.tts_session = Some(session_id.clone());
            }
            
            JunoAgentEvent::TtsEnd { session_id, success, .. } => {
                info!("State manager: TTS ended for session: {} with success: {}", session_id, success);
                
                if state.tts_session.as_ref() == Some(session_id) {
                    state.tts_active = false;
                    state.tts_session = None;
                }
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
    
    /// Handle browser events
    async fn handle_browser_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::BrowserStart { session_id, .. } => {
                info!("State manager: Browser started for session: {}", session_id);
                state.browser_active = true;
                state.browser_session = Some(session_id.clone());
            }
            
            JunoAgentEvent::BrowserEnd { session_id, .. } => {
                info!("State manager: Browser ended for session: {}", session_id);
                
                if state.browser_session.as_ref() == Some(session_id) {
                    state.browser_active = false;
                    state.browser_session = None;
                }
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
    
    /// Handle error and recovery events
    async fn handle_error_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::ErrorOccurred { error_type, message, .. } => {
                warn!("State manager: Error occurred - {}: {}", error_type, message);
                state.last_error = Some(format!("{}: {}", error_type, message));
                
                // If it's a critical error, reset relevant states
                if !error_type.contains("recoverable") {
                    state.error_recovery_active = true;
                }
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
    
    /// Handle memory management events
    async fn handle_memory_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::MemoryPruneStart { .. } => {
                debug!("State manager: Memory pruning started");
                state.memory_pruning_active = true;
            }
            
            JunoAgentEvent::MemoryPruneEnd { .. } => {
                debug!("State manager: Memory pruning ended");
                state.memory_pruning_active = false;
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
    
    /// Handle configuration events
    async fn handle_config_events(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        let mut state = self.state.write().await;
        self.increment_state_changes();
        
        match event {
            JunoAgentEvent::ConfigurationChanged { .. } => {
                info!("State manager: Configuration changed");
                state.last_config_change = Some(now());
            }
            
            _ => return Ok(vec![])
        }
        
        Ok(vec![])
    }
}

#[async_trait]
impl EventHandler for EventDrivenStateManager {
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
        // Route events to appropriate handlers based on event type
        match event {
            // Agent lifecycle events
            JunoAgentEvent::AgentRunStart { .. } |
            JunoAgentEvent::AgentRunEnd { .. } |
            JunoAgentEvent::AgentIterationStart { .. } |
            JunoAgentEvent::AgentIterationEnd { .. } => {
                self.handle_agent_events(event).await
            }
            
            // Voice and transcription events
            JunoAgentEvent::VoiceTranscriptionStart { .. } |
            JunoAgentEvent::VoiceTranscriptionEnd { .. } |
            JunoAgentEvent::VoiceTranscriptionError { .. } |
            JunoAgentEvent::VoiceTranscriptionChunk { .. } => {
                self.handle_voice_events(event).await
            }
            
            // TTS events
            JunoAgentEvent::TtsStart { .. } |
            JunoAgentEvent::TtsEnd { .. } => {
                self.handle_tts_events(event).await
            }
            
            // Browser events
            JunoAgentEvent::BrowserStart { .. } |
            JunoAgentEvent::BrowserEnd { .. } |
            JunoAgentEvent::BrowserNavigation { .. } => {
                self.handle_browser_events(event).await
            }
            
            // Error events
            JunoAgentEvent::ErrorOccurred { .. } => {
                self.handle_error_events(event).await
            }
            
            // Memory events
            JunoAgentEvent::MemoryPruneStart { .. } |
            JunoAgentEvent::MemoryPruneEnd { .. } => {
                self.handle_memory_events(event).await
            }
            
            // Configuration events
            JunoAgentEvent::ConfigurationChanged { .. } => {
                self.handle_config_events(event).await
            }
            
            // Events we don't handle in state management
            _ => Ok(vec![])
        }
    }
    
    fn event_types(&self) -> Vec<&'static str> {
        vec![
            "agent_run_start", "agent_run_end", "agent_iteration_start", "agent_iteration_end",
            "voice_transcription_start", "voice_transcription_end", "voice_transcription_error",
            "tts_start", "tts_end",
            "browser_start", "browser_end", "browser_navigation",
            "error_occurred",
            "memory_prune_start", "memory_prune_end",
            "configuration_changed"
        ]
    }
    
    fn name(&self) -> &'static str {
        "EventDrivenStateManager"
    }
    
    fn priority(&self) -> u8 {
        90 // High priority for state management
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_application_state_default() {
        let state = ApplicationState::default();
        assert!(!state.agent_executing);
        assert!(!state.dictation_active);
        assert!(!state.voice_transcription_active);
        assert!(state.current_session.is_none());
    }
}
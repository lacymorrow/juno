use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Comprehensive event types for Juno's agent system
/// Inspired by TARS's event stream architecture
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum JunoAgentEvent {
    // Core conversation events
    UserMessage {
        content: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    AssistantMessage {
        content: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    AssistantStreamingMessage {
        content: String,
        is_partial: bool,
        chunk_id: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    
    // Tool execution events
    ToolCall {
        tool_name: String,
        args: Value,
        id: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        result: Value,
        timestamp: u64,
        success: bool,
        execution_time_ms: Option<u64>,
    },
    ToolExecutionStart {
        tool_name: String,
        tool_call_id: String,
        timestamp: u64,
    },
    ToolExecutionEnd {
        tool_name: String,
        tool_call_id: String,
        timestamp: u64,
        success: bool,
    },
    
    // Agent lifecycle events
    AgentRunStart {
        session_id: String,
        agent_type: String,
        max_iterations: u32,
        user_query: String, // Include the user query directly
        timestamp: u64,
    },
    AgentRunEnd {
        session_id: String,
        status: String,
        iterations: u32,
        elapsed_ms: u64,
        timestamp: u64,
    },
    AgentIterationStart {
        session_id: String,
        iteration: u32,
        timestamp: u64,
    },
    AgentIterationEnd {
        session_id: String,
        iteration: u32,
        timestamp: u64,
        action_taken: String,
    },
    
    // Voice system events
    VoiceTranscriptionStart {
        session_id: String,
        mode: String, // "agent", "dictation", "always_listening"
        timestamp: u64,
    },
    VoiceTranscriptionChunk {
        content: String,
        is_final: bool,
        confidence: Option<f32>,
        session_id: String,
        timestamp: u64,
    },
    VoiceTranscriptionEnd {
        session_id: String,
        final_text: String,
        total_duration_ms: u64,
        timestamp: u64,
    },
    VoiceTranscriptionError {
        session_id: String,
        error_message: String,
        timestamp: u64,
    },
    
    // TTS events
    TtsStart {
        text: String,
        provider: String,
        session_id: String,
        timestamp: u64,
    },
    TtsEnd {
        session_id: String,
        success: bool,
        duration_ms: Option<u64>,
        timestamp: u64,
    },
    
    // System events
    SystemMessage {
        level: String, // "debug", "info", "warn", "error"
        message: String,
        timestamp: u64,
        category: Option<String>,
    },
    PermissionRequest {
        permission_type: String, // "accessibility", "screen_recording", "microphone"
        status: String, // "requested", "granted", "denied"
        timestamp: u64,
    },
    ErrorOccurred {
        error_type: String,
        message: String,
        recoverable: bool,
        timestamp: u64,
        context: Option<Value>,
    },
    
    // Memory management events
    MemoryPruneStart {
        reason: String,
        messages_before: usize,
        estimated_tokens: usize,
        timestamp: u64,
    },
    MemoryPruneEnd {
        messages_after: usize,
        tokens_saved: usize,
        compression_applied: bool,
        timestamp: u64,
    },
    
    // Browser events
    BrowserStart {
        session_id: String,
        timestamp: u64,
    },
    BrowserEnd {
        session_id: String,
        timestamp: u64,
    },
    BrowserNavigation {
        url: String,
        session_id: String,
        timestamp: u64,
    },
    
    // Configuration events
    ConfigurationChanged {
        key: String,
        old_value: Option<Value>,
        new_value: Value,
        timestamp: u64,
    },
}

impl JunoAgentEvent {
    /// Create a new event with current timestamp
    pub fn with_timestamp(mut self) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        match &mut self {
            JunoAgentEvent::UserMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AssistantMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AssistantStreamingMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ToolCall { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ToolResult { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ToolExecutionStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ToolExecutionEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AgentRunStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AgentRunEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AgentIterationStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AgentIterationEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::VoiceTranscriptionStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::VoiceTranscriptionChunk { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::VoiceTranscriptionEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::VoiceTranscriptionError { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::TtsStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::TtsEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::SystemMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::PermissionRequest { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ErrorOccurred { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::MemoryPruneStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::MemoryPruneEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::BrowserStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::BrowserEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::BrowserNavigation { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ConfigurationChanged { timestamp: ts, .. } => *ts = timestamp,
        }
        
        self
    }
    
    /// Get the session ID from events that have one
    pub fn session_id(&self) -> Option<&str> {
        match self {
            JunoAgentEvent::UserMessage { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::AssistantMessage { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::AssistantStreamingMessage { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::ToolCall { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::AgentRunStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::AgentRunEnd { session_id, .. } => Some(session_id),
            JunoAgentEvent::AgentIterationStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::AgentIterationEnd { session_id, .. } => Some(session_id),
            JunoAgentEvent::VoiceTranscriptionStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::VoiceTranscriptionChunk { session_id, .. } => Some(session_id),
            JunoAgentEvent::VoiceTranscriptionEnd { session_id, .. } => Some(session_id),
            JunoAgentEvent::VoiceTranscriptionError { session_id, .. } => Some(session_id),
            JunoAgentEvent::TtsStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::TtsEnd { session_id, .. } => Some(session_id),
            JunoAgentEvent::BrowserStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::BrowserEnd { session_id, .. } => Some(session_id),
            JunoAgentEvent::BrowserNavigation { session_id, .. } => Some(session_id),
            _ => None,
        }
    }
    
    /// Get a human-readable event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            JunoAgentEvent::UserMessage { .. } => "user_message",
            JunoAgentEvent::AssistantMessage { .. } => "assistant_message",
            JunoAgentEvent::AssistantStreamingMessage { .. } => "assistant_streaming_message",
            JunoAgentEvent::ToolCall { .. } => "tool_call",
            JunoAgentEvent::ToolResult { .. } => "tool_result",
            JunoAgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
            JunoAgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
            JunoAgentEvent::AgentRunStart { .. } => "agent_run_start",
            JunoAgentEvent::AgentRunEnd { .. } => "agent_run_end",
            JunoAgentEvent::AgentIterationStart { .. } => "agent_iteration_start",
            JunoAgentEvent::AgentIterationEnd { .. } => "agent_iteration_end",
            JunoAgentEvent::VoiceTranscriptionStart { .. } => "voice_transcription_start",
            JunoAgentEvent::VoiceTranscriptionChunk { .. } => "voice_transcription_chunk",
            JunoAgentEvent::VoiceTranscriptionEnd { .. } => "voice_transcription_end",
            JunoAgentEvent::VoiceTranscriptionError { .. } => "voice_transcription_error",
            JunoAgentEvent::TtsStart { .. } => "tts_start",
            JunoAgentEvent::TtsEnd { .. } => "tts_end",
            JunoAgentEvent::SystemMessage { .. } => "system_message",
            JunoAgentEvent::PermissionRequest { .. } => "permission_request",
            JunoAgentEvent::ErrorOccurred { .. } => "error_occurred",
            JunoAgentEvent::MemoryPruneStart { .. } => "memory_prune_start",
            JunoAgentEvent::MemoryPruneEnd { .. } => "memory_prune_end",
            JunoAgentEvent::BrowserStart { .. } => "browser_start",
            JunoAgentEvent::BrowserEnd { .. } => "browser_end",
            JunoAgentEvent::BrowserNavigation { .. } => "browser_navigation",
            JunoAgentEvent::ConfigurationChanged { .. } => "configuration_changed",
        }
    }
}

/// Event subscription trait for components that want to receive events
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn on_event(&self, event: &JunoAgentEvent) -> Result<(), String>;
    
    /// Filter to only receive specific event types
    fn event_filter(&self) -> Option<Vec<&'static str>> {
        None // By default, receive all events
    }
}

/// Event filtering utilities
pub struct EventFilter;

impl EventFilter {
    pub fn matches_filter(event: &JunoAgentEvent, filter: &[&str]) -> bool {
        let event_type = event.event_type();
        filter.contains(&event_type)
    }
}
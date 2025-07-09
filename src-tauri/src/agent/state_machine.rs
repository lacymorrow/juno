use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tracing::{info, warn};
use std::collections::HashMap;

/// Represents the current state of an agent execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent is idle and ready for new work
    Idle,
    
    /// Agent is processing a user request
    Processing { 
        session_id: String,
        current_step: u32,
        max_steps: u32,
        started_at: u64,
    },
    
    /// Agent is waiting for a tool to complete execution
    WaitingForTool { 
        session_id: String, 
        tool_call_id: String,
        current_step: u32,
        tool_name: String,
    },
    
    /// Agent is generating a response to the user
    Responding { 
        session_id: String,
        current_step: u32,
        partial_response: Option<String>,
    },
    
    /// Agent encountered an error
    Error { 
        session_id: String, 
        error: String,
        current_step: u32,
        recoverable: bool,
    },
    
    /// Agent completed successfully
    Completed {
        session_id: String,
        final_step: u32,
        elapsed_ms: u64,
        response: String,
    },
    
    /// Agent was cancelled by user
    Cancelled {
        session_id: String,
        final_step: u32,
        elapsed_ms: u64,
    },
}

impl AgentState {
    /// Get the session ID associated with this state
    pub fn session_id(&self) -> Option<&str> {
        match self {
            AgentState::Idle => None,
            AgentState::Processing { session_id, .. } => Some(session_id),
            AgentState::WaitingForTool { session_id, .. } => Some(session_id),
            AgentState::Responding { session_id, .. } => Some(session_id),
            AgentState::Error { session_id, .. } => Some(session_id),
            AgentState::Completed { session_id, .. } => Some(session_id),
            AgentState::Cancelled { session_id, .. } => Some(session_id),
        }
    }
    
    /// Get the current step number
    pub fn current_step(&self) -> u32 {
        match self {
            AgentState::Idle => 0,
            AgentState::Processing { current_step, .. } => *current_step,
            AgentState::WaitingForTool { current_step, .. } => *current_step,
            AgentState::Responding { current_step, .. } => *current_step,
            AgentState::Error { current_step, .. } => *current_step,
            AgentState::Completed { final_step, .. } => *final_step,
            AgentState::Cancelled { final_step, .. } => *final_step,
        }
    }
    
    /// Check if this is a terminal state (agent is done)
    pub fn is_terminal(&self) -> bool {
        matches!(self, 
            AgentState::Completed { .. } | 
            AgentState::Cancelled { .. } | 
            AgentState::Error { recoverable: false, .. }
        )
    }
    
    /// Check if agent is actively working
    pub fn is_active(&self) -> bool {
        !matches!(self, AgentState::Idle) && !self.is_terminal()
    }
    
    /// Get a human-readable status
    pub fn status(&self) -> &'static str {
        match self {
            AgentState::Idle => "idle",
            AgentState::Processing { .. } => "processing",
            AgentState::WaitingForTool { .. } => "waiting_for_tool",
            AgentState::Responding { .. } => "responding",
            AgentState::Error { .. } => "error",
            AgentState::Completed { .. } => "completed",
            AgentState::Cancelled { .. } => "cancelled",
        }
    }
}

/// Manages agent state transitions with validation and history
pub struct AgentStateMachine {
    /// Current state
    current_state: Arc<RwLock<AgentState>>,
    /// State history for debugging
    state_history: Arc<RwLock<Vec<(AgentState, u64)>>>,
    /// Configuration
    config: StateMachineConfig,
}

#[derive(Debug, Clone)]
pub struct StateMachineConfig {
    /// Maximum number of state history entries to keep
    pub max_history: usize,
    /// Whether to log state transitions
    pub log_transitions: bool,
}

impl Default for StateMachineConfig {
    fn default() -> Self {
        Self {
            max_history: 100,
            log_transitions: true,
        }
    }
}

impl AgentStateMachine {
    pub fn new() -> Self {
        Self::with_config(StateMachineConfig::default())
    }
    
    pub fn with_config(config: StateMachineConfig) -> Self {
        Self {
            current_state: Arc::new(RwLock::new(AgentState::Idle)),
            state_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }
    
    /// Get the current state
    pub async fn get_state(&self) -> AgentState {
        self.current_state.read().await.clone()
    }
    
    /// Transition to a new state with validation
    pub async fn transition_to(&self, new_state: AgentState) -> Result<(), String> {
        let mut state = self.current_state.write().await;
        let old_state = state.clone();
        
        // Validate transition
        if !self.is_valid_transition(&old_state, &new_state) {
            return Err(format!(
                "Invalid state transition from {} to {}",
                old_state.status(), new_state.status()
            ));
        }
        
        // Update state
        *state = new_state.clone();
        
        // Log transition
        if self.config.log_transitions {
            info!("Agent state transition: {} -> {} (session: {:?})", 
                  old_state.status(), new_state.status(), new_state.session_id());
        }
        
        // Store in history
        self.add_to_history(old_state).await;
        
        Ok(())
    }
    
    /// Force a state transition without validation (use carefully)
    pub async fn force_transition_to(&self, new_state: AgentState) {
        let mut state = self.current_state.write().await;
        let old_state = state.clone();
        
        *state = new_state.clone();
        
        warn!("Forced state transition: {} -> {} (session: {:?})", 
              old_state.status(), new_state.status(), new_state.session_id());
        
        self.add_to_history(old_state).await;
    }
    
    /// Reset to idle state
    pub async fn reset(&self) {
        let mut state = self.current_state.write().await;
        let old_state = state.clone();
        
        *state = AgentState::Idle;
        
        if self.config.log_transitions {
            info!("Agent state reset: {} -> idle", old_state.status());
        }
        
        self.add_to_history(old_state).await;
    }
    
    /// Get state history
    pub async fn get_history(&self) -> Vec<(AgentState, u64)> {
        self.state_history.read().await.clone()
    }
    
    /// Get states for a specific session
    pub async fn get_session_history(&self, session_id: &str) -> Vec<(AgentState, u64)> {
        let history = self.state_history.read().await;
        history
            .iter()
            .filter(|(state, _)| {
                state.session_id().map_or(false, |id| id == session_id)
            })
            .cloned()
            .collect()
    }
    
    /// Clear state history
    pub async fn clear_history(&self) {
        let mut history = self.state_history.write().await;
        history.clear();
        info!("Cleared agent state history");
    }
    
    /// Add state to history
    async fn add_to_history(&self, state: AgentState) {
        let mut history = self.state_history.write().await;
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        history.push((state, timestamp));
        
        // Prune old history
        if history.len() > self.config.max_history {
            let drain_count = history.len() - self.config.max_history;
            history.drain(0..drain_count);
        }
    }
    
    /// Validate if a state transition is allowed
    fn is_valid_transition(&self, from: &AgentState, to: &AgentState) -> bool {
        use AgentState::*;
        
        match (from, to) {
            // From Idle - can start new work
            (Idle, Processing { .. }) => true,
            
            // From Processing - can go to various states
            (Processing { .. }, WaitingForTool { .. }) => true,
            (Processing { .. }, Responding { .. }) => true,
            (Processing { .. }, Error { .. }) => true,
            (Processing { .. }, Cancelled { .. }) => true,
            (Processing { .. }, Completed { .. }) => true,
            
            // From WaitingForTool - can continue or fail
            (WaitingForTool { .. }, Processing { .. }) => true,
            (WaitingForTool { .. }, Responding { .. }) => true,
            (WaitingForTool { .. }, Error { .. }) => true,
            (WaitingForTool { .. }, Cancelled { .. }) => true,
            
            // From Responding - usually leads to completion
            (Responding { .. }, Completed { .. }) => true,
            (Responding { .. }, Error { .. }) => true,
            (Responding { .. }, Cancelled { .. }) => true,
            
            // From Error - can recover if error is recoverable
            (Error { recoverable: true, .. }, Processing { .. }) => true,
            (Error { recoverable: true, .. }, Idle) => true,
            
            // To Idle - reset is always allowed
            (_, Idle) => true,
            
            // Same state transitions (for updates)
            (Processing { session_id: id1, .. }, Processing { session_id: id2, .. }) => id1 == id2,
            (WaitingForTool { session_id: id1, .. }, WaitingForTool { session_id: id2, .. }) => id1 == id2,
            (Responding { session_id: id1, .. }, Responding { session_id: id2, .. }) => id1 == id2,
            
            // Terminal states can start new sessions or go to Idle
            (Completed { session_id: old_id, .. }, Processing { session_id: new_id, .. }) => old_id != new_id,
            (Completed { .. }, Idle) => true,
            (Cancelled { session_id: old_id, .. }, Processing { session_id: new_id, .. }) => old_id != new_id,
            (Cancelled { .. }, Idle) => true,
            (Error { recoverable: false, session_id: old_id, .. }, Processing { session_id: new_id, .. }) => old_id != new_id,
            (Error { recoverable: false, .. }, Idle) => true,
            
            _ => false,
        }
    }
    
    /// Get state machine statistics
    pub async fn get_stats(&self) -> StateMachineStats {
        let history = self.state_history.read().await;
        let current_state = self.current_state.read().await;
        
        let mut state_counts = HashMap::new();
        for (state, _) in history.iter() {
            *state_counts.entry(state.status().to_string()).or_insert(0) += 1;
        }
        
        // Include current state
        *state_counts.entry(current_state.status().to_string()).or_insert(0) += 1;
        
        StateMachineStats {
            current_state: current_state.clone(),
            total_transitions: history.len(),
            state_counts,
        }
    }
}

#[derive(Debug)]
pub struct StateMachineStats {
    pub current_state: AgentState,
    pub total_transitions: usize,
    pub state_counts: HashMap<String, usize>,
}

/// Default implementation for AgentStateMachine
impl Default for AgentStateMachine {
    fn default() -> Self {
        Self::new()
    }
}
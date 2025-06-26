//! # Timer Event Handlers
//!
//! This module handles timer-expired events to enable agent resumption with context restoration.
//! Supports use cases like chess games, page monitoring, and user interruption recovery.
//!
//! ## Core Capabilities:
//! - Process timer-expired events with context validation
//! - Agent state detection and restart logic
//! - Edge case handling for concurrent execution scenarios
//! - Resource management and security validation

use crate::agent::tools::timer_tools::{TimerTask, TimerType};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use crate::constants::{events, errors::templates};

/// Agent system states for determining restart eligibility
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentSystemState {
    /// No agent activity, safe to start
    Idle,
    /// Agent is processing a user query
    ProcessingQuery,
    /// Agent is waiting for user input or interaction
    WaitingForUserInput,
    /// Agent is in error/recovery state
    ErrorState,
    /// System is shutting down
    ShuttingDown,
}

/// Strategies for handling timer expiration based on current system state
#[derive(Debug, Clone, PartialEq)]
pub enum TimerHandlingStrategy {
    /// Execute timer restart immediately
    ExecuteImmediately,
    /// Queue timer for later execution when agent becomes available
    QueueForLater,
    /// Interrupt current execution (high priority timers)
    InterruptCurrent,
    /// Queue with elevated priority
    QueueWithPriority,
    /// Evaluate context to determine priority
    EvaluateContext,
    /// Discard expired timer (too old or invalid)
    DiscardExpired,
}

/// Timer event handler configuration
#[derive(Debug, Clone)]
pub struct TimerEventConfig {
    /// Maximum size allowed for timer context (in bytes)
    pub max_context_size_bytes: usize,
    /// Maximum age of timer context before considered stale (in seconds)
    pub max_context_age_seconds: u64,
    /// Maximum number of concurrent timer processing operations
    pub max_concurrent_processing: u32,
    /// Enable comprehensive context validation
    pub enable_context_validation: bool,
    /// Enable security scanning of timer contexts
    pub enable_security_scanning: bool,
    /// Rate limit for timer processing (timers per minute)
    pub rate_limit_per_minute: u32,
}

impl Default for TimerEventConfig {
    fn default() -> Self {
        Self {
            max_context_size_bytes: 10 * 1024 * 1024, // 10MB
            max_context_age_seconds: 24 * 60 * 60,    // 24 hours
            max_concurrent_processing: 5,
            enable_context_validation: true,
            enable_security_scanning: true,
            rate_limit_per_minute: 20,
        }
    }
}

/// Timer event processing errors
#[derive(Debug, thiserror::Error)]
pub enum TimerEventError {
    #[error("Invalid timer context: {0}")]
    InvalidContext(String),

    #[error("Agent system unavailable: {0}")]
    AgentUnavailable(String),

    #[error("Context validation failed: {0}")]
    ContextValidation(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("Concurrent execution conflict: {0}")]
    ConcurrencyConflict(String),

    #[error("System state error: {0}")]
    SystemState(String),

    #[error("Timer expired or invalid: {0}")]
    TimerExpired(String),

    #[error("Context security violation: {0}")]
    SecurityViolation(String),
}

/// Recovery actions for handling corrupted or invalid contexts
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryAction {
    /// Attempt to repair common JSON/data issues
    AttemptRepair,
    /// Use minimal context with timer description only
    UseMinimalContext,
    /// Discard timer and log security alert
    DiscardAndAlert,
    /// Truncate context to essential data only
    TruncateContext,
    /// Retry with exponential backoff
    RetryWithBackoff,
}

/// Context validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum ContextValidationError {
    /// Invalid JSON structure
    InvalidJson,
    /// Missing required fields
    MissingRequired,
    /// Context size exceeds limits
    TooLarge,
    /// Context is too old/stale
    TooOld,
    /// Security violation detected
    SecurityViolation,
    /// Incompatible version
    IncompatibleVersion,
}

/// Timer event handler for processing timer-expired events
pub struct TimerEventHandler {
    app_handle: AppHandle,
    config: TimerEventConfig,
    processing_semaphore: Arc<tokio::sync::Semaphore>,
    pending_timers: Arc<Mutex<Vec<TimerTask>>>,
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

/// Simple rate limiter for timer processing
#[derive(Debug)]
struct RateLimiter {
    requests: Vec<SystemTime>,
    max_requests_per_minute: u32,
}

impl RateLimiter {
    fn new(max_requests_per_minute: u32) -> Self {
        Self {
            requests: Vec::new(),
            max_requests_per_minute,
        }
    }

    fn check_rate_limit(&mut self) -> bool {
        let now = SystemTime::now();
        let one_minute_ago = now - Duration::from_secs(60);

        // Remove old requests
        self.requests.retain(|&time| time > one_minute_ago);

        // Check if we can make another request
        if self.requests.len() < self.max_requests_per_minute as usize {
            self.requests.push(now);
            true
        } else {
            false
        }
    }
}

impl TimerEventHandler {
    /// Creates a new timer event handler with default configuration
    pub fn new(app_handle: AppHandle) -> Self {
        let config = TimerEventConfig::default();
        Self::with_config(app_handle, config)
    }

    /// Creates a new timer event handler with custom configuration
    pub fn with_config(app_handle: AppHandle, config: TimerEventConfig) -> Self {
        let processing_semaphore = Arc::new(tokio::sync::Semaphore::new(
            config.max_concurrent_processing as usize,
        ));
        let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(config.rate_limit_per_minute)));

        Self {
            app_handle,
            config,
            processing_semaphore,
            pending_timers: Arc::new(Mutex::new(Vec::new())),
            rate_limiter,
        }
    }

    /// Main handler for timer-expired events
    pub async fn handle_timer_expired(&self, timer_data: TimerTask) -> Result<(), TimerEventError> {
        info!(
            "Processing timer-expired event: {} - {}",
            timer_data.id, timer_data.description
        );

        // Check rate limit
        if !self.check_rate_limit().await {
            warn!(
                "Rate limit exceeded for timer processing, queuing timer: {}",
                timer_data.id
            );
            self.queue_timer_for_later(timer_data).await?;
            return Ok(());
        }

        // Acquire processing permit
        let permit = self.processing_semaphore.try_acquire().map_err(|_| {
            TimerEventError::ResourceLimit("Too many concurrent timer operations".to_string())
        })?;

        let result = self.process_timer_internal(timer_data).await;

        // Release permit
        drop(permit);

        result
    }

    /// Internal timer processing logic
    async fn process_timer_internal(&self, timer_data: TimerTask) -> Result<(), TimerEventError> {
        // Capture timer ID before potential move
        let timer_id = timer_data.id.clone();

        // 1. Validate timer context
        if self.config.enable_context_validation {
            self.validate_timer_context(&timer_data).await?;
        }

        // 2. Check agent system state
        let agent_state = self.detect_agent_system_state().await?;
        info!("Current agent state: {:?}", agent_state);

        // 3. Determine handling strategy
        let strategy = self
            .determine_handling_strategy(&timer_data, &agent_state)
            .await?;
        info!("Timer handling strategy: {:?}", strategy);

        // 4. Execute based on strategy
        match strategy {
            TimerHandlingStrategy::ExecuteImmediately => {
                self.restart_agent_with_context(&timer_data.context, &timer_data.description)
                    .await?;
            }
            TimerHandlingStrategy::QueueForLater | TimerHandlingStrategy::QueueWithPriority => {
                self.queue_timer_for_later(timer_data).await?;
            }
            TimerHandlingStrategy::InterruptCurrent => {
                self.interrupt_and_restart(&timer_data).await?;
            }
            TimerHandlingStrategy::EvaluateContext => {
                // Additional context evaluation logic would go here
                self.restart_agent_with_context(&timer_data.context, &timer_data.description)
                    .await?;
            }
            TimerHandlingStrategy::DiscardExpired => {
                warn!("Discarding expired/invalid timer: {}", timer_data.id);
                return Ok(());
            }
        }

        info!(
            "Successfully processed timer-expired event: {}",
            timer_id
        );
        Ok(())
    }

    /// Validate timer context for integrity and security
    async fn validate_timer_context(&self, timer_data: &TimerTask) -> Result<(), TimerEventError> {
        // Check context size
        let context_str = serde_json::to_string(&timer_data.context).map_err(|e| {
            TimerEventError::ContextValidation(format!("JSON serialization failed: {}", e))
        })?;

        if context_str.len() > self.config.max_context_size_bytes {
            return Err(TimerEventError::ContextValidation(format!(
                "Context size {} exceeds limit {}",
                context_str.len(),
                self.config.max_context_size_bytes
            )));
        }

        // Check context age
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| TimerEventError::SystemState(format!("System time error: {}", e)))?
            .as_secs();

        let context_age = now.saturating_sub(timer_data.created_at);
        if context_age > self.config.max_context_age_seconds {
            return Err(TimerEventError::ContextValidation(format!(
                "Context age {} seconds exceeds limit {}",
                context_age, self.config.max_context_age_seconds
            )));
        }

        // Security scanning if enabled
        if self.config.enable_security_scanning {
            self.scan_context_for_security_issues(&timer_data.context)
                .await?;
        }

        Ok(())
    }

    /// Scan context for potential security issues
    async fn scan_context_for_security_issues(
        &self,
        context: &Value,
    ) -> Result<(), TimerEventError> {
        let context_str = context.to_string().to_lowercase();

        // Basic security patterns to detect
        let dangerous_patterns = [
            "eval(",
            "exec(",
            "system(",
            "shell(",
            "__import__",
            "subprocess",
            "os.system",
            "rm -rf",
            "del /",
            "<script",
            "javascript:",
            "data:text/html",
        ];

        for pattern in &dangerous_patterns {
            if context_str.contains(pattern) {
                return Err(TimerEventError::SecurityViolation(format!(
                    "Dangerous pattern detected in context: {}",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Detect current agent system state
    async fn detect_agent_system_state(&self) -> Result<AgentSystemState, TimerEventError> {
        let app_state = self.app_handle.state::<AppState>();

        // Check if agent is currently processing
        if app_state.is_agent_executing() {
            return Ok(AgentSystemState::ProcessingQuery);
        }

        // For now, we'll assume no specific error state detection method exists
        // This could be extended with actual error state checking logic
        // Default to idle if no specific state detected
        Ok(AgentSystemState::Idle)
    }

    /// Determine the appropriate handling strategy for the timer
    async fn determine_handling_strategy(
        &self,
        timer_data: &TimerTask,
        agent_state: &AgentSystemState,
    ) -> Result<TimerHandlingStrategy, TimerEventError> {
        match agent_state {
            AgentSystemState::Idle => Ok(TimerHandlingStrategy::ExecuteImmediately),
            AgentSystemState::ProcessingQuery => {
                // Determine priority based on timer type
                match timer_data.timer_type {
                    TimerType::Simple => Ok(TimerHandlingStrategy::QueueForLater),
                    TimerType::ScreenMonitor { .. } => Ok(TimerHandlingStrategy::InterruptCurrent),
                    TimerType::FileMonitor { .. } => Ok(TimerHandlingStrategy::QueueWithPriority),
                    TimerType::ApplicationMonitor { .. } => {
                        Ok(TimerHandlingStrategy::EvaluateContext)
                    }
                }
            }
            AgentSystemState::WaitingForUserInput => Ok(TimerHandlingStrategy::ExecuteImmediately),
            AgentSystemState::ErrorState => Ok(TimerHandlingStrategy::QueueForLater),
            AgentSystemState::ShuttingDown => Ok(TimerHandlingStrategy::DiscardExpired),
        }
    }

    /// Restart agent with provided context
    async fn restart_agent_with_context(
        &self,
        context: &Value,
        description: &str,
    ) -> Result<(), TimerEventError> {
        info!("Restarting agent with timer context: {}", description);

        // Extract query from context or use description as fallback
        let query = context
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or(description);

        // Call the main agent submission function
        if let Err(e) = crate::anthropic::submit_query(
            query.to_string(),
            self.app_handle.state::<AppState>(),
            self.app_handle.clone(),
        )
        .await
        {
            return Err(TimerEventError::AgentUnavailable(format!(
                "Failed to start agent restart: {}",
                e
            )));
        }

        info!("Successfully restarted agent with timer context");
        Ok(())
    }

    /// Queue timer for later processing
    async fn queue_timer_for_later(&self, timer_data: TimerTask) -> Result<(), TimerEventError> {
        let mut pending = self.pending_timers.lock().await;
        pending.push(timer_data.clone());

        info!(
            "Queued timer for later processing: {} (total pending: {})",
            timer_data.id,
            pending.len()
        );

        // Emit event to notify about queued timer
        if let Err(e) = self.app_handle.emit(events::timer::QUEUED, &timer_data) {
            warn!("{}", format!("Failed to emit {}: {}", "timer-queued event", e));
        }

        Ok(())
    }

    /// Interrupt current execution and restart with timer context
    async fn interrupt_and_restart(&self, timer_data: &TimerTask) -> Result<(), TimerEventError> {
        info!(
            "Interrupting current execution for high-priority timer: {}",
            timer_data.id
        );

        // Signal cancellation to current agent
        let app_state = self.app_handle.state::<AppState>();
        app_state.signal_cancel();

        // Wait briefly for graceful shutdown
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Restart with timer context
        self.restart_agent_with_context(&timer_data.context, &timer_data.description)
            .await
    }

    /// Check rate limit for timer processing
    async fn check_rate_limit(&self) -> bool {
        let mut rate_limiter = self.rate_limiter.lock().await;
        rate_limiter.check_rate_limit()
    }

    /// Process any pending queued timers
    pub async fn process_queued_timers(&self) -> Result<usize, TimerEventError> {
        let mut pending = self.pending_timers.lock().await;
        let count = pending.len();

        if count == 0 {
            return Ok(0);
        }

        info!("Processing {} queued timers", count);

        // Take all pending timers
        let timers_to_process = pending.drain(..).collect::<Vec<_>>();
        drop(pending); // Release lock

        let mut processed = 0;
        let mut timers_iter = timers_to_process.into_iter().enumerate();

        for (_index, timer) in timers_iter.by_ref() {
            // Check if agent is still busy
            let agent_state = self.detect_agent_system_state().await?;
            if agent_state == AgentSystemState::ProcessingQuery {
                // Re-queue current timer and all remaining timers
                let mut pending = self.pending_timers.lock().await;
                pending.push(timer);

                // Re-queue all remaining timers that weren't processed
                for (_, remaining_timer) in timers_iter {
                    pending.push(remaining_timer);
                }

                info!("Agent became busy, re-queued {} unprocessed timers", pending.len());
                break;
            }

            // Process the timer
            if let Err(e) = self.process_timer_internal(timer.clone()).await {
                error!("Failed to process queued timer {}: {}", timer.id, e);
                continue;
            }

            processed += 1;

            // Add small delay between timer processing
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        info!("Processed {}/{} queued timers", processed, count);
        Ok(processed)
    }

    /// Get current configuration
    pub fn get_config(&self) -> &TimerEventConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: TimerEventConfig) {
        self.config = config;
    }
}

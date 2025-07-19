//! # Structured Error Types
//!
//! Provides comprehensive error handling with structured types
//! instead of string-based errors.

use thiserror::Error;
use std::io;
use tauri::Error as TauriError;

/// Main application error type
#[derive(Error, Debug)]
pub enum AppError {
    /// Monitor-related errors
    #[error("Monitor error: {0}")]
    Monitor(#[from] MonitorError),
    
    /// Agent-related errors
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),
    
    /// Dictation-related errors
    #[error("Dictation error: {0}")]
    Dictation(#[from] DictationError),
    
    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    /// Tauri errors
    #[error("Tauri error: {0}")]
    Tauri(#[from] TauriError),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// Permission errors
    #[error("Permission denied: {0}")]
    Permission(String),
    
    /// Network errors
    #[error("Network error: {0}")]
    Network(String),
    
    /// Generic errors
    #[error("{0}")]
    Generic(String),
}

/// Monitor-specific errors
#[derive(Error, Debug)]
pub enum MonitorError {
    #[error("Monitor already initialized")]
    AlreadyInitialized,
    
    #[error("Monitor not initialized")]
    NotInitialized,
    
    #[error("Monitor is already active")]
    AlreadyActive,
    
    #[error("Monitor is in cooldown period ({0}ms remaining)")]
    InCooldown(u64),
    
    #[error("Monitor state corruption detected")]
    StateCorruption,
    
    #[error("Monitor timeout after {0}ms")]
    Timeout(u64),
    
    #[error("Failed to send monitor event: {0}")]
    EventSendFailed(String),
}

/// Agent-specific errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum AgentError {
    #[error("Agent terminated by user")]
    Terminated,
    
    #[error("Maximum steps reached ({0})")]
    MaxStepsReached(u32),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Provider error: {0}")]
    ProviderError(String),
    
    #[error("Tool execution error: {0}")]
    ToolExecutionError(String),
    
    #[error("Invalid tool parameters: {0}")]
    InvalidParameters(String),
    
    #[error("Agent initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Memory error: {0}")]
    MemoryError(String),
}

/// Dictation-specific errors
#[derive(Error, Debug)]
pub enum DictationError {
    #[error("Voice controller not available")]
    ControllerNotAvailable,
    
    #[error("Dictation already active")]
    AlreadyActive,
    
    #[error("Dictation not active")]
    NotActive,
    
    #[error("Microphone access denied")]
    MicrophoneAccessDenied,
    
    #[error("Voice recognition failed: {0}")]
    RecognitionFailed(String),
    
    #[error("Transcription timeout")]
    TranscriptionTimeout,
    
    #[error("Plugin error: {0}")]
    PluginError(String),
}

/// Result type alias for AppError
pub type AppResult<T> = Result<T, AppError>;

/// Result type alias for MonitorError
pub type MonitorResult<T> = Result<T, MonitorError>;

/// Result type alias for AgentError
pub type AgentResult<T> = Result<T, AgentError>;

/// Result type alias for DictationError
pub type DictationResult<T> = Result<T, DictationError>;

/// Convert AppError to string for Tauri commands
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

/// Extension trait for converting various errors to AppError
pub trait IntoAppError<T> {
    fn into_app_error(self) -> Result<T, AppError>;
}

impl<T, E> IntoAppError<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_app_error(self) -> Result<T, AppError> {
        self.map_err(|e| AppError::Generic(e.to_string()))
    }
}

/// Error context extension for adding context to errors
pub trait ErrorContext<T> {
    fn context(self, msg: &str) -> Result<T, AppError>;
    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: Into<AppError>,
{
    fn context(self, msg: &str) -> Result<T, AppError> {
        self.map_err(|e| {
            let base_error = e.into();
            AppError::Generic(format!("{}: {}", msg, base_error))
        })
    }
    
    fn with_context<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            AppError::Generic(format!("{}: {}", f(), base_error))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_conversion() {
        let monitor_error = MonitorError::AlreadyInitialized;
        let app_error: AppError = monitor_error.into();
        assert!(matches!(app_error, AppError::Monitor(_)));
    }
    
    #[test]
    fn test_error_context() {
        let result: Result<(), io::Error> = Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        let with_context = result.context("Failed to read config");
        assert!(with_context.is_err());
        let error_msg = with_context.unwrap_err().to_string();
        assert!(error_msg.contains("Failed to read config"));
    }
}
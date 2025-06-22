//! # Error Message Constants
//!
//! Centralized error messages and patterns used throughout the application.
//! This helps maintain consistency and makes error handling more maintainable.

// Common error message patterns for string matching
pub mod patterns {
    // Permission-related error patterns
    pub const PERMISSION_DENIED: &str = "permission denied";
    pub const ACCESS_DENIED: &str = "access denied";
    pub const PERMISSION_REQUIRED: &str = "permission required";

    // Network-related error patterns
    pub const CONNECTION_REFUSED: &str = "connection refused";
    pub const CONNECTION_FAILED: &str = "connection failed";
    pub const NETWORK_UNREACHABLE: &str = "network unreachable";
    pub const TIMEOUT: &str = "timeout";
    pub const TIMED_OUT: &str = "timed out";

    // File system error patterns
    pub const NOT_FOUND: &str = "not found";
    pub const DOES_NOT_EXIST: &str = "does not exist";
    pub const FILE_NOT_FOUND: &str = "file not found";
    pub const DIRECTORY_NOT_FOUND: &str = "directory not found";

    // Application-specific error patterns
    pub const ELEMENT_NOT_FOUND: &str = "element not found";
    pub const PAGE_NOT_FOUND: &str = "page not found";
    pub const BROWSER_NOT_AVAILABLE: &str = "browser not available";
    pub const AGENT_EXECUTION_FAILED: &str = "agent execution failed";
}

// Standard error messages for user display
pub mod user_messages {
    // Permission error messages
    pub const ACCESSIBILITY_PERMISSION_REQUIRED: &str =
        "Accessibility permission is required for this operation";
    pub const PERMISSION_DENIED_GENERIC: &str =
        "Permission denied. Please check your system permissions.";

    // Network error messages
    pub const CONNECTION_TIMEOUT: &str =
        "The operation timed out. Please check your internet connection.";
    pub const SERVER_UNREACHABLE: &str = "Unable to connect to the server. Please try again later.";

    // File operation error messages
    pub const FILE_READ_ERROR: &str =
        "Unable to read the file. Please check if it exists and you have permission.";
    pub const FILE_WRITE_ERROR: &str =
        "Unable to write to the file. Please check permissions and disk space.";

    // Agent operation error messages
    pub const AGENT_TIMEOUT: &str = "The agent operation timed out. Please try again.";
    pub const AGENT_UNAVAILABLE: &str =
        "The AI agent is currently unavailable. Please try again later.";

    // Browser operation error messages
    pub const BROWSER_CONNECTION_FAILED: &str =
        "Failed to connect to the browser. Please ensure it's running.";
    pub const ELEMENT_INTERACTION_FAILED: &str = "Failed to interact with the specified element.";
}

// Technical error messages for logging/debugging
pub mod technical_messages {
    // System errors
    pub const FAILED_TO_ACQUIRE_LOCK: &str = "Failed to acquire mutex lock";
    pub const THREAD_SPAWN_FAILED: &str = "Failed to spawn background thread";
    pub const CHANNEL_SEND_FAILED: &str = "Failed to send message through channel";
    pub const CHANNEL_RECV_FAILED: &str = "Failed to receive message from channel";

    // Resource errors
    pub const MEMORY_ALLOCATION_FAILED: &str = "Failed to allocate memory";
    pub const DISK_SPACE_INSUFFICIENT: &str = "Insufficient disk space";
    pub const FILE_HANDLE_UNAVAILABLE: &str = "Unable to obtain file handle";

    // Network errors
    pub const TCP_CONNECTION_FAILED: &str = "TCP connection establishment failed";
    pub const HTTP_REQUEST_FAILED: &str = "HTTP request failed";
    pub const WEBSOCKET_CONNECTION_FAILED: &str = "WebSocket connection failed";

    // Agent-specific errors
    pub const TOOL_EXECUTION_FAILED: &str = "Tool execution failed";
    pub const CONTEXT_SWITCH_FAILED: &str = "Failed to switch agent context";
    pub const MEMORY_CORRUPTION_DETECTED: &str = "Agent memory corruption detected";
}

// Error codes for structured error handling
pub mod error_codes {
    // Permission-related error codes
    pub const ERR_PERMISSION_DENIED: &str = "ERR_PERMISSION_DENIED";
    pub const ERR_ACCESS_DENIED: &str = "ERR_ACCESS_DENIED";

    // Network-related error codes
    pub const ERR_CONNECTION_TIMEOUT: &str = "ERR_CONNECTION_TIMEOUT";
    pub const ERR_CONNECTION_REFUSED: &str = "ERR_CONNECTION_REFUSED";
    pub const ERR_NETWORK_UNREACHABLE: &str = "ERR_NETWORK_UNREACHABLE";

    // File system error codes
    pub const ERR_FILE_NOT_FOUND: &str = "ERR_FILE_NOT_FOUND";
    pub const ERR_DIRECTORY_NOT_FOUND: &str = "ERR_DIRECTORY_NOT_FOUND";
    pub const ERR_INSUFFICIENT_SPACE: &str = "ERR_INSUFFICIENT_SPACE";

    // Agent operation error codes
    pub const ERR_AGENT_TIMEOUT: &str = "ERR_AGENT_TIMEOUT";
    pub const ERR_TOOL_EXECUTION_FAILED: &str = "ERR_TOOL_EXECUTION_FAILED";
    pub const ERR_CONTEXT_SWITCH_FAILED: &str = "ERR_CONTEXT_SWITCH_FAILED";

    // Browser operation error codes
    pub const ERR_BROWSER_CONNECTION_FAILED: &str = "ERR_BROWSER_CONNECTION_FAILED";
    pub const ERR_ELEMENT_NOT_FOUND: &str = "ERR_ELEMENT_NOT_FOUND";
    pub const ERR_PAGE_LOAD_TIMEOUT: &str = "ERR_PAGE_LOAD_TIMEOUT";
}

// Recovery suggestions for different error types
pub mod recovery_suggestions {
    pub const PERMISSION_DENIED: &str =
        "Try running with elevated privileges or check system permissions";
    pub const CONNECTION_TIMEOUT: &str = "Check your internet connection and try again";
    pub const FILE_NOT_FOUND: &str = "Verify the file path and ensure the file exists";
    pub const BROWSER_CONNECTION_FAILED: &str =
        "Ensure the browser is running and restart if necessary";
    pub const AGENT_TIMEOUT: &str =
        "The operation may be complex - try breaking it into smaller steps";
}

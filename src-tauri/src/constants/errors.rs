//! # Error Constants
//!
//! Error codes and messages used throughout the application.

// Standard JSON-RPC error codes
pub mod codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR_START: i32 = -32099;
    pub const SERVER_ERROR_END: i32 = -32000;

    // Application-specific error codes
    pub const TOOL_EXECUTION_ERROR: i32 = -32000;
    pub const ELEMENT_NOT_FOUND: i32 = -32001;
    pub const CACHE_MISS: i32 = -32002;
    pub const UNSUPPORTED_PLATFORM: i32 = -32003;

    // macOS specific error codes
    pub const MACOS_AX_NO_VALUE: i32 = -25212;
    pub const MACOS_AX_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
    pub const MACOS_AX_GET_ATTRIBUTE_FAILED: i32 = -25204;

    // Enhanced orchestration error codes
    pub const ORCHESTRATION_TASK_FAILED: i32 = -32010;
    pub const ORCHESTRATION_BATCH_FAILED: i32 = -32011;
    pub const ORCHESTRATION_AGENT_UNAVAILABLE: i32 = -32012;
    pub const ORCHESTRATION_RESOURCE_EXHAUSTED: i32 = -32013;
    pub const ORCHESTRATION_CASCADING_FAILURE: i32 = -32014;
    pub const ORCHESTRATION_TIMEOUT: i32 = -32015;
    pub const ORCHESTRATION_INVALID_TASK: i32 = -32016;
    pub const ORCHESTRATION_DEPENDENCY_FAILED: i32 = -32017;
}

// Error messages
pub mod messages {
    pub const INVALID_PARAMS: &str = "invalid params";
    pub const METHOD_NOT_FOUND: &str = "method not found";
    pub const PARSE_ERROR: &str = "parse error";
    pub const ELEMENT_NOT_FOUND: &str = "element not found";
    pub const CACHE_MISS: &str = "cache miss";
    pub const UNSUPPORTED_OPERATION: &str = "unsupported operation";
    pub const UNSUPPORTED_PLATFORM: &str = "unsupported platform";
    pub const TOOL_EXECUTION_ERROR: &str = "tool execution error";

    // Enhanced orchestration error messages
    pub const ORCHESTRATION_TASK_FAILED: &str = "orchestration task failed";
    pub const ORCHESTRATION_BATCH_FAILED: &str = "orchestration batch execution failed";
    pub const ORCHESTRATION_AGENT_UNAVAILABLE: &str = "orchestration agent unavailable";
    pub const ORCHESTRATION_RESOURCE_EXHAUSTED: &str = "orchestration resource exhausted";
    pub const ORCHESTRATION_CASCADING_FAILURE: &str = "orchestration cascading failure detected";
    pub const ORCHESTRATION_TIMEOUT: &str = "orchestration timeout exceeded";
    pub const ORCHESTRATION_INVALID_TASK: &str = "orchestration invalid task configuration";
    pub const ORCHESTRATION_DEPENDENCY_FAILED: &str = "orchestration task dependency failed";
}

// Error recovery constants
pub mod recovery {
    // Recovery attempt delays
    pub const ELEMENT_NOT_FOUND_DELAY_MS: u64 = 1000;
    pub const NETWORK_ERROR_DELAY_MS: u64 = 2000;
    pub const TIMEOUT_RECOVERY_DELAY_MS: u64 = 5000;
    pub const RATE_LIMIT_BACKOFF_MS: u64 = 60000;
    pub const BROWSER_NOT_READY_DELAY_MS: u64 = 3000;

    // Default recovery configuration
    pub const DEFAULT_BASE_RETRY_DELAY_MS: u64 = 500;
    pub const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 10000;
    pub const DEFAULT_TIMEOUT_THRESHOLD_MS: u64 = 30000;

    // Backoff configuration
    pub const BACKOFF_MULTIPLIER: u32 = 2;
    pub const MAX_BACKOFF_EXPONENT: u32 = 5;

    // Enhanced orchestration recovery constants
    pub const ORCHESTRATION_TASK_RETRY_DELAY_MS: u64 = 1500;
    pub const ORCHESTRATION_BATCH_FAILURE_DELAY_MS: u64 = 3000;
    pub const ORCHESTRATION_AGENT_FAILURE_DELAY_MS: u64 = 2000;
    pub const ORCHESTRATION_RESOURCE_EXHAUSTION_DELAY_MS: u64 = 5000;
    pub const ORCHESTRATION_CASCADING_FAILURE_THRESHOLD: u32 = 3;

    // Orchestration performance thresholds
    pub const MIN_PARALLEL_FACTOR: f32 = 1.0;
    pub const TARGET_PARALLEL_FACTOR: f32 = 2.0;
    pub const OPTIMAL_PARALLEL_FACTOR: f32 = 3.0;
    pub const MAX_BATCH_RETRY_ATTEMPTS: u32 = 2;
}



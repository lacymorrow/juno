//! # Performance and System Constants
//!
//! Constants for performance limits, networking, and system resource management
//! to eliminate magic numbers throughout the performance-critical system.

/// Network and bandwidth constants
pub mod network {
    /// Default network bandwidth limit (1 Mbps)
    pub const DEFAULT_NETWORK_BANDWIDTH_KBPS: u32 = 1000;

    /// Low network bandwidth limit (100 Kbps)
    pub const LOW_NETWORK_BANDWIDTH_KBPS: u32 = 100;

    /// High network bandwidth limit (10 Mbps)
    pub const HIGH_NETWORK_BANDWIDTH_KBPS: u32 = 10000;

    /// Default response time limit (milliseconds)
    pub const DEFAULT_RESPONSE_TIME_MS: u32 = 1000;

    /// High performance response time limit (milliseconds)
    pub const HIGH_PERFORMANCE_RESPONSE_TIME_MS: u32 = 10000;

    /// Minimum throughput requirement
    pub const MIN_THROUGHPUT: f64 = 10.0;
}

/// Queue and processing constants
pub mod queues {
    /// Maximum queue size for orchestrator
    pub const MAX_ORCHESTRATOR_QUEUE_SIZE: usize = 100;

    /// Default queue processing interval (milliseconds)
    pub const DEFAULT_QUEUE_PROCESSING_INTERVAL_MS: u64 = 250;

    /// Orchestrator queue processing interval (milliseconds)
    pub const ORCHESTRATOR_QUEUE_PROCESSING_INTERVAL_MS: u64 = 500;

    /// Maximum task count for testing
    pub const MAX_TEST_TASK_COUNT: usize = 20;

    /// Minimum task count for testing
    pub const MIN_TEST_TASK_COUNT: usize = 1;
}

/// Timeout and retry constants
pub mod timeouts {
    /// Default operation timeout (seconds)
    pub const DEFAULT_OPERATION_TIMEOUT_SECONDS: u64 = 60;

    /// Extended operation timeout (seconds)
    pub const EXTENDED_OPERATION_TIMEOUT_SECONDS: u64 = 120;

    /// Speed benchmark timeout (milliseconds)
    pub const SPEED_BENCHMARK_TIMEOUT_MS: u64 = 10000;

    /// Long benchmark timeout (milliseconds)
    pub const LONG_BENCHMARK_TIMEOUT_MS: u64 = 600000; // 10 minutes

    /// Default computer action duration (milliseconds)
    pub const DEFAULT_COMPUTER_ACTION_DURATION_MS: u64 = 1000;

    /// Minimum stop coordinator delay (milliseconds)
    pub const MIN_STOP_COORDINATOR_DELAY_MS: u64 = 500;
}

/// Resource and memory limits
pub mod resources {
    /// Maximum file size for processing (bytes)
    pub const MAX_PROCESSING_FILE_SIZE_BYTES: usize = 100 * 1024 * 1024; // 100 MB

    /// Default memory limit (MB)
    pub const DEFAULT_MEMORY_LIMIT_MB: u32 = 512;

    /// High memory limit (MB)
    pub const HIGH_MEMORY_LIMIT_MB: u32 = 1024;

    /// Maximum error recovery checkpoints
    pub const MAX_ERROR_RECOVERY_CHECKPOINTS: usize = 10;

    /// Default retry delay (milliseconds)
    pub const DEFAULT_RETRY_DELAY_MS: u64 = 500;
}

/// Performance monitoring and metrics
pub mod metrics {
    /// CPU usage baseline percentage
    pub const BASELINE_CPU_USAGE_PERCENT: f64 = 25.0;

    /// Default baseline agent performance ratio
    pub const BASELINE_AGENT_PERFORMANCE_RATIO: f64 = 0.25; // 25% of human performance

    /// Maximum steps for testing scenarios
    pub const MAX_TESTING_STEPS_BASIC: u32 = 15;

    /// Maximum steps for complex testing scenarios
    pub const MAX_TESTING_STEPS_COMPLEX: u32 = 25;

    /// Violation calculation divisor
    pub const VIOLATION_CALCULATION_DIVISOR: usize = 10;

    /// MCP health check perfect score (no servers = 100% healthy)
    pub const MCP_PERFECT_HEALTH_SCORE: u32 = 100;
}

/// UI and window constants
pub mod ui {
    /// Floating panel height offset for positioning
    pub const FLOATING_PANEL_HEIGHT_OFFSET: f64 = 100.0;

    /// Default window minimum height
    pub const DEFAULT_WINDOW_MIN_HEIGHT: f64 = 300.0;

    /// Node.js maximum event listeners
    pub const NODEJS_MAX_EVENT_LISTENERS: &str = "20";
}

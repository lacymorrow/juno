//! # CLI Constants
//!
//! Constants specific to the command-line interface and headless operations.

/// CLI operation types
pub mod operation {
    pub const QUERY: &str = "query";
    pub const DICTATION: &str = "dictation";
    pub const AGENT_MODE: &str = "agent-mode";
    pub const VOICE_QUERY: &str = "voice-query";
    pub const STATUS: &str = "status";
    pub const ITERATE: &str = "iterate";
    pub const SELF_CALL: &str = "self-call";
    pub const DAEMON: &str = "daemon";
    pub const BATCH: &str = "batch";
    pub const INTERACTIVE: &str = "interactive";
}

/// CLI output formats
pub mod output {
    pub const JSON: &str = "json";
    pub const TEXT: &str = "text";
    pub const MARKDOWN: &str = "markdown";
    pub const QUIET: &str = "quiet";
}

/// CLI modes
pub mod mode {
    pub const HEADLESS: &str = "headless";
    pub const INTERACTIVE: &str = "interactive";
    pub const BATCH: &str = "batch";
    pub const DAEMON: &str = "daemon";
}

/// CLI exit codes
pub mod exit_code {
    pub const SUCCESS: i32 = 0;
    pub const GENERAL_ERROR: i32 = 1;
    pub const INVALID_ARGUMENTS: i32 = 2;
    pub const PERMISSION_ERROR: i32 = 3;
    pub const NETWORK_ERROR: i32 = 4;
    pub const AGENT_ERROR: i32 = 5;
    pub const VOICE_ERROR: i32 = 6;
}

/// CLI timeouts (in seconds)
pub mod timeout {
    pub const DEFAULT_QUERY_TIMEOUT: u64 = 300; // 5 minutes
    pub const VOICE_TIMEOUT: u64 = 60; // 1 minute
    pub const ITERATION_TIMEOUT: u64 = 600; // 10 minutes
    pub const STARTUP_TIMEOUT: u64 = 30; // 30 seconds
}

/// CLI verbosity levels
pub mod verbosity {
    pub const QUIET: u8 = 0;
    pub const NORMAL: u8 = 1;
    pub const VERBOSE: u8 = 2;
    pub const DEBUG: u8 = 3;
}

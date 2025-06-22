//! # CLI Module
//!
//! Comprehensive command-line interface for Juno AI Computer Use Agent
//! Supports both interactive and headless operation modes

use clap::{Parser, Subcommand, Args};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub mod runner;
pub mod headless;
// Note: interactive and daemon modules will be implemented in Phase 3
// pub mod interactive;
// pub mod daemon;

/// Main CLI structure for Juno AI Computer Use Agent
#[derive(Parser, Debug)]
#[command(
    name = "juno",
    version = env!("CARGO_PKG_VERSION"),
    about = "Juno AI Computer Use Agent - Intelligent desktop automation",
    long_about = "Juno AI Computer Use Agent provides intelligent desktop automation capabilities \
                  through natural language processing and computer vision. Can run in GUI, headless, \
                  or daemon modes."
)]
pub struct Cli {
    /// Global verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<std::path::PathBuf>,

    /// Run in headless mode (no GUI)
    #[arg(long)]
    pub headless: bool,

    /// Run as daemon (background service)
    #[arg(long)]
    pub daemon: bool,

    /// Environment mode (development, staging, production)
    #[arg(long, default_value = "production")]
    pub env: String,

    #[command(subcommand)]
    pub command: Option<Commands>,

    // Legacy compatibility flags
    #[arg(long, hide = true)]
    pub tts_provider: Option<String>,
    #[arg(long, hide = true)]
    pub tts_text: Option<String>,
    #[arg(long, hide = true)]
    pub test_focused_element_ns: bool,
    #[arg(long, hide = true)]
    pub check_accessibility: bool,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Execute a single agent query and exit
    Query {
        /// The query to execute
        #[arg(short, long)]
        text: String,

        /// Output format (text, json, xml)
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,

        /// Maximum execution time in seconds
        #[arg(long, default_value_t = 300)]
        timeout: u64,

        /// Save screenshot after execution
        #[arg(long)]
        screenshot: bool,

        /// Output file for results
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// Start interactive session
    Interactive {
        /// Session name
        #[arg(short, long)]
        name: Option<String>,

        /// Load previous session
        #[arg(long)]
        resume: Option<String>,
    },

    /// Run as daemon service
    Daemon {
        /// Service port
        #[arg(short, long, default_value_t = 8080)]
        port: u16,

        /// Bind address
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,

        /// API key for authentication
        #[arg(long)]
        api_key: Option<String>,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },

    /// Tool management
    Tools {
        #[command(subcommand)]
        action: ToolCommands,
    },

    /// Provider management
    Providers {
        #[command(subcommand)]
        action: ProviderCommands,
    },

    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionCommands,
    },

    /// System diagnostics
    Doctor {
        /// Run comprehensive health check
        #[arg(long)]
        full: bool,

        /// Check specific component
        #[arg(long)]
        component: Option<String>,
    },

    /// Legacy test commands (hidden)
    #[command(hide = true)]
    Test {
        #[command(subcommand)]
        test_type: TestCommands,
    },
}

/// Configuration subcommands
#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Configuration section to show
        #[arg(short, long)]
        section: Option<String>,
    },
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Get configuration value
    Get {
        /// Configuration key
        key: String,
    },
    /// Reset configuration to defaults
    Reset {
        /// Reset specific section only
        #[arg(short, long)]
        section: Option<String>,
        /// Confirm reset without prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Import configuration from file
    Import {
        /// Configuration file path
        file: std::path::PathBuf,
    },
    /// Export configuration to file
    Export {
        /// Output file path
        file: std::path::PathBuf,
        /// Export format
        #[arg(short, long, default_value = "json")]
        format: ConfigFormat,
    },
}

/// Tool management subcommands
#[derive(Subcommand, Debug)]
pub enum ToolCommands {
    /// List available tools
    List {
        /// Show only enabled tools
        #[arg(long)]
        enabled: bool,
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,
    },
    /// Enable tool
    Enable {
        /// Tool name
        name: String,
    },
    /// Disable tool
    Disable {
        /// Tool name
        name: String,
    },
    /// Show tool information
    Info {
        /// Tool name
        name: String,
    },
    /// Test tool functionality
    Test {
        /// Tool name
        name: String,
        /// Test input (JSON)
        #[arg(short, long)]
        input: Option<String>,
    },
}

/// Provider management subcommands
#[derive(Subcommand, Debug)]
pub enum ProviderCommands {
    /// List available providers
    List,
    /// Set active provider
    Set {
        /// Provider name
        name: String,
        /// Model name
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Test provider connection
    Test {
        /// Provider name
        name: String,
        /// Test query
        #[arg(short, long, default_value = "Hello, world!")]
        query: String,
    },
    /// Show provider status
    Status,
}

/// Session management subcommands
#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List saved sessions
    List,
    /// Save current session
    Save {
        /// Session name
        name: String,
    },
    /// Load saved session
    Load {
        /// Session name
        name: String,
    },
    /// Delete saved session
    Delete {
        /// Session name
        name: String,
        /// Confirm deletion without prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Clear all session data
    Clear {
        /// Confirm clear without prompt
        #[arg(short, long)]
        yes: bool,
    },
}

/// Legacy test commands (hidden)
#[derive(Subcommand, Debug)]
pub enum TestCommands {
    Tts {
        provider: String,
        #[arg(short, long)]
        text: Option<String>,
    },
    Accessibility,
    FocusedElement,
}

/// Output format options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
    Xml,
    Yaml,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            "xml" => Ok(OutputFormat::Xml),
            "yaml" => Ok(OutputFormat::Yaml),
            _ => Err(format!("Invalid output format: {}", s)),
        }
    }
}

/// Configuration format options
#[derive(Debug, Clone)]
pub enum ConfigFormat {
    Json,
    Yaml,
    Toml,
}

impl std::str::FromStr for ConfigFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ConfigFormat::Json),
            "yaml" => Ok(ConfigFormat::Yaml),
            "toml" => Ok(ConfigFormat::Toml),
            _ => Err(format!("Invalid config format: {}", s)),
        }
    }
}

/// CLI execution result
#[derive(Debug, Serialize)]
pub struct CliResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub execution_time: std::time::Duration,
}

impl CliResult {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
            execution_time: std::time::Duration::from_secs(0),
        }
    }

    pub fn success_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            execution_time: std::time::Duration::from_secs(0),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            execution_time: std::time::Duration::from_secs(0),
        }
    }

    pub fn with_execution_time(mut self, duration: std::time::Duration) -> Self {
        self.execution_time = duration;
        self
    }
}

/// Headless runtime configuration
#[derive(Debug, Clone)]
pub struct HeadlessConfig {
    pub max_execution_time: Duration,
    pub enable_screenshots: bool,
    pub output_format: OutputFormat,
    pub verbose: bool,
    pub save_session: bool,
}

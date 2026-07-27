use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

pub mod headless;
pub mod runner;

// Default constants
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_MAX_ITERATIONS: u32 = 10;
const DEFAULT_BATCH_PARALLELISM: u32 = 4;
const DEFAULT_VOICE_TIMEOUT: u32 = 60;

#[derive(Parser, Debug)]
#[command(
    name = "juno",
    author,
    version,
    about = "Juno AI Computer Use Agent - Anthropic Computer Use implementation with headless capabilities",
    long_about = "Juno AI Computer Use Agent provides both GUI and headless command-line interfaces for AI-powered computer automation using Anthropic's Computer Use API."
)]
pub struct Cli {
    /// Global verbose flag for detailed output
    #[arg(short, long, global = true, help = "Enable verbose output")]
    pub verbose: bool,

    /// Global quiet flag to suppress non-essential output
    #[arg(
        short,
        long,
        global = true,
        help = "Suppress non-essential output",
        conflicts_with = "verbose"
    )]
    pub quiet: bool,

    /// Output format for results
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text, help = "Output format for results")]
    pub output: OutputFormat,

    /// Timeout for operations in seconds
    #[arg(long, global = true, default_value_t = DEFAULT_TIMEOUT_SECONDS, help = "Timeout for operations in seconds")]
    pub timeout: u64,

    /// Configuration file path (optional)
    #[arg(long, global = true, help = "Path to configuration file")]
    pub config: Option<String>,

    /// Enable headless mode (no GUI)
    #[arg(long, global = true, help = "Run in headless mode without GUI")]
    pub headless: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,

    // Legacy CLI flags for backward compatibility
    /// Optional: Run a test to get focused element info using NSWorkspace (macOS only)
    #[arg(long, help = "Run focused element test (macOS only)")]
    pub test_focused_element_ns: bool,

    /// Optional: Run the accessibility check (macOS only)
    #[arg(long, help = "Run accessibility permissions check (macOS only)")]
    pub check_accessibility: bool,

    /// Optional: Specify the TTS provider to test (system, elevenlabs, replicate)
    #[arg(long, requires = "tts_text", help = "TTS provider to test")]
    pub tts_provider: Option<String>,

    /// Optional: Text to speak for TTS test (defaults to a standard phrase)
    #[arg(long, help = "Text to speak for TTS test")]
    pub tts_text: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Execute AI agent query
    #[command(about = "Submit a query to the AI agent")]
    Query {
        /// The query text to send to the agent
        #[arg(help = "Query text to send to the AI agent")]
        text: String,

        /// Maximum number of iterations for the agent
        #[arg(short, long, default_value_t = DEFAULT_MAX_ITERATIONS, help = "Maximum iterations")]
        max_iterations: u32,

        /// Continue previous conversation
        #[arg(short, long, help = "Continue from previous conversation context")]
        continue_conversation: bool,

        /// Save conversation to file
        #[arg(long, help = "Save conversation transcript to file")]
        save_transcript: Option<String>,

        /// Model to use for the query
        #[arg(long, help = "AI model to use (e.g., claude-opus-5)")]
        model: Option<String>,

        /// Provider to use for the query
        #[arg(long, help = "AI provider to use (anthropic, openai, etc.)")]
        provider: Option<String>,
    },

    /// Voice-related operations
    #[command(about = "Voice input and transcription operations")]
    Voice {
        #[command(subcommand)]
        command: VoiceCommands,
    },

    /// Dictation operations
    #[command(about = "Dictation and text input operations")]
    Dictation {
        #[command(subcommand)]
        command: DictationCommands,
    },

    /// Agent management operations
    #[command(about = "Agent status and management")]
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },

    /// Configuration management
    #[command(about = "Configuration management")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// MCP management
    #[command(about = "Manage MCP servers")]
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// System and diagnostic operations
    #[command(about = "System information and diagnostics")]
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },

    /// Batch operations
    #[command(about = "Execute batch operations from file")]
    Batch {
        /// Path to batch file containing commands
        #[arg(help = "Path to batch file")]
        file: String,

        /// Continue on errors
        #[arg(long, help = "Continue executing even if some commands fail")]
        continue_on_error: bool,

        /// Maximum parallel operations
        #[arg(long, default_value_t = DEFAULT_BATCH_PARALLELISM, help = "Maximum parallel operations")]
        parallelism: u32,
    },

    /// Interactive CLI mode
    #[command(about = "Start interactive CLI session")]
    Interactive {
        /// Initial prompt or greeting
        #[arg(long, help = "Initial prompt to display")]
        prompt: Option<String>,

        /// History file path
        #[arg(long, help = "Path to command history file")]
        history: Option<String>,
    },

    /// Daemon mode operations
    #[command(about = "Daemon mode for background operations")]
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },

    /// Testing and validation
    #[command(about = "Run tests and validation")]
    Test {
        #[command(subcommand)]
        command: TestCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum VoiceCommands {
    /// Start voice recording
    #[command(about = "Start voice recording session")]
    Record {
        /// Duration in seconds (0 for manual stop)
        #[arg(
            short,
            long,
            default_value_t = 0,
            help = "Recording duration in seconds"
        )]
        duration: u32,

        /// Output file path
        #[arg(short = 'O', long = "output-file", help = "Output audio file path")]
        output_file: Option<String>,

        /// Audio format
        #[arg(long, value_enum, default_value_t = AudioFormat::Wav, help = "Audio format")]
        format: AudioFormat,
    },

    /// Transcribe audio file or input
    #[command(about = "Transcribe audio to text")]
    Transcribe {
        /// Audio file path (optional, uses microphone if not provided)
        #[arg(help = "Audio file to transcribe")]
        file: Option<String>,

        /// Language code (e.g., en, es, fr)
        #[arg(short, long, help = "Language code for transcription")]
        language: Option<String>,

        /// Transcription model
        #[arg(long, help = "Transcription model to use")]
        model: Option<String>,
    },

    /// Query using voice input
    #[command(about = "Submit query using voice input")]
    Query {
        /// Maximum recording duration
        #[arg(short, long, default_value_t = DEFAULT_VOICE_TIMEOUT, help = "Maximum recording duration")]
        duration: u32,

        /// Continue previous conversation
        #[arg(short, long, help = "Continue from previous conversation")]
        continue_conversation: bool,

        /// Language for transcription
        #[arg(short, long, help = "Language code for transcription")]
        language: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DictationCommands {
    /// Start dictation mode
    #[command(about = "Start dictation mode")]
    Start {
        /// Target application or field
        #[arg(short, long, help = "Target application for dictation")]
        target: Option<String>,

        /// Language for dictation
        #[arg(short, long, help = "Language code for dictation")]
        language: Option<String>,

        /// Auto-punctuation
        #[arg(long, help = "Enable automatic punctuation")]
        auto_punctuation: bool,
    },

    /// Stop dictation mode
    #[command(about = "Stop active dictation")]
    Stop,

    /// Get dictation status
    #[command(about = "Get current dictation status")]
    Status,

    /// Configure dictation settings
    #[command(about = "Configure dictation settings")]
    Configure {
        /// Sensitivity level (1-10)
        #[arg(long, help = "Voice sensitivity level (1-10)")]
        sensitivity: Option<u8>,

        /// Enable clipboard integration
        #[arg(long, help = "Enable clipboard integration")]
        clipboard: Option<bool>,

        /// Dictation timeout in seconds
        #[arg(long, help = "Dictation timeout in seconds")]
        timeout: Option<u64>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// Get agent status
    #[command(about = "Get current agent status")]
    Status,

    /// Stop all agent operations
    #[command(about = "Stop all running agent operations")]
    Stop {
        /// Force stop without confirmation
        #[arg(short, long, help = "Force stop without confirmation")]
        force: bool,
    },

    /// Get agent capabilities
    #[command(about = "List agent capabilities and tools")]
    Capabilities {
        /// Show detailed tool information
        #[arg(short, long, help = "Show detailed tool information")]
        detailed: bool,

        /// Filter by category
        #[arg(short, long, help = "Filter tools by category")]
        category: Option<String>,
    },

    /// Manage agent iterations
    #[command(about = "Manage agent iteration settings")]
    Iterations {
        /// Set maximum iterations
        #[arg(short, long, help = "Set maximum iterations")]
        max: Option<u32>,

        /// Get current iteration count
        #[arg(long, help = "Show current iteration count")]
        current: bool,

        /// Reset iteration counter
        #[arg(long, help = "Reset iteration counter")]
        reset: bool,
    },

    /// Agent self-awareness operations
    #[command(about = "Agent self-awareness and introspection")]
    SelfAwareness {
        /// Show agent information
        #[arg(long, help = "Show agent self-information")]
        show: bool,

        /// Execute self-diagnostic
        #[arg(long, help = "Run agent self-diagnostic")]
        diagnostic: bool,

        /// Self-build command
        #[arg(long, help = "Attempt to build itself")]
        build: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    #[command(about = "Display current configuration")]
    Show {
        /// Configuration section to show
        #[arg(help = "Configuration section (providers, tools, shortcuts, etc.)")]
        section: Option<String>,

        /// Show sensitive values (API keys, etc.)
        #[arg(long, help = "Show sensitive configuration values")]
        show_sensitive: bool,
    },

    /// Set configuration value
    #[command(about = "Set configuration value")]
    Set {
        /// Configuration key
        #[arg(help = "Configuration key (e.g., provider.anthropic.api_key)")]
        key: String,

        /// Configuration value
        #[arg(help = "Configuration value")]
        value: String,

        /// Don't save to persistent storage
        #[arg(long, help = "Don't save to persistent storage")]
        no_save: bool,
    },

    /// Get configuration value
    #[command(about = "Get configuration value")]
    Get {
        /// Configuration key
        #[arg(help = "Configuration key")]
        key: String,
    },

    /// Reset configuration
    #[command(about = "Reset configuration to defaults")]
    Reset {
        /// Configuration section to reset
        #[arg(help = "Configuration section to reset")]
        section: Option<String>,

        /// Force reset without confirmation
        #[arg(short, long, help = "Force reset without confirmation")]
        force: bool,
    },

    /// Export configuration
    #[command(about = "Export configuration to file")]
    Export {
        /// Output file path
        #[arg(help = "Output file path")]
        file: String,

        /// Include sensitive values
        #[arg(long, help = "Include sensitive values in export")]
        include_sensitive: bool,
    },

    /// Import configuration
    #[command(about = "Import configuration from file")]
    Import {
        /// Input file path
        #[arg(help = "Input file path")]
        file: String,

        /// Merge with existing configuration
        #[arg(short, long, help = "Merge with existing configuration")]
        merge: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Add an MCP server (HTTP transport)
    #[command(about = "Add an MCP server (HTTP JSON-RPC)")]
    AddServer {
        /// Server name
        #[arg(long, help = "Server name")]
        name: String,

        /// HTTP endpoint URL
        #[arg(long, help = "HTTP JSON-RPC endpoint URL")]
        http_url: String,

        /// Enable the server
        #[arg(long, default_value_t = true, help = "Enable server")]
        enabled: bool,

        /// Auto-start the server
        #[arg(long, default_value_t = true, help = "Auto-start server")]
        auto_start: bool,

        /// Timeout seconds
        #[arg(long, default_value_t = 30, help = "Timeout seconds")]
        timeout: u64,
    },

    /// Start Juno as an MCP server (stdio JSON-RPC)
    #[command(about = "Run Juno as an MCP server over stdin/stdout")]
    Serve,
}

#[derive(Subcommand, Debug)]
pub enum SystemCommands {
    /// Show system information
    #[command(about = "Display system information")]
    Info {
        /// Include hardware details
        #[arg(long, help = "Include hardware information")]
        hardware: bool,

        /// Include permissions status
        #[arg(long, help = "Include permissions status")]
        permissions: bool,

        /// Include performance metrics
        #[arg(long, help = "Include performance metrics")]
        performance: bool,
    },

    /// Check system health
    #[command(about = "Run system health check")]
    Health {
        /// Include detailed diagnostics
        #[arg(short, long, help = "Include detailed diagnostics")]
        detailed: bool,

        /// Run performance tests
        #[arg(long, help = "Run performance benchmarks")]
        benchmark: bool,
    },

    /// Manage permissions
    #[command(about = "Manage system permissions")]
    Permissions {
        /// Check permission status
        #[arg(long, help = "Check all permission statuses")]
        check: bool,

        /// Request missing permissions
        #[arg(long, help = "Request missing permissions")]
        request: bool,

        /// Open system preferences
        #[arg(long, help = "Open system preferences")]
        open_settings: bool,
    },

    /// Performance monitoring
    #[command(about = "Performance monitoring and metrics")]
    Performance {
        /// Start performance monitoring
        #[arg(long, help = "Start performance monitoring")]
        start: bool,

        /// Stop performance monitoring
        #[arg(long, help = "Stop performance monitoring")]
        stop: bool,

        /// Show current metrics
        #[arg(long, help = "Show current performance metrics")]
        show: bool,

        /// Reset metrics
        #[arg(long, help = "Reset performance metrics")]
        reset: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DaemonCommands {
    /// Start daemon mode
    #[command(about = "Start daemon mode")]
    Start {
        /// Daemon configuration file
        #[arg(short, long, help = "Daemon configuration file")]
        config: Option<String>,

        /// Run in foreground
        #[arg(short, long, help = "Run in foreground mode")]
        foreground: bool,

        /// PID file path
        #[arg(long, help = "PID file path")]
        pid_file: Option<String>,
    },

    /// Stop daemon
    #[command(about = "Stop running daemon")]
    Stop {
        /// Force stop
        #[arg(short, long, help = "Force stop daemon")]
        force: bool,

        /// PID file path
        #[arg(long, help = "PID file path")]
        pid_file: Option<String>,
    },

    /// Get daemon status
    #[command(about = "Get daemon status")]
    Status {
        /// PID file path
        #[arg(long, help = "PID file path")]
        pid_file: Option<String>,
    },

    /// Restart daemon
    #[command(about = "Restart daemon")]
    Restart {
        /// Daemon configuration file
        #[arg(short, long, help = "Daemon configuration file")]
        config: Option<String>,

        /// PID file path
        #[arg(long, help = "PID file path")]
        pid_file: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TestCommands {
    /// Run accessibility tests
    #[command(about = "Test accessibility permissions and functionality")]
    Accessibility,

    /// Run TTS tests
    #[command(about = "Test text-to-speech functionality")]
    Tts {
        /// TTS provider to test
        #[arg(help = "TTS provider (system, elevenlabs, replicate)")]
        provider: Option<String>,

        /// Text to speak
        #[arg(help = "Text to speak for test")]
        text: Option<String>,
    },

    /// Run voice tests
    #[command(about = "Test voice recording and transcription")]
    Voice {
        /// Test duration in seconds
        #[arg(short, long, default_value_t = 5, help = "Test duration in seconds")]
        duration: u32,

        /// Language for transcription test
        #[arg(short, long, help = "Language for transcription test")]
        language: Option<String>,
    },

    /// Run system tests
    #[command(about = "Run comprehensive system tests")]
    System {
        /// Include performance tests
        #[arg(long, help = "Include performance benchmarks")]
        performance: bool,

        /// Test specific component
        #[arg(short, long, help = "Test specific component")]
        component: Option<String>,
    },

    /// Run all tests
    #[command(about = "Run all available tests")]
    All {
        /// Stop on first failure
        #[arg(long, help = "Stop on first test failure")]
        fail_fast: bool,

        /// Generate detailed report
        #[arg(long, help = "Generate detailed test report")]
        report: bool,
    },
}

#[derive(ValueEnum, Clone, Debug, Default, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Text,
    /// JSON structured output
    Json,
    /// Markdown formatted output
    Markdown,
    /// Minimal/quiet output
    Quiet,
    /// YAML structured output
    Yaml,
    /// Table formatted output
    Table,
}

#[derive(ValueEnum, Clone, Debug, Default, Serialize, Deserialize)]
pub enum AudioFormat {
    /// WAV audio format
    #[default]
    Wav,
    /// MP3 audio format
    Mp3,
    /// M4A audio format
    M4a,
    /// FLAC audio format
    Flac,
}

#[derive(ValueEnum, Clone, Debug, Default, Serialize, Deserialize)]
pub enum VerbosityLevel {
    /// Minimal output
    Quiet,
    /// Normal output
    #[default]
    Normal,
    /// Verbose output
    Verbose,
    /// Debug level output
    Debug,
    /// Trace level output
    Trace,
}

// Utility functions for CLI
impl Cli {
    /// Get the effective verbosity level based on flags
    pub fn get_verbosity_level(&self) -> VerbosityLevel {
        if self.quiet {
            VerbosityLevel::Quiet
        } else if self.verbose {
            VerbosityLevel::Verbose
        } else {
            VerbosityLevel::Normal
        }
    }

    /// Check if headless mode is enabled
    pub fn is_headless(&self) -> bool {
        self.headless || self.command.is_some()
    }

    /// Check if headless mode is required (either explicitly set or legacy flags present)
    pub fn is_headless_required(&self) -> bool {
        self.headless || self.has_legacy_flags() || self.command.is_some()
    }

    /// Check if any legacy CLI flags are used
    pub fn has_legacy_flags(&self) -> bool {
        self.test_focused_element_ns
            || self.check_accessibility
            || self.tts_provider.is_some()
            || self.tts_text.is_some()
    }

    /// Get timeout as Duration
    pub fn get_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout)
    }
}

impl OutputFormat {
    /// Get file extension for the output format
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Text => "txt",
            OutputFormat::Json => "json",
            OutputFormat::Markdown => "md",
            OutputFormat::Quiet => "txt",
            OutputFormat::Yaml => "yaml",
            OutputFormat::Table => "txt",
        }
    }

    /// Get MIME type for the output format
    pub fn mime_type(&self) -> &'static str {
        match self {
            OutputFormat::Text => "text/plain",
            OutputFormat::Json => "application/json",
            OutputFormat::Markdown => "text/markdown",
            OutputFormat::Quiet => "text/plain",
            OutputFormat::Yaml => "application/x-yaml",
            OutputFormat::Table => "text/plain",
        }
    }

    /// Get string representation of the output format
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
            OutputFormat::Markdown => "markdown",
            OutputFormat::Quiet => "quiet",
            OutputFormat::Yaml => "yaml",
            OutputFormat::Table => "table",
        }
    }
}

impl AudioFormat {
    /// Get file extension for the audio format
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4a => "m4a",
            AudioFormat::Flac => "flac",
        }
    }

    /// Get MIME type for the audio format
    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::M4a => "audio/mp4",
            AudioFormat::Flac => "audio/flac",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert()
    }

    #[test]
    fn test_parse_basic_query() {
        let args = vec!["juno", "query", "Hello world"];
        let cli = Cli::parse_from(args);

        if let Some(Commands::Query { text, .. }) = cli.command {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Query command");
        }
    }

    #[test]
    fn test_parse_headless_flag() {
        let args = vec!["juno", "--headless", "query", "test"];
        let cli = Cli::parse_from(args);

        assert!(cli.headless);
        assert!(cli.is_headless());
    }

    #[test]
    fn test_parse_output_format() {
        let args = vec!["juno", "--output", "json", "query", "test"];
        let cli = Cli::parse_from(args);

        assert!(matches!(cli.output, OutputFormat::Json));
    }

    #[test]
    fn test_parse_verbose_flags() {
        let verbose_args = vec!["juno", "--verbose", "query", "test"];
        let cli_verbose = Cli::parse_from(verbose_args);
        assert!(matches!(
            cli_verbose.get_verbosity_level(),
            VerbosityLevel::Verbose
        ));

        let quiet_args = vec!["juno", "--quiet", "query", "test"];
        let cli_quiet = Cli::parse_from(quiet_args);
        assert!(matches!(
            cli_quiet.get_verbosity_level(),
            VerbosityLevel::Quiet
        ));
    }

    #[test]
    fn test_parse_legacy_tts_args() {
        let args = vec![
            "juno",
            "--tts-provider",
            "system",
            "--tts-text",
            "Hello world",
        ];
        let cli = Cli::parse_from(args);

        assert_eq!(cli.tts_provider, Some("system".to_string()));
        assert_eq!(cli.tts_text, Some("Hello world".to_string()));
        assert!(cli.has_legacy_flags());
    }

    #[test]
    fn test_parse_voice_record_command() {
        let args = vec![
            "juno",
            "voice",
            "record",
            "--duration",
            "10",
            "--format",
            "mp3",
        ];
        let cli = Cli::parse_from(args);

        if let Some(Commands::Voice {
            command: VoiceCommands::Record {
                duration, format, ..
            },
        }) = cli.command
        {
            assert_eq!(duration, 10);
            assert!(matches!(format, AudioFormat::Mp3));
        } else {
            panic!("Expected Voice Record command");
        }
    }

    #[test]
    fn test_parse_agent_status_command() {
        let args = vec!["juno", "agent", "status"];
        let cli = Cli::parse_from(args);

        if let Some(Commands::Agent {
            command: AgentCommands::Status,
        }) = cli.command
        {
            // Success
        } else {
            panic!("Expected Agent Status command");
        }
    }

    #[test]
    fn test_parse_config_show_command() {
        let args = vec!["juno", "config", "show", "providers"];
        let cli = Cli::parse_from(args);

        if let Some(Commands::Config {
            command: ConfigCommands::Show { section, .. },
        }) = cli.command
        {
            assert_eq!(section, Some("providers".to_string()));
        } else {
            panic!("Expected Config Show command");
        }
    }

    #[test]
    fn test_parse_batch_command() {
        let args = vec!["juno", "batch", "commands.txt", "--continue-on-error"];
        let cli = Cli::parse_from(args);

        if let Some(Commands::Batch {
            file,
            continue_on_error,
            ..
        }) = cli.command
        {
            assert_eq!(file, "commands.txt");
            assert!(continue_on_error);
        } else {
            panic!("Expected Batch command");
        }
    }

    #[test]
    fn test_output_format_extensions() {
        assert_eq!(OutputFormat::Json.extension(), "json");
        assert_eq!(OutputFormat::Markdown.extension(), "md");
        assert_eq!(OutputFormat::Yaml.extension(), "yaml");
    }

    #[test]
    fn test_audio_format_mime_types() {
        assert_eq!(AudioFormat::Wav.mime_type(), "audio/wav");
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
        assert_eq!(AudioFormat::M4a.mime_type(), "audio/mp4");
    }

    #[test]
    fn test_timeout_conversion() {
        let args = vec!["juno", "--timeout", "30", "query", "test"];
        let cli = Cli::parse_from(args);

        assert_eq!(cli.timeout, 30);
        assert_eq!(cli.get_timeout(), std::time::Duration::from_secs(30));
    }

    #[test]
    fn test_conflicts_verbose_quiet() {
        let args = vec!["juno", "--verbose", "--quiet", "query", "test"];
        let result = Cli::try_parse_from(args);

        // Should fail due to conflicts_with constraint
        assert!(result.is_err());
    }
}

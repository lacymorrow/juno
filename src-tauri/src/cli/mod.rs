use clap::Parser;

pub mod runner;
pub mod headless;

pub use headless::{HeadlessRuntime, HeadlessResult, OutputFormat};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    /// Run in headless mode without UI
    #[arg(long, global = true)]
    pub(crate) headless: bool,

    /// Output format (json, text, markdown, quiet)
    #[arg(long, global = true, default_value = "text")]
    pub(crate) output: String,

    /// Verbosity level (0-3)
    #[arg(short, long, global = true, default_value = "1")]
    pub(crate) verbose: u8,

    /// Timeout for operations in seconds
    #[arg(long, global = true)]
    pub(crate) timeout: Option<u64>,

    // === CORE AGENT COMMANDS ===
    /// Submit a text query to the agent
    #[arg(long, value_name = "TEXT")]
    pub(crate) query: Option<String>,

    /// Start dictation mode and type the result
    #[arg(long)]
    pub(crate) dictation: bool,

    /// Start agent mode with voice input
    #[arg(long)]
    pub(crate) agent_mode: bool,

    /// Submit a voice query (combines voice input + agent processing)
    #[arg(long)]
    pub(crate) voice_query: bool,

    /// Show agent status
    #[arg(long)]
    pub(crate) status: bool,

    // === AGENT ITERATION COMMANDS ===
    /// Number of iterations for agent self-improvement
    #[arg(long, value_name = "COUNT")]
    pub(crate) iterate: Option<u32>,

    /// Allow agent to call itself with new queries
    #[arg(long)]
    pub(crate) self_call: bool,

    /// Context for agent iterations
    #[arg(long, value_name = "TEXT")]
    pub(crate) context: Option<String>,

    /// Maximum depth for recursive agent calls
    #[arg(long, value_name = "DEPTH", default_value = "3")]
    pub(crate) max_depth: u32,

    // === OPERATIONAL MODES ===
    /// Run in daemon mode (continuous operation)
    #[arg(long)]
    pub(crate) daemon: bool,

    /// Run in batch mode with commands from file
    #[arg(long, value_name = "FILE")]
    pub(crate) batch: Option<String>,

    /// Run in interactive mode
    #[arg(long)]
    pub(crate) interactive: bool,

    // === VOICE AND DICTATION OPTIONS ===
    /// Voice input timeout in seconds
    #[arg(long, value_name = "SECONDS", default_value = "60")]
    pub(crate) voice_timeout: u64,

    /// Enable voice debugging
    #[arg(long)]
    pub(crate) voice_debug: bool,

    /// Skip TTS output (text only)
    #[arg(long)]
    pub(crate) no_tts: bool,

    /// Force system TTS (bypass provider settings)
    #[arg(long)]
    pub(crate) system_tts: bool,

    // === EXISTING COMMANDS (kept for compatibility) ===
    /// Optional: Run a test to get focused element info using NSWorkspace (macOS only)
    #[arg(long)]
    pub(crate) test_focused_element_ns: bool,

    /// Optional: Run the accessibility check (macOS only)
    #[arg(long)]
    pub(crate) check_accessibility: bool,

    /// Optional: Specify the TTS provider to test (system, elevenlabs, replicate)
    #[arg(long, requires = "tts_text")]
    pub(crate) tts_provider: Option<String>,

    /// Optional: Text to speak for TTS test (defaults to a standard phrase)
    #[arg(long)]
    pub(crate) tts_text: Option<String>,

    // === SELF-IMPROVEMENT COMMANDS (Development Mode Only) ===
    /// Initialize the self-improvement system
    #[arg(long)]
    pub(crate) self_improvement_init: bool,

    /// Start an improvement cycle
    #[arg(long)]
    pub(crate) self_improvement_start: bool,

    /// Get current self-improvement status
    #[arg(long)]
    pub(crate) self_improvement_status: bool,

    /// Analyze system performance
    #[arg(long)]
    pub(crate) self_improvement_analyze: bool,

    /// Get improvement archive/history
    #[arg(long)]
    pub(crate) self_improvement_archive: bool,

    /// Get details for a specific iteration
    #[arg(long, value_name = "ITERATION_ID")]
    pub(crate) self_improvement_iteration: Option<String>,

    /// Update self-improvement configuration
    #[arg(long, value_name = "CONFIG_JSON")]
    pub(crate) self_improvement_config: Option<String>,

    /// Emergency stop improvement cycle
    #[arg(long)]
    pub(crate) self_improvement_stop: bool,

    /// Generate improvement proposal
    #[arg(long)]
    pub(crate) self_improvement_proposal: bool,

    /// Run performance benchmarks
    #[arg(long, value_name = "BENCHMARK_TYPE")]
    pub(crate) self_improvement_benchmark: Option<String>,

    /// Get system health metrics
    #[arg(long)]
    pub(crate) self_improvement_health: bool,

    /// List available benchmarks
    #[arg(long)]
    pub(crate) self_improvement_benchmarks: bool,

    /// Set improvement cycle to run continuously (development mode)
    #[arg(long)]
    pub(crate) self_improvement_continuous: bool,

    /// Set verbosity level for self-improvement (0-3)
    #[arg(long, value_name = "LEVEL", default_value = "1")]
    pub(crate) self_improvement_verbose: u8,
}

impl Cli {
    /// Check if any core agent commands are present
    pub fn has_agent_commands(&self) -> bool {
        self.query.is_some() ||
        self.dictation ||
        self.agent_mode ||
        self.voice_query ||
        self.status ||
        self.iterate.is_some() ||
        self.self_call ||
        self.daemon ||
        self.batch.is_some() ||
        self.interactive
    }

    /// Check if any voice commands are present
    pub fn has_voice_commands(&self) -> bool {
        self.dictation || self.agent_mode || self.voice_query
    }

    /// Check if headless mode is required
    pub fn is_headless_required(&self) -> bool {
        self.headless || self.has_agent_commands()
    }

    /// Check if this is an operational mode command
    pub fn has_operational_mode(&self) -> bool {
        self.daemon || self.batch.is_some() || self.interactive
    }

    /// Get the operation timeout
    pub fn get_timeout(&self) -> u64 {
        self.timeout.unwrap_or_else(|| {
            if self.daemon {
                3600 // 1 hour for daemon mode
            } else if self.iterate.is_some() {
                1800 // 30 minutes for iterations
            } else if self.has_voice_commands() {
                self.voice_timeout
            } else {
                300 // 5 minutes default
            }
        })
    }

    /// Check if this is a legacy test command
    pub fn is_test_command(&self) -> bool {
        self.test_focused_element_ns ||
        self.check_accessibility ||
        self.tts_provider.is_some() ||
        self.tts_text.is_some()
    }

    /// Check if this is a self-improvement command
    pub fn is_self_improvement_command(&self) -> bool {
        self.self_improvement_init ||
        self.self_improvement_start ||
        self.self_improvement_status ||
        self.self_improvement_analyze ||
        self.self_improvement_archive ||
        self.self_improvement_iteration.is_some() ||
        self.self_improvement_config.is_some() ||
        self.self_improvement_stop ||
        self.self_improvement_proposal ||
        self.self_improvement_benchmark.is_some() ||
        self.self_improvement_health ||
        self.self_improvement_benchmarks ||
        self.self_improvement_continuous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn test_parse_tts_args() {
        let cli = Cli::try_parse_from(&["juno", "--tts-provider", "system", "--tts-text", "hello"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert_eq!(cli.tts_provider, Some("system".to_string()));
        assert_eq!(cli.tts_text, Some("hello".to_string()));
    }

    #[test]
    fn test_parse_headless_query() {
        let cli = Cli::try_parse_from(&["juno", "--query", "take a screenshot"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert_eq!(cli.query, Some("take a screenshot".to_string()));
        assert!(cli.has_agent_commands());
        assert!(cli.is_headless_required());
    }

    #[test]
    fn test_parse_voice_commands() {
        let cli = Cli::try_parse_from(&["juno", "--voice-query", "--voice-timeout", "30"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert!(cli.voice_query);
        assert_eq!(cli.voice_timeout, 30);
        assert!(cli.has_voice_commands());
    }

    #[test]
    fn test_parse_iteration_commands() {
        let cli = Cli::try_parse_from(&["juno", "--iterate", "5", "--context", "test context", "--max-depth", "2"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert_eq!(cli.iterate, Some(5));
        assert_eq!(cli.context, Some("test context".to_string()));
        assert_eq!(cli.max_depth, 2);
        assert!(cli.has_agent_commands());
    }

    #[test]
    fn test_parse_tts_provider_only_fails() {
        let cli = Cli::try_parse_from(&["juno", "--tts-provider", "system"]);
        assert!(cli.is_err()); // Should fail because tts-provider requires tts-text
    }

    #[test]
    fn test_parse_tts_text_without_provider() {
        let cli = Cli::try_parse_from(&["juno", "--tts-text", "hello"]);
        assert!(cli.is_ok()); // Should work because tts-text doesn't require tts-provider
        let cli = cli.unwrap();
        assert_eq!(cli.tts_text, Some("hello".to_string()));
        assert_eq!(cli.tts_provider, None);
    }

    #[test]
    fn test_parse_no_tts_args() {
        let cli = Cli::try_parse_from(&["juno"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert_eq!(cli.tts_provider, None);
        assert_eq!(cli.tts_text, None);
    }

    #[test]
    fn test_parse_operational_modes() {
        // Test daemon mode
        let cli = Cli::try_parse_from(&["juno", "--daemon"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert!(cli.daemon);
        assert!(cli.has_operational_mode());

        // Test batch mode
        let cli = Cli::try_parse_from(&["juno", "--batch", "commands.txt"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert_eq!(cli.batch, Some("commands.txt".to_string()));
        assert!(cli.has_operational_mode());

        // Test interactive mode
        let cli = Cli::try_parse_from(&["juno", "--interactive"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert!(cli.interactive);
        assert!(cli.has_operational_mode());
    }

    #[test]
    fn test_timeout_calculation() {
        // Test default timeout
        let cli = Cli::try_parse_from(&["juno", "--query", "test"]).unwrap();
        assert_eq!(cli.get_timeout(), 300); // 5 minutes default

        // Test daemon timeout
        let cli = Cli::try_parse_from(&["juno", "--daemon"]).unwrap();
        assert_eq!(cli.get_timeout(), 3600); // 1 hour for daemon

        // Test iteration timeout
        let cli = Cli::try_parse_from(&["juno", "--iterate", "3"]).unwrap();
        assert_eq!(cli.get_timeout(), 1800); // 30 minutes for iterations

        // Test custom timeout
        let cli = Cli::try_parse_from(&["juno", "--query", "test", "--timeout", "120"]).unwrap();
        assert_eq!(cli.get_timeout(), 120); // Custom timeout
    }
}

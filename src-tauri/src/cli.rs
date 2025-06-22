use clap::Parser;

pub mod runner; // Declare the runner submodule

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
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

    // Add other test flags here, e.g.:
    // #[arg(long)]
    // pub(crate) test_list_apps: bool,

    // #[arg(long)]
    // pub(crate) test_screenshot: bool,
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
        let args = vec!["program_name", "--tts-provider", "system", "--tts-text", "Hello world"];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.tts_provider, Some("system".to_string()));
        assert_eq!(cli.tts_text, Some("Hello world".to_string()));
        assert!(!cli.test_focused_element_ns);
        assert!(!cli.check_accessibility);
    }

    #[test]
    fn test_parse_tts_provider_only_fails() {
        // clap handles the `requires` constraint
        let args = vec!["program_name", "--tts-provider", "system"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
    }

     #[test]
    fn test_parse_tts_text_without_provider() {
        // This is allowed, but won't trigger the TTS test logic
        let args = vec!["program_name", "--tts-text", "Just text"];
        let cli = Cli::parse_from(args);
        assert!(cli.tts_provider.is_none());
        assert_eq!(cli.tts_text, Some("Just text".to_string()));
    }

    #[test]
    fn test_parse_no_tts_args() {
        let args = vec!["program_name"];
        let cli = Cli::parse_from(args);
        assert!(cli.tts_provider.is_none());
        assert!(cli.tts_text.is_none());
    }
}

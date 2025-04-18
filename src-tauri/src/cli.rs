use clap::Parser;

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

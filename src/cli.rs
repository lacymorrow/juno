use clap::Parser;

pub mod runner; // Declare the runner submodule

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Optional provider for TTS test (system, elevenlabs, replicate)
    #[arg(long)]
    pub tts_provider: Option<String>,

    /// Optional text for TTS test
    #[arg(long)]
    pub tts_text: Option<String>,

    /// Run macOS focused element test using NSWorkspace
    #[arg(long)]
    pub test_focused_element_ns: bool,

    /// Check macOS accessibility status
    #[arg(long)]
    pub check_accessibility: bool,
}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    /// Run the NSWorkspace-based test for focused element
    #[arg(long)]
    pub(crate) test_focused_element_ns: bool,

    /// Check if the process has accessibility permissions
    #[arg(long)]
    pub(crate) check_accessibility: bool,

    // Add other test flags here, e.g.:
    // #[arg(long)]
    // pub(crate) test_list_apps: bool,

    // #[arg(long)]
    // pub(crate) test_screenshot: bool,
}

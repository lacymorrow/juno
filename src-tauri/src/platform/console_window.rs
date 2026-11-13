//! Child processes without a console window.
//!
//! Juno is a GUI app (`windows_subsystem = "windows"`), so on Windows every
//! console program it starts (claude, node, an MCP server, cmd) gets a fresh
//! console window unless it is told not to. `hide_console_window` sets
//! `CREATE_NO_WINDOW` there and does nothing anywhere else. Its output still
//! reaches the pipes Juno reads.

/// `CREATE_NO_WINDOW` from the Win32 process creation flags.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub trait HideConsoleWindow {
    /// Start the child without a console window (Windows); a no-op elsewhere.
    fn hide_console_window(&mut self) -> &mut Self;
}

impl HideConsoleWindow for std::process::Command {
    fn hide_console_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

impl HideConsoleWindow for tokio::process::Command {
    fn hide_console_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

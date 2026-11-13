//! The pages' console, in Juno's own log.
//!
//! Every window forwards its `console` output and uncaught errors here
//! (`src/lib/forwardLogs.ts`), so ~/Library/Logs/Juno holds both halves of
//! what happened. Without this a bar button that did nothing left no trace at
//! all: the page knew, and the log only had Rust's side.

use serde::Deserialize;
use tracing::{debug, error, info, warn};

/// The longest message kept, in characters. A page logging a whole
/// conversation or a data URL must not bury the rest of the log.
const MAX_MESSAGE_CHARS: usize = 4000;

/// One console call, as the page sends it.
#[derive(Debug, Deserialize)]
pub struct FrontendLogEntry {
    /// `error`, `warn`, `info`, `log` or `debug`.
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// The level a console method maps to. `log` is ordinary output, so INFO; an
/// unknown name is kept rather than dropped.
pub fn web_level(level: &str) -> WebLevel {
    match level {
        "error" => WebLevel::Error,
        "warn" => WebLevel::Warn,
        "debug" | "trace" => WebLevel::Debug,
        _ => WebLevel::Info,
    }
}

/// The line written for one entry: which window said it, then what it said,
/// on one line and capped.
pub fn web_line(window: &str, message: &str) -> String {
    let flat: String = message
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .take(MAX_MESSAGE_CHARS)
        .collect();
    let cut = if message.chars().count() > MAX_MESSAGE_CHARS {
        " …"
    } else {
        ""
    };
    format!("[Web:{window}] {flat}{cut}")
}

/// Write a batch of a page's console output into the log.
#[tauri::command]
pub fn frontend_log(window: tauri::WebviewWindow, entries: Vec<FrontendLogEntry>) {
    let label = window.label().to_string();
    for entry in entries {
        let line = web_line(&label, &entry.message);
        match web_level(&entry.level) {
            WebLevel::Error => error!("{}", line),
            WebLevel::Warn => warn!("{}", line),
            WebLevel::Info => info!("{}", line),
            WebLevel::Debug => debug!("{}", line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_methods_map_to_log_levels() {
        assert_eq!(web_level("error"), WebLevel::Error);
        assert_eq!(web_level("warn"), WebLevel::Warn);
        assert_eq!(web_level("info"), WebLevel::Info);
        assert_eq!(web_level("log"), WebLevel::Info);
        assert_eq!(web_level("debug"), WebLevel::Debug);
        // Unknown is kept, not dropped.
        assert_eq!(web_level("table"), WebLevel::Info);
    }

    #[test]
    fn a_line_names_its_window_and_stays_one_line() {
        assert_eq!(
            web_line("floating-bar", "Island: focus\nlistener failed"),
            "[Web:floating-bar] Island: focus listener failed"
        );
    }

    #[test]
    fn a_long_message_is_capped_without_splitting_a_character() {
        let long = "é".repeat(MAX_MESSAGE_CHARS + 10);
        let line = web_line("main", &long);
        assert!(line.ends_with(" …"));
        assert_eq!(
            line.chars().count(),
            "[Web:main] ".chars().count() + MAX_MESSAGE_CHARS + 2
        );
    }
}

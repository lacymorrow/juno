//! # File Constants
//!
//! File patterns, extensions, and path constants.

// File extensions
pub mod extensions {
    pub const JSON_EXT: &str = ".json";
    pub const RUST_EXT: &str = ".rs";
    pub const TYPESCRIPT_EXT: &str = ".ts";
    pub const JAVASCRIPT_EXT: &str = ".js";
    pub const MARKDOWN_EXT: &str = ".md";
    pub const LOG_EXTENSION: &str = ".log";
    pub const TMP_EXTENSION: &str = ".tmp";
    pub const CACHE_EXTENSION: &str = ".cache";
    pub const BACKUP_EXTENSION: &str = ".backup";

    // Additional extensions moved from frontend constants.ts
    pub const TEXT_EXT: &str = ".txt";
    pub const CSV_EXT: &str = ".csv";
}

// File prefixes
pub mod prefixes {
    pub const LOG_PREFIX: &str = "juno_";
    pub const SCREENSHOT_PREFIX: &str = "screenshot_";
    pub const TEMP_PREFIX: &str = "temp_";
}

// Directory names
pub mod directories {
    pub const LOGS_DIR: &str = "logs";
    pub const CACHE_DIR: &str = "cache";
    pub const CONFIG_DIR: &str = ".juno";
    pub const SCREENSHOTS_DIR: &str = "screenshots";
}

// Common files
pub mod common {
    pub const PACKAGE_JSON: &str = "package.json";
    pub const CARGO_TOML: &str = "Cargo.toml";
    pub const REQUIREMENTS_TXT: &str = "requirements.txt";
    pub const COMPOSER_JSON: &str = "composer.json";
    pub const README_MD: &str = "README.md";
    pub const README_TXT: &str = "README.txt";
    pub const TSCONFIG_JSON: &str = "tsconfig.json";
    pub const MAIN_PY: &str = "main.py";
    pub const INDEX_JS: &str = "index.js";
    pub const MAIN_RS: &str = "main.rs";
    pub const APP_TSX: &str = "App.tsx";
}

// Shell commands
pub mod shell_commands {
    pub const OPEN: &str = "open";
    pub const OSASCRIPT: &str = "osascript";
    pub const KILLALL: &str = "killall";
    pub const PS: &str = "ps";
    pub const GREP: &str = "grep";
    pub const CURL: &str = "curl";
    pub const WHICH: &str = "which";

    // Command flags
    pub const BACKGROUND_FLAG: &str = "&";
    pub const QUIET_FLAG: &str = "-q";
    pub const VERBOSE_FLAG: &str = "-v";
    pub const FORCE_FLAG: &str = "-f";
    pub const RECURSIVE_FLAG: &str = "-r";

    // Browser binaries
    pub const CHROME_BINARY_MACOS: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    pub const CHROMIUM_BINARY_MACOS: &str = "/Applications/Chromium.app/Contents/MacOS/Chromium";
}

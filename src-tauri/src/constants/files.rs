//! # File Constants
//!
//! File patterns, extensions, and path constants.

// File extensions
pub mod extensions {
    // File extensions with dots (for file operations)
    pub const JSON_EXT: &str = ".json";
    pub const RUST_EXT: &str = ".rs";
    pub const TYPESCRIPT_EXT: &str = ".ts";
    pub const JAVASCRIPT_EXT: &str = ".js";
    pub const MARKDOWN_EXT: &str = ".md";
    pub const LOG_EXT: &str = ".log";
    pub const TMP_EXT: &str = ".tmp";
    pub const CACHE_EXT: &str = ".cache";
    pub const BACKUP_EXT: &str = ".backup";
    pub const TEXT_EXT: &str = ".txt";
    pub const CSV_EXT: &str = ".csv";

    // File extensions without dots (for security validation and pattern matching)
    pub const TXT: &str = "txt";
    pub const MD: &str = "md";
    pub const RS: &str = "rs";
    pub const JS: &str = "js";
    pub const TS: &str = "ts";
    pub const PY: &str = "py";
    pub const JAVA: &str = "java";
    pub const C: &str = "c";
    pub const CPP: &str = "cpp";
    pub const H: &str = "h";
    pub const HPP: &str = "hpp";
    pub const CSS: &str = "css";
    pub const HTML: &str = "html";
    pub const XML: &str = "xml";
    pub const JSON: &str = "json";
    pub const YAML: &str = "yaml";
    pub const YML: &str = "yml";
    pub const TOML: &str = "toml";
    pub const CFG: &str = "cfg";
    pub const INI: &str = "ini";
    pub const SH: &str = "sh";
    pub const BAT: &str = "bat";
    pub const PS1: &str = "ps1";
    pub const SQL: &str = "sql";
    pub const GO: &str = "go";
    pub const RB: &str = "rb";
    pub const PHP: &str = "php";
    pub const SWIFT: &str = "swift";
    pub const KT: &str = "kt";
    pub const SCALA: &str = "scala";
    pub const LOG: &str = "log";
    pub const OUT: &str = "out";
    pub const ERR: &str = "err";
    pub const TMP: &str = "tmp";

    // File extension arrays for SecurityConfig (production)
    pub const PRODUCTION_EXTENSIONS: &[&str] = &[
        "txt", "md", "rs", "js", "ts", "py", "java", "c", "cpp", "h", "hpp",
        "css", "html", "xml", "json", "yaml", "yml", "toml", "cfg", "ini",
        "sh", "bat", "ps1", "sql", "go", "rb", "php", "swift", "kt", "scala"
    ];

    // File extension arrays for SecurityConfig (development mode)
    pub const DEVELOPMENT_EXTENSIONS: &[&str] = &[
        "txt", "md", "rs", "js", "ts", "py", "java", "c", "cpp", "h", "hpp",
        "css", "html", "xml", "json", "yaml", "yml", "toml", "cfg", "ini",
        "sh", "bat", "ps1", "sql", "go", "rb", "php", "swift", "kt", "scala",
        "log", "out", "err", "tmp"
    ];
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

// Path patterns for security validation
pub mod path_patterns {
    pub const PATH_TRAVERSAL_UNIX: &str = "../";
    pub const PATH_TRAVERSAL_WINDOWS: &str = "..\\";
    pub const HOME_DIRECTORY: &str = "~/";
}

// Line ending patterns
pub mod line_endings {
    pub const CRLF: &str = "\r\n";
    pub const LF: &str = "\n";
    pub const DEFAULT_LF: &str = "\n"; // Default to LF for new files
}

// Tool parameter names
pub mod parameters {
    // Computer tool parameters
    pub const ACTION: &str = "action";
    pub const COORDINATE: &str = "coordinate";
    pub const KEY: &str = "key";
    pub const TEXT: &str = "text";
    pub const DURATION_MS: &str = "duration_ms";
    pub const DURATION: &str = "duration";
    pub const SCROLL_DIRECTION: &str = "scroll_direction";
    pub const SCROLL_CLICKS: &str = "scroll_clicks";
    pub const SECONDS: &str = "seconds";

    // Bash tool parameters
    pub const COMMAND: &str = "command";

    // str_replace_tool parameters
    pub const PATH: &str = "path";
    pub const VIEW_RANGE: &str = "view_range";
    pub const OLD_STR: &str = "old_str";
    pub const NEW_STR: &str = "new_str";
    pub const FILE_TEXT: &str = "file_text";

    // Additional computer tool parameters
    pub const SCROLL_AMOUNT: &str = "scroll_amount";

    // Line number constants for line numbering functions
    pub const LINE_NUMBER_FORMAT_SIMPLE: &str = "{}: {}";
    pub const LINE_NUMBER_FORMAT_START_OFFSET: &str = "{}: {}";
}

// Default values for parameters
pub mod defaults {
    pub const SCROLL_DIRECTION_DEFAULT: &str = "up";
    pub const EMPTY_STRING: &str = "";
    pub const CRLF_REPLACEMENT: &str = "\r\n";
    pub const LF_REPLACEMENT: &str = "\n";
}

// str_replace_tool commands
pub mod str_replace_commands {
    pub const VIEW: &str = "view";
    pub const STR_REPLACE: &str = "str_replace";
    pub const CREATE: &str = "create";
}

// JSON response fields
pub mod response_fields {
    pub const SUCCESS: &str = "success";
    pub const MESSAGE: &str = "message";
    pub const CONTENT: &str = "content";
    pub const VIEW_RANGE: &str = "view_range";
    pub const BASE64_IMAGE: &str = "base64_image";
    pub const COORDINATE: &str = "coordinate";
    pub const OUTPUT: &str = "output";
    pub const EXIT_CODE: &str = "exit_code";
    pub const STDOUT: &str = "stdout";
    pub const STDERR: &str = "stderr";
}

// --- File/Directory Constants ---
pub const AGENT_LOG_FILE: &str = "agent_activity.log";
pub const CONVERSATION_CACHE_FILE: &str = "conversation_cache.json";
pub const WORKSPACE_CONFIG_FILE: &str = ".workspace_config.json";

// --- Tool Parameter Constants ---
pub const APP_NAME: &str = "app_name";
pub const TEXT: &str = "text";
pub const COORDINATE: &str = "coordinate";

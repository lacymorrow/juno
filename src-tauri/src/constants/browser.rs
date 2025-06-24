//! # Browser Constants
//!
//! Browser automation and JavaScript constants.

// Chrome debug URLs
pub mod chrome_debug_urls {
    pub const PRIMARY: &str = "http://localhost:9222";
    pub const ALTERNATIVE_1: &str = "http://localhost:9223";
    pub const ALTERNATIVE_2: &str = "http://localhost:9224";

    pub fn get_all_urls() -> [&'static str; 3] {
        [PRIMARY, ALTERNATIVE_1, ALTERNATIVE_2]
    }
}

// Chrome flags
pub mod chrome_flags {
    pub const REMOTE_DEBUG_PORT_FLAG: &str = "--remote-debugging-port=9222";
    pub const HEADLESS_FLAG: &str = "--headless";
    pub const NO_SANDBOX_FLAG: &str = "--no-sandbox";
    pub const DISABLE_GPU_FLAG: &str = "--disable-gpu";
    pub const DISABLE_DEV_SHM_FLAG: &str = "--disable-dev-shm-usage";
}

// System URL opening commands (cross-platform)
pub mod system_url_commands {
    #[cfg(target_os = "macos")]
    pub const OPEN_URL_COMMAND: &str = "open";

    #[cfg(target_os = "windows")]
    pub const OPEN_URL_COMMAND: &str = "cmd";

    #[cfg(target_os = "linux")]
    pub const OPEN_URL_COMMAND: &str = "xdg-open";

    #[cfg(target_os = "windows")]
    pub const WINDOWS_START_ARGS: [&str; 2] = ["/C", "start"];
}

// URL protocols
pub mod url_protocols {
    pub const HTTP: &str = "http://";
    pub const HTTPS: &str = "https://";
    pub const FILE: &str = "file://";
    pub const FTP: &str = "ftp://";

    // Common custom protocols that should be opened by system
    pub const CUSTOM_PROTOCOLS: [&str; 12] = [
        "mailto:",
        "tel:",
        "sms:",
        "slack:",
        "discord:",
        "zoom:",
        "teams:",
        "spotify:",
        "notion:",
        "obsidian:",
        "vscode:",
        "jetbrains:",
    ];

    /// Check if URL has a custom protocol that should be handled by the system
    pub fn is_custom_protocol(url: &str) -> bool {
        let url_lower = url.to_lowercase();

        // Allow standard web protocols in browser
        if url_lower.starts_with(HTTP) || url_lower.starts_with(HTTPS) {
            return false;
        }

        // Check for custom protocols
        CUSTOM_PROTOCOLS.iter().any(|protocol| url_lower.starts_with(protocol))
    }

    /// Check if URL should be opened in the system's default handler
    pub fn should_use_system_handler(url: &str) -> bool {
        // First check for explicit custom protocols
        if is_custom_protocol(url) {
            return true;
        }

        let url_lower = url.to_lowercase();

        // Don't use system handler for standard web protocols
        if url_lower.starts_with("http://") || url_lower.starts_with("https://") {
            return false;
        }

        // Don't use system handler for relative paths (browser navigation)
        if url.starts_with("/") || url.starts_with("./") || url.starts_with("../") || !url.contains(":") {
            return false;
        }

        // If URL contains ":" but isn't http/https and isn't a custom protocol,
        // it might be another protocol scheme - use system handler
        url.contains(":")
    }
}

// Browser JavaScript constants
pub mod browser_js {
    pub const QUERY_SELECTOR_ALL: &str = "document.querySelectorAll";
    pub const QUERY_SELECTOR: &str = "document.querySelector";
    pub const TEXT_CONTENT: &str = "textContent";
    pub const GET_ATTRIBUTE: &str = "getAttribute";
    pub const CLICK: &str = "click";
    pub const FOCUS: &str = "focus";
}

// JavaScript templates
pub mod javascript_templates {
    pub const QUERY_ALL_TEMPLATE: &str = "document.querySelectorAll('{}')";
    pub const QUERY_SINGLE_TEMPLATE: &str = "document.querySelector('{}')";
    pub const GET_TEXT_CONTENT: &str = ".textContent";
    pub const GET_INNER_TEXT: &str = ".innerText";
    pub const GET_VALUE: &str = ".value";

    // Element interaction templates
    pub const CLICK_ELEMENT: &str = ".click()";
    pub const FOCUS_ELEMENT: &str = ".focus()";
    pub const SCROLL_INTO_VIEW: &str = ".scrollIntoView()";

    // Attribute templates
    pub const GET_ATTRIBUTE_TEMPLATE: &str = ".getAttribute('{}')";
    pub const SET_ATTRIBUTE_TEMPLATE: &str = ".setAttribute('{}', '{}')";
    pub const GET_STYLE_TEMPLATE: &str = ".style.{}";

    // Common selectors
    pub const BUTTON_SELECTOR: &str = "button";
    pub const INPUT_SELECTOR: &str = "input";
    pub const LINK_SELECTOR: &str = "a";
    pub const FORM_SELECTOR: &str = "form";
}

#[cfg(test)]
mod tests {
    use super::url_protocols::*;

    #[test]
    fn test_web_urls_should_not_use_system_handler() {
        assert!(!should_use_system_handler("https://example.com"));
        assert!(!should_use_system_handler("http://example.com"));
        assert!(!should_use_system_handler("HTTP://EXAMPLE.COM"));
        assert!(!should_use_system_handler("HTTPS://EXAMPLE.COM"));
    }

    #[test]
    fn test_custom_protocols_should_use_system_handler() {
        assert!(should_use_system_handler("mailto:user@example.com"));
        assert!(should_use_system_handler("tel:+1234567890"));
        assert!(should_use_system_handler("slack://channel/general"));
        assert!(should_use_system_handler("zoom://meeting/123456"));
        assert!(should_use_system_handler("MAILTO:user@example.com")); // Case insensitive
        assert!(should_use_system_handler("TEL:+1234567890"));
    }

    #[test]
    fn test_relative_urls_should_not_use_system_handler() {
        assert!(!should_use_system_handler("/path/to/page"));
        assert!(!should_use_system_handler("./relative/path"));
        assert!(!should_use_system_handler("../parent/path"));
        assert!(!should_use_system_handler("page.html"));
        assert!(!should_use_system_handler("subfolder/page.html"));
        assert!(!should_use_system_handler("index"));
    }

    #[test]
    fn test_unknown_protocols_should_use_system_handler() {
        assert!(should_use_system_handler("customapp://data"));
        assert!(should_use_system_handler("ftp://files.example.com"));
        assert!(should_use_system_handler("steam://run/123456"));
    }

    #[test]
    fn test_protocol_detection() {
        assert!(is_custom_protocol("mailto:test@example.com"));
        assert!(is_custom_protocol("tel:123"));
        assert!(!is_custom_protocol("https://example.com"));
        assert!(!is_custom_protocol("http://example.com"));
        assert!(is_custom_protocol("vscode://file/path/to/file"));
        assert!(is_custom_protocol("notion://page/123"));
    }
}

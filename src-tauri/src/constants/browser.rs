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

    /// Web protocols that should be handled by the browser
    const WEB_PROTOCOLS: [&str; 2] = ["http://", "https://"];

    /// Check if URL uses a web protocol that should be handled by the browser
    pub fn is_web_protocol(url: &str) -> bool {
        let url_lower = url.to_lowercase();
        WEB_PROTOCOLS
            .iter()
            .any(|protocol| url_lower.starts_with(protocol))
    }

    /// Check if URL has a protocol scheme (contains ':' after the scheme part)
    pub fn has_protocol_scheme(url: &str) -> bool {
        // Check if URL contains ':' but isn't a relative path
        if !url.contains(':') {
            return false;
        }

        // Find the position of the first ':'
        if let Some(colon_pos) = url.find(':') {
            // Check if there are any path separators before the colon
            // If so, it's likely a file path like "C:" on Windows
            let before_colon = &url[..colon_pos];

            // If the part before colon contains path separators, it's not a protocol
            if before_colon.contains('/') || before_colon.contains('\\') {
                return false;
            }

            // If it's just a single character (like "C:"), it's likely a Windows drive
            if before_colon.len() == 1 {
                return false;
            }

            // Otherwise, it's likely a protocol scheme
            return true;
        }

        false
    }

    /// Check if URL should be opened in the system's default handler
    pub fn should_use_system_handler(url: &str) -> bool {
        // Handle relative paths and local files - these should be handled by browser navigation
        if url.starts_with("/") || url.starts_with("./") || url.starts_with("../") {
            return false;
        }

        // Handle URLs without any path separators and no protocol (like "index" or "page.html")
        if !url.contains(':') && !url.contains('/') {
            return false;
        }

        // If it's a web protocol, handle in browser
        if is_web_protocol(url) {
            return false;
        }

        // If it has a protocol scheme that's not web, use system handler
        if has_protocol_scheme(url) {
            return true;
        }

        // For file paths with extensions but no protocol, let browser handle
        // (these might be relative paths to local files)
        false
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
        assert!(should_use_system_handler("vscode://file/path/to/file"));
        assert!(should_use_system_handler("notion://page/123"));
        assert!(should_use_system_handler("discord://channel/123"));
        assert!(should_use_system_handler("steam://run/123456"));
        assert!(should_use_system_handler("spotify://playlist/123"));
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
        assert!(should_use_system_handler("myapp://custom/action"));
    }

    #[test]
    fn test_protocol_detection() {
        assert!(has_protocol_scheme("mailto:test@example.com"));
        assert!(has_protocol_scheme("tel:123"));
        assert!(has_protocol_scheme("https://example.com"));
        assert!(has_protocol_scheme("http://example.com"));
        assert!(has_protocol_scheme("vscode://file/path/to/file"));
        assert!(has_protocol_scheme("notion://page/123"));
        assert!(has_protocol_scheme("ftp://server.com"));
        assert!(has_protocol_scheme("steam://run/123"));

        // These should NOT be detected as protocols
        assert!(!has_protocol_scheme("/path/to/file"));
        assert!(!has_protocol_scheme("./relative"));
        assert!(!has_protocol_scheme("../parent"));
        assert!(!has_protocol_scheme("filename.txt"));
        assert!(!has_protocol_scheme("C:\\Windows\\file.txt")); // Windows path
        assert!(!has_protocol_scheme("C:/Windows/file.txt")); // Windows path with forward slashes
    }

    #[test]
    fn test_web_protocol_detection() {
        assert!(is_web_protocol("https://example.com"));
        assert!(is_web_protocol("http://example.com"));
        assert!(is_web_protocol("HTTP://EXAMPLE.COM"));
        assert!(is_web_protocol("HTTPS://EXAMPLE.COM"));

        assert!(!is_web_protocol("mailto:user@example.com"));
        assert!(!is_web_protocol("ftp://server.com"));
        assert!(!is_web_protocol("file://path/to/file"));
        assert!(!is_web_protocol("vscode://file"));
    }
}

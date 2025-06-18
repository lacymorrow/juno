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

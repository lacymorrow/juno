//! # Menu Constants
//!
//! Menu identifiers for both tray and app menus.

pub mod tray_menu_ids {
    pub const QUIT: &str = "quit";
    pub const TOGGLE_FLOATING_BAR: &str = "toggle-floating-bar";
    pub const SHOW_FLOATING_BAR: &str = "show-floating-bar";
    pub const HIDE_FLOATING_BAR: &str = "hide-floating-bar";
    pub const SHOW_DEVTOOLS: &str = "show-devtools";
    pub const SHOW_MAIN_WINDOW: &str = "show-main-window";
    pub const HIDE_MAIN_WINDOW: &str = "hide-main-window";
    /// Not "new-chat": that is the app menu's File > New Chat, and the app
    /// menu is matched first, so a shared id never reached the tray handler.
    pub const NEW_CHAT: &str = "tray-new-chat";
    pub const SETTINGS: &str = "tray-settings";

    // Additional menu constants
    /// Show or hide Juno: the floating bar, or the chat window when that is
    /// what is on screen. One row, labelled for what it will do.
    pub const SHOW_HIDE: &str = "show-hide";
    /// The bar's right-click menu: open or close the bar's own chat.
    pub const BAR_CHAT: &str = "bar-chat";
    pub const SHOW_HIDE_FLOATING_BAR: &str = "show-hide-floating-bar";
    pub const DEVELOPER_TOOLS: &str = "developer-tools";
    /// Disabled first row showing the state word
    pub const STATUS: &str = "tray-status";
    /// Talk to Juno while she is idle, Cancel while she is busy: one row
    /// whose text and action follow her state.
    pub const TALK: &str = "tray-talk";
    /// Cancel the current turn
    pub const CANCEL: &str = "tray-cancel";
    /// Put the floating bar back in the middle of its display. The way back
    /// when a drag leaves it somewhere it cannot be grabbed.
    pub const CENTER_FLOATING_BAR: &str = "center-floating-bar";
}

pub mod app_menu_ids {
    // Juno Menu
    pub const ABOUT: &str = "about";
    pub const SETTINGS: &str = "settings";
    pub const CHECK_FOR_UPDATES: &str = "check-for-updates";

    // File Menu
    pub const NEW_CHAT: &str = "new-chat";
    pub const IMPORT_CHAT: &str = "import-chat";
    pub const EXPORT_CHAT: &str = "export-chat";
    pub const CLOSE_WINDOW: &str = "close-window";

    // View Menu
    pub const TOGGLE_FLOATING_BAR: &str = "toggle-floating-bar";
    pub const TOGGLE_DEV_PANEL: &str = "toggle-dev-panel";
    pub const SHOW_DEVTOOLS: &str = "show-devtools";
    pub const SHOW_PERMISSIONS: &str = "show-permissions";
    pub const TOGGLE_FULLSCREEN: &str = "toggle-fullscreen";
    pub const ZOOM_IN: &str = "zoom-in";
    pub const ZOOM_OUT: &str = "zoom-out";
    pub const ACTUAL_SIZE: &str = "actual-size";

    // Window Menu
    pub const MINIMIZE: &str = "minimize";
    pub const ZOOM: &str = "zoom";
    pub const BRING_ALL_TO_FRONT: &str = "bring-all-to-front";

    // Help Menu
    pub const HELP: &str = "help";
    pub const KEYBOARD_SHORTCUTS: &str = "keyboard-shortcuts";
    pub const SEND_FEEDBACK: &str = "send-feedback";
    pub const REPORT_ISSUE: &str = "report-issue";
    pub const VISIT_WEBSITE: &str = "visit-website";
}

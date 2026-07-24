//! # macOS Platform Constants
//!
//! Shared macOS-specific constants used by both the main app and MCP server.
//! This eliminates duplication between different constants files.

// Use u16 for key codes and u32 for modifier flags to avoid dependency issues
pub type CGKeyCode = u16;
pub type CGEventFlags = u32;

// Key codes - shared between main app and MCP server
pub mod key_codes {
    use super::CGKeyCode;

    // Basic keys
    pub const RETURN: CGKeyCode = 36;
    pub const TAB: CGKeyCode = 48;
    pub const SPACE: CGKeyCode = 49;
    pub const DELETE: CGKeyCode = 51;
    pub const ESCAPE: CGKeyCode = 53;

    // Arrow keys
    pub const ARROW_LEFT: CGKeyCode = 123;
    pub const ARROW_RIGHT: CGKeyCode = 124;
    pub const ARROW_DOWN: CGKeyCode = 125;
    pub const ARROW_UP: CGKeyCode = 126;

    // Letters (alphabetical order)
    pub const A: CGKeyCode = 0;
    pub const B: CGKeyCode = 11;
    pub const C: CGKeyCode = 8;
    pub const D: CGKeyCode = 2;
    pub const E: CGKeyCode = 14;
    pub const F: CGKeyCode = 3;
    pub const G: CGKeyCode = 5;
    pub const H: CGKeyCode = 4;
    pub const I: CGKeyCode = 34;
    pub const J: CGKeyCode = 38;
    pub const K: CGKeyCode = 40;
    pub const L: CGKeyCode = 37;
    pub const M: CGKeyCode = 46;
    pub const N: CGKeyCode = 45;
    pub const O: CGKeyCode = 31;
    pub const P: CGKeyCode = 35;
    pub const Q: CGKeyCode = 12;
    pub const R: CGKeyCode = 15;
    pub const S: CGKeyCode = 1;
    pub const T: CGKeyCode = 17;
    pub const U: CGKeyCode = 32;
    pub const V: CGKeyCode = 9;
    pub const W: CGKeyCode = 13;
    pub const X: CGKeyCode = 7;
    pub const Y: CGKeyCode = 16;
    pub const Z: CGKeyCode = 6;

    // Numbers
    pub const NUM_0: CGKeyCode = 29;
    pub const NUM_1: CGKeyCode = 18;
    pub const NUM_2: CGKeyCode = 19;
    pub const NUM_3: CGKeyCode = 20;
    pub const NUM_4: CGKeyCode = 21;
    pub const NUM_5: CGKeyCode = 23;
    pub const NUM_6: CGKeyCode = 22;
    pub const NUM_7: CGKeyCode = 26;
    pub const NUM_8: CGKeyCode = 28;
    pub const NUM_9: CGKeyCode = 25;

    // Modifier keys
    pub const COMMAND: CGKeyCode = 55;
    pub const SHIFT: CGKeyCode = 56;
    pub const CAPS_LOCK: CGKeyCode = 57;
    pub const OPTION: CGKeyCode = 58;
    pub const CONTROL: CGKeyCode = 59;
    pub const RIGHT_SHIFT: CGKeyCode = 60;
    pub const RIGHT_OPTION: CGKeyCode = 61;
    pub const RIGHT_CONTROL: CGKeyCode = 62;
    pub const FUNCTION: CGKeyCode = 63;
}

// Modifier flags
pub mod modifiers {
    use super::CGEventFlags;

    // macOS CGEventFlags constants as numeric values
    pub const COMMAND: CGEventFlags = 0x00100000; // CGEventFlagCommand
    pub const SHIFT: CGEventFlags = 0x00020000; // CGEventFlagShift
    pub const OPTION: CGEventFlags = 0x00080000; // CGEventFlagAlternate
    pub const CONTROL: CGEventFlags = 0x00040000; // CGEventFlagControl
    pub const FUNCTION: CGEventFlags = 0x00800000; // CGEventFlagSecondaryFn
}

// System values
pub mod system {
    // AXValue type constants
    pub const AXVALUE_CGPOINT_TYPE: u32 = 1;
    pub const AXVALUE_CGSIZE_TYPE: u32 = 2;

    // NSTrackingArea options
    pub const NS_TRACKING_MOUSE_ENTERED_AND_EXITED: u64 = 0x01;
    pub const NS_TRACKING_ACTIVE_ALWAYS: u64 = 0x80;

    // System permission timeouts
    pub const ACCESSIBILITY_PERMISSION_CHECK_DELAY_MS: u64 = 1000;
    pub const SCREEN_RECORDING_PERMISSION_CHECK_DELAY_MS: u64 = 2000;
    pub const MAX_ACCESSIBILITY_RETRIES: usize = 3;
    pub const SYSTEM_PERMISSION_TIMEOUT_MS: u64 = 5000;
}

// System preferences URLs
pub mod system_prefs {
    pub const MICROPHONE_PRIVACY_URL: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
    pub const SCREEN_RECORDING_PRIVACY_URL: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
    pub const INPUT_MONITORING_PRIVACY_URL: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";
    pub const ACCESSIBILITY_PRIVACY_URL: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

    // Bundle identifiers
    pub const SYSTEM_PREFERENCES_BUNDLE: &str = "com.apple.systempreferences";
    pub const SYSTEM_SETTINGS_BUNDLE: &str = "com.apple.systemsettings";
    pub const SECURITY_PREFPANE_PATH: &str = "/System/Library/PreferencePanes/Security.prefPane";
}

// Helper functions for key mapping (used by MCP server)
pub fn key_name_to_keycode(key_name: &str) -> Option<CGKeyCode> {
    let key_lower = key_name.to_lowercase();

    match key_lower.as_str() {
        "return" | "enter" => Some(key_codes::RETURN),
        "tab" => Some(key_codes::TAB),
        "space" => Some(key_codes::SPACE),
        "delete" | "backspace" => Some(key_codes::DELETE),
        "escape" | "esc" => Some(key_codes::ESCAPE),
        "left" | "arrowleft" => Some(key_codes::ARROW_LEFT),
        "right" | "arrowright" => Some(key_codes::ARROW_RIGHT),
        "down" | "arrowdown" => Some(key_codes::ARROW_DOWN),
        "up" | "arrowup" => Some(key_codes::ARROW_UP),
        _ => {
            if key_lower.len() == 1 {
                let c = key_lower.chars().next()?;

                // Handle alphabetic keys
                if c.is_ascii_alphabetic() {
                    return Some(match c {
                        'a' => key_codes::A,
                        'b' => key_codes::B,
                        'c' => key_codes::C,
                        'd' => key_codes::D,
                        'e' => key_codes::E,
                        'f' => key_codes::F,
                        'g' => key_codes::G,
                        'h' => key_codes::H,
                        'i' => key_codes::I,
                        'j' => key_codes::J,
                        'k' => key_codes::K,
                        'l' => key_codes::L,
                        'm' => key_codes::M,
                        'n' => key_codes::N,
                        'o' => key_codes::O,
                        'p' => key_codes::P,
                        'q' => key_codes::Q,
                        'r' => key_codes::R,
                        's' => key_codes::S,
                        't' => key_codes::T,
                        'u' => key_codes::U,
                        'v' => key_codes::V,
                        'w' => key_codes::W,
                        'x' => key_codes::X,
                        'y' => key_codes::Y,
                        'z' => key_codes::Z,
                        _ => return None,
                    });
                }

                // Handle numeric keys
                if c.is_ascii_digit() {
                    return Some(match c {
                        '0' => key_codes::NUM_0,
                        '1' => key_codes::NUM_1,
                        '2' => key_codes::NUM_2,
                        '3' => key_codes::NUM_3,
                        '4' => key_codes::NUM_4,
                        '5' => key_codes::NUM_5,
                        '6' => key_codes::NUM_6,
                        '7' => key_codes::NUM_7,
                        '8' => key_codes::NUM_8,
                        '9' => key_codes::NUM_9,
                        _ => return None,
                    });
                }
            }
            None
        }
    }
}

pub fn modifier_name_to_flags(modifier_name: &str) -> Option<CGEventFlags> {
    match modifier_name.to_lowercase().as_str() {
        "command" | "cmd" => Some(modifiers::COMMAND),
        "shift" => Some(modifiers::SHIFT),
        "option" | "alt" => Some(modifiers::OPTION),
        "control" | "ctrl" => Some(modifiers::CONTROL),
        "function" | "fn" => Some(modifiers::FUNCTION),
        _ => None,
    }
}

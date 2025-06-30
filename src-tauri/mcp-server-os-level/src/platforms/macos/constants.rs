// Re-export shared platform constants from main app
// This eliminates duplication with src-tauri/src/constants/platform/macos.rs

// For now, define minimal constants needed until we can properly import
// TODO: Set up proper shared constants between main app and MCP server
pub(crate) const K_AXVALUE_CGPOINT_TYPE: u32 = 1;
pub(crate) const K_AXVALUE_CGSIZE_TYPE: u32 = 2;

// Use shared key codes from main constants module
pub(crate) use core_graphics::event::{CGEventFlags, CGKeyCode};

// Key codes - these should be imported from shared constants
pub(crate) const KEY_RETURN: CGKeyCode = 36;
pub(crate) const KEY_TAB: CGKeyCode = 48;
pub(crate) const KEY_SPACE: CGKeyCode = 49;
pub(crate) const KEY_DELETE: CGKeyCode = 51;
pub(crate) const KEY_ESCAPE: CGKeyCode = 53;
pub(crate) const KEY_ARROW_LEFT: CGKeyCode = 123;
pub(crate) const KEY_ARROW_RIGHT: CGKeyCode = 124;
pub(crate) const KEY_ARROW_DOWN: CGKeyCode = 125;
pub(crate) const KEY_ARROW_UP: CGKeyCode = 126;
pub(crate) const KEY_V: CGKeyCode = 9;

// Punctuation and symbol key codes
pub(crate) const KEY_COMMA: CGKeyCode = 43;           // ,
pub(crate) const KEY_PERIOD: CGKeyCode = 47;          // .
pub(crate) const KEY_SEMICOLON: CGKeyCode = 41;       // ;
pub(crate) const KEY_QUOTE: CGKeyCode = 39;           // '
pub(crate) const KEY_SLASH: CGKeyCode = 44;           // /
pub(crate) const KEY_BACKSLASH: CGKeyCode = 42;       // \
pub(crate) const KEY_BRACKET_LEFT: CGKeyCode = 33;    // [
pub(crate) const KEY_BRACKET_RIGHT: CGKeyCode = 30;   // ]
pub(crate) const KEY_MINUS: CGKeyCode = 27;           // -
pub(crate) const KEY_EQUAL: CGKeyCode = 24;           // =
pub(crate) const KEY_BACKQUOTE: CGKeyCode = 50;       // `

// Add these constants for modifier keys
pub(crate) const MODIFIER_COMMAND: CGEventFlags = CGEventFlags::CGEventFlagCommand;
pub(crate) const MODIFIER_SHIFT: CGEventFlags = CGEventFlags::CGEventFlagShift;
pub(crate) const MODIFIER_OPTION: CGEventFlags = CGEventFlags::CGEventFlagAlternate;
pub(crate) const MODIFIER_CONTROL: CGEventFlags = CGEventFlags::CGEventFlagControl;
pub(crate) const MODIFIER_FN: CGEventFlags = CGEventFlags::CGEventFlagSecondaryFn;

// Modifier Key Codes (Example values - verify these or use direct mapping)
// These are often used for identifying the key itself, not setting flags.
pub const COMMAND_KEYCODE: CGKeyCode = 55;
pub const SHIFT_KEYCODE: CGKeyCode = 56;
pub const CAPS_LOCK_KEYCODE: CGKeyCode = 57;
pub const OPTION_KEYCODE: CGKeyCode = 58; // Alt/Option
pub const CONTROL_KEYCODE: CGKeyCode = 59;
pub const RIGHT_SHIFT_KEYCODE: CGKeyCode = 60;
pub const RIGHT_OPTION_KEYCODE: CGKeyCode = 61;
pub const RIGHT_CONTROL_KEYCODE: CGKeyCode = 62;
pub const FUNCTION_KEYCODE: CGKeyCode = 63; // Fn key

// Add other key codes as needed...

// Helper function to map key name string to CGKeyCode
pub(crate) fn key_name_to_keycode(key_name: &str) -> Option<CGKeyCode> {
    let key_lower = key_name.to_lowercase();

    // First check our predefined special keys
    match key_lower.as_str() {
        "return" | "enter" => Some(KEY_RETURN),
        "tab" => Some(KEY_TAB),
        "space" => Some(KEY_SPACE),
        "delete" | "backspace" => Some(KEY_DELETE),
        "escape" | "esc" => Some(KEY_ESCAPE),
        "left" | "arrowleft" => Some(KEY_ARROW_LEFT),
        "right" | "arrowright" => Some(KEY_ARROW_RIGHT),
        "down" | "arrowdown" => Some(KEY_ARROW_DOWN),
        "up" | "arrowup" => Some(KEY_ARROW_UP),
        "v" => Some(KEY_V),
        // Punctuation marks
        "," | "comma" => Some(KEY_COMMA),
        "." | "period" => Some(KEY_PERIOD),
        ";" | "semicolon" => Some(KEY_SEMICOLON),
        "'" | "quote" | "apostrophe" => Some(KEY_QUOTE),
        "/" | "slash" => Some(KEY_SLASH),
        "\\" | "backslash" => Some(KEY_BACKSLASH),
        "[" | "bracketleft" | "leftbracket" => Some(KEY_BRACKET_LEFT),
        "]" | "bracketright" | "rightbracket" => Some(KEY_BRACKET_RIGHT),
        "-" | "minus" | "dash" => Some(KEY_MINUS),
        "=" | "equal" | "equals" => Some(KEY_EQUAL),
        "`" | "backquote" | "grave" => Some(KEY_BACKQUOTE),
        // Handle other single-character keys
        _ => {
            // If not a special key, try treating as a single character
            if key_lower.len() == 1 {
                let c = key_lower.chars().next().unwrap();

                // Handle alphabetic keys (a-z)
                if c.is_ascii_alphabetic() {
                    return Some(match c {
                        'a' => 0,
                        'b' => 11,
                        'c' => 8,
                        'd' => 2,
                        'e' => 14,
                        'f' => 3,
                        'g' => 5,
                        'h' => 4,
                        'i' => 34,
                        'j' => 38,
                        'k' => 40,
                        'l' => 37,
                        'm' => 46,
                        'n' => 45,
                        'o' => 31,
                        'p' => 35,
                        'q' => 12,
                        'r' => 15,
                        's' => 1,
                        't' => 17,
                        'u' => 32,
                        'v' => 9,
                        'w' => 13,
                        'x' => 7,
                        'y' => 16,
                        'z' => 6,
                        _ => return None,
                    });
                }

                // Handle numeric keys (0-9)
                if c.is_ascii_digit() {
                    return Some(match c {
                        '0' => 29,
                        '1' => 18,
                        '2' => 19,
                        '3' => 20,
                        '4' => 21,
                        '5' => 23,
                        '6' => 22,
                        '7' => 26,
                        '8' => 28,
                        '9' => 25,
                        _ => return None,
                    });
                }
            }

            // Not recognized
            None
        }
    }
}

// Helper function to map modifier name string to CGEventFlags
pub(crate) fn modifier_name_to_flags(modifier_name: &str) -> Option<CGEventFlags> {
    match modifier_name.to_lowercase().as_str() {
        "command" | "cmd" => Some(MODIFIER_COMMAND),
        "shift" => Some(MODIFIER_SHIFT),
        "option" | "alt" => Some(MODIFIER_OPTION),
        "control" | "ctrl" => Some(MODIFIER_CONTROL),
        "function" | "fn" => Some(MODIFIER_FN),
        _ => None,
    }
}

// Window role constants
// ... (rest of the file remains the same) ...

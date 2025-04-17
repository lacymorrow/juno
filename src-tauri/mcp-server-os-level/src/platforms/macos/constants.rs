use core_graphics::event::CGEventFlags;
use core_graphics::event::CGKeyCode;

// Add these constant definitions instead - these are the official values from Apple's headers
pub(crate) const K_AXVALUE_CGPOINT_TYPE: u32 = 1;
pub(crate) const K_AXVALUE_CGSIZE_TYPE: u32 = 2;

// Add these constant definitions for key codes
pub(crate) const KEY_RETURN: u16 = 36;
pub(crate) const KEY_TAB: u16 = 48;
pub(crate) const KEY_SPACE: u16 = 49;
pub(crate) const KEY_DELETE: u16 = 51;
pub(crate) const KEY_ESCAPE: u16 = 53;
pub(crate) const KEY_ARROW_LEFT: u16 = 123;
pub(crate) const KEY_ARROW_RIGHT: u16 = 124;
pub(crate) const KEY_ARROW_DOWN: u16 = 125;
pub(crate) const KEY_ARROW_UP: u16 = 126;
pub(crate) const KEY_V: u16 = 9;

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
    match key_name.to_lowercase().as_str() {
        "return" | "enter" => Some(KEY_RETURN),
        "tab" => Some(KEY_TAB),
        "space" => Some(KEY_SPACE),
        "delete" | "backspace" => Some(KEY_DELETE),
        "escape" => Some(KEY_ESCAPE),
        "left" | "arrowleft" => Some(KEY_ARROW_LEFT),
        "right" | "arrowright" => Some(KEY_ARROW_RIGHT),
        "down" | "arrowdown" => Some(KEY_ARROW_DOWN),
        "up" | "arrowup" => Some(KEY_ARROW_UP),
        "v" => Some(KEY_V),
        // Add more mappings as needed
        _ => None,
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

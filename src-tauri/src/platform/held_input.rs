//! Is the key that started a hold still physically down?
//!
//! A hold ends on its release event. When that event never arrives (the key
//! went up while the event tap was disabled, a sleep, a secure-input field)
//! the microphone would stay open. The answer used to be a clock: any hold
//! past 30 seconds was force-stopped, which also cut off everyone who simply
//! talked for longer. This asks the hardware instead. A hold whose key is
//! still down keeps going for as long as it is held; one whose key is up
//! missed its release and is stopped.
//!
//! Read from the HID system state (`CGEventSourceFlagsState`,
//! `CGEventSourceButtonState`), which reflects the physical keyboard and
//! mouse whatever app is focused. No Accessibility or Input Monitoring
//! prompt: these report state, not events.

use crate::triggers::{bare_modifier, modifier_word, Binding, Gesture, ModifierKey, TriggerTarget};

/// `NSEventModifierFlag*` / `kCGEventFlagMask*`: the same bits in both.
const SHIFT_FLAG: u64 = 1 << 17;
const CONTROL_FLAG: u64 = 1 << 18;
const OPTION_FLAG: u64 = 1 << 19;
const COMMAND_FLAG: u64 = 1 << 20;
const FN_FLAG: u64 = 1 << 23;
/// `NX_DEVICERALTKEYMASK`: the right Option key itself.
const RIGHT_OPTION_DEVICE_FLAG: u64 = 0x40;

/// The modifiers a flags word says are down.
fn held_modifiers(flags: u64) -> ModifierKey {
    let mut held = ModifierKey::NONE;
    for (bit, key) in [
        (FN_FLAG, ModifierKey::FN),
        (CONTROL_FLAG, ModifierKey::CONTROL),
        (OPTION_FLAG, ModifierKey::OPTION),
        (SHIFT_FLAG, ModifierKey::SHIFT),
        (COMMAND_FLAG, ModifierKey::COMMAND),
    ] {
        if flags & bit != 0 {
            held = held.union(key);
        }
    }
    if flags & OPTION_FLAG != 0 && flags & RIGHT_OPTION_DEVICE_FLAG != 0 {
        held = held.union(ModifierKey::RIGHT_OPTION);
    }
    held
}

/// The modifiers a binding needs down while it is held, when the binding is
/// one whose hold can be read from modifier state.
///
/// A bare modifier is exactly that modifier. A combo ("Control+Option+Space")
/// is its modifiers: nobody holds the combo with them up, and a release of
/// the modifiers is the end of the hold. A combo with no modifier at all
/// cannot be read this way.
fn required_modifiers(shortcut: &str) -> Option<ModifierKey> {
    if let Some(key) = bare_modifier(shortcut) {
        return Some(key);
    }
    let parts: Vec<&str> = shortcut.split('+').map(str::trim).collect();
    let (_, modifiers) = parts.split_last()?;
    let mut need = ModifierKey::NONE;
    for word in modifiers {
        need = need.union(modifier_word(&word.to_ascii_lowercase())?);
    }
    (!need.is_empty()).then_some(need)
}

/// Whether `binding` is held, given the current flags word and a reader for
/// mouse buttons. `None` when this binding cannot be read.
fn binding_held(binding: &Binding, flags: u64, button_down: impl Fn(u16) -> bool) -> Option<bool> {
    match binding {
        Binding::Keyboard { shortcut } => {
            let need = required_modifiers(shortcut)?;
            Some(held_modifiers(flags).contains(need))
        }
        Binding::Mouse { button } => Some(button_down(*button)),
    }
}

/// Whether any of `bindings` is physically down. `None` when none of them
/// can be read, so the caller decides what an unknown means.
pub fn any_held(bindings: &[Binding]) -> Option<bool> {
    let flags = current_flags()?;
    let mut answer = None;
    for binding in bindings {
        match binding_held(binding, flags, button_down) {
            Some(true) => return Some(true),
            Some(false) => answer = Some(false),
            None => {}
        }
    }
    answer
}

/// Whether a hold for `target` still has its key down: any enabled hold
/// trigger for it, since the monitor does not record which one started it.
/// `None` when none of them can be read.
pub fn hold_still_down(app: &tauri::AppHandle, target: TriggerTarget) -> Option<bool> {
    use tauri::Manager;
    let triggers = app.state::<crate::state::AppState>().get_triggers().ok()?;
    let bindings: Vec<Binding> = triggers
        .into_iter()
        .filter(|t| t.enabled && t.gesture == Gesture::Hold && t.target == target)
        .filter_map(|t| t.binding)
        .collect();
    any_held(&bindings)
}

#[cfg(target_os = "macos")]
mod hid {
    /// `kCGEventSourceStateHIDSystemState`: the hardware, not one app's view.
    const HID_SYSTEM_STATE: i32 = 1;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceFlagsState(state_id: i32) -> u64;
        fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
    }

    pub fn flags() -> u64 {
        // SAFETY: a pure state query with no pointers; safe from any thread.
        unsafe { CGEventSourceFlagsState(HID_SYSTEM_STATE) }
    }

    pub fn button(button: u16) -> bool {
        // SAFETY: as above.
        unsafe { CGEventSourceButtonState(HID_SYSTEM_STATE, u32::from(button)) }
    }
}

#[cfg(target_os = "macos")]
fn current_flags() -> Option<u64> {
    Some(hid::flags())
}

#[cfg(not(target_os = "macos"))]
fn current_flags() -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
fn button_down(button: u16) -> bool {
    hid::button(button)
}

#[cfg(not(target_os = "macos"))]
fn button_down(_button: u16) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(shortcut: &str) -> Binding {
        Binding::Keyboard {
            shortcut: shortcut.to_string(),
        }
    }

    const NO_BUTTONS: fn(u16) -> bool = |_| false;

    #[test]
    fn a_bare_modifier_is_held_while_its_flag_is_set() {
        assert_eq!(binding_held(&key("Fn"), FN_FLAG, NO_BUTTONS), Some(true));
        assert_eq!(binding_held(&key("Fn"), 0, NO_BUTTONS), Some(false));
    }

    #[test]
    fn a_combo_is_held_while_all_its_modifiers_are() {
        let combo = key("Control+Option+Space");
        assert_eq!(
            binding_held(&combo, CONTROL_FLAG | OPTION_FLAG, NO_BUTTONS),
            Some(true)
        );
        assert_eq!(binding_held(&combo, CONTROL_FLAG, NO_BUTTONS), Some(false));
    }

    #[test]
    fn right_option_needs_the_right_hand_key() {
        let right = key("Right Option");
        assert_eq!(binding_held(&right, OPTION_FLAG, NO_BUTTONS), Some(false));
        assert_eq!(
            binding_held(&right, OPTION_FLAG | RIGHT_OPTION_DEVICE_FLAG, NO_BUTTONS),
            Some(true)
        );
    }

    #[test]
    fn a_combo_with_no_modifier_cannot_be_read() {
        assert_eq!(binding_held(&key("F13"), 0, NO_BUTTONS), None);
    }

    #[test]
    fn a_mouse_button_reads_its_own_state() {
        let side = Binding::Mouse { button: 3 };
        assert_eq!(binding_held(&side, 0, |b| b == 3), Some(true));
        assert_eq!(binding_held(&side, 0, NO_BUTTONS), Some(false));
    }
}

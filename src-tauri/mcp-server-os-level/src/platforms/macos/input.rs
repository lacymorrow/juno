/*
This file is no longer needed as Enigo is being removed.
Input actions will be handled directly in engine.rs using CGEvent.

Original content commented out:

use crate::{AutomationError, element::UIElement};
use enigo::{Enigo, Key, Keyboard, Settings, Mouse}; // Removed Richtung
use once_cell::sync::Lazy; // For lazy static initialization
use parking_lot::Mutex; // For thread-safe mutable static
use std::{thread, time::Duration};
use tracing::{debug, error, warn}; // Added warn import

// Lazy static initialization of Enigo instance for thread safety
// Removing static ENIGO due to Send/Sync issues
// static ENIGO: Lazy<Mutex<Enigo>> = Lazy::new(|| Mutex::new(Enigo::new(&Settings::default()).unwrap()));

// Helper function to map string representation to enigo::Key
fn string_to_key(key_str: &str) -> Result<Key, AutomationError> {
    match key_str.to_lowercase().as_str() {
        // Modifiers
        "command" | "cmd" | "meta" | "windows" | "win" | "super" => Ok(Key::Meta),
        "option" | "opt" | "alt" => Ok(Key::Alt),
        "control" | "ctrl" => Ok(Key::Control),
        "shift" => Ok(Key::Shift),

        // Special Keys
        "enter" | "return" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "space" | "spacebar" => Ok(Key::Space),
        "backspace" => Ok(Key::Backspace),
        "delete" | "del" => Ok(Key::Delete),
        "escape" | "esc" => Ok(Key::Escape),
        "up" | "up_arrow" => Ok(Key::UpArrow),
        "down" | "down_arrow" => Ok(Key::DownArrow),
        "left" | "left_arrow" => Ok(Key::LeftArrow),
        "right" | "right_arrow" => Ok(Key::RightArrow),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" | "page_up" => Ok(Key::PageUp),
        "pagedown" | "page_down" => Ok(Key::PageDown),
        "capslock" | "caps_lock" => Ok(Key::CapsLock),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),

        // Treat single characters as layout keys
        s if s.chars().count() == 1 => Ok(Key::Layout(s.chars().next().unwrap())),

        _ => Err(AutomationError::InvalidArgument(format!(
            "Invalid or unsupported key name: {}",
            key_str
        ))),
    }
}


// --- Keyboard Actions ---

pub fn press_key(key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
    debug!("Pressing key: '{}' with modifier: {:?}", key_name, modifier);
    // Instantiate Enigo locally
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| AutomationError::Internal(format!("Failed to create Enigo instance: {}", e)))?;

    let main_key = string_to_key(key_name)?;

    let modifier_key = match modifier {
        Some(m_str) => Some(string_to_key(m_str)?),
        None => None,
    };

    // Press modifier if present
    if let Some(m_key) = modifier_key {
        if let Err(e) = enigo.key_down(m_key) {
             return Err(AutomationError::Internal(format!("Enigo failed to press modifier key down {}: {}", modifier.unwrap_or(""), e)));
        }
        // Small delay after modifier press can sometimes help reliability
        thread::sleep(Duration::from_millis(20));
    }

    // Click the main key
    if let Err(e) = enigo.key_click(main_key) {
         // Attempt to release modifier even if main click fails
         if let Some(m_key) = modifier_key {
             let _ = enigo.key_up(m_key); // Ignore error during cleanup
         }
         return Err(AutomationError::Internal(format!("Enigo failed to click key {}: {}", key_name, e)));
    }

    // Release modifier if present
    if let Some(m_key) = modifier_key {
        // Small delay before modifier release can sometimes help reliability
        thread::sleep(Duration::from_millis(20));
        if let Err(e) = enigo.key_up(m_key) {
            // Log error but don't return failure just for modifier release issue
            error!("Enigo failed to release modifier key {}: {}", modifier.unwrap_or(""), e);
        }
    }

    Ok(())
}

pub fn type_text(text: &str) -> Result<(), AutomationError> {
    debug!("Typing text: '{}'", text);
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| AutomationError::Internal(format!("Failed to create Enigo instance: {}", e)))?;
    if let Err(e) = enigo.text(text) {
         Err(AutomationError::Internal(format!("Enigo failed to type text: {}", e)))
    } else {
        Ok(())
    }
}


pub fn hold_key(key: &str) -> Result<(), AutomationError> {
    debug!("Holding key: '{}'", key);
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| AutomationError::Internal(format!("Failed to create Enigo instance: {}", e)))?;
    let key_to_hold = string_to_key(key)?;
     if let Err(e) = enigo.key_down(key_to_hold) {
         Err(AutomationError::Internal(format!("Enigo failed to hold key {}: {}", key, e)))
     } else {
        Ok(())
     }
}

pub fn release_key(key: &str) -> Result<(), AutomationError> {
    debug!("Releasing key: '{}'", key);
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| AutomationError::Internal(format!("Failed to create Enigo instance: {}", e)))?;
    let key_to_release = string_to_key(key)?;
    if let Err(e) = enigo.key_up(key_to_release) {
        Err(AutomationError::Internal(format!("Enigo failed to release key {}: {}", key, e)))
    } else {
        Ok(())
    }
}

// --- Mouse Actions (using Enigo) ---

/* // Removing Enigo mouse functions as they are now handled by CGEvent in engine.rs
pub fn get_mouse_location() -> Result<(i32, i32), AutomationError> {
     let enigo = ENIGO.lock();
     match enigo.location() {
         Ok(coords) => Ok(coords),
         Err(e) => Err(AutomationError::Internal(format!("Enigo failed to get mouse location: {}", e))),
     }
}

pub fn mouse_move_to(x: f64, y: f64) -> Result<(), AutomationError> {
    debug!("Moving mouse to ({}, {})", x, y);
    let mut enigo = ENIGO.lock();
    if let Err(e) = enigo.move_mouse(x as i32, y as i32, enigo::Coordinate::Abs) {
        Err(AutomationError::Internal(format!("Enigo failed to move mouse: {}", e)))
    } else {
        Ok(())
    }
}

pub fn mouse_down(button: enigo::Button) -> Result<(), AutomationError> {
    debug!("Mouse down: {:?}", button);
    let mut enigo = ENIGO.lock();
     if let Err(e) = enigo.button(button, enigo::Direction::Press) {
         Err(AutomationError::Internal(format!("Enigo failed mouse down for {:?}: {}", button, e)))
     } else {
        Ok(())
     }
}

pub fn mouse_up(button: enigo::Button) -> Result<(), AutomationError> {
    debug!("Mouse up: {:?}", button);
    let mut enigo = ENIGO.lock();
    if let Err(e) = enigo.button(button, enigo::Direction::Release) {
        Err(AutomationError::Internal(format!("Enigo failed mouse up for {:?}: {}", button, e)))
    } else {
        Ok(())
    }
}

pub fn mouse_click(button: enigo::Button) -> Result<(), AutomationError> {
    debug!("Mouse click: {:?}", button);
    let mut enigo = ENIGO.lock();
    if let Err(e) = enigo.button(button, enigo::Direction::Click) {
        Err(AutomationError::Internal(format!("Enigo failed mouse click for {:?}: {}", button, e)))
    } else {
        Ok(())
    }
}

pub fn mouse_scroll(direction: &str, amount: f64) -> Result<(), AutomationError> {
     debug!("Scrolling {} by {}", direction, amount);
     let mut enigo = ENIGO.lock();
     let axis = match direction.to_lowercase().as_str() {
         "up" => Richtung::Up,
         "down" => Richtung::Down,
         "left" => Richtung::Left,
         "right" => Richtung::Right,
         _ => return Err(AutomationError::InvalidArgument(format!("Invalid scroll direction: {}", direction))),
     };
     // Enigo scroll takes an i32 length. We'll use the amount as number of 'lines' or 'units'.
     let length = amount.round() as i32;
     if length == 0 {
         return Ok(()); // No scrolling needed
     }

     if let Err(e) = enigo.scroll(length, axis) {
          Err(AutomationError::Internal(format!("Enigo failed to scroll {} by {}: {}", direction, length, e)))
     } else {
        Ok(())
     }
}
*/

// --- Other ---
pub fn wait(duration_ms: u64) -> Result<(), AutomationError> {
    debug!("Waiting for {} ms", duration_ms);
    thread::sleep(Duration::from_millis(duration_ms));
    Ok(())
}

// --- Stubs or simplified versions for functions previously using AXUIElement ---
// These need careful review and potential reimplementation if direct element interaction is needed
// beyond what Enigo provides (e.g., getting text, attributes).

pub fn get_element_text(_element: &UIElement) -> Result<String, AutomationError> { // Changed to &UIElement
    // Enigo cannot directly get text from an arbitrary element.
    // This might require using the clipboard or AX API again.
    Err(AutomationError::UnsupportedOperation("Getting element text directly is not supported by Enigo implementation.".to_string()))
}

pub fn click_element(_element: &UIElement, _hold_keys: Option<Vec<String>>) -> Result<(), AutomationError> { // Changed to &UIElement
     // Enigo clicks at coordinates, not elements. Need element bounds first.
     // This requires integrating AX API calls to get bounds before using Enigo mouse actions.
     Err(AutomationError::UnsupportedOperation("Clicking element requires element bounds; not implemented in base Enigo switch.".to_string()))
 }

pub fn type_into_element(_element: &UIElement, text: &str, _hold_keys: Option<Vec<String>>) -> Result<(), AutomationError> { // Changed to &UIElement
    // Could potentially focus element first (using AX API), then use enigo.text()
    // For now, just use global type_text as a fallback.
    warn!("type_into_element called, falling back to global type_text. Element focusing not implemented.");
    // type_text(text) // This would call the Enigo version, which we are removing
    Err(AutomationError::UnsupportedOperation("Typing into specific element requires focus + CGEvent implementation".to_string()))
}

pub fn press_key_in_element(_element: &UIElement, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> { // Changed to &UIElement
    // Could potentially focus element first (using AX API), then use enigo press_key
    // For now, just use global press_key as a fallback.
     warn!("press_key_in_element called, falling back to global press_key. Element focusing not implemented.");
     // press_key(key_name, modifier) // This would call the Enigo version, which we are removing
     Err(AutomationError::UnsupportedOperation("Pressing key in specific element requires focus + CGEvent implementation".to_string()))
}

*/

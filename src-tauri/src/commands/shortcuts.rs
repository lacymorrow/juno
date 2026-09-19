// Commands for managing keyboard shortcuts configuration

use crate::settings::manager::SettingsManager;
use crate::state::{AppState, KeyboardShortcuts};

use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tracing::{error, info, warn};

/// Get the current keyboard shortcuts configuration
#[tauri::command]
pub async fn get_keyboard_shortcuts(
    state: State<'_, AppState>,
) -> Result<KeyboardShortcuts, String> {
    state
        .get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))
}

/// Load keyboard shortcuts from centralized settings
pub async fn load_shortcuts_from_centralized_settings(
    app: &AppHandle,
    state: &AppState,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    match settings_manager.get_keyboard_shortcuts().await {
        Ok(settings_shortcuts) => {
            // Convert from settings::KeyboardShortcuts to state::KeyboardShortcuts
            let state_shortcuts = convert_settings_to_state_shortcuts(&settings_shortcuts);
            state
                .set_keyboard_shortcuts(state_shortcuts)
                .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;
            info!("Loaded keyboard shortcuts from centralized settings");
        }
        Err(e) => {
            warn!(
                "Failed to load shortcuts from centralized settings: {}, using defaults",
                e
            );
            let default_shortcuts = crate::state::KeyboardShortcuts::default();
            state
                .set_keyboard_shortcuts(default_shortcuts)
                .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;
        }
    }

    Ok(())
}

/// Convert from settings::KeyboardShortcuts to state::KeyboardShortcuts.
///
/// The stop and settings combos are taken from the constants, not from the
/// store. They are no longer configurable, so a value written by an older
/// build (or by hand) must not be able to move Escape or Cmd+Comma: reading
/// the constant here is what makes "not configurable" true rather than merely
/// hidden.
fn convert_settings_to_state_shortcuts(
    settings: &crate::settings::KeyboardShortcuts,
) -> crate::state::KeyboardShortcuts {
    crate::state::KeyboardShortcuts {
        agent_mode: settings.agent_mode.clone(),
        dictation_input: settings.dictation_input.clone(),
        stop_current_task: crate::constants::settings::defaults::STOP_CURRENT_TASK.to_string(),
        open_settings: crate::constants::settings::defaults::OPEN_SETTINGS.to_string(),
    }
}

/// Validate shortcut format with enhanced checks and detailed error messages
fn validate_shortcut_format(shortcut: &str) -> Result<(), String> {
    if shortcut.trim().is_empty() {
        return Err("Shortcut cannot be empty".to_string());
    }

    // Try to parse the shortcut to ensure it's valid
    use crate::parse_shortcut_string;
    if parse_shortcut_string(shortcut).is_none() {
        return Err(format!("Invalid shortcut format: '{}'. Use combinations like 'Alt+D', 'Ctrl+Shift+F1', 'Cmd+Space', etc.", shortcut));
    }

    // Check for potentially problematic shortcuts with enhanced platform-specific detection
    let lower_shortcut = shortcut.to_lowercase().replace(" ", "");

    // Enhanced system shortcuts detection with platform awareness
    let system_shortcuts = vec![
        ("cmd+q", "Quit application", true),       // Critical on macOS
        ("ctrl+q", "Quit application", false),     // Critical on Linux/Windows
        ("cmd+w", "Close window", true),           // Critical on macOS
        ("ctrl+w", "Close window", false),         // Critical on Linux/Windows
        ("cmd+a", "Select all", true),             // Common on macOS
        ("ctrl+a", "Select all", false),           // Common on Linux/Windows
        ("cmd+c", "Copy", true),                   // Common on macOS
        ("ctrl+c", "Copy", false),                 // Common on Linux/Windows
        ("cmd+v", "Paste", true),                  // Common on macOS
        ("ctrl+v", "Paste", false),                // Common on Linux/Windows
        ("cmd+x", "Cut", true),                    // Common on macOS
        ("ctrl+x", "Cut", false),                  // Common on Linux/Windows
        ("cmd+z", "Undo", true),                   // Common on macOS
        ("ctrl+z", "Undo", false),                 // Common on Linux/Windows
        ("cmd+y", "Redo", true),                   // Common on macOS
        ("ctrl+y", "Redo", false),                 // Common on Linux/Windows
        ("cmd+shift+z", "Redo", true),             // Alternative redo on macOS
        ("ctrl+shift+z", "Redo", false),           // Alternative redo on Linux/Windows
        ("cmd+tab", "Switch applications", true),  // Critical on macOS
        ("ctrl+tab", "Switch tabs", false),        // Common on Linux/Windows
        ("alt+tab", "Switch applications", false), // Critical on Windows/Linux
        ("cmd+space", "Spotlight search", true),   // Critical on macOS
        ("cmd+shift+space", "Previous input source", true), // macOS system
        ("ctrl+space", "Input method/Autocomplete", false), // Common on Linux/Windows
        ("cmd+`", "Switch windows", true),         // macOS window cycling
        ("alt+`", "Switch windows", false),        // Windows/Linux alt-tab variant
        ("f11", "Fullscreen toggle", false),       // Cross-platform
        ("alt+f4", "Close window", false),         // Critical on Windows
        ("cmd+m", "Minimize window", true),        // macOS minimize
        ("ctrl+alt+del", "System interrupt", false), // Windows system
        ("cmd+ctrl+space", "Emoji picker", true),  // macOS emoji
        ("cmd+option+esc", "Force quit dialog", true), // macOS force quit
        ("ctrl+shift+esc", "Task manager", false), // Windows task manager
        ("cmd+shift+3", "Screenshot", true),       // macOS full screenshot
        ("cmd+shift+4", "Area screenshot", true),  // macOS area screenshot
        ("cmd+shift+5", "Screenshot options", true), // macOS screenshot tool
        ("print", "Print screen", false),          // Windows/Linux screenshot
        ("printscreen", "Print screen", false),    // Alternative print screen
    ];

    // Check current platform for more specific warnings
    let is_macos = cfg!(target_os = "macos");

    for (system_shortcut, description, is_macos_specific) in &system_shortcuts {
        if lower_shortcut == system_shortcut.replace(" ", "") {
            // Provide platform-specific warnings
            if *is_macos_specific && is_macos {
                return Err(format!("Warning: '{}' conflicts with the macOS system shortcut for '{}'. This will likely not work as expected.", shortcut, description));
            } else if !*is_macos_specific && !is_macos {
                return Err(format!("Warning: '{}' conflicts with a system shortcut for '{}'. This may not work as expected.", shortcut, description));
            } else if !*is_macos_specific {
                // Cross-platform shortcut warning
                return Err(format!("Warning: '{}' conflicts with a common system shortcut for '{}'. This may not work as expected on some platforms.", shortcut, description));
            }
        }
    }

    // Enhanced standalone key validation with more specific guidance
    if !shortcut.contains('+') {
        let single_key = shortcut.to_lowercase();

        // Expanded list of allowed standalone keys
        let allowed_standalone = [
            "escape",
            "esc",
            "f1",
            "f2",
            "f3",
            "f4",
            "f5",
            "f6",
            "f7",
            "f8",
            "f9",
            "f10",
            "f11",
            "f12",
            "f13",
            "f14",
            "f15",
            "f16",
            "f17",
            "f18",
            "f19",
            "f20", // Extended function keys
            "home",
            "end",
            "pageup",
            "pagedown",
            "insert",
            "delete",
            "printscreen",
            "print",
            "scrolllock",
            "pause",
        ];

        if !allowed_standalone.contains(&single_key.as_str()) {
            // Provide more specific guidance based on key type
            if single_key.len() == 1 && single_key.chars().next().is_some_and(|c| c.is_alphabetic())
            {
                return Err(format!("Letter keys like '{}' should include a modifier (Alt, Ctrl, Cmd, Shift) to avoid conflicts with typing. Try 'Alt+{}' or 'Ctrl+{}'.", shortcut, shortcut.to_uppercase(), shortcut.to_uppercase()));
            } else if single_key.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("Number keys like '{}' should include a modifier to avoid conflicts with typing. Try 'Alt+{}' or 'Ctrl+{}'.", shortcut, shortcut, shortcut));
            } else {
                return Err(format!("The key '{}' should include a modifier (Alt, Ctrl, Cmd, Shift) to avoid conflicts. Try adding a modifier like 'Alt+{}'.", shortcut, shortcut));
            }
        }
    }

    // Additional validation for complex modifier combinations
    let parts: Vec<&str> = shortcut.split('+').map(|s| s.trim()).collect();
    if parts.len() > 4 {
        return Err(
            "Shortcuts with more than 3 modifiers plus one key are not recommended for usability"
                .to_string(),
        );
    }

    // Check for duplicate modifiers (e.g., "Ctrl+Ctrl+A")
    let modifier_parts = &parts[..parts.len() - 1];
    let mut seen_modifiers = std::collections::HashSet::new();
    for modifier in modifier_parts {
        let normalized_modifier = match modifier.to_lowercase().as_str() {
            "alt" | "option" => "alt",
            "cmd" | "command" | "meta" => "cmd",
            "ctrl" | "control" => "ctrl",
            "shift" => "shift",
            _ => modifier,
        };

        if !seen_modifiers.insert(normalized_modifier) {
            return Err(format!(
                "Duplicate modifier '{}' in shortcut '{}'. Each modifier should only appear once.",
                modifier, shortcut
            ));
        }
    }

    // Warn about potentially difficult key combinations
    if modifier_parts.len() >= 3 {
        return Err(format!("Warning: '{}' uses {} modifiers, which may be difficult to press consistently. Consider using fewer modifiers for better usability.", shortcut, modifier_parts.len()));
    }

    Ok(())
}

/// Check for conflicts between shortcuts
fn check_shortcut_conflicts(
    new_shortcut: &str,
    current_shortcuts: &crate::state::KeyboardShortcuts,
    exclude_key: Option<&str>,
) -> Result<(), String> {
    let normalized_new = new_shortcut.to_lowercase().replace(" ", "");

    let shortcuts_to_check = [
        ("agent_mode", &current_shortcuts.agent_mode),
        ("dictation_input", &current_shortcuts.dictation_input),
        ("stop_current_task", &current_shortcuts.stop_current_task),
        ("open_settings", &current_shortcuts.open_settings),
    ];

    for (key, existing_shortcut) in &shortcuts_to_check {
        if let Some(exclude) = exclude_key {
            if *key == exclude {
                continue; // Skip the one we're currently editing
            }
        }

        let normalized_existing = existing_shortcut.to_lowercase().replace(" ", "");
        if normalized_new == normalized_existing {
            return Err(format!(
                "Shortcut '{}' is already assigned to '{}'",
                new_shortcut,
                get_shortcut_display_name_for_validation(key)
            ));
        }
    }

    Ok(())
}

/// Helper function for validation error messages
fn get_shortcut_display_name_for_validation(shortcut_name: &str) -> &str {
    match shortcut_name {
        "agent_mode" => "Agent Mode",
        "dictation_input" => "Dictation Input",
        "stop_current_task" => "Stop Current Task",
        "open_settings" => "Open Settings",
        _ => shortcut_name,
    }
}

/// Register global shortcuts with proper error handling for missing permissions
pub async fn update_global_shortcuts(app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Check if we have Input Monitoring permissions first
    info!("Checking Input Monitoring permissions before registering shortcuts");

    #[cfg(target_os = "macos")]
    {
        // Carbon hot keys (what the global-shortcut plugin registers) work
        // without Input Monitoring, so this never blocks registration. It is a
        // real IOKit read (`IOHIDCheckAccess`), logged so a "my shortcut does
        // nothing" report can be matched against the actual TCC state. Never
        // request here: prompting belongs to onboarding.
        let access = crate::platform::input_monitoring::check_input_monitoring_access();
        info!(
            "macOS detected - Input Monitoring is {:?}; proceeding with shortcut registration",
            access
        );
    }

    // Unregister existing shortcuts with error handling
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!(
            "Failed to unregister existing shortcuts (this is often normal): {}",
            e
        );
    }

    // Import parse_shortcut_string from lib.rs
    use crate::parse_shortcut_string;

    // Register every keyboard binding across the enabled activation triggers.
    // The set is deduped: two triggers may not share a binding (validated on
    // save), but the same combo must never be registered twice. Mouse bindings
    // and voice phrases are handled by their own subsystems, not here.
    //
    // This is also the one place that decides which watcher a key goes to. A
    // bare modifier such as Fn is a keyboard binding like any other as far as
    // the model and the settings window are concerned; it just produces no
    // ordinary key event, so the global-shortcut plugin cannot register it and
    // the flags-changed monitor takes it instead. The branch is here, in the
    // registration layer, rather than in the shape of a binding.
    let triggers = state.get_triggers().unwrap_or_default();
    let mut registered_combos: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut mouse_bindings: Vec<crate::platform::mouse_button_monitor::MouseBinding> = Vec::new();
    let mut modifier_bindings: Vec<crate::platform::modifier_key_monitor::ModifierBinding> =
        Vec::new();
    for trigger in triggers.iter().filter(|t| t.enabled) {
        let Some(binding) = trigger.binding.as_ref() else {
            continue;
        };
        match crate::triggers::watcher_for(binding) {
            crate::triggers::Watcher::GlobalShortcut => {
                let crate::triggers::Binding::Keyboard { shortcut: combo } = binding else {
                    continue;
                };
                let key = combo.to_lowercase();
                if !registered_combos.insert(key) {
                    continue; // already registered this combo
                }
                match parse_shortcut_string(combo) {
                    Some(shortcut) => match app.global_shortcut().register(shortcut) {
                        Ok(()) => info!(
                            "✅ Registered trigger shortcut: {} ({:?} -> {:?})",
                            combo, trigger.method, trigger.target
                        ),
                        Err(e) => error!(
                            "❌ Failed to register trigger shortcut ({}): {} - may be missing Input Monitoring permissions",
                            combo, e
                        ),
                    },
                    None => warn!("Failed to parse trigger shortcut: {}", combo),
                }
            }
            crate::triggers::Watcher::ModifierKey(key) => {
                modifier_bindings.push((key, trigger.method, trigger.target));
            }
            crate::triggers::Watcher::MouseButton(button) => {
                mouse_bindings.push((button, trigger.method, trigger.target));
            }
        }
    }

    // Install (or tear down) the passive mouse-button observer for any
    // mouse-bound triggers. Keyboard goes through the global-shortcut plugin
    // above; mouse buttons cannot, so they use the NSEvent monitor.
    if let Err(e) = crate::platform::mouse_button_monitor::sync(app, mouse_bindings) {
        error!("Failed to sync mouse-button monitor: {}", e);
    }

    // Same again for bare modifiers such as Fn, which produce no ordinary key
    // event and so cannot go through the plugin either.
    if let Err(e) = crate::platform::modifier_key_monitor::sync(app, modifier_bindings) {
        error!("Failed to sync modifier-key monitor: {}", e);
    }

    // There is no voice-activation shortcut any more. A voice trigger being
    // enabled is the on switch, so there was never anything to toggle.

    // NOTE: Escape key is now registered dynamically only when needed
    // This prevents capturing it when there's nothing to cancel

    // Note: Settings shortcut is handled by the menu system

    info!("Completed global shortcut registration (escape key will be registered dynamically when needed)");
    Ok(())
}

/// Check if input monitoring permissions are granted (macOS only)
/// This is required for global shortcuts to work
pub fn check_input_monitoring_permissions() -> Result<bool, String> {
    crate::commands::native_permissions::NativePermissionChecker::check_input_monitoring_permission(
    )
}

/// Validate a keyboard shortcut in real-time (for frontend feedback)
#[tauri::command]
pub async fn validate_keyboard_shortcut(
    state: State<'_, AppState>,
    shortcut_value: String,
    shortcut_name: Option<String>,
) -> Result<String, String> {
    if shortcut_value.trim().is_empty() {
        return Ok("Enter a shortcut combination".to_string());
    }

    // Validate format
    validate_shortcut_format(&shortcut_value)?;

    // Get current shortcuts for conflict checking
    let current_shortcuts = state
        .get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Check for conflicts
    check_shortcut_conflicts(
        &shortcut_value,
        &current_shortcuts,
        shortcut_name.as_deref(),
    )?;

    Ok("Valid shortcut".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::settings::defaults;

    #[test]
    fn stored_stop_and_settings_combos_are_ignored() {
        // Someone upgrading from a build where these were editable may have a
        // custom pair on disk. They are constants now, so the store must not
        // be able to move Escape or Cmd+Comma.
        let stored = crate::settings::KeyboardShortcuts {
            agent_mode: "Option+D".to_string(),
            dictation_input: "Option+Space".to_string(),
            stop_current_task: "Cmd+Escape".to_string(),
            open_settings: "Option+K".to_string(),
        };
        let live = convert_settings_to_state_shortcuts(&stored);
        assert_eq!(live.stop_current_task, defaults::STOP_CURRENT_TASK);
        assert_eq!(live.open_settings, defaults::OPEN_SETTINGS);
        // The activation combos still come from the store, because those are
        // derived from the triggers the person actually configured.
        assert_eq!(live.agent_mode, "Option+D");
        assert_eq!(live.dictation_input, "Option+Space");
    }
}

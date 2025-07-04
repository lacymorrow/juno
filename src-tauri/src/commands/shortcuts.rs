// Commands for managing keyboard shortcuts configuration

use crate::state::{AppState, KeyboardShortcuts};
use crate::settings::manager::SettingsManager;
use tauri::{State, AppHandle};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tracing::{info, error, warn};
use serde_json;
use rand::{seq::SliceRandom, thread_rng, Rng};
use crate::commands::native_permissions::NativePermissionChecker;

/// Get the current keyboard shortcuts configuration
#[tauri::command]
pub async fn get_keyboard_shortcuts(
    state: State<'_, AppState>,
) -> Result<KeyboardShortcuts, String> {
    state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))
}

/// Update a specific keyboard shortcut
#[tauri::command]
pub async fn set_keyboard_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    shortcut_name: String,
    shortcut_value: String,
) -> Result<(), String> {
    // Validate the shortcut format
    validate_shortcut_format(&shortcut_value)?;

    // Get current shortcuts for conflict checking
    let current_shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Check for conflicts (excluding the current shortcut being edited)
    check_shortcut_conflicts(&shortcut_value, &current_shortcuts, Some(&shortcut_name))?;

    // Get current shortcuts and update the specific one
    let mut shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    match shortcut_name.as_str() {
        "agent_mode_toggle" => shortcuts.agent_mode_toggle = shortcut_value.clone(),
        "dictation_input" => shortcuts.dictation_input = shortcut_value.clone(),
        "stop_current_task" => shortcuts.stop_current_task = shortcut_value.clone(),
        "open_settings" => return Err("The settings shortcut cannot be changed".to_string()),
        _ => return Err(format!("Unknown shortcut name: {}", shortcut_name)),
    }

    // Update the shortcut in state
    state.set_keyboard_shortcuts(shortcuts)
        .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;

    // Save to centralized settings
    save_shortcuts_to_centralized_settings(&app, &state).await?;

    // Re-register global shortcuts with new values
    update_global_shortcuts(&app, &state).await?;

    info!("Updated keyboard shortcut '{}' to '{}'", shortcut_name, shortcut_value);
    Ok(())
}

/// Update multiple keyboard shortcuts at once
#[tauri::command]
pub async fn set_keyboard_shortcuts(
    app: AppHandle,
    state: State<'_, AppState>,
    shortcuts: KeyboardShortcuts,
) -> Result<(), String> {
    // Validate all shortcuts
    validate_shortcut_format(&shortcuts.agent_mode_toggle)?;
    validate_shortcut_format(&shortcuts.dictation_input)?;
    validate_shortcut_format(&shortcuts.stop_current_task)?;
    validate_shortcut_format(&shortcuts.open_settings)?;

    // Check for internal conflicts within the new shortcuts
    let shortcut_pairs = [
        ("agent_mode_toggle", &shortcuts.agent_mode_toggle),
        ("dictation_input", &shortcuts.dictation_input),
        ("stop_current_task", &shortcuts.stop_current_task),
        ("open_settings", &shortcuts.open_settings),
    ];

    for (i, (name1, shortcut1)) in shortcut_pairs.iter().enumerate() {
        for (name2, shortcut2) in shortcut_pairs.iter().skip(i + 1) {
            if shortcut1.to_lowercase().replace(" ", "") == shortcut2.to_lowercase().replace(" ", "") {
                return Err(format!("Shortcuts '{}' and '{}' cannot have the same value: '{}'",
                    get_shortcut_display_name_for_validation(name1),
                    get_shortcut_display_name_for_validation(name2),
                    shortcut1
                ));
            }
        }
    }

    // Update state
    state.set_keyboard_shortcuts(shortcuts.clone())
        .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;

    // Save to centralized settings
    save_shortcuts_to_centralized_settings(&app, &state).await?;

    // Re-register global shortcuts
    update_global_shortcuts(&app, &state).await?;

    info!("Updated all keyboard shortcuts");
    Ok(())
}

/// Reset keyboard shortcuts to defaults
#[tauri::command]
pub async fn reset_keyboard_shortcuts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let default_shortcuts = KeyboardShortcuts::default();

    // Update state
    state.set_keyboard_shortcuts(default_shortcuts.clone())
        .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;

    // Save to centralized settings
    save_shortcuts_to_centralized_settings(&app, &state).await?;

    // Re-register global shortcuts
    update_global_shortcuts(&app, &state).await?;

    info!("Reset keyboard shortcuts to defaults");
    Ok(())
}

/// Load keyboard shortcuts from centralized settings
pub async fn load_shortcuts_from_centralized_settings(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    match settings_manager.get_keyboard_shortcuts().await {
        Ok(settings_shortcuts) => {
            // Convert from settings::KeyboardShortcuts to state::KeyboardShortcuts
            let state_shortcuts = convert_settings_to_state_shortcuts(&settings_shortcuts);
            state.set_keyboard_shortcuts(state_shortcuts)
                .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;
            info!("Loaded keyboard shortcuts from centralized settings");
        }
        Err(e) => {
            warn!("Failed to load shortcuts from centralized settings: {}, using defaults", e);
            let default_shortcuts = crate::state::KeyboardShortcuts::default();
            state.set_keyboard_shortcuts(default_shortcuts)
                .map_err(|e| format!("Failed to set keyboard shortcuts: {}", e))?;
        }
    }

    Ok(())
}

/// Save keyboard shortcuts to centralized settings
async fn save_shortcuts_to_centralized_settings(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let state_shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Convert from state::KeyboardShortcuts to settings::KeyboardShortcuts
    let settings_shortcuts = convert_state_to_settings_shortcuts(&state_shortcuts);

    settings_manager.set_keyboard_shortcuts(&settings_shortcuts).await
        .map_err(|e| format!("Failed to save shortcuts to centralized settings: {}", e))?;

    info!("Saved keyboard shortcuts to centralized settings");
    Ok(())
}

/// Convert from settings::KeyboardShortcuts to state::KeyboardShortcuts
fn convert_settings_to_state_shortcuts(settings: &crate::settings::KeyboardShortcuts) -> crate::state::KeyboardShortcuts {
    crate::state::KeyboardShortcuts {
        agent_mode_toggle: settings.agent_mode_toggle.clone(),
        dictation_input: settings.dictation_input.clone(),
        stop_current_task: settings.stop_current_task.clone(),
        open_settings: settings.open_settings.clone(),
    }
}

/// Convert from state::KeyboardShortcuts to settings::KeyboardShortcuts
fn convert_state_to_settings_shortcuts(state: &crate::state::KeyboardShortcuts) -> crate::settings::KeyboardShortcuts {
    crate::settings::KeyboardShortcuts {
        agent_mode_toggle: state.agent_mode_toggle.clone(),
        dictation_input: state.dictation_input.clone(),
        stop_current_task: state.stop_current_task.clone(),
        open_settings: state.open_settings.clone(),
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
        ("cmd+q", "Quit application", true),        // Critical on macOS
        ("ctrl+q", "Quit application", false),      // Critical on Linux/Windows
        ("cmd+w", "Close window", true),            // Critical on macOS
        ("ctrl+w", "Close window", false),          // Critical on Linux/Windows
        ("cmd+a", "Select all", true),              // Common on macOS
        ("ctrl+a", "Select all", false),            // Common on Linux/Windows
        ("cmd+c", "Copy", true),                    // Common on macOS
        ("ctrl+c", "Copy", false),                  // Common on Linux/Windows
        ("cmd+v", "Paste", true),                   // Common on macOS
        ("ctrl+v", "Paste", false),                 // Common on Linux/Windows
        ("cmd+x", "Cut", true),                     // Common on macOS
        ("ctrl+x", "Cut", false),                   // Common on Linux/Windows
        ("cmd+z", "Undo", true),                    // Common on macOS
        ("ctrl+z", "Undo", false),                  // Common on Linux/Windows
        ("cmd+y", "Redo", true),                    // Common on macOS
        ("ctrl+y", "Redo", false),                  // Common on Linux/Windows
        ("cmd+shift+z", "Redo", true),              // Alternative redo on macOS
        ("ctrl+shift+z", "Redo", false),            // Alternative redo on Linux/Windows
        ("cmd+tab", "Switch applications", true),   // Critical on macOS
        ("ctrl+tab", "Switch tabs", false),         // Common on Linux/Windows
        ("alt+tab", "Switch applications", false),  // Critical on Windows/Linux
        ("cmd+space", "Spotlight search", true),    // Critical on macOS
        ("cmd+shift+space", "Previous input source", true), // macOS system
        ("ctrl+space", "Input method/Autocomplete", false), // Common on Linux/Windows
        ("cmd+`", "Switch windows", true),          // macOS window cycling
        ("alt+`", "Switch windows", false),         // Windows/Linux alt-tab variant
        ("f11", "Fullscreen toggle", false),        // Cross-platform
        ("alt+f4", "Close window", false),          // Critical on Windows
        ("cmd+m", "Minimize window", true),         // macOS minimize
        ("ctrl+alt+del", "System interrupt", false), // Windows system
        ("cmd+ctrl+space", "Emoji picker", true),   // macOS emoji
        ("cmd+option+esc", "Force quit dialog", true), // macOS force quit
        ("ctrl+shift+esc", "Task manager", false),  // Windows task manager
        ("cmd+shift+3", "Screenshot", true),        // macOS full screenshot
        ("cmd+shift+4", "Area screenshot", true),   // macOS area screenshot
        ("cmd+shift+5", "Screenshot options", true), // macOS screenshot tool
        ("print", "Print screen", false),           // Windows/Linux screenshot
        ("printscreen", "Print screen", false),     // Alternative print screen
        ("print_screen", "Screenshot", false),      // Windows screenshot
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
            "escape", "esc",
            "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
            "f13", "f14", "f15", "f16", "f17", "f18", "f19", "f20", // Extended function keys
            "home", "end", "pageup", "pagedown", "insert", "delete",
            "printscreen", "print", "scrolllock", "pause"
        ];

        if !allowed_standalone.contains(&single_key.as_str()) {
            // Provide more specific guidance based on key type
            if single_key.len() == 1 && single_key.chars().next().map_or(false, |c| c.is_alphabetic()) {
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
        return Err("Shortcuts with more than 3 modifiers plus one key are not recommended for usability".to_string());
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
            return Err(format!("Duplicate modifier '{}' in shortcut '{}'. Each modifier should only appear once.", modifier, shortcut));
        }
    }

    // Warn about potentially difficult key combinations
    if modifier_parts.len() >= 3 {
        return Err(format!("Warning: '{}' uses {} modifiers, which may be difficult to press consistently. Consider using fewer modifiers for better usability.", shortcut, modifier_parts.len()));
    }

    Ok(())
}

/// Check for conflicts between shortcuts
fn check_shortcut_conflicts(new_shortcut: &str, current_shortcuts: &crate::state::KeyboardShortcuts, exclude_key: Option<&str>) -> Result<(), String> {
    let shortcuts_map = serde_json::from_str::<std::collections::HashMap<String, String>>(
        &serde_json::to_string(current_shortcuts).unwrap_or_default()
    ).unwrap_or_default();

    for (key, val) in shortcuts_map.iter() {
        if Some(key.as_str()) != exclude_key && val.to_lowercase().replace(" ", "") == new_shortcut.to_lowercase().replace(" ", "") {
            return Err(format!("Shortcut '{}' is already in use by '{}'", new_shortcut, get_shortcut_display_name_for_validation(key)));
        }
    }

    Ok(())
}

/// Helper function for validation error messages
fn get_shortcut_display_name_for_validation(shortcut_name: &str) -> &str {
    match shortcut_name {
        "agent_mode_toggle" => "Toggle Agent Mode",
        "dictation_input" => "Dictation Input",
        "stop_current_task" => "Stop Current Task",
        "open_settings" => "Open Settings",
        _ => shortcut_name,
    }
}

/// Register global shortcuts with proper error handling for missing permissions
pub async fn update_global_shortcuts(app: &AppHandle, state: &AppState) -> Result<(), String> {
    info!("Updating global keyboard shortcuts...");

    let shortcuts = state.get_keyboard_shortcuts().map_err(|e| format!("{}", e))?;
    let global_shortcut = app.global_shortcut();

    // Unregister all existing shortcuts before registering new ones
    if let Err(e) = global_shortcut.unregister_all() {
        warn!("Failed to unregister all global shortcuts, continuing anyway: {}", e);
    }

    let shortcuts_to_register = vec![
        ("agent_mode_toggle", shortcuts.agent_mode_toggle.clone()),
        ("dictation_input", shortcuts.dictation_input.clone()),
        ("stop_current_task", shortcuts.stop_current_task.clone()),
        ("open_settings", shortcuts.open_settings.clone()),
    ];

    let mut registered_shortcuts = Vec::new();

    for (name, shortcut_str) in shortcuts_to_register {
        if shortcut_str.is_empty() {
            warn!("Shortcut for '{}' is empty, skipping registration.", name);
            continue;
        }

        match global_shortcut.register(&shortcut_str) {
            Ok(_) => {
                info!("Registered global shortcut for '{}': {}", name, shortcut_str);
                registered_shortcuts.push(shortcut_str);
            }
            Err(e) => {
                error!(
                    "Failed to register global shortcut for '{}' ({}): {}. This might be due to a conflict.",
                    name, shortcut_str, e
                );
                // Continue to register other shortcuts even if one fails
            }
        }
    }

    info!("Global shortcuts updated. Registered: {:?}", registered_shortcuts);
    Ok(())
}

/// Check if the app has input monitoring permissions (macOS specific).
/// This is a simplified check. More robust checks might be needed.
#[cfg(target_os = "macos")]
pub fn check_input_monitoring_permissions() -> Result<bool, String> {
    NativePermissionChecker::check_input_monitoring_permission()
}

#[cfg(not(target_os = "macos"))]
pub fn check_input_monitoring_permissions() -> Result<bool, String> {
    // For non-macOS platforms, we can assume permissions are granted
    // or are not required in the same way.
    Ok(true)
}

/// Validate a keyboard shortcut in real-time (for frontend feedback)
#[tauri::command]
pub async fn validate_keyboard_shortcut(
    state: State<'_, AppState>,
    shortcut_value: String,
    shortcut_name: Option<String>,
) -> Result<String, String> {
    // Validate format first
    if let Err(e) = validate_shortcut_format(&shortcut_value) {
        return Err(e);
    }

    // Check for conflicts
    let current_shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    if let Err(e) = check_shortcut_conflicts(&shortcut_value, &current_shortcuts, shortcut_name.as_deref()) {
        return Err(e);
    }

    Ok("Valid".to_string())
}

/// Get smart shortcut suggestions based on platform and context
#[tauri::command]
pub async fn get_shortcut_suggestions(
    shortcut_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let base_shortcut = match shortcut_name.as_str() {
        "agent_mode_toggle" => "Alt+D",
        "dictation_input" => "Alt+Space",
        "stop_current_task" => "Escape",
        _ => "Alt+S",
    };

    let modifiers = if cfg!(target_os = "macos") {
        vec!["Cmd", "Option", "Ctrl", "Shift"]
    } else {
        vec!["Ctrl", "Alt", "Shift"]
    };

    let keys = vec![
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
        "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
        "Space", "Enter", "Tab",
    ];

    let mut suggestions = Vec::new();
    let mut rng = thread_rng();

    // Get current shortcuts to avoid suggesting conflicting ones
    let current_shortcuts = state.get_keyboard_shortcuts().unwrap_or_default();

    // Add base shortcut if it's not conflicting
    if validate_keyboard_shortcut(state.clone(), base_shortcut.to_string(), Some(shortcut_name.clone())).await.is_ok() {
        suggestions.push(base_shortcut.to_string());
    }

    while suggestions.len() < 5 {
        let mod1 = modifiers.choose(&mut rng).unwrap();
        let mod2 = modifiers.choose(&mut rng).unwrap();
        let key = keys.choose(&mut rng).unwrap();

        let new_shortcut = if mod1 != mod2 && rng.gen() {
            format!("{}+{}", mod1, key)
        } else {
            format!("{}+{}+{}", mod1, mod2, key)
        };

        if !suggestions.contains(&new_shortcut) {
             if validate_keyboard_shortcut(state.clone(), new_shortcut.clone(), Some(shortcut_name.clone())).await.is_ok() {
                suggestions.push(new_shortcut);
            }
        }
    }

    Ok(suggestions)
}

/// Get platform-specific shortcut recommendations and best practices
#[tauri::command]
pub async fn get_shortcut_best_practices() -> Result<serde_json::Value, String> {
    let best_practices = if cfg!(target_os = "macos") {
        serde_json::json!([
            {
                "practice": "Use modifier keys",
                "description": "Combine keys like Cmd, Option, Ctrl, and Shift with a letter or number (e.g., 'Option+D').",
                "good_examples": ["Option+D", "Cmd+Shift+S"],
                "bad_examples": ["D", "Space"]
            },
            {
                "practice": "Avoid single-key shortcuts",
                "description": "Single keys can interfere with typing. Always use at least one modifier.",
                "good_examples": ["Ctrl+C"],
                "bad_examples": ["C"]
            },
            {
                "practice": "Be aware of system shortcuts",
                "description": "Avoid common macOS shortcuts like 'Cmd+Q' (Quit), 'Cmd+W' (Close Window), or 'Cmd+Space' (Spotlight).",
                "good_examples": ["Option+J"],
                "bad_examples": ["Cmd+Q", "Cmd+Space"]
            },
            {
                "practice": "Use mnemonics",
                "description": "Choose letters that relate to the action, like 'D' for Dictation or 'A' for Agent.",
                "good_examples": ["Option+D for Dictation"],
                "bad_examples": ["Ctrl+Q for Dictation"]
            }
        ])
    } else {
        serde_json::json!([
            {
                "practice": "Use modifier keys",
                "description": "Combine keys like Ctrl, Alt, and Shift with a letter or number (e.g., 'Alt+D').",
                "good_examples": ["Alt+D", "Ctrl+Shift+S"],
                "bad_examples": ["D", "Space"]
            },
            {
                "practice": "Avoid single-key shortcuts",
                "description": "Single keys can interfere with typing. Always use at least one modifier.",
                "good_examples": ["Ctrl+C"],
                "bad_examples": ["C"]
            },
            {
                "practice": "Be aware of system shortcuts",
                "description": "Avoid common Windows/Linux shortcuts like 'Ctrl+C' (Copy), 'Ctrl+V' (Paste), or 'Alt+F4' (Close Window).",
                "good_examples": ["Alt+J"],
                "bad_examples": ["Ctrl+C", "Alt+F4"]
            },
            {
                "practice": "Use mnemonics",
                "description": "Choose letters that relate to the action, like 'D' for Dictation or 'A' for Agent.",
                "good_examples": ["Alt+D for Dictation"],
                "bad_examples": ["Ctrl+Q for Dictation"]
            }
        ])
    };
    Ok(best_practices)
}

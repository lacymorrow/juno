// Commands for managing keyboard shortcuts configuration

use crate::state::{AppState, KeyboardShortcuts};
use crate::settings::manager::SettingsManager;
use tauri::{State, AppHandle};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tracing::{debug, info, error, warn};
use serde_json;

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
        "agent_mode" => shortcuts.agent_mode = shortcut_value.clone(),
        "dictation_input" => shortcuts.dictation_input = shortcut_value.clone(),
        "stop_current_task" => shortcuts.stop_current_task = shortcut_value.clone(),
        "voice_activation" => shortcuts.voice_activation = shortcut_value.clone(),
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
    validate_shortcut_format(&shortcuts.agent_mode)?;
    validate_shortcut_format(&shortcuts.dictation_input)?;
    validate_shortcut_format(&shortcuts.stop_current_task)?;
    validate_shortcut_format(&shortcuts.open_settings)?;
    validate_shortcut_format(&shortcuts.voice_activation)?;

    // Check for internal conflicts within the new shortcuts
    let shortcut_pairs = [
        ("agent_mode", &shortcuts.agent_mode),
        ("dictation_input", &shortcuts.dictation_input),
        ("stop_current_task", &shortcuts.stop_current_task),
        ("open_settings", &shortcuts.open_settings),
        ("voice_activation", &shortcuts.voice_activation),
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
        agent_mode: settings.agent_mode.clone(),
        dictation_input: settings.dictation_input.clone(),
        stop_current_task: settings.stop_current_task.clone(),
        open_settings: settings.open_settings.clone(),
        voice_activation: settings.voice_activation.clone(),
    }
}

/// Convert from state::KeyboardShortcuts to settings::KeyboardShortcuts
fn convert_state_to_settings_shortcuts(state: &crate::state::KeyboardShortcuts) -> crate::settings::KeyboardShortcuts {
    crate::settings::KeyboardShortcuts {
        agent_mode: state.agent_mode.clone(),
        dictation_input: state.dictation_input.clone(),
        stop_current_task: state.stop_current_task.clone(),
        open_settings: state.open_settings.clone(),
        voice_activation: state.voice_activation.clone(),
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
            if single_key.len() == 1 && single_key.chars().next().is_some_and(|c| c.is_alphabetic()) {
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
    let normalized_new = new_shortcut.to_lowercase().replace(" ", "");

    let shortcuts_to_check = [
        ("agent_mode", &current_shortcuts.agent_mode),
        ("dictation_input", &current_shortcuts.dictation_input),
        ("stop_current_task", &current_shortcuts.stop_current_task),
        ("open_settings", &current_shortcuts.open_settings),
        ("voice_activation", &current_shortcuts.voice_activation),
    ];

    for (key, existing_shortcut) in &shortcuts_to_check {
        if let Some(exclude) = exclude_key {
            if *key == exclude {
                continue; // Skip the one we're currently editing
            }
        }

        let normalized_existing = existing_shortcut.to_lowercase().replace(" ", "");
        if normalized_new == normalized_existing {
            return Err(format!("Shortcut '{}' is already assigned to '{}'", new_shortcut, get_shortcut_display_name_for_validation(key)));
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
        "voice_activation" => "Voice Activation",
        _ => shortcut_name,
    }
}

/// Register global shortcuts with proper error handling for missing permissions
pub async fn update_global_shortcuts(app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Check if we have Input Monitoring permissions first
    info!("Checking Input Monitoring permissions before registering shortcuts");

    #[cfg(target_os = "macos")]
    {
        // On macOS, we need Input Monitoring permissions for global shortcuts
        // But for now, we'll proceed with registration and handle errors gracefully
        // TODO: Implement proper IOHIDRequestAccess() check in the future
        info!("macOS detected - proceeding with shortcut registration (permission check disabled for now)");
    }

    // Unregister existing shortcuts with error handling
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!("Failed to unregister existing shortcuts (this is often normal): {}", e);
    }

    let shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Import parse_shortcut_string from lib.rs
    use crate::parse_shortcut_string;

    // Register the agent mode shortcut with error handling
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.agent_mode) {
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                info!("✅ Successfully registered agent mode shortcut: {}", shortcuts.agent_mode);
            },
            Err(e) => {
                error!("❌ Failed to register agent mode shortcut ({}): {} - This may be due to missing Input Monitoring permissions", shortcuts.agent_mode, e);
                // Don't fail - continue with other shortcuts
            }
        }
    } else {
        warn!("Failed to parse agent mode shortcut: {}", shortcuts.agent_mode);
    }

    // Register the dictation input shortcut with error handling
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.dictation_input) {
        debug!(
            "Attempting to register dictation input shortcut: {} -> {:?}",
            shortcuts.dictation_input, shortcut
        );
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                info!("✅ Successfully registered dictation input shortcut: {} -> {:?}", shortcuts.dictation_input, shortcut);
            },
            Err(e) => {
                error!("❌ Failed to register dictation input shortcut ({}): {} - This may be due to missing Input Monitoring permissions", shortcuts.dictation_input, e);
                // Don't fail - continue with other shortcuts
            }
        }
    } else {
        warn!("Failed to parse dictation input shortcut: {}", shortcuts.dictation_input);
    }

    // Register the voice activation shortcut with error handling
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.voice_activation) {
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                info!("✅ Successfully registered voice activation shortcut: {}", shortcuts.voice_activation);
            },
            Err(e) => {
                error!("❌ Failed to register voice activation shortcut ({}): {} - This may be due to missing Input Monitoring permissions", shortcuts.voice_activation, e);
            }
        }
    } else {
        warn!("Failed to parse voice activation shortcut: {}", shortcuts.voice_activation);
    }

    // NOTE: Escape key is now registered dynamically only when needed
    // This prevents capturing it when there's nothing to cancel

    // Note: Settings shortcut is handled by the menu system

    info!("Completed global shortcut registration (escape key will be registered dynamically when needed)");
    Ok(())
}

/// Check if input monitoring permissions are granted (macOS only)
/// This is required for global shortcuts to work
pub fn check_input_monitoring_permissions() -> Result<bool, String> {
    crate::commands::native_permissions::NativePermissionChecker::check_input_monitoring_permission()
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
    let current_shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    // Check for conflicts
    check_shortcut_conflicts(&shortcut_value, &current_shortcuts, shortcut_name.as_deref())?;

    Ok("Valid shortcut".to_string())
}

/// Get smart shortcut suggestions based on platform and context
#[tauri::command]
pub async fn get_shortcut_suggestions(
    shortcut_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let is_macos = cfg!(target_os = "macos");

    // Get current shortcuts to avoid suggesting conflicts
    let current_shortcuts = state.get_keyboard_shortcuts()
        .map_err(|e| format!("Failed to get keyboard shortcuts: {}", e))?;

    let mut suggestions = Vec::new();

    match shortcut_name.as_str() {
        "agent_mode" => {
            if is_macos {
                suggestions.extend([
                    "Option+D".to_string(),
                    "Option+A".to_string(),
                    "Cmd+Option+A".to_string(),
                    "Option+J".to_string(),
                    "F5".to_string(),
                    "F6".to_string(),
                    "Cmd+Shift+A".to_string(),
                ]);
            } else {
                suggestions.extend([
                    "Alt+D".to_string(),
                    "Alt+A".to_string(),
                    "Ctrl+Alt+A".to_string(),
                    "Alt+J".to_string(),
                    "F5".to_string(),
                    "F6".to_string(),
                    "Ctrl+Shift+A".to_string(),
                ]);
            }
        },
        "dictation_input" => {
            if is_macos {
                suggestions.extend([
                    "Option+Space".to_string(),
                    "Option+V".to_string(),
                    "Cmd+Option+V".to_string(),
                    "Option+M".to_string(),
                    "F7".to_string(),
                    "F8".to_string(),
                    "Cmd+Shift+V".to_string(),
                ]);
            } else {
                suggestions.extend([
                    "Alt+Space".to_string(),
                    "Alt+V".to_string(),
                    "Ctrl+Alt+V".to_string(),
                    "Alt+M".to_string(),
                    "F7".to_string(),
                    "F8".to_string(),
                    "Ctrl+Shift+V".to_string(),
                ]);
            }
        },
        "stop_current_task" => {
            suggestions.extend([
                "Escape".to_string(),
                "F12".to_string(),
                "Ctrl+Shift+Escape".to_string(),
            ]);
            if is_macos {
                suggestions.push("Cmd+.".to_string());
            } else {
                suggestions.push("Ctrl+Break".to_string());
            }
        },
        "voice_activation" => {
            if is_macos {
                suggestions.extend([
                    "Option+Shift+V".to_string(),
                    "Option+Shift+M".to_string(),
                    "Option+F5".to_string(),
                    "Ctrl+Option+V".to_string(),
                    "Option+Shift+R".to_string(),
                ]);
            } else {
                suggestions.extend([
                    "Alt+Shift+V".to_string(),
                    "Alt+Shift+M".to_string(),
                    "Alt+F5".to_string(),
                    "Ctrl+Alt+V".to_string(),
                    "Alt+Shift+R".to_string(),
                ]);
            }
        },
        _ => {
            // Generic suggestions for unknown shortcut types
            if is_macos {
                suggestions.extend([
                    "Option+F1".to_string(),
                    "Option+F2".to_string(),
                    "Cmd+Option+F1".to_string(),
                    "Cmd+Shift+F1".to_string(),
                ]);
            } else {
                suggestions.extend([
                    "Alt+F1".to_string(),
                    "Alt+F2".to_string(),
                    "Ctrl+Alt+F1".to_string(),
                    "Ctrl+Shift+F1".to_string(),
                ]);
            }
        }
    }

    // Filter out suggestions that conflict with current shortcuts
    let current_values: Vec<String> = vec![
        current_shortcuts.agent_mode.clone(),
        current_shortcuts.dictation_input.clone(),
        current_shortcuts.stop_current_task.clone(),
        current_shortcuts.open_settings.clone(),
        current_shortcuts.voice_activation.clone(),
    ];

    suggestions.retain(|suggestion| {
        let normalized_suggestion = suggestion.to_lowercase().replace(" ", "");
        !current_values.iter().any(|current| {
            current.to_lowercase().replace(" ", "") == normalized_suggestion
        })
    });

    // Validate each suggestion and keep only valid ones
    let mut valid_suggestions = Vec::new();
    for suggestion in suggestions {
        if validate_shortcut_format(&suggestion).is_ok() {
            valid_suggestions.push(suggestion);
        }
    }

    // Limit to top 5 suggestions
    valid_suggestions.truncate(5);

    Ok(valid_suggestions)
}

/// Get platform-specific shortcut recommendations and best practices
#[tauri::command]
pub async fn get_shortcut_best_practices() -> Result<serde_json::Value, String> {
    let is_macos = cfg!(target_os = "macos");

    let best_practices = serde_json::json!({
        "platform": if is_macos { "macOS" } else { "Windows/Linux" },
        "recommendations": {
            "modifiers": {
                "primary": if is_macos { "Cmd" } else { "Ctrl" },
                "secondary": if is_macos { "Option" } else { "Alt" },
                "tertiary": "Shift"
            },
            "avoid": [
                "Single letters without modifiers",
                "Common system shortcuts",
                "More than 3 modifiers",
                if is_macos { "Ctrl+C, Ctrl+V (use Cmd instead)" } else { "Cmd+C, Cmd+V (use Ctrl instead)" }
            ],
            "good_choices": [
                if is_macos { "Option + Letter" } else { "Alt + Letter" },
                "Function keys (F1-F12)",
                if is_macos { "Cmd + Option + Letter" } else { "Ctrl + Alt + Letter" },
                "Function keys with modifiers"
            ],
            "examples": {
                "excellent": [
                    if is_macos { "Option+D" } else { "Alt+D" },
                    "F5",
                    if is_macos { "Cmd+Option+Space" } else { "Ctrl+Alt+Space" }
                ],
                "good": [
                    if is_macos { "Cmd+Shift+F1" } else { "Ctrl+Shift+F1" },
                    if is_macos { "Option+Enter" } else { "Alt+Enter" }
                ],
                "avoid": [
                    "A", "Ctrl+C", "Cmd+Tab", "Alt+Tab", "Space"
                ]
            }
        },
        "tips": [
            "Test shortcuts in different applications to ensure they work globally",
            "Use function keys for frequently used actions",
            "Combine modifiers with less common keys for reliability",
            "Consider ergonomics - avoid hard-to-reach key combinations",
            "Keep shortcuts memorable and logical for your workflow"
        ]
    });

    Ok(best_practices)
}

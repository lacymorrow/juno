// Commands for managing keyboard shortcuts configuration

use crate::state::{AppState, KeyboardShortcuts};
use tauri::{State, AppHandle};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, Code};
use tauri_plugin_store::StoreExt;
use tracing::{info, error, warn};
use serde_json;

// Global escape key management
static ESCAPE_KEY_REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static ESCAPE_KEY_USERS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Get the current keyboard shortcuts configuration
#[tauri::command]
pub async fn get_keyboard_shortcuts(
    state: State<'_, AppState>,
) -> Result<KeyboardShortcuts, String> {
    let shortcuts = state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?
        .clone();

    Ok(shortcuts)
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
    let current_shortcuts = {
        let shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        shortcuts.clone()
    };

    // Check for conflicts (excluding the current shortcut being edited)
    check_shortcut_conflicts(&shortcut_value, &current_shortcuts, Some(&shortcut_name))?;

    // Update the shortcut in state
    {
        let mut shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;

        match shortcut_name.as_str() {
            "agent_mode_toggle" => shortcuts.agent_mode_toggle = shortcut_value.clone(),
            "dictation_input" => shortcuts.dictation_input = shortcut_value.clone(),
            "stop_current_task" => shortcuts.stop_current_task = shortcut_value.clone(),
            "open_settings" => return Err("The settings shortcut cannot be changed".to_string()),
            _ => return Err(format!("Unknown shortcut name: {}", shortcut_name)),
        }
    }

    // Save to persistent storage
    save_shortcuts_to_store(&app, &state).await?;

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
    {
        let mut current_shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        *current_shortcuts = shortcuts.clone();
    }

    // Save to persistent storage
    save_shortcuts_to_store(&app, &state).await?;

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
    {
        let mut shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        *shortcuts = default_shortcuts.clone();
    }

    // Save to persistent storage
    save_shortcuts_to_store(&app, &state).await?;

    // Re-register global shortcuts
    update_global_shortcuts(&app, &state).await?;

    info!("Reset keyboard shortcuts to defaults");
    Ok(())
}

/// Load keyboard shortcuts from persistent storage
pub async fn load_shortcuts_from_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let store = app.store("keyboard_shortcuts.json")
        .map_err(|e| format!("Failed to access shortcuts store: {}", e))?;

        // Try to load each shortcut from the store
    let agent_mode_toggle = store.get("agent_mode_toggle")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let dictation_input = store.get("dictation_input")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let stop_current_task = store.get("stop_current_task")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let open_settings = store.get("open_settings")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    // If any shortcuts exist in store, use them, otherwise use defaults
    if agent_mode_toggle.is_some() || dictation_input.is_some() ||
       stop_current_task.is_some() || open_settings.is_some() {

        let defaults = KeyboardShortcuts::default();
        let loaded_shortcuts = KeyboardShortcuts {
            agent_mode_toggle: agent_mode_toggle.unwrap_or(defaults.agent_mode_toggle),
            dictation_input: dictation_input.unwrap_or(defaults.dictation_input),
            stop_current_task: stop_current_task.unwrap_or(defaults.stop_current_task),
            open_settings: open_settings.unwrap_or(defaults.open_settings),
        };

        let mut shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        *shortcuts = loaded_shortcuts;
        info!("Loaded keyboard shortcuts from store");
    } else {
        info!("No shortcuts found in store, using defaults");
    }

    Ok(())
}

/// Save keyboard shortcuts to persistent storage
async fn save_shortcuts_to_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let store = app.store("keyboard_shortcuts.json")
        .map_err(|e| format!("Failed to access shortcuts store: {}", e))?;

    let shortcuts = state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?
        .clone();

        // Store each shortcut individually
    store.set("agent_mode_toggle", serde_json::Value::String(shortcuts.agent_mode_toggle));
    store.set("dictation_input", serde_json::Value::String(shortcuts.dictation_input));
    store.set("stop_current_task", serde_json::Value::String(shortcuts.stop_current_task));
    store.set("open_settings", serde_json::Value::String(shortcuts.open_settings));

    // Save the store to disk
    store.save()
        .map_err(|e| format!("Failed to save shortcuts store: {}", e))?;

    info!("Saved keyboard shortcuts to store");
    Ok(())
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
            if single_key.len() == 1 && single_key.chars().next().unwrap().is_alphabetic() {
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
        ("agent_mode_toggle", &current_shortcuts.agent_mode_toggle),
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
            return Err(format!("Shortcut '{}' is already assigned to '{}'", new_shortcut, get_shortcut_display_name_for_validation(key)));
        }
    }

    Ok(())
}

/// Helper function for validation error messages
fn get_shortcut_display_name_for_validation(shortcut_name: &str) -> &str {
    match shortcut_name {
        "agent_mode_toggle" => "Agent Mode Toggle",
        "dictation_input" => "Dictation Input",
        "stop_current_task" => "Stop Current Task",
        "open_settings" => "Open Settings",
        _ => shortcut_name,
    }
}

/// Register the escape key for cancellation (only when something can be cancelled)
pub async fn register_escape_key_handler(app_handle: AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    // Increment the user count
    let user_count = ESCAPE_KEY_USERS.fetch_add(1, Ordering::SeqCst) + 1;

    // Only register if not already registered
    if !ESCAPE_KEY_REGISTERED.load(Ordering::SeqCst) {
        let escape_shortcut = Shortcut::new(None, Code::Escape);
        match app_handle.global_shortcut().register(escape_shortcut) {
            Ok(()) => {
                ESCAPE_KEY_REGISTERED.store(true, Ordering::SeqCst);
                info!("Dynamically registered escape key for cancellation (users: {})", user_count);
            },
            Err(e) => {
                // Rollback user count on failure
                ESCAPE_KEY_USERS.fetch_sub(1, Ordering::SeqCst);
                error!("Failed to register escape key shortcut: {} - This may be due to missing Input Monitoring permissions", e);
                return Err(format!("Failed to register escape key: {}", e));
            }
        }
    } else {
        info!("Escape key already registered, increased user count to: {}", user_count);
    }

    Ok(())
}

/// Unregister the escape key (when nothing needs to be cancelled)
pub async fn unregister_escape_key_handler(app_handle: AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    // Decrement the user count
    let user_count = ESCAPE_KEY_USERS.fetch_sub(1, Ordering::SeqCst).saturating_sub(1);

    // Only unregister if no more users and currently registered
    if user_count == 0 && ESCAPE_KEY_REGISTERED.load(Ordering::SeqCst) {
        let escape_shortcut = Shortcut::new(None, Code::Escape);
        match app_handle.global_shortcut().unregister(escape_shortcut) {
            Ok(()) => {
                ESCAPE_KEY_REGISTERED.store(false, Ordering::SeqCst);
                info!("Dynamically unregistered escape key - no more active users");
            },
            Err(e) => {
                // Rollback user count on failure
                ESCAPE_KEY_USERS.fetch_add(1, Ordering::SeqCst);
                warn!("Failed to unregister escape key shortcut: {} - continuing anyway", e);
                // Don't return error for unregistration failures as it's not critical
            }
        }
    } else {
        info!("Escape key still has {} users, keeping registered", user_count);
    }

    Ok(())
}

/// Get current escape key registration status (for debugging)
fn get_escape_key_status_internal() -> (bool, u32) {
    use std::sync::atomic::Ordering;
    (
        ESCAPE_KEY_REGISTERED.load(Ordering::SeqCst),
        ESCAPE_KEY_USERS.load(Ordering::SeqCst)
    )
}

/// Get current escape key registration status (for debugging) - Tauri command
#[tauri::command]
pub async fn get_escape_key_status() -> Result<serde_json::Value, String> {
    let (is_registered, user_count) = get_escape_key_status_internal();
    let description = if user_count == 0 {
        "Escape key is not registered - passes through to other apps".to_string()
    } else if user_count == 1 {
        if is_registered {
            "Escape key is registered for 1 user (agent or dictation)".to_string()
        } else {
            "ERROR: 1 user but not registered".to_string()
        }
    } else {
        if is_registered {
            format!("Escape key is registered for {} users (agent and dictation)", user_count)
        } else {
            format!("ERROR: {} users but not registered", user_count)
        }
    };

    Ok(serde_json::json!({
        "escape_key_registered": is_registered,
        "user_count": user_count,
        "description": description
    }))
}

/// Register global shortcuts with proper error handling for missing permissions
pub async fn update_global_shortcuts(app: &AppHandle, state: &AppState) -> Result<(), String> {
    // Check if we have Input Monitoring permissions first
    info!("Checking Input Monitoring permissions before registering shortcuts");

    #[cfg(target_os = "macos")]
    {
        // On macOS, we need Input Monitoring permissions for global shortcuts
        use std::process::Command;

        let has_input_monitoring = check_input_monitoring_permissions().unwrap_or(false);
        if !has_input_monitoring {
            warn!("Input Monitoring permissions not granted - shortcuts may not work properly");

            // Try to open System Settings for Input Monitoring
            let output = Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
                .output();

            match output {
                Ok(result) if result.status.success() => {
                    info!("Opened System Settings for Input Monitoring permissions");
                },
                Ok(result) => {
                    warn!("Failed to open System Settings: {}", String::from_utf8_lossy(&result.stderr));
                },
                Err(e) => {
                    warn!("Failed to execute open command: {}", e);
                }
            }

            // Return early but don't fail - app should still function
            return Ok(());
        }
    }

    // Unregister existing shortcuts with error handling
    if let Err(e) = app.global_shortcut().unregister_all() {
        warn!("Failed to unregister existing shortcuts (this is often normal): {}", e);
    }

    let shortcuts = state.keyboard_shortcuts.lock()
        .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?
        .clone();

    // Import parse_shortcut_string from lib.rs
    use crate::parse_shortcut_string;

    // Register the agent mode toggle shortcut with error handling
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.agent_mode_toggle) {
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                info!("Registered agent mode toggle shortcut: {}", shortcuts.agent_mode_toggle);
            },
            Err(e) => {
                error!("Failed to register agent mode toggle shortcut ({}): {} - This may be due to missing Input Monitoring permissions", shortcuts.agent_mode_toggle, e);
                // Don't fail - continue with other shortcuts
            }
        }
    } else {
        warn!("Failed to parse agent mode toggle shortcut: {}", shortcuts.agent_mode_toggle);
    }

    // Register the dictation input shortcut with error handling
    if let Some(shortcut) = parse_shortcut_string(&shortcuts.dictation_input) {
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                info!("Registered dictation input shortcut: {}", shortcuts.dictation_input);
            },
            Err(e) => {
                error!("Failed to register dictation input shortcut ({}): {} - This may be due to missing Input Monitoring permissions", shortcuts.dictation_input, e);
                // Don't fail - continue with other shortcuts
            }
        }
    } else {
        warn!("Failed to parse dictation input shortcut: {}", shortcuts.dictation_input);
    }

    // NOTE: Escape key is now registered dynamically only when needed
    // This prevents capturing it when there's nothing to cancel

    // Note: Settings shortcut is handled by the menu system

    info!("Completed global shortcut registration (escape key will be registered dynamically when needed)");
    Ok(())
}

/// Check if Input Monitoring permissions are granted (macOS only)
#[cfg(target_os = "macos")]
fn check_input_monitoring_permissions() -> Result<bool, String> {
    // This is a basic check - in a real implementation you would use proper macOS APIs
    // For now, we'll assume permissions are needed and return true to avoid blocking
    // A proper implementation would use IOHIDRequestAccess() or similar APIs
    Ok(true) // Assume granted for now to avoid blocking the app
}

#[cfg(not(target_os = "macos"))]
fn check_input_monitoring_permissions() -> Result<bool, String> {
    Ok(true) // Always true on non-macOS platforms
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
    if let Err(e) = validate_shortcut_format(&shortcut_value) {
        return Err(e);
    }

    // Get current shortcuts for conflict checking
    let current_shortcuts = {
        let shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        shortcuts.clone()
    };

    // Check for conflicts
    if let Err(e) = check_shortcut_conflicts(&shortcut_value, &current_shortcuts, shortcut_name.as_deref()) {
        return Err(e);
    }

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
    let current_shortcuts = {
        let shortcuts = state.keyboard_shortcuts.lock()
            .map_err(|e| format!("Failed to lock keyboard shortcuts: {}", e))?;
        shortcuts.clone()
    };

    let mut suggestions = Vec::new();

    match shortcut_name.as_str() {
        "agent_mode_toggle" => {
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
        current_shortcuts.agent_mode_toggle.clone(),
        current_shortcuts.dictation_input.clone(),
        current_shortcuts.stop_current_task.clone(),
        current_shortcuts.open_settings.clone(),
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

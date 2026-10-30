// Commands related to keyboard actions (typing, pressing keys)

use crate::commands::debug_utils::{
    log_debug_operation, send_debug_notification, should_enable_debug, validators, DebugConfig,
};
use crate::constants::events;
use crate::state::AppState;
use crate::utils::key_parsing;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use tracing::{error, info};

#[tauri::command]
pub(crate) async fn type_text(
    text: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    log_debug_operation(
        "type_text",
        &format!("Typing text length: {}", text.len()),
        &debug_config,
    );
    // Length only, never the text: this is how the agent types passwords and
    // card numbers, and the log on disk is kept for a week.
    info!("Executing type_text ({} chars)", text.chars().count());

    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);

    match result {
        Ok(_) => {
            info!("Typed {} chars", text.chars().count());

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app_handle,
                    "Type Text",
                    &format!("Typed {} characters", text.len()),
                );
            }

            // Emit key press visualization for typing (show the text being typed)
            if let Err(e) = app_handle.emit(events::ui::KEY_PRESS_VISUALIZATION, json!({
                "key": format!("Type: {}", text.chars().take(crate::constants::ui::text_display::MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH).collect::<String>()),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for type_text: {}", e);
            }
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to type text: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn press_key(
    key: String,
    modifier: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    log_debug_operation(
        "press_key",
        &format!("Key: '{}', modifier: {:?}", key, modifier),
        &debug_config,
    );
    info!(
        "Executing press_key for key: '{}' with modifier: {:?}",
        key, modifier
    );

    let desktop = state.get_desktop()?;

    // Handle the case where key is a combination like "cmd+shift+a"
    let (final_key, final_modifier) = if key.contains('+') {
        // Parse the key combination using our centralized parser
        match key_parsing::split_key_and_modifier(&key) {
            Ok((k, m)) => (k, m.or(modifier)), // Use parsed modifier, or fall back to passed modifier
            Err(e) => {
                let error_msg = format!("Failed to parse key combination '{}': {}", key, e);
                error!("{}", error_msg);
                return Err(error_msg);
            }
        }
    } else {
        // Normal key without combination
        (key, modifier)
    };

    match desktop.press_key(&final_key, final_modifier.as_deref()) {
        Ok(_) => {
            info!(
                "Successfully pressed key: '{}' with modifier: {:?}",
                final_key, final_modifier
            );

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app_handle,
                    "Press Key",
                    &format!(
                        "Pressed '{}' with modifier: {:?}",
                        final_key, final_modifier
                    ),
                );
            }

            // Emit key press visualization event
            if let Err(e) = app_handle.emit(
                events::ui::KEY_PRESS_VISUALIZATION,
                json!({
                    "key": final_key,
                    "modifier": final_modifier
                }),
            ) {
                error!("Failed to emit key press visualization: {}", e);
            }
            Ok(())
        }
        Err(e) => {
            let error_msg = format!(
                "Failed to press key '{}' with modifier '{:?}': {}",
                final_key, final_modifier, e
            );
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn global_type_text(
    text: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    log_debug_operation(
        "global_type_text",
        &format!("Global typing text length: {}", text.len()),
        &debug_config,
    );
    info!(
        "Executing global_type_text ({} chars)",
        text.chars().count()
    );

    let desktop = state.get_desktop()?;
    let result = desktop.type_text(&text);

    match result {
        Ok(_) => {
            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app_handle,
                    "Global Type Text",
                    &format!("Globally typed {} characters", text.len()),
                );
            }

            // Emit key press visualization for global typing
            if let Err(e) = app_handle.emit(events::ui::KEY_PRESS_VISUALIZATION, json!({
                "key": format!("Global: {}", text.chars().take(crate::constants::ui::text_display::MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH).collect::<String>()),
                "modifier": null
            })) {
                error!("Failed to emit key press visualization for global_type_text: {}", e);
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during global type text: {}", e)),
    }
}

#[tauri::command]
pub(crate) async fn hold_key(
    key: String,
    duration_ms: Option<u64>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // The hold duration cap is load-bearing: it is the only bound on how long
    // `hold_key` pins a modifier down, so it runs unconditionally rather than
    // behind the debug-only `validate_inputs` gate (LAC-4013). The companion
    // non-empty-key check was dev-sanity only and is gone with the gate.
    if let Some(duration) = duration_ms {
        validators::reasonable_hold_key_duration_ms(duration)?;
    }

    log_debug_operation(
        "hold_key",
        &format!("Key: '{}', duration: {:?}ms", key, duration_ms),
        &debug_config,
    );
    info!(
        "Executing hold_key for key: '{}', duration: {:?} ms",
        key, duration_ms
    );

    let desktop = state.get_desktop()?;

    // Parse key combinations for hold_key as well
    let parsed_key = if key.contains('+') {
        // For hold_key, we pass the full combination as the key_name
        key.clone()
    } else {
        key.clone()
    };

    let result = desktop.hold_key(&parsed_key, duration_ms);
    match result {
        Ok(_) => {
            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app_handle,
                    "Hold Key",
                    &format!("Held '{}' for {:?}ms", parsed_key, duration_ms),
                );
            }

            // Emit key press visualization for hold key
            if let Err(e) = app_handle.emit(
                events::ui::KEY_PRESS_VISUALIZATION,
                json!({
                    "key": format!("Hold: {}", parsed_key),
                    "modifier": duration_ms.map(|d| format!("{}ms", d))
                }),
            ) {
                error!("Failed to emit key press visualization for hold_key: {}", e);
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during hold key: {}", e)),
    }
}

#[tauri::command]
pub(crate) async fn release_key(
    key: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    log_debug_operation("release_key", &format!("Key: '{}'", key), &debug_config);
    info!("Executing release_key for key: '{}'", key);

    let desktop = state.get_desktop()?;

    // Parse key combinations for release_key as well
    let parsed_key = if key.contains('+') {
        // For release_key, we pass the full combination as the key_name
        key.clone()
    } else {
        key.clone()
    };

    let result = desktop.release_key(&parsed_key);
    match result {
        Ok(_) => {
            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app_handle,
                    "Release Key",
                    &format!("Released '{}'", parsed_key),
                );
            }

            // Emit key press visualization for release key
            if let Err(e) = app_handle.emit(
                events::ui::KEY_PRESS_VISUALIZATION,
                json!({
                    "key": format!("Release: {}", parsed_key),
                    "modifier": null
                }),
            ) {
                error!(
                    "Failed to emit key press visualization for release_key: {}",
                    e
                );
            }
            Ok(())
        }
        Err(e) => Err(format!("Error during release key: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    /// The files that hand typed text to the keyboard. What the agent types
    /// includes passwords and card numbers, and the log on disk is kept for a
    /// week, so a log line may carry a length but never the text itself.
    const TYPING_SOURCES: [(&str, &str); 3] = [
        ("commands/keyboard.rs", include_str!("keyboard.rs")),
        (
            "agent/tools/browser_controller.rs",
            include_str!("../agent/tools/browser_controller.rs"),
        ),
        ("cloud/commands.rs", include_str!("../cloud/commands.rs")),
    ];

    const LOG_MACROS: [&str; 5] = ["info!(", "debug!(", "warn!(", "error!(", "trace!("];

    /// Every log statement in `source`, joined onto one line.
    fn log_statements(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current: Option<String> = None;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if current.is_none() && LOG_MACROS.iter().any(|m| trimmed.contains(m)) {
                current = Some(String::new());
            }
            if let Some(statement) = current.as_mut() {
                statement.push_str(trimmed);
                statement.push(' ');
                if trimmed.ends_with(");") {
                    out.extend(current.take());
                }
            }
        }
        out
    }

    /// Whether a log statement passes `text` or `value` itself as an argument.
    fn prints_typed_text(statement: &str) -> bool {
        statement
            .trim()
            .rsplit('"')
            .next()
            .unwrap_or("")
            .trim_end_matches(';')
            .trim_end_matches(')')
            .split(',')
            .map(str::trim)
            .any(|arg| arg == "text" || arg == "value")
    }

    #[test]
    fn the_check_catches_the_lines_that_used_to_leak() {
        assert!(prints_typed_text(
            "info!(\"Executing type_text for text: '{}'\", text);"
        ));
        assert!(prints_typed_text(
            "log::info!(\"Typing '{}' into element: {}\", value, selector);"
        ));
        assert!(!prints_typed_text(
            "info!(\"Typed {} chars\", text.chars().count());"
        ));
    }

    #[test]
    fn no_log_line_carries_the_typed_text() {
        let mut offenders = Vec::new();
        for (file, source) in TYPING_SOURCES {
            for statement in log_statements(source) {
                if prints_typed_text(&statement) {
                    offenders.push(format!("{}: {}", file, statement.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "log lines must not print typed text:\n  {}",
            offenders.join("\n  ")
        );
    }

    #[test]
    fn the_scan_sees_log_statements() {
        // Guards the test above against passing because it parsed nothing.
        for (file, source) in TYPING_SOURCES {
            assert!(
                !log_statements(source).is_empty(),
                "{} has no log statements; the scan is broken",
                file
            );
        }
    }
}

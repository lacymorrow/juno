use computer_use_ai_sdk::Desktop;
use serde_json::{json, Value};
use std::fs;
use tracing::{error, info, warn};

// --- Parameter Helper Functions ---

// Helper to extract string param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_string_param(input: &Value, key: &str) -> Result<String, Value> {
    input[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| json!({ "error": format!("Missing or invalid string parameter: {}", key) }))
}

// Helper to extract optional string param (Corrected)
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_optional_string_param(input: &Value, key: &str) -> Result<Option<String>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else {
                value.as_str()
                    .map(|s| Ok(Some(s.to_string())))
                    .unwrap_or_else(|| Err(json!({ "error": format!("Invalid optional string parameter type: {}", key) })))
            }
        }
        None => Ok(None), // Key not present is Ok(None)
    }
}

// Helper to extract f64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_f64_param(input: &Value, key: &str) -> Result<f64, Value> {
    input[key]
        .as_f64()
        .ok_or_else(|| json!({ "error": format!("Missing or invalid number parameter: {}", key) }))
}

// Helper to extract u64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_u64_param(input: &Value, key: &str) -> Result<u64, Value> {
    input[key]
        .as_u64()
        .ok_or_else(|| json!({ "error": format!("Missing or invalid integer parameter: {}", key) }))
}

// Helper to extract i64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_i64_param(input: &Value, key: &str) -> Result<i64, Value> {
    input[key]
        .as_i64()
        .ok_or_else(|| json!({ "error": format!("Missing or invalid integer parameter: {}", key) }))
}

// Helper function to get an optional u64 parameter from JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_optional_u64_param(input: &Value, key: &str) -> Result<Option<u64>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else if let Some(num) = value.as_u64() {
                Ok(Some(num))
            } else {
                // Use value.to_string() or describe the type in the error message
                Err(json!({ "error": format!("Invalid type for parameter '{}': expected u64 or null, got type {}", key, value.to_string()) }))
            }
        }
        None => Ok(None), // Key not present
    }
}

// Helper to extract boolean param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
pub fn get_optional_bool_param(input: &Value, key: &str) -> Result<Option<bool>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else if let Some(bool_value) = value.as_bool() {
                Ok(Some(bool_value))
            } else {
                // Use value.to_string() or describe the type in the error message
                Err(json!({ "error": format!("Invalid type for parameter '{}': expected bool or null, got type {}", key, value.to_string()) }))
            }
        }
        None => Ok(None), // Key not present
    }
}


// --- Tool Implementations (Specific Helpers) ---

// Tool function for find and replace in a file
pub fn str_replace_editor(file_path: String, find_text: String, replace_text: String) -> Result<String, String> {
    info!(file_path = %file_path, find = %find_text, "Attempting str_replace_editor");

    // Read the file content
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to read file '{}': {}", file_path, e);
            error!(error = %err_msg, "str_replace_editor failed");
            return Err(err_msg);
        }
    };

    // Perform the replacement
    let new_content = content.replace(&find_text, &replace_text);

    // Write the new content back to the file
    match fs::write(&file_path, new_content) {
        Ok(_) => {
            let success_msg = format!("Successfully updated file '{}'", file_path);
            info!(success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let err_msg = format!("Failed to write file '{}': {}", file_path, e);
            error!(error = %err_msg, "str_replace_editor failed");
            Err(err_msg)
        }
    }
}

// --- Simulation Helper Functions ---

pub fn get_optional_modifier_keys(input: &Value) -> Result<Option<Vec<String>>, Value> {
    input.get("modifier_keys")
        .map_or(Ok(None), |v| {
            if v.is_null() {
                Ok(None)
            } else if let Some(arr) = v.as_array() {
                let keys = arr.iter()
                    .filter_map(|val| val.as_str().map(String::from))
                    .collect::<Vec<String>>();
                // Check if all elements were strings
                if keys.len() == arr.len() {
                    Ok(Some(keys))
                } else {
                    Err(json!({ "error": "Invalid non-string value found in modifier_keys array" }))
                }
            } else {
                Err(json!({ "error": "Invalid type for modifier_keys: expected an array of strings or null" }))
            }
        })
}

/// Holds specified keys, runs an action, then releases keys.
/// Returns the result of the action, or an error if holding/releasing fails.
pub fn hold_keys_and_run<F, T>(
    desktop: &std::sync::Arc<Desktop>,
    keys: &[String],
    action: F,
) -> Result<T, Value>
    where
        F: FnOnce() -> Result<T, computer_use_ai_sdk::AutomationError>,
{
    // Hold keys
    for key in keys {
        if let Err(e) = desktop.hold_key(key) {
            // Attempt to release any already held keys before returning error
            for held_key in keys.iter().take_while(|&k| k != key) {
                desktop.release_key(held_key).ok(); // Ignore release error during cleanup
            }
            return Err(json!({ "error": format!("Failed to hold modifier key '{}': {}", key, e) }));
        }
    }

    // Perform action
    let action_result = action();

    // Release keys (attempt regardless of action result)
    let mut release_errors = Vec::new();
    for key in keys.iter().rev() { // Release in reverse order
        if let Err(e) = desktop.release_key(key) {
            release_errors.push(format!("Failed to release key '{}': {}", key, e));
        }
    }

    // Handle results
    match action_result {
        Ok(res) if release_errors.is_empty() => Ok(res),
        Ok(_) => Err(json!({ "error": format!("Action succeeded, but failed to release modifiers: {}", release_errors.join(", ")) })),
        Err(e) if release_errors.is_empty() => Err(json!({ "error": format!("Action failed: {}. Modifiers released.", e) })),
        Err(e) => Err(json!({ "error": format!("Action failed: {}. Also failed to release modifiers: {}", e, release_errors.join(", ")) })),
    }
}

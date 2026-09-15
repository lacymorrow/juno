//! Tauri commands for background mode and mouse-control consent.
//!
//! The UI answers a pending takeover request through `respond_to_input_control`
//! and reads or changes the two settings behind it. The dock-icon commands named
//! in `constants::commands::input_control` belong to the window-management work
//! and are implemented there, not here.

use super::{
    invalidate_background_mode_cache, record_decision, InputControlDecision, PhysicalCursorRequest,
};
use crate::constants::settings::defaults;
use crate::settings::manager::SettingsManager;
use crate::settings::AgentSettings;
use tauri::{AppHandle, Manager};
use tracing::info;

/// Read the agent settings, failing with a message the UI can show.
async fn load_agent_settings(app_handle: &AppHandle) -> Result<AgentSettings, String> {
    let manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "Settings manager is not available".to_string())?;
    manager.get_agent_settings().await
}

/// Write agent settings back whole, so fields owned by other subsystems survive.
async fn save_agent_settings(
    app_handle: &AppHandle,
    settings: &AgentSettings,
) -> Result<(), String> {
    let manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "Settings manager is not available".to_string())?;
    manager.set_agent_settings(settings).await
}

/// Answer a pending request to take the physical cursor.
///
/// `decision` is "once", "always" or "deny". Returns false when the request is no
/// longer pending, which means it already timed out and the cursor was not taken.
#[tauri::command]
pub async fn respond_to_input_control(
    request_id: String,
    decision: String,
) -> Result<bool, String> {
    let parsed = InputControlDecision::parse(&decision)
        .ok_or_else(|| format!("Unknown input control decision: {}", decision))?;
    info!("Input control response for {}: {:?}", request_id, parsed);
    Ok(record_decision(&request_id, parsed).await)
}

/// Current consent setting for driving the physical mouse: "ask" or "always".
#[tauri::command]
pub async fn get_mouse_control(app_handle: AppHandle) -> Result<String, String> {
    Ok(load_agent_settings(&app_handle).await?.mouse_control)
}

/// Set the consent setting for driving the physical mouse.
#[tauri::command]
pub async fn set_mouse_control(value: String, app_handle: AppHandle) -> Result<(), String> {
    let normalized = value.trim().to_lowercase();
    if normalized != defaults::MOUSE_CONTROL && normalized != defaults::MOUSE_CONTROL_ALWAYS {
        return Err(format!(
            "Invalid mouse control value '{}'. Use '{}' or '{}'.",
            value,
            defaults::MOUSE_CONTROL,
            defaults::MOUSE_CONTROL_ALWAYS
        ));
    }

    let mut settings = load_agent_settings(&app_handle).await?;
    settings.mouse_control = normalized;
    save_agent_settings(&app_handle, &settings).await
}

/// Remember that the user does not want to be offered "stop asking" again.
#[tauri::command]
pub async fn dismiss_mouse_control_prompt(app_handle: AppHandle) -> Result<(), String> {
    let mut settings = load_agent_settings(&app_handle).await?;
    settings.mouse_control_prompt_dismissed = true;
    save_agent_settings(&app_handle, &settings).await
}

/// Is the agent working in the background?
#[tauri::command]
pub async fn get_background_mode(app_handle: AppHandle) -> Result<bool, String> {
    Ok(load_agent_settings(&app_handle).await?.background_mode)
}

/// Turn background operation on or off.
#[tauri::command]
pub async fn set_background_mode(enabled: bool, app_handle: AppHandle) -> Result<(), String> {
    let mut settings = load_agent_settings(&app_handle).await?;
    settings.background_mode = enabled;
    save_agent_settings(&app_handle, &settings).await?;
    // The cache is refreshed on the next read rather than written here, so the
    // persisted value stays the only source of truth.
    invalidate_background_mode_cache();
    super::sync_background_mode(&app_handle).await;
    Ok(())
}

/// Build the request shown to the user when a step needs the physical cursor.
///
/// Kept here so the wording the UI renders lives next to the command that
/// answers it.
pub fn describe_request(tool: &str, target_app: Option<&str>) -> PhysicalCursorRequest {
    let reason = match tool {
        "left_click_drag" => "This drag cannot be done without moving the real pointer.",
        "scroll" => "This view only scrolls under the real pointer.",
        "left_mouse_down" | "left_mouse_up" => {
            "Holding the mouse button here needs the real pointer."
        }
        "mouse_move" => "Moving the real pointer is the only way to reach this.",
        "key" | "hold_key" | "type" => {
            "This app only accepts keystrokes from the real keyboard focus."
        }
        _ => "This app did not accept a click sent in the background.",
    };

    PhysicalCursorRequest {
        tool: tool.to_string(),
        reason: reason.to_string(),
        target_app: target_app.map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_gets_a_plain_reason() {
        for tool in [
            "left_click",
            "left_click_drag",
            "scroll",
            "left_mouse_down",
            "mouse_move",
            "key",
        ] {
            let request = describe_request(tool, Some("Safari"));
            assert_eq!(request.tool, tool);
            assert_eq!(request.target_app.as_deref(), Some("Safari"));
            assert!(!request.reason.is_empty());
            // The line is shown to a person, so it has to read as a sentence.
            assert!(request.reason.ends_with('.'));
        }
    }
}

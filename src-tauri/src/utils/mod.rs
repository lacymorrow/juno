// Add coordinates module
pub mod coordinates;

use computer_use_ai_sdk::Desktop;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Allow dead code as this is a test/debug function
pub(crate) fn run_test_focused_element(desktop: &Desktop) -> Result<(), String> {
    println!("--- Running Test: Get Focused Element (Original Method) ---");
    match desktop.focused_element() {
        Ok(element) => {
            let attrs = element.attributes();
            println!("Focused Element Found:");
            println!("  Role: {}", attrs.role);
            println!("  Label: {:?}", attrs.label);
            println!("  Value: {:?}", attrs.value);
            println!("  Description: {:?}", attrs.description);
            println!("  Properties:");
            for (key, value) in attrs.properties {
                println!("    {}: {:?}", key, value);
            }
             if let Ok((x, y, w, h)) = element.bounds() {
                println!("  Bounds: x={}, y={}, width={}, height={}", x, y, w, h);
            } else {
                println!("  Bounds: Failed to retrieve");
            }
            println!("--- Test Focused Element (Original Method): Success ---");
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to get focused element: {}", e);
            eprintln!("Error: {}", err_msg);
            println!("--- Test Focused Element (Original Method): Failed ---");
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Allow dead code as this is a test/debug function
pub(crate) fn run_test_focused_element_ns() -> Result<(), String> {
    println!("--- Running Test: Get Focused Element (NSWorkspace Method) ---");
    match get_focused_element_ns_workspace(false, true) {
        Ok(element) => {
            let attrs = element.attributes();
            println!("Focused Element Found:");
            println!("  Role: {}", attrs.role);
            println!("  Label: {:?}", attrs.label);
            println!("  Value: {:?}", attrs.value);
            println!("  Description: {:?}", attrs.description);
            println!("  Properties:");
            for (key, value) in attrs.properties {
                println!("    {}: {:?}", key, value);
            }
             if let Ok((x, y, w, h)) = element.bounds() {
                println!("  Bounds: x={}, y={}, width={}, height={}", x, y, w, h);
            } else {
                println!("  Bounds: Failed to retrieve");
            }
            println!("--- Test Focused Element (NSWorkspace Method): Success ---");
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to get focused element via NSWorkspace: {}", e);
            eprintln!("Error: {}", err_msg);
            println!("--- Test Focused Element (NSWorkspace Method): Failed ---");
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Allow dead code as this is a test/debug function
pub(crate) fn run_check_accessibility() -> Result<(), String> {
    println!("--- Running Test: Check Accessibility Permissions ---");
    match computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions(true)
    {
        Ok(granted) => {
            println!("Accessibility permissions granted: {}", granted);
            if !granted {
                println!("Please grant accessibility permissions in System Settings.");
            }
            println!("--- Test Check Accessibility: Success ---");
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to check accessibility permissions: {}", e);
            eprintln!("Error: {}", err_msg);
            println!("--- Test Check Accessibility: Failed ---");
            Err(err_msg)
        }
    }
}

pub mod log_formatter;

use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Get current timestamp in seconds
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Format elapsed time in a human-readable way
pub fn format_elapsed_time(start_ms: u64, end_ms: u64) -> String {
    let elapsed_ms = end_ms.saturating_sub(start_ms);

    if elapsed_ms < 1000 {
        format!("{}ms", elapsed_ms)
    } else if elapsed_ms < 60_000 {
        format!("{:.1}s", elapsed_ms as f64 / 1000.0)
    } else {
        let minutes = elapsed_ms / 60_000;
        let seconds = (elapsed_ms % 60_000) / 1000;
        format!("{}m{}s", minutes, seconds)
    }
}

/// System context information to pass to agents
#[derive(Debug, serde::Serialize)]
pub struct SystemContext {
    pub current_time: String,
    pub current_timestamp: u64,
    pub focused_window: Option<FocusedWindowInfo>,
    pub system_info: SystemInfo,
}

/// Information about the currently focused window
#[derive(Debug, serde::Serialize)]
pub struct FocusedWindowInfo {
    pub title: String,
    pub application: Option<String>,
    pub element_type: Option<String>,
    pub has_text_input: bool,
}

/// Basic system information
#[derive(Debug, serde::Serialize)]
pub struct SystemInfo {
    pub platform: String,
    pub timezone: String,
    pub screen_resolution: Option<(u32, u32)>,
}

/// Gather comprehensive system context for agent initialization
pub async fn gather_system_context(app_state: Option<&crate::state::AppState>) -> Result<SystemContext, String> {
    // Get current time
    let now = chrono::Local::now();
    let current_time = now.format("%A, %B %d, %Y at %I:%M %p").to_string();
    let current_timestamp = current_timestamp_ms();

    // Get timezone
    let timezone = "Local".to_string(); // Simplified timezone info

    // Get focused window information
    let focused_window = get_focused_window_info(app_state).await;

    // Get screen resolution if available
    let screen_resolution = get_screen_resolution();

    Ok(SystemContext {
        current_time,
        current_timestamp,
        focused_window,
        system_info: SystemInfo {
            platform: std::env::consts::OS.to_string(),
            timezone,
            screen_resolution,
        },
    })
}

/// Get information about the currently focused window
async fn get_focused_window_info(_app_state: Option<&crate::state::AppState>) -> Option<FocusedWindowInfo> {
    // Try to get focused element information
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;

        match get_focused_element_ns_workspace(false, false) {
            Ok(element) => {
                let attrs = element.attributes();

                // Try to determine if this is a text input
                let has_text_input = is_text_input_element(&attrs);

                // Extract window/application information
                let title = attrs.label.clone()
                    .or_else(|| attrs.value.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                // Try to get application name if available
                let application = get_application_name_from_element(&element);

                Some(FocusedWindowInfo {
                    title: title.chars().take(100).collect(), // Limit length
                    application,
                    element_type: Some(attrs.role.clone()),
                    has_text_input,
                })
            }
            Err(e) => {
                log::debug!("Could not get focused element info: {}", e);
                None
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Check if an element is likely a text input
#[cfg(target_os = "macos")]
fn is_text_input_element(attrs: &computer_use_ai_sdk::UIElementAttributes) -> bool {
    let role = &attrs.role;
    role.contains("TextField") ||
    role.contains("TextArea") ||
    role.contains("ComboBox") ||
    role.contains("SearchField") ||
    attrs.properties.contains_key("AXValue")
}

/// Try to get application name from element using NSWorkspace
#[cfg(target_os = "macos")]
fn get_application_name_from_element(element: &computer_use_ai_sdk::UIElement) -> Option<String> {
    use objc::{class, msg_send, sel, sel_impl};

    // Try to get application name by checking if element has app-related properties
    let attrs = element.attributes();

    // Check if we can find a PID in the properties
    if let Some(Some(pid_value)) = attrs.properties.get("AXPid") {
        if let Some(pid_str) = pid_value.as_str() {
            if let Ok(app_pid) = pid_str.parse::<i32>() {
                // Use NSWorkspace to get the application name by PID
                unsafe {
                    let workspace_class = class!(NSWorkspace);
                    let shared_workspace: *mut objc::runtime::Object =
                        msg_send![workspace_class, sharedWorkspace];
                    let apps: *mut objc::runtime::Object =
                        msg_send![shared_workspace, runningApplications];
                    let count: usize = msg_send![apps, count];

                    for i in 0..count {
                        let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                        let pid: i32 = msg_send![app, processIdentifier];

                        if pid == app_pid {
                            let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];
                            if !app_name_obj.is_null() {
                                let app_name_str: &str = {
                                    let nsstring = app_name_obj as *const objc::runtime::Object;
                                    let bytes: *const std::os::raw::c_char =
                                        msg_send![nsstring, UTF8String];
                                    let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                                    let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                                    std::str::from_utf8_unchecked(bytes_slice)
                                };
                                return Some(app_name_str.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: try to use NSWorkspace to get the frontmost application
    unsafe {
        let workspace_class = class!(NSWorkspace);
        let shared_workspace: *mut objc::runtime::Object =
            msg_send![workspace_class, sharedWorkspace];
        let frontmost_app: *mut objc::runtime::Object =
            msg_send![shared_workspace, frontmostApplication];

        if !frontmost_app.is_null() {
            let app_name_obj: *mut objc::runtime::Object = msg_send![frontmost_app, localizedName];
            if !app_name_obj.is_null() {
                let app_name_str: &str = {
                    let nsstring = app_name_obj as *const objc::runtime::Object;
                    let bytes: *const std::os::raw::c_char =
                        msg_send![nsstring, UTF8String];
                    let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                    let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                    std::str::from_utf8_unchecked(bytes_slice)
                };
                return Some(app_name_str.to_string());
            }
        }
    }

    None
}

/// Get screen resolution using macOS display APIs
fn get_screen_resolution() -> Option<(u32, u32)> {
    #[cfg(target_os = "macos")]
    {
        // Use the existing screenshot functionality to get screen dimensions
        use computer_use_ai_sdk::platforms::macos::utils::capture_and_encode_screenshot;
        use base64::Engine;

        match capture_and_encode_screenshot() {
            Ok(screenshot_data) => {
                // Parse the base64 PNG to get dimensions
                let engine = base64::engine::general_purpose::STANDARD;
                if let Ok(image_data) = engine.decode(&screenshot_data) {
                    if let Ok(img) = image::load_from_memory(&image_data) {
                        let width = img.width();
                        let height = img.height();
                        log::debug!("Got screen resolution from screenshot: {}x{}", width, height);
                        return Some((width, height));
                    }
                }
            }
            Err(e) => {
                log::debug!("Failed to get screenshot for resolution: {}", e);
            }
        }

        // Fallback: use a reasonable default for macOS if we can't get the actual resolution
        log::debug!("Using fallback screen resolution");
        None
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Format system context as a user-friendly string for agent prompts
pub fn format_system_context_for_agent(context: &SystemContext) -> String {
    let mut context_parts = vec![
        format!("Current time: {}", context.current_time),
        format!("Platform: {}", context.system_info.platform),
    ];

    if let Some(focused) = &context.focused_window {
        context_parts.push(format!(
            "Currently focused: {} ({})",
            focused.title,
            focused.element_type.as_ref().unwrap_or(&"Unknown".to_string())
        ));

        if focused.has_text_input {
            context_parts.push("Note: A text input field is currently focused".to_string());
        }

        if let Some(app) = &focused.application {
            context_parts.push(format!("Application: {}", app));
        }
    }

    if let Some((width, height)) = context.system_info.screen_resolution {
        context_parts.push(format!("Screen resolution: {}×{}", width, height));
    }

    format!("System Context:\n{}", context_parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_elapsed_time() {
        assert_eq!(format_elapsed_time(0, 500), "500ms");
        assert_eq!(format_elapsed_time(0, 1500), "1.5s");
        assert_eq!(format_elapsed_time(0, 65000), "1m5s");
    }
}

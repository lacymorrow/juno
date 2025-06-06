// Add coordinates module
pub mod coordinates;
pub mod command_macros;

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
    pub running_applications: Vec<RunningApplicationInfo>,
    pub installed_applications: Vec<InstalledApplicationInfo>,
    pub user_preferences: UserPreferences,
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

/// Information about a running application
#[derive(Debug, serde::Serialize)]
pub struct RunningApplicationInfo {
    pub name: String,
    pub bundle_id: Option<String>,
    pub pid: i32,
    pub is_frontmost: bool,
    pub activation_policy: String,
}

/// Information about an installed application
#[derive(Debug, serde::Serialize)]
pub struct InstalledApplicationInfo {
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: String,
    pub is_running: bool,
}

/// User preferences for agent behavior
#[derive(Debug, serde::Serialize)]
pub struct UserPreferences {
    pub preferred_applications: Vec<PreferredApp>,
    pub browser_preference: Option<String>,
    pub editor_preference: Option<String>,
    pub terminal_preference: Option<String>,
    pub productivity_suite: Option<String>,
}

/// Preferred application for a specific category
#[derive(Debug, serde::Serialize)]
pub struct PreferredApp {
    pub category: String,
    pub app_name: String,
    pub confidence: f32, // 0.0 to 1.0 based on usage patterns
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

    // Get running applications
    let running_applications = get_running_applications_info().await;

    // Get installed applications (limited scan for performance)
    let installed_applications = get_installed_applications_info(&running_applications).await;

    // Get user preferences based on application usage patterns
    let user_preferences = get_user_preferences(&running_applications, &installed_applications).await;

    Ok(SystemContext {
        current_time,
        current_timestamp,
        focused_window,
        system_info: SystemInfo {
            platform: std::env::consts::OS.to_string(),
            timezone,
            screen_resolution,
        },
        running_applications,
        installed_applications,
        user_preferences,
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

/// Get information about currently running applications
async fn get_running_applications_info() -> Vec<RunningApplicationInfo> {
    #[cfg(target_os = "macos")]
    {
        use objc::{class, msg_send, sel, sel_impl};

        let mut running_apps = Vec::new();

        unsafe {
            let workspace_class = class!(NSWorkspace);
            let shared_workspace: *mut objc::runtime::Object =
                msg_send![workspace_class, sharedWorkspace];
            let apps: *mut objc::runtime::Object = msg_send![shared_workspace, runningApplications];
            let count: usize = msg_send![apps, count];

            // Get the frontmost application for comparison
            let frontmost_app: *mut objc::runtime::Object = msg_send![shared_workspace, frontmostApplication];
            let frontmost_pid: i32 = if !frontmost_app.is_null() {
                msg_send![frontmost_app, processIdentifier]
            } else {
                -1
            };

            for i in 0..count {
                let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                let pid: i32 = msg_send![app, processIdentifier];
                let activation_policy: i32 = msg_send![app, activationPolicy];

                // Get application name
                let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];
                let name = if !app_name_obj.is_null() {
                    let nsstring = app_name_obj as *const objc::runtime::Object;
                    let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                    let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                    let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                    std::str::from_utf8_unchecked(bytes_slice).to_string()
                } else {
                    format!("Unknown App (PID {})", pid)
                };

                // Get bundle identifier
                let bundle_id_obj: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
                let bundle_id = if !bundle_id_obj.is_null() {
                    let nsstring = bundle_id_obj as *const objc::runtime::Object;
                    let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                    let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                    let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                    Some(std::str::from_utf8_unchecked(bytes_slice).to_string())
                } else {
                    None
                };

                // Convert activation policy to string
                let activation_policy_str = match activation_policy {
                    0 => "Regular".to_string(),
                    1 => "Accessory".to_string(),
                    2 => "Prohibited".to_string(),
                    _ => format!("Unknown({})", activation_policy),
                };

                // Skip system background processes unless they're user-relevant
                if let Some(ref bundle_id_str) = bundle_id {
                    if bundle_id_str.contains(".worker") ||
                       bundle_id_str.contains("com.apple.WebKit") ||
                       bundle_id_str.contains("com.apple.CoreServices") ||
                       (bundle_id_str.contains(".helper") && activation_policy == 2) ||
                       (bundle_id_str.contains(".agent") && activation_policy == 2) {
                        continue;
                    }
                }

                running_apps.push(RunningApplicationInfo {
                    name,
                    bundle_id,
                    pid,
                    is_frontmost: pid == frontmost_pid,
                    activation_policy: activation_policy_str,
                });
            }
        }

        log::debug!("Found {} running applications", running_apps.len());
        running_apps
    }

    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// Get information about installed applications (performance-optimized scan)
async fn get_installed_applications_info(running_apps: &[RunningApplicationInfo]) -> Vec<InstalledApplicationInfo> {
    #[cfg(target_os = "macos")]
    {
        let mut installed_apps = Vec::new();
        let running_bundle_ids: std::collections::HashSet<String> = running_apps
            .iter()
            .filter_map(|app| app.bundle_id.clone())
            .collect();

        // Common application directories to scan
        let home_applications = format!("{}Applications", std::env::var("HOME").unwrap_or_default() + "/");
        let app_dirs = vec![
            "/Applications",
            "/System/Applications",
            "/Applications/Utilities",
            &home_applications,
        ];

        for app_dir in app_dirs {
            if let Ok(entries) = std::fs::read_dir(app_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            let path = entry.path();
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                if name.ends_with(".app") {
                                    let app_name = name.trim_end_matches(".app").to_string();
                                    let path_str = path.to_string_lossy().to_string();

                                    // Try to get bundle ID from Info.plist
                                    let bundle_id = get_bundle_id_from_app_path(&path_str);
                                    let is_running = bundle_id.as_ref()
                                        .map(|id| running_bundle_ids.contains(id))
                                        .unwrap_or(false);

                                    installed_apps.push(InstalledApplicationInfo {
                                        name: app_name,
                                        bundle_id,
                                        path: path_str,
                                        is_running,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Remove duplicates by bundle ID or name, preferring /Applications over other locations
        installed_apps.sort_by(|a, b| {
            // Prefer /Applications over other locations
            let a_priority = if a.path.starts_with("/Applications/") { 0 }
                           else if a.path.starts_with("/Applications") { 1 }
                           else { 2 };
            let b_priority = if b.path.starts_with("/Applications/") { 0 }
                           else if b.path.starts_with("/Applications") { 1 }
                           else { 2 };
            a_priority.cmp(&b_priority)
        });

        let mut unique_apps = Vec::new();
        let mut seen_bundle_ids = std::collections::HashSet::new();
        let mut seen_names = std::collections::HashSet::new();

        for app in installed_apps {
            let should_add = if let Some(ref bundle_id) = app.bundle_id {
                !seen_bundle_ids.contains(bundle_id)
            } else {
                !seen_names.contains(&app.name)
            };

            if should_add {
                if let Some(ref bundle_id) = app.bundle_id {
                    seen_bundle_ids.insert(bundle_id.clone());
                }
                seen_names.insert(app.name.clone());
                unique_apps.push(app);
            }
        }

        log::debug!("Found {} installed applications", unique_apps.len());
        unique_apps
    }

    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// Extract bundle ID from an app's Info.plist file
#[cfg(target_os = "macos")]
fn get_bundle_id_from_app_path(app_path: &str) -> Option<String> {
    let info_plist_path = format!("{}/Contents/Info.plist", app_path);

    if let Ok(contents) = std::fs::read_to_string(&info_plist_path) {
        // Simple regex to extract CFBundleIdentifier
        if let Some(start) = contents.find("<key>CFBundleIdentifier</key>") {
            if let Some(string_start) = contents[start..].find("<string>") {
                let string_content_start = start + string_start + 8; // "<string>".len()
                if let Some(string_end) = contents[string_content_start..].find("</string>") {
                    let bundle_id = contents[string_content_start..string_content_start + string_end].trim();
                    return Some(bundle_id.to_string());
                }
            }
        }
    }
    None
}

/// Determine user preferences based on application patterns
async fn get_user_preferences(
    running_apps: &[RunningApplicationInfo],
    installed_apps: &[InstalledApplicationInfo],
) -> UserPreferences {
    let mut preferred_applications = Vec::new();
    let mut browser_preference = None;
    let mut editor_preference = None;
    let mut terminal_preference = None;
    let mut productivity_suite = None;

    // Browser detection based on running and installed apps
    let browsers = ["Safari", "Google Chrome", "Firefox", "Microsoft Edge", "Arc", "Brave Browser"];
    for browser in browsers {
        if running_apps.iter().any(|app| app.name.contains(browser)) {
            browser_preference = Some(browser.to_string());
            preferred_applications.push(PreferredApp {
                category: "browser".to_string(),
                app_name: browser.to_string(),
                confidence: 0.9, // High confidence if currently running
            });
            break;
        } else if installed_apps.iter().any(|app| app.name.contains(browser)) {
            if browser_preference.is_none() {
                browser_preference = Some(browser.to_string());
                preferred_applications.push(PreferredApp {
                    category: "browser".to_string(),
                    app_name: browser.to_string(),
                    confidence: 0.6, // Medium confidence if just installed
                });
            }
        }
    }

    // Editor detection
    let editors = ["Visual Studio Code", "Xcode", "Sublime Text", "Atom", "IntelliJ IDEA", "WebStorm", "PyCharm", "TextEdit", "Vim", "Neovim"];
    for editor in editors {
        if running_apps.iter().any(|app| app.name.contains(editor)) {
            editor_preference = Some(editor.to_string());
            preferred_applications.push(PreferredApp {
                category: "editor".to_string(),
                app_name: editor.to_string(),
                confidence: 0.9,
            });
            break;
        } else if installed_apps.iter().any(|app| app.name.contains(editor)) {
            if editor_preference.is_none() {
                editor_preference = Some(editor.to_string());
                preferred_applications.push(PreferredApp {
                    category: "editor".to_string(),
                    app_name: editor.to_string(),
                    confidence: 0.6,
                });
            }
        }
    }

    // Terminal detection
    let terminals = ["Terminal", "iTerm", "Hyper", "Alacritty", "Kitty", "Warp"];
    for terminal in terminals {
        if running_apps.iter().any(|app| app.name.contains(terminal)) {
            terminal_preference = Some(terminal.to_string());
            preferred_applications.push(PreferredApp {
                category: "terminal".to_string(),
                app_name: terminal.to_string(),
                confidence: 0.9,
            });
            break;
        } else if installed_apps.iter().any(|app| app.name.contains(terminal)) {
            if terminal_preference.is_none() {
                terminal_preference = Some(terminal.to_string());
                preferred_applications.push(PreferredApp {
                    category: "terminal".to_string(),
                    app_name: terminal.to_string(),
                    confidence: 0.6,
                });
            }
        }
    }

    // Productivity suite detection
    let productivity_suites = ["Microsoft Office", "Microsoft Word", "Microsoft Excel", "Microsoft PowerPoint", "Pages", "Numbers", "Keynote", "Google Chrome"]; // Chrome for Google Workspace
    for suite in productivity_suites {
        if running_apps.iter().any(|app| app.name.contains(suite)) {
            if suite.contains("Microsoft") {
                productivity_suite = Some("Microsoft Office".to_string());
            } else if ["Pages", "Numbers", "Keynote"].contains(&suite) {
                productivity_suite = Some("Apple iWork".to_string());
            } else if suite == "Google Chrome" && browser_preference.as_ref().map_or(false, |b| b.contains("Chrome")) {
                productivity_suite = Some("Google Workspace".to_string());
            }
            break;
        }
    }

    UserPreferences {
        preferred_applications,
        browser_preference,
        editor_preference,
        terminal_preference,
        productivity_suite,
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

    // Add running applications information
    if !context.running_applications.is_empty() {
        context_parts.push("\n--- Running Applications ---".to_string());

        // Show frontmost app first
        if let Some(frontmost) = context.running_applications.iter().find(|app| app.is_frontmost) {
            context_parts.push(format!("Frontmost app: {}", frontmost.name));
        }

        // Show other running apps (limit to most relevant ones)
        let mut other_apps: Vec<_> = context.running_applications
            .iter()
            .filter(|app| !app.is_frontmost && app.activation_policy == "Regular")
            .collect();
        other_apps.sort_by(|a, b| a.name.cmp(&b.name));

        if !other_apps.is_empty() {
            let app_names: Vec<String> = other_apps
                .iter()
                .take(10) // Limit to top 10 to avoid overwhelming the context
                .map(|app| app.name.clone())
                .collect();
            context_parts.push(format!("Other running apps: {}", app_names.join(", ")));

            if other_apps.len() > 10 {
                context_parts.push(format!("... and {} more running applications", other_apps.len() - 10));
            }
        }
    }

    // Add user preferences information
    if !context.user_preferences.preferred_applications.is_empty() ||
       context.user_preferences.browser_preference.is_some() ||
       context.user_preferences.editor_preference.is_some() ||
       context.user_preferences.terminal_preference.is_some() {

        context_parts.push("\n--- User Preferences ---".to_string());

        if let Some(browser) = &context.user_preferences.browser_preference {
            context_parts.push(format!("Preferred browser: {}", browser));
        }

        if let Some(editor) = &context.user_preferences.editor_preference {
            context_parts.push(format!("Preferred editor: {}", editor));
        }

        if let Some(terminal) = &context.user_preferences.terminal_preference {
            context_parts.push(format!("Preferred terminal: {}", terminal));
        }

        if let Some(productivity) = &context.user_preferences.productivity_suite {
            context_parts.push(format!("Productivity suite: {}", productivity));
        }
    }

    // Add installed applications summary (just count and categories)
    if !context.installed_applications.is_empty() {
        context_parts.push(format!("\n--- Installed Applications ---"));
        context_parts.push(format!("Total installed apps: {}", context.installed_applications.len()));

        // Categorize some popular applications
        let browsers = context.installed_applications.iter()
            .filter(|app| ["Safari", "Chrome", "Firefox", "Edge", "Arc", "Brave"].iter()
                .any(|browser| app.name.contains(browser)))
            .map(|app| app.name.clone())
            .collect::<Vec<_>>();

        let editors = context.installed_applications.iter()
            .filter(|app| ["Visual Studio Code", "Xcode", "Sublime", "Atom", "IntelliJ", "WebStorm", "PyCharm", "TextEdit"].iter()
                .any(|editor| app.name.contains(editor)))
            .map(|app| app.name.clone())
            .collect::<Vec<_>>();

        if !browsers.is_empty() {
            context_parts.push(format!("Available browsers: {}", browsers.join(", ")));
        }

        if !editors.is_empty() {
            context_parts.push(format!("Available editors: {}", editors.join(", ")));
        }
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

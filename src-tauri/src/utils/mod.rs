// Add coordinates module
pub mod async_runtime;
pub mod atomic_state;
pub mod command_macros;
pub mod coordinates;
pub mod coordinate_validation;
pub mod key_parsing;
pub mod network;
pub mod string_cache;


pub mod log_formatter;
pub mod resource_manager;
pub mod rate_limiter;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Get the preferred directory for agent-created files
///
/// Returns ~/Juno/ as the preferred location for files created autonomously by the agent.
/// This keeps agent work organized and isolated from user files.
///
/// The directory is created if it doesn't exist.
pub fn get_agent_preferred_directory() -> Result<PathBuf, String> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;

    let juno_dir = home_dir.join("Juno");

    // Create the directory if it doesn't exist
    if !juno_dir.exists() {
        std::fs::create_dir_all(&juno_dir)
            .map_err(|e| format!("Failed to create Juno directory: {}", e))?;
    }

    Ok(juno_dir)
}

/// Get a file path in the agent's preferred directory
///
/// # Arguments
/// * `filename` - The filename to create in the Juno directory
///
/// # Returns
/// Full path to the file in ~/Juno/filename
pub fn get_agent_preferred_file_path(filename: &str) -> Result<PathBuf, String> {
    let juno_dir = get_agent_preferred_directory()?;
    Ok(juno_dir.join(filename))
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

/// Permission validation utilities for graceful error handling
pub mod permission_validator {
    use crate::agent::core::AgentError;
    use crate::commands::permissions::check_permissions_status_native;
    use crate::state::AppState;
    use tauri::{AppHandle, Manager};
    use tracing::{debug, info, warn};

    /// Required permissions for different tool categories
    #[derive(Debug, Clone)]
    pub enum RequiredPermission {
        Accessibility,
        ScreenRecording,
        Microphone,
        InputMonitoring,
        AccessibilityAndScreenRecording,
    }

    impl RequiredPermission {
        /// Get user-friendly description of the permission
        pub fn description(&self) -> &'static str {
            match self {
                RequiredPermission::Accessibility => {
                    "accessibility permissions for desktop automation"
                }
                RequiredPermission::ScreenRecording => {
                    "screen recording permissions for screenshots"
                }
                RequiredPermission::Microphone => "microphone access for voice features",
                RequiredPermission::InputMonitoring => "input monitoring for global shortcuts",
                RequiredPermission::AccessibilityAndScreenRecording => {
                    "accessibility and screen recording permissions"
                }
            }
        }

        /// Get specific instructions for granting the permission
        pub fn instructions(&self) -> &'static str {
            match self {
                RequiredPermission::Accessibility => "Please grant accessibility permissions in System Settings > Privacy & Security > Accessibility and restart the app",
                RequiredPermission::ScreenRecording => "Please grant screen recording permissions in System Settings > Privacy & Security > Screen Recording",
                RequiredPermission::Microphone => "Please grant microphone permissions in System Settings > Privacy & Security > Microphone",
                RequiredPermission::InputMonitoring => "Please grant input monitoring permissions in System Settings > Privacy & Security > Input Monitoring",
                RequiredPermission::AccessibilityAndScreenRecording => "Please grant accessibility and screen recording permissions in System Settings > Privacy & Security and restart the app",
            }
        }
    }

    /// Validate that required permissions are granted before tool execution
    pub async fn validate_permission(
        app_handle: &AppHandle,
        required: RequiredPermission,
        tool_name: &str,
    ) -> Result<(), AgentError> {
        // First check if desktop is available (basic accessibility check)
        let app_state = app_handle.state::<AppState>();

        debug!(
            "Validating {} for tool '{}'",
            required.description(),
            tool_name
        );

        // Check the specific permissions based on requirement
        match required {
            RequiredPermission::Accessibility => {
                if !app_state.is_desktop_available() {
                    warn!(
                        "Tool '{}' requires accessibility permissions but desktop is not available",
                        tool_name
                    );
                    return Err(AgentError::PermissionDenied(format!(
                        "Tool '{}' requires {} but they are not granted. {}",
                        tool_name,
                        required.description(),
                        required.instructions()
                    )));
                }
            }
            RequiredPermission::ScreenRecording => {
                // Check screen recording permissions using our permission system
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => {
                        if !permissions.screen_recording.granted {
                            warn!("Tool '{}' requires screen recording permissions but they are not granted", tool_name);
                            return Err(AgentError::PermissionDenied(format!(
                                "Tool '{}' requires {} but they are not granted. {}",
                                tool_name,
                                required.description(),
                                required.instructions()
                            )));
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check screen recording permissions for tool '{}': {}",
                            tool_name, e
                        );
                        return Err(AgentError::PermissionDenied(format!(
                            "Tool '{}' requires {} but permission status could not be verified. {}",
                            tool_name,
                            required.description(),
                            required.instructions()
                        )));
                    }
                }
            }
            RequiredPermission::Microphone => {
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => {
                        if !permissions.microphone.granted {
                            info!("Tool '{}' requires microphone permissions but they are not granted - this may be optional for some tools", tool_name);
                            // Note: Microphone is often optional, so we might just warn instead of error
                            // For now, let's still error to be consistent, but tools can handle this gracefully
                            return Err(AgentError::PermissionDenied(format!(
                                "Tool '{}' requires {} but they are not granted. {}",
                                tool_name,
                                required.description(),
                                required.instructions()
                            )));
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check microphone permissions for tool '{}': {}",
                            tool_name, e
                        );
                        // Microphone permission check failures are often not critical
                        info!(
                            "Proceeding with tool '{}' despite microphone permission check failure",
                            tool_name
                        );
                    }
                }
            }
            RequiredPermission::InputMonitoring => {
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => {
                        if !permissions.input_monitoring.granted {
                            info!("Tool '{}' requires input monitoring permissions but they are not granted - this is often optional", tool_name);
                            // Input monitoring is typically optional for global shortcuts
                            // We'll warn but not block execution
                        }
                    }
                    Err(_) => {
                        // Input monitoring check failures are usually not critical
                        debug!(
                            "Input monitoring permission check failed for tool '{}' - continuing",
                            tool_name
                        );
                    }
                }
            }
            RequiredPermission::AccessibilityAndScreenRecording => {
                // Check both permissions
                if !app_state.is_desktop_available() {
                    warn!(
                        "Tool '{}' requires accessibility permissions but desktop is not available",
                        tool_name
                    );
                    return Err(AgentError::PermissionDenied(format!(
                        "Tool '{}' requires {} but accessibility permissions are not granted. {}",
                        tool_name,
                        required.description(),
                        required.instructions()
                    )));
                }

                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => {
                        if !permissions.screen_recording.granted {
                            warn!("Tool '{}' requires screen recording permissions but they are not granted", tool_name);
                            return Err(AgentError::PermissionDenied(format!(
                                "Tool '{}' requires {} but screen recording permissions are not granted. {}",
                                tool_name,
                                required.description(),
                                required.instructions()
                            )));
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check screen recording permissions for tool '{}': {}",
                            tool_name, e
                        );
                        return Err(AgentError::PermissionDenied(format!(
                            "Tool '{}' requires {} but permission status could not be verified. {}",
                            tool_name,
                            required.description(),
                            required.instructions()
                        )));
                    }
                }
            }
        }

        info!(
            "Permission validation passed for tool '{}' - {} are available",
            tool_name,
            required.description()
        );
        Ok(())
    }

    /// Check if a specific permission type is granted without failing
    pub async fn is_permission_granted(
        app_handle: &AppHandle,
        permission: RequiredPermission,
    ) -> bool {
        match permission {
            RequiredPermission::Accessibility => {
                let app_state = app_handle.state::<AppState>();
                app_state.is_desktop_available()
            }
            RequiredPermission::ScreenRecording => {
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => permissions.screen_recording.granted,
                    Err(_) => false,
                }
            }
            RequiredPermission::Microphone => {
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => permissions.microphone.granted,
                    Err(_) => false,
                }
            }
            RequiredPermission::InputMonitoring => {
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => permissions.input_monitoring.granted,
                    Err(_) => false,
                }
            }
            RequiredPermission::AccessibilityAndScreenRecording => {
                let app_state = app_handle.state::<AppState>();
                if !app_state.is_desktop_available() {
                    return false;
                }
                match check_permissions_status_native(app_handle.clone()).await {
                    Ok(permissions) => permissions.screen_recording.granted,
                    Err(_) => false,
                }
            }
        }
    }

    /// Get tools that require specific permissions (for documentation/error messages)
    pub fn get_tools_requiring_permission(permission: &RequiredPermission) -> Vec<&'static str> {
        match permission {
            RequiredPermission::Accessibility => vec![
                "desktop_click",
                "left_click",
                "right_click",
                "double_click",
                "triple_click",
                "type_text",
                "press_key",
                "key",
                "hold_key",
                "mouse_move",
                "left_click_drag",
                "scroll",
                "get_focused_element_info",
                "open_application",
                "focus_application",
                "get_running_applications",
                "list_windows",
                "focus_window",
                "element_interaction",
            ],
            RequiredPermission::ScreenRecording => vec![
                "capture_screenshot",
                "screenshot",
                "capture_element_screenshot",
                "browser_screenshot",
                "computer",
            ],
            RequiredPermission::Microphone => vec![
                "voice_transcription",
                "microphone_input",
                "always_listening",
            ],
            RequiredPermission::InputMonitoring => vec!["global_shortcuts", "hotkey_registration"],
            RequiredPermission::AccessibilityAndScreenRecording => {
                vec!["computer", "desktop_automation_with_visual_feedback"]
            }
        }
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
    // Enhanced context sources
    pub clipboard_content: Option<String>,
    pub selected_text: Option<String>,
    pub hardware_info: Option<HardwareInfo>,
    pub voice_audio_state: Option<VoiceAudioState>,
    // NEW: Display information for agent context
    pub display_info: Option<DisplayInfo>,
    // Approximate location from IP geolocation (cached)
    pub location_info: Option<GeoLocation>,
}

/// Geographic location resolved via native macOS Core Location or IP geolocation fallback.
/// Cached after first successful lookup so subsequent queries are free.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeoLocation {
    pub city: String,
    pub region: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

/// Cached geolocation result (global, resolved once per app session).
static GEO_CACHE: std::sync::OnceLock<tokio::sync::OnceCell<Option<GeoLocation>>> =
    std::sync::OnceLock::new();

/// Cached slow system context fields (running apps, installed apps, preferences, hardware, display).
/// These change infrequently and are expensive to gather, so we cache them with a short TTL.
struct CachedSlowContext {
    running_applications: Vec<RunningApplicationInfo>,
    installed_applications: Vec<InstalledApplicationInfo>,
    user_preferences: UserPreferences,
    hardware_info: Option<HardwareInfo>,
    voice_audio_state: Option<VoiceAudioState>,
    display_info: Option<DisplayInfo>,
    cached_at: std::time::Instant,
}

static SLOW_CONTEXT_CACHE: std::sync::OnceLock<tokio::sync::Mutex<Option<CachedSlowContext>>> =
    std::sync::OnceLock::new();

fn slow_context_cache() -> &'static tokio::sync::Mutex<Option<CachedSlowContext>> {
    SLOW_CONTEXT_CACHE.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// TTL for cached slow context fields (10 seconds)
const SLOW_CONTEXT_TTL: std::time::Duration = std::time::Duration::from_secs(10);

fn geo_cell() -> &'static tokio::sync::OnceCell<Option<GeoLocation>> {
    GEO_CACHE.get_or_init(tokio::sync::OnceCell::new)
}

/// Resolve location using native macOS Core Location, falling back to IP geolocation.
/// Returns cached result on subsequent calls.
async fn resolve_geolocation() -> Option<GeoLocation> {
    geo_cell()
        .get_or_init(|| async {
            // Try native macOS Core Location first (precise, no network needed)
            #[cfg(target_os = "macos")]
            {
                match fetch_native_location().await {
                    Ok(loc) => {
                        tracing::info!(
                            "📍 Native location resolved: {}, {} ({})",
                            loc.city,
                            loc.region,
                            loc.timezone
                        );
                        return Some(loc);
                    }
                    Err(e) => {
                        tracing::info!(
                            "📍 Native location unavailable ({}), falling back to IP geolocation",
                            e
                        );
                    }
                }
            }

            // Fallback: IP geolocation (city-level accuracy, requires network)
            match fetch_ip_geolocation().await {
                Ok(loc) => {
                    tracing::info!(
                        "📍 IP geolocation resolved: {}, {} ({})",
                        loc.city,
                        loc.region,
                        loc.timezone
                    );
                    Some(loc)
                }
                Err(e) => {
                    tracing::warn!("⚠️ All geolocation methods failed (non-fatal): {}", e);
                    None
                }
            }
        })
        .await
        .clone()
}

/// Embedded Swift script that uses macOS Core Location + CLGeocoder.
/// Outputs a JSON object with location data or an error.
const CORE_LOCATION_SWIFT: &str = r#"
import CoreLocation
import Foundation

class LocationDelegate: NSObject, CLLocationManagerDelegate {
    var done = false
    var result: [String: Any] = ["error": "timeout"]
    let geocoder = CLGeocoder()

    func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard let loc = locations.last else {
            result = ["error": "no_location"]
            done = true
            return
        }
        // Reverse geocode to get city/region/country
        geocoder.reverseGeocodeLocation(loc) { placemarks, error in
            let pm = placemarks?.first
            self.result = [
                "latitude": loc.coordinate.latitude,
                "longitude": loc.coordinate.longitude,
                "city": pm?.locality ?? "Unknown",
                "region": pm?.administrativeArea ?? "Unknown",
                "country": pm?.country ?? "Unknown",
                "timezone": TimeZone.current.identifier
            ]
            self.done = true
        }
    }

    func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        result = ["error": error.localizedDescription]
        done = true
    }

    func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        switch manager.authorizationStatus {
        case .authorizedAlways, .authorized:
            manager.requestLocation()
        case .denied, .restricted:
            result = ["error": "denied"]
            done = true
        case .notDetermined:
            break
        @unknown default:
            break
        }
    }
}

let mgr = CLLocationManager()
let del = LocationDelegate()
mgr.delegate = del
mgr.desiredAccuracy = kCLLocationAccuracyKilometer

switch mgr.authorizationStatus {
case .authorizedAlways, .authorized:
    mgr.requestLocation()
case .notDetermined:
    mgr.requestWhenInUseAuthorization()
case .denied, .restricted:
    del.result = ["error": "denied"]
    del.done = true
@unknown default:
    del.result = ["error": "unknown_auth_status"]
    del.done = true
}

let deadline = Date(timeIntervalSinceNow: 10)
while !del.done && Date() < deadline {
    RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.1))
}

if let data = try? JSONSerialization.data(withJSONObject: del.result),
   let str = String(data: data, encoding: .utf8) {
    print(str)
} else {
    print("{\"error\":\"serialization_failed\"}")
}
"#;

/// Fetch location using macOS native Core Location via a Swift subprocess.
/// This provides GPS/Wi-Fi/cell-tower accuracy (meters) vs IP geolocation (city-level).
#[cfg(target_os = "macos")]
async fn fetch_native_location() -> Result<GeoLocation, String> {
    use std::io::Write;

    // Write Swift script to a temp file
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("juno_location.swift");

    // Write script (blocking I/O in spawn_blocking)
    let script_path_clone = script_path.clone();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::create(&script_path_clone)
            .map_err(|e| format!("Failed to create temp script: {}", e))?;
        file.write_all(CORE_LOCATION_SWIFT.as_bytes())
            .map_err(|e| format!("Failed to write temp script: {}", e))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e: String| e)?;

    // Execute Swift script with timeout
    let script_path_str = script_path.to_string_lossy().to_string();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("/usr/bin/swift")
            .arg(&script_path_str)
            .output()
            .map_err(|e| format!("Failed to execute swift: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e: String| e)?;

    // Clean up temp file (best-effort)
    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Swift script failed: {}", stderr.chars().take(200).collect::<String>()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("Failed to parse location JSON: {} (output: {})", e, stdout.chars().take(100).collect::<String>()))?;

    // Check for error response
    if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
        return Err(format!("Core Location error: {}", err));
    }

    Ok(GeoLocation {
        city: json["city"].as_str().unwrap_or("Unknown").to_string(),
        region: json["region"].as_str().unwrap_or("Unknown").to_string(),
        country: json["country"].as_str().unwrap_or("Unknown").to_string(),
        latitude: json["latitude"].as_f64().unwrap_or(0.0),
        longitude: json["longitude"].as_f64().unwrap_or(0.0),
        timezone: json["timezone"].as_str().unwrap_or("Unknown").to_string(),
    })
}

/// Fallback: IP geolocation via ipapi.co (city-level accuracy, 3s timeout).
async fn fetch_ip_geolocation() -> Result<GeoLocation, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("HTTP client init: {}", e))?;

    let resp: serde_json::Value = client
        .get("https://ipapi.co/json/")
        .header("User-Agent", "Juno-Desktop/1.0")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?
        .json()
        .await
        .map_err(|e| format!("JSON parse failed: {}", e))?;

    Ok(GeoLocation {
        city: resp["city"].as_str().unwrap_or("Unknown").to_string(),
        region: resp["region"].as_str().unwrap_or("Unknown").to_string(),
        country: resp["country_name"]
            .as_str()
            .or_else(|| resp["country"].as_str())
            .unwrap_or("Unknown")
            .to_string(),
        latitude: resp["latitude"].as_f64().unwrap_or(0.0),
        longitude: resp["longitude"].as_f64().unwrap_or(0.0),
        timezone: resp["timezone"].as_str().unwrap_or("Unknown").to_string(),
    })
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
#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningApplicationInfo {
    pub name: String,
    pub bundle_id: Option<String>,
    pub pid: i32,
    pub is_frontmost: bool,
    pub activation_policy: String,
}

/// Information about an installed application
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstalledApplicationInfo {
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: String,
    pub is_running: bool,
}

/// User preferences for agent behavior
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserPreferences {
    pub preferred_applications: Vec<PreferredApp>,
    pub browser_preference: Option<String>,
    pub editor_preference: Option<String>,
    pub terminal_preference: Option<String>,
    pub productivity_suite: Option<String>,
}

/// Preferred application for a specific category
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreferredApp {
    pub category: String,
    pub app_name: String,
    pub confidence: f32, // 0.0 to 1.0 based on usage patterns
}

/// Hardware monitoring information
#[derive(Debug, Clone, serde::Serialize)]
pub struct HardwareInfo {
    pub cpu_usage: Option<f32>,
    pub memory_usage: Option<f32>,
    pub disk_usage: Option<f32>,
    pub screen_resolution: Option<String>,
}

/// Voice and audio state information
#[derive(Debug, Clone, serde::Serialize)]
pub struct VoiceAudioState {
    pub mode: String, // "dictation", "agent", "idle"
    pub is_listening: bool,
    pub is_transcribing: bool,
    pub is_speaking: bool,
    pub current_transcription: Option<String>,
    pub audio_level: f32,
    pub has_error: bool,
    pub error_message: Option<String>,
}

/// Display information for agent context
#[derive(Debug, Clone, serde::Serialize)]
pub struct DisplayInfo {
    pub main_display: Option<MainDisplayInfo>,
    pub all_displays: Vec<DisplayDetails>,
    pub center_point: Option<(i32, i32)>,
    pub standard_resolution: Option<(u32, u32)>,
}

/// Main display information
#[derive(Debug, Clone, serde::Serialize)]
pub struct MainDisplayInfo {
    pub bounds: DisplayBounds,
    pub resolution: (u32, u32),
    pub is_main: bool,
}

/// Display bounds information
#[derive(Debug, Clone, serde::Serialize)]
pub struct DisplayBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Individual display details
#[derive(Debug, Clone, serde::Serialize)]
pub struct DisplayDetails {
    pub id: u32,
    pub bounds: DisplayBounds,
    pub resolution: (u32, u32),
    pub is_main: bool,
}

/// Safely get display information for context
async fn get_display_info_safe() -> Option<DisplayInfo> {
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::display::{get_main_display, get_active_displays};
        use crate::utils::coordinates::get_current_standard_resolution;

        let main_display = match get_main_display() {
            Ok(display) => {
                let main_info = MainDisplayInfo {
                    bounds: DisplayBounds {
                        x: display.bounds.origin.x as i32,
                        y: display.bounds.origin.y as i32,
                        width: display.bounds.size.width as u32,
                        height: display.bounds.size.height as u32,
                    },
                    resolution: (display.bounds.size.width as u32, display.bounds.size.height as u32),
                    is_main: true,
                };
                Some(main_info)
            }
            Err(e) => {
                log::debug!("Could not get main display info: {}", e);
                None
            }
        };

        let all_displays = match get_active_displays() {
            Ok(displays) => {
                displays
                    .into_iter()
                    .enumerate()
                    .map(|(i, display)| DisplayDetails {
                        id: i as u32,
                        bounds: DisplayBounds {
                            x: display.bounds.origin.x as i32,
                            y: display.bounds.origin.y as i32,
                            width: display.bounds.size.width as u32,
                            height: display.bounds.size.height as u32,
                        },
                        resolution: (display.bounds.size.width as u32, display.bounds.size.height as u32),
                        is_main: i == 0, // First display is typically the main one
                    })
                    .collect()
            }
            Err(e) => {
                log::debug!("Could not get active displays: {}", e);
                vec![]
            }
        };

        // Calculate center point based on main display or first display
        let center_point = if let Some(ref main) = main_display {
            Some((
                main.bounds.x + (main.bounds.width as i32) / 2,
                main.bounds.y + (main.bounds.height as i32) / 2,
            ))
        } else if !all_displays.is_empty() {
            let first_display = &all_displays[0];
            Some((
                first_display.bounds.x + (first_display.bounds.width as i32) / 2,
                first_display.bounds.y + (first_display.bounds.height as i32) / 2,
            ))
        } else {
            None
        };

        // Get standard resolution for coordinate system compatibility
        let standard_resolution = match get_current_standard_resolution() {
            Ok((width, height)) => Some((width, height)),
            Err(e) => {
                log::debug!("Could not get standard resolution: {}", e);
                None
            }
        };

        Some(DisplayInfo {
            main_display,
            all_displays,
            center_point,
            standard_resolution,
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Gather comprehensive system context for agent initialization.
///
/// Performance: splits work into "fresh" (must run every query) and "slow" (cached 10s TTL).
/// Fresh operations run in parallel via `tokio::join!`. Screen resolution uses CoreGraphics
/// (cached until display config changes). Geolocation is cached per-session.
pub async fn gather_system_context(
    app_state: Option<&crate::state::AppState>,
) -> Result<SystemContext, String> {
    let start = std::time::Instant::now();

    // --- Always-fresh: time, timezone ---
    let now = chrono::Local::now();
    let current_time = now.format("%A, %B %d, %Y at %I:%M %p").to_string();
    let current_timestamp = current_timestamp_ms();
    let timezone = now.format("%Z (UTC%:z)").to_string();

    // --- Instant: screen resolution (CoreGraphics, cached until display change) ---
    let screen_resolution = get_screen_resolution();

    // --- Fresh operations: run in parallel (these depend on current user state) ---
    let (focused_window, clipboard_content, selected_text) = tokio::join!(
        get_focused_window_info(app_state),
        get_clipboard_content_safe(app_state),
        get_selected_text_safe(app_state),
    );

    // --- Slow operations: use cached values if available (10s TTL) ---
    let (running_applications, installed_applications, user_preferences, hardware_info, voice_audio_state, display_info) = {
        let cache_guard = slow_context_cache().lock().await;

        // Extract from cache if valid
        let cached_result = cache_guard.as_ref().and_then(|cached| {
            let age = std::time::Instant::now().duration_since(cached.cached_at);
            if age < SLOW_CONTEXT_TTL {
                log::debug!("Using cached slow context (age: {:?})", age);
                Some((
                    cached.running_applications.clone(),
                    cached.installed_applications.clone(),
                    cached.user_preferences.clone(),
                    cached.hardware_info.clone(),
                    cached.voice_audio_state.clone(),
                    cached.display_info.clone(),
                ))
            } else {
                None
            }
        });

        if let Some(result) = cached_result {
            result
        } else {
            // Cache miss or TTL expired — drop lock, refresh, re-acquire
            drop(cache_guard);
            let fresh = gather_slow_context(app_state).await;
            let result = (
                fresh.running_applications.clone(),
                fresh.installed_applications.clone(),
                fresh.user_preferences.clone(),
                fresh.hardware_info.clone(),
                fresh.voice_audio_state.clone(),
                fresh.display_info.clone(),
            );
            let mut cache_guard = slow_context_cache().lock().await;
            *cache_guard = Some(fresh);
            result
        }
    };

    // --- Geolocation: cached per-session (already has its own OnceCell) ---
    let location_info = resolve_geolocation().await;

    log::debug!("gather_system_context completed in {:?}", start.elapsed());

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
        clipboard_content,
        selected_text,
        hardware_info,
        voice_audio_state,
        display_info,
        location_info,
    })
}

/// Gather the "slow" context fields that are safe to cache.
/// Runs expensive operations in parallel where possible.
async fn gather_slow_context(
    app_state: Option<&crate::state::AppState>,
) -> CachedSlowContext {
    // Run independent slow operations in parallel
    let (running_applications, hardware_info, voice_audio_state, display_info) = tokio::join!(
        get_running_applications_info(),
        get_hardware_info_safe(),
        get_voice_audio_state_safe(app_state),
        get_display_info_safe(),
    );

    // These depend on running_applications, so run sequentially
    let installed_applications = get_installed_applications_info(&running_applications).await;
    let user_preferences =
        get_user_preferences(&running_applications, &installed_applications).await;

    CachedSlowContext {
        running_applications,
        installed_applications,
        user_preferences,
        hardware_info,
        voice_audio_state,
        display_info,
        cached_at: std::time::Instant::now(),
    }
}

/// Get information about the currently focused window
async fn get_focused_window_info(
    _app_state: Option<&crate::state::AppState>,
) -> Option<FocusedWindowInfo> {
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
                let title = attrs
                    .label
                    .clone()
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
    role.contains("TextField")
        || role.contains("TextArea")
        || role.contains("ComboBox")
        || role.contains("SearchField")
        || attrs.properties.contains_key("AXValue")
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
                            let app_name_obj: *mut objc::runtime::Object =
                                msg_send![app, localizedName];
                            if !app_name_obj.is_null() {
                                let app_name_str: &str = {
                                    let nsstring = app_name_obj as *const objc::runtime::Object;
                                    let bytes: *const std::os::raw::c_char =
                                        msg_send![nsstring, UTF8String];
                                    let len: usize =
                                        msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                                    let bytes_slice =
                                        std::slice::from_raw_parts(bytes as *const u8, len);
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
                    let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
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

/// Get screen resolution using CoreGraphics display APIs (microseconds, no screenshot).
///
/// Previously this captured a full screenshot, base64-decoded it, and loaded it as a PNG
/// just to read width/height — adding 50-150ms per query. Now uses the same CoreGraphics
/// API that `get_display_info_safe` already calls, which returns in microseconds.
fn get_screen_resolution() -> Option<(u32, u32)> {
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::display::get_main_display;

        match get_main_display() {
            Ok(display) => {
                let width = display.bounds.size.width as u32;
                let height = display.bounds.size.height as u32;
                Some((width, height))
            }
            Err(e) => {
                log::debug!("Failed to get display info for resolution: {}", e);
                None
            }
        }
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
            let frontmost_app: *mut objc::runtime::Object =
                msg_send![shared_workspace, frontmostApplication];
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
                    if bundle_id_str.contains(".worker")
                        || bundle_id_str.contains("com.apple.WebKit")
                        || bundle_id_str.contains("com.apple.CoreServices")
                        || (bundle_id_str.contains(".helper") && activation_policy == 2)
                        || (bundle_id_str.contains(".agent") && activation_policy == 2)
                    {
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
async fn get_installed_applications_info(
    _running_apps: &[RunningApplicationInfo],
) -> Vec<InstalledApplicationInfo> {
    #[cfg(target_os = "macos")]
    {
        let mut installed_apps = Vec::new();
        let running_bundle_ids: std::collections::HashSet<String> = _running_apps
            .iter()
            .filter_map(|app| app.bundle_id.clone())
            .collect();

        // Common application directories to scan
        let home_applications = format!(
            "{}Applications",
            std::env::var("HOME").unwrap_or_default() + "/"
        );
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
                                    let is_running = bundle_id
                                        .as_ref()
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
            let a_priority = if a.path.starts_with("/Applications/") {
                0
            } else if a.path.starts_with("/Applications") {
                1
            } else {
                2
            };
            let b_priority = if b.path.starts_with("/Applications/") {
                0
            } else if b.path.starts_with("/Applications") {
                1
            } else {
                2
            };
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
                    let bundle_id =
                        contents[string_content_start..string_content_start + string_end].trim();
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
    let browsers = [
        "Safari",
        "Google Chrome",
        "Firefox",
        "Microsoft Edge",
        "Arc",
        "Brave Browser",
    ];
    for browser in browsers {
        if running_apps.iter().any(|app| app.name.contains(browser)) {
            browser_preference = Some(browser.to_string());
            preferred_applications.push(PreferredApp {
                category: "browser".to_string(),
                app_name: browser.to_string(),
                confidence: 0.9, // High confidence if currently running
            });
            break;
        } else if installed_apps.iter().any(|app| app.name.contains(browser)) && browser_preference.is_none() {
            browser_preference = Some(browser.to_string());
            preferred_applications.push(PreferredApp {
                category: "browser".to_string(),
                app_name: browser.to_string(),
                confidence: 0.6, // Medium confidence if just installed
            });
        }
    }

    // Editor detection
    let editors = [
        "Visual Studio Code",
        "Xcode",
        "Sublime Text",
        "Atom",
        "IntelliJ IDEA",
        "WebStorm",
        "PyCharm",
        "TextEdit",
        "Vim",
        "Neovim",
    ];
    for editor in editors {
        if running_apps.iter().any(|app| app.name.contains(editor)) {
            editor_preference = Some(editor.to_string());
            preferred_applications.push(PreferredApp {
                category: "editor".to_string(),
                app_name: editor.to_string(),
                confidence: 0.9,
            });
            break;
        } else if installed_apps.iter().any(|app| app.name.contains(editor)) && editor_preference.is_none() {
            editor_preference = Some(editor.to_string());
            preferred_applications.push(PreferredApp {
                category: "editor".to_string(),
                app_name: editor.to_string(),
                confidence: 0.6,
            });
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
        } else if installed_apps.iter().any(|app| app.name.contains(terminal)) && terminal_preference.is_none() {
            terminal_preference = Some(terminal.to_string());
            preferred_applications.push(PreferredApp {
                category: "terminal".to_string(),
                app_name: terminal.to_string(),
                confidence: 0.6,
            });
        }
    }

    // Productivity suite detection
    let productivity_suites = [
        "Microsoft Office",
        "Microsoft Word",
        "Microsoft Excel",
        "Microsoft PowerPoint",
        "Pages",
        "Numbers",
        "Keynote",
        "Google Chrome",
    ]; // Chrome for Google Workspace
    for suite in productivity_suites {
        if running_apps.iter().any(|app| app.name.contains(suite)) {
            if suite.contains("Microsoft") {
                productivity_suite = Some("Microsoft Office".to_string());
            } else if ["Pages", "Numbers", "Keynote"].contains(&suite) {
                productivity_suite = Some("Apple iWork".to_string());
            } else if suite == "Google Chrome"
                && browser_preference
                    .as_ref()
                    .is_some_and(|b| b.contains("Chrome"))
            {
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

/// Safely get clipboard content for context (with length limits)
async fn get_clipboard_content_safe(app_state: Option<&crate::state::AppState>) -> Option<String> {
    if let Some(state) = app_state {
        match state.desktop.get_clipboard_content() {
            Ok(content) => {
                // Limit clipboard content length to avoid overwhelming context
                const MAX_CLIPBOARD_LENGTH: usize = 500;
                if content.len() > MAX_CLIPBOARD_LENGTH {
                    Some(format!(
                        "{}... (truncated from {} chars)",
                        &content[..MAX_CLIPBOARD_LENGTH],
                        content.len()
                    ))
                } else if content.trim().is_empty() {
                    Some("[Empty clipboard]".to_string())
                } else {
                    Some(content)
                }
            }
            Err(e) => {
                log::debug!("Could not get clipboard content: {}", e);
                None
            }
        }
    } else {
        log::debug!("No app state available for clipboard access");
        None
    }
}

/// Safely get selected text for context (with length limits)
async fn get_selected_text_safe(app_state: Option<&crate::state::AppState>) -> Option<String> {
    if let Some(state) = app_state {
        // Try accessibility API approach first
        if let Some(selected_text) = get_selected_text_via_accessibility(state).await {
            return Some(selected_text);
        }

        // Fall back to clipboard trick approach as mentioned in web search results
        if let Some(selected_text) = get_selected_text_via_clipboard_trick(state).await {
            return Some(selected_text);
        }

        log::debug!("No selected text found via any method");
        Some("".to_string()) // Return empty string as fallback
    } else {
        log::debug!("No app state available for selected text access");
        Some("".to_string()) // Return empty string as fallback
    }
}

/// Try to get selected text using macOS accessibility APIs
#[cfg(target_os = "macos")]
async fn get_selected_text_via_accessibility(app_state: &crate::state::AppState) -> Option<String> {
    match app_state.desktop.get_desktop() {
        Ok(desktop) => {
            match desktop.focused_element() {
                Ok(element) => {
                    let attrs = element.attributes();

                    // Check for AXSelectedText attribute which contains actual selected text
                    if let Some(Some(selected_text)) = attrs.properties.get("AXSelectedText") {
                        if let Some(selected_str) = selected_text.as_str() {
                            if !selected_str.trim().is_empty() {
                                const MAX_SELECTED_TEXT_LENGTH: usize = 300;
                                let text = if selected_str.len() > MAX_SELECTED_TEXT_LENGTH {
                                    format!(
                                        "{}... (truncated from {} chars)",
                                        &selected_str[..MAX_SELECTED_TEXT_LENGTH],
                                        selected_str.len()
                                    )
                                } else {
                                    selected_str.to_string()
                                };
                                log::debug!("Found selected text via AXSelectedText: {}", text);
                                return Some(text);
                            }
                        }
                    }

                    // Check for AXSelectedTextRange and AXValue combination
                    if let (Some(Some(range_value)), Some(Some(value))) = (
                        attrs.properties.get("AXSelectedTextRange"),
                        attrs.properties.get("AXValue"),
                    ) {
                        if let (Some(range_str), Some(value_str)) =
                            (range_value.as_str(), value.as_str())
                        {
                            if let Some(selected_text) =
                                extract_text_from_range(range_str, value_str)
                            {
                                if !selected_text.trim().is_empty() {
                                    const MAX_SELECTED_TEXT_LENGTH: usize = 300;
                                    let text = if selected_text.len() > MAX_SELECTED_TEXT_LENGTH {
                                        format!(
                                            "{}... (truncated from {} chars)",
                                            &selected_text[..MAX_SELECTED_TEXT_LENGTH],
                                            selected_text.len()
                                        )
                                    } else {
                                        selected_text
                                    };
                                    log::debug!(
                                        "Found selected text via AXSelectedTextRange: {}",
                                        text
                                    );
                                    return Some(text);
                                }
                            }
                        }
                    }

                    log::debug!("No selected text found in accessibility attributes");
                    None
                }
                Err(e) => {
                    log::debug!("Could not get focused element: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            log::debug!("Desktop not available: {}", e);
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn get_selected_text_via_accessibility(
    _app_state: &crate::state::AppState,
) -> Option<String> {
    None
}

/// Extract text from AXSelectedTextRange format (e.g., "{location=5, length=10}")
#[cfg(target_os = "macos")]
fn extract_text_from_range(range_str: &str, full_text: &str) -> Option<String> {
    // Parse range format like "{location=5, length=10}"
    if let (Some(location_start), Some(length_start)) =
        (range_str.find("location="), range_str.find("length="))
    {
        let location_end = range_str[location_start + 9..]
            .find(',')
            .map(|i| i + location_start + 9)
            .or_else(|| {
                range_str[location_start + 9..]
                    .find('}')
                    .map(|i| i + location_start + 9)
            })
            .unwrap_or(range_str.len());
        let length_end = range_str[length_start + 7..]
            .find(',')
            .map(|i| i + length_start + 7)
            .or_else(|| {
                range_str[length_start + 7..]
                    .find('}')
                    .map(|i| i + length_start + 7)
            })
            .unwrap_or(range_str.len());

        if let (Ok(location), Ok(length)) = (
            range_str[location_start + 9..location_end]
                .trim()
                .parse::<usize>(),
            range_str[length_start + 7..length_end]
                .trim()
                .parse::<usize>(),
        ) {
            if location + length <= full_text.len() && length > 0 {
                return Some(full_text[location..location + length].to_string());
            }
        }
    }
    None
}

/// Fall back to clipboard trick approach (as mentioned in web search results)
async fn get_selected_text_via_clipboard_trick(
    app_state: &crate::state::AppState,
) -> Option<String> {
    // Save current clipboard content
    let original_clipboard = app_state.desktop.get_clipboard_content().ok();

    // Try to copy selected text using Cmd+C
    match app_state.desktop.get_desktop() {
        Ok(desktop) => {
            // Send Cmd+C to copy selected text
            if let Err(e) = desktop.press_key("c", Some("cmd")) {
                log::debug!("Failed to press Cmd+C: {}", e);
                return None;
            }

            // Small delay to allow copy operation to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Get clipboard content
            let copied_text = match app_state.desktop.get_clipboard_content() {
                Ok(content) => {
                    // Check if clipboard content changed (indicating something was copied)
                    if original_clipboard.as_ref() != Some(&content) && !content.trim().is_empty() {
                        Some(content)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    log::debug!("Failed to get clipboard content: {}", e);
                    None
                }
            };

            // Restore original clipboard content if we have it
            if let Some(original) = original_clipboard {
                if let Err(e) = app_state.desktop.set_clipboard_content(&original) {
                    log::debug!("Failed to restore clipboard content: {}", e);
                }
            }

            if let Some(text) = copied_text {
                const MAX_SELECTED_TEXT_LENGTH: usize = 300;
                let result = if text.len() > MAX_SELECTED_TEXT_LENGTH {
                    format!(
                        "{}... (truncated from {} chars)",
                        &text[..MAX_SELECTED_TEXT_LENGTH],
                        text.len()
                    )
                } else {
                    text
                };
                log::debug!("Found selected text via clipboard trick: {}", result);
                Some(result)
            } else {
                log::debug!("No selected text found via clipboard trick");
                None
            }
        }
        Err(e) => {
            log::debug!("Desktop not available for clipboard trick: {}", e);
            None
        }
    }
}

/// Safely get hardware information for context
async fn get_hardware_info_safe() -> Option<HardwareInfo> {
    // Implement hardware monitoring directly here to avoid circular dependencies
    let (cpu_usage, memory_usage, disk_usage) = tokio::join!(
        get_cpu_usage_direct(),
        get_memory_usage_direct(),
        get_disk_usage_direct()
    );

    // Only include if we have at least some hardware info
    if cpu_usage.is_some() || memory_usage.is_some() || disk_usage.is_some() {
        Some(HardwareInfo {
            cpu_usage,
            memory_usage,
            disk_usage,
            screen_resolution: None, // Screen resolution already available in SystemInfo
        })
    } else {
        log::debug!("No hardware information available");
        None
    }
}

/// Get CPU usage directly (simplified version)
async fn get_cpu_usage_direct() -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        use tokio::process::Command;

        match Command::new("top")
            .args(["-l", "1", "-n", "0"])
            .output()
            .await
        {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                parse_cpu_usage_direct(&output_str)
            }
            Err(e) => {
                log::debug!("Failed to get CPU usage: {}", e);
                None
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Parse CPU usage from top command output
fn parse_cpu_usage_direct(output: &str) -> Option<f32> {
    for line in output.lines() {
        if line.contains("CPU usage:") {
            // Parse line like "CPU usage: 15.2% user, 8.1% sys, 76.7% idle"
            if let Some(user_part) = line.split("CPU usage:").nth(1) {
                if let Some(user_str) = user_part.split('%').next() {
                    if let Ok(user_cpu) = user_str.trim().parse::<f32>() {
                        return Some(user_cpu);
                    }
                }
            }
        }
    }
    None
}

/// Get memory usage directly (simplified version)
async fn get_memory_usage_direct() -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        use tokio::process::Command;

        match Command::new("vm_stat").output().await {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                parse_memory_usage_direct(&output_str)
            }
            Err(e) => {
                log::debug!("Failed to get memory usage: {}", e);
                None
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Parse memory usage from vm_stat output
fn parse_memory_usage_direct(output: &str) -> Option<f32> {
    let mut free_pages = 0u64;
    let mut inactive_pages = 0u64;
    let mut active_pages = 0u64;
    let mut wired_pages = 0u64;

    for line in output.lines() {
        if line.contains("Pages free:") {
            if let Some(num_str) = line.split(':').nth(1) {
                if let Ok(num) = num_str.trim().replace('.', "").parse::<u64>() {
                    free_pages = num;
                }
            }
        } else if line.contains("Pages active:") {
            if let Some(num_str) = line.split(':').nth(1) {
                if let Ok(num) = num_str.trim().replace('.', "").parse::<u64>() {
                    active_pages = num;
                }
            }
        } else if line.contains("Pages inactive:") {
            if let Some(num_str) = line.split(':').nth(1) {
                if let Ok(num) = num_str.trim().replace('.', "").parse::<u64>() {
                    inactive_pages = num;
                }
            }
        } else if line.contains("Pages wired down:") {
            if let Some(num_str) = line.split(':').nth(1) {
                if let Ok(num) = num_str.trim().replace('.', "").parse::<u64>() {
                    wired_pages = num;
                }
            }
        }
    }

    let total_pages = free_pages + inactive_pages + active_pages + wired_pages;
    let used_pages = active_pages + wired_pages;

    if total_pages > 0 {
        Some((used_pages as f32 / total_pages as f32) * 100.0)
    } else {
        None
    }
}

/// Get disk usage directly (simplified version)
async fn get_disk_usage_direct() -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        use tokio::process::Command;

        match Command::new("df").args(["-h", "/"]).output().await {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                parse_disk_usage_direct(&output_str)
            }
            Err(e) => {
                log::debug!("Failed to get disk usage: {}", e);
                None
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Parse disk usage from df output
fn parse_disk_usage_direct(output: &str) -> Option<f32> {
    for line in output.lines().skip(1) {
        // Skip header
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            // Extract percentage from format like "85%"
            let usage_str = parts[4];
            if let Some(percent_str) = usage_str.strip_suffix('%') {
                if let Ok(usage) = percent_str.parse::<f32>() {
                    return Some(usage);
                }
            }
        }
    }
    None
}

/// Safely get voice/audio state for context
async fn get_voice_audio_state_safe(
    app_state: Option<&crate::state::AppState>,
) -> Option<VoiceAudioState> {
    if let Some(state) = app_state {
        // Get voice controller state if available
        let is_dictation_active = state.is_dictation_active();
        let is_agent_executing = state.is_agent_executing();

        // Determine mode based on app state
        let mode = if is_agent_executing {
            "agent".to_string()
        } else if is_dictation_active {
            "dictation".to_string()
        } else {
            "idle".to_string()
        };

        // Get TTS state if available
        let is_speaking = if let Ok(_tts_provider) = state.get_tts_provider() {
            // Check if TTS is currently active (this is a simplified check)
            false // TODO: Implement actual TTS state checking
        } else {
            false
        };

        Some(VoiceAudioState {
            mode,
            is_listening: is_dictation_active,
            is_transcribing: false, // TODO: Get actual transcription state
            is_speaking,
            current_transcription: None, // TODO: Get current transcription if available
            audio_level: 0.0,            // TODO: Get actual audio level
            has_error: false,            // TODO: Check for voice errors
            error_message: None,
        })
    } else {
        log::debug!("No app state available for voice/audio state");
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
            focused
                .element_type
                .as_ref()
                .unwrap_or(&"Unknown".to_string())
        ));

        if focused.has_text_input {
            context_parts.push("Note: A text input field is currently focused".to_string());
        }

        if let Some(app) = &focused.application {
            context_parts.push(format!("Application: {}", app));
        }
    }

    // Display information from the display_info field
    if let Some(ref display_info) = context.display_info {
        // Use standard resolution if available, otherwise use main display resolution
        if let Some((width, height)) = display_info.standard_resolution {
            context_parts.push(format!("Screen resolution: {}×{}", width, height));
        } else if let Some(ref main_display) = display_info.main_display {
            context_parts.push(format!("Screen resolution: {}×{}", main_display.resolution.0, main_display.resolution.1));
        }

        // Add center point information - this is key for the agent to know where to click
        if let Some((center_x, center_y)) = display_info.center_point {
            context_parts.push(format!("Screen center point: ({}, {})", center_x, center_y));
        }

        // Add display count for multi-monitor awareness
        if display_info.all_displays.len() > 1 {
            context_parts.push(format!("Multiple displays detected: {} displays", display_info.all_displays.len()));
        }
    } else {
        // Fallback to the old method if display_info is not available
        if context.system_info.screen_resolution.is_some() {
            // Get the standard resolution that screenshots are scaled to
            use crate::utils::coordinates::get_current_standard_resolution;
            if let Ok((standard_width, standard_height)) = get_current_standard_resolution() {
                context_parts.push(format!("Screen resolution: {}×{}", standard_width, standard_height));
            }
        }
    }

    // Add running applications information
    if !context.running_applications.is_empty() {
        let app_list = context
            .running_applications
            .iter()
            .take(10) // Limit to first 10 apps to avoid overwhelming the context
            .map(|app| app.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        context_parts.push(format!("Running applications: {}", app_list));

        if context.running_applications.len() > 10 {
            context_parts.push(format!(
                "... and {} more applications",
                context.running_applications.len() - 10
            ));
        }
    }

    // Add location information if available
    if let Some(ref loc) = context.location_info {
        context_parts.push(format!(
            "User location: {}, {}, {} (timezone: {}, coordinates: {:.2},{:.2})",
            loc.city, loc.region, loc.country, loc.timezone, loc.latitude, loc.longitude
        ));
    }

    context_parts.join("\n")
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

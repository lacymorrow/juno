use base64;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::Viewport;
use chromiumoxide::handler::Handler;
use chromiumoxide::page::{Page, ScreenshotParams};
use futures::StreamExt;
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tokio::sync::Mutex;

use crate::agent::core::{AgentError, ToolResult};
use crate::constants::{
    browser::{system_url_commands, url_protocols},
    chrome_debug_urls,
    files::shell_commands,
    timeouts,
};

// Helper type alias for brevity
type ControllerResult<T> = Result<T, AgentError>;

// Timeout defaults
// const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 30000;

/// Drive the CDP connection.
///
/// chromiumoxide returns a `Handler` alongside the `Browser`; nothing happens on
/// the connection unless that handler is continuously polled. We must use
/// `tauri::async_runtime::spawn` here — `tokio::spawn` panics with "no reactor
/// running" inside the Tauri context.
fn spawn_cdp_handler(mut handler: Handler) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                log::debug!("CDP handler stream ended");
                break;
            }
        }
    })
}

#[derive(Clone)]
pub struct BrowserController {
    // Browser needs &mut for close()/kill(), so it lives behind a mutex.
    // Never hold this lock while acquiring `page` (see CLAUDE.md deadlock rules).
    browser: Arc<Mutex<Browser>>,
    // Store page in mutex for thread safety
    page: Arc<Mutex<Option<Page>>>,
    // Track connection method for debugging
    connection_method: String,
    // Keeps the CDP event pump alive; aborted on cleanup.
    handler_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    // True when we opened the current tab ourselves, false when we adopted one
    // that was already there. Cleanup closes only tabs we own — closing a tab
    // the user opened destroys their work, but never closing one leaks a tab per
    // session.
    owns_page: Arc<AtomicBool>,
}

impl BrowserController {
    pub async fn new() -> ControllerResult<Self> {
        // chromiumoxide builds a reqwest client with rustls but no bundled
        // crypto provider, and such a client *panics* on construction unless a
        // default provider is already installed. `run()` installs one at
        // startup, but this type is also reached from the CLI, headless mode,
        // and tests, none of which go through `run()`. Installing here makes the
        // guarantee local to the code that depends on it.
        crate::install_crypto_provider();

        log::info!("BrowserController::new called - attempting optimized browser connection...");

        // Try three connection strategies in order of speed/preference

        // Strategy 1: Connect to existing browser instance via CDP (fastest - ~1-2 seconds)
        if let Ok(controller) = Self::try_connect_to_existing_browser().await {
            log::info!("Successfully connected to existing browser instance via CDP");
            return Ok(controller);
        }

        // Strategy 2: Launch with persistent user profile (fast - ~10-15 seconds)
        if let Ok(controller) = Self::try_launch_with_user_profile().await {
            log::info!("Successfully launched browser with user profile");
            return Ok(controller);
        }

        // Strategy 3: Fallback to fresh instance (current behavior - 90+ seconds)
        log::info!("Falling back to fresh browser instance...");
        Self::launch_fresh_instance().await
    }

    /// Evaluate a JavaScript function expression and return its result as JSON.
    ///
    /// Returns `Value::Null` when the script yields nothing, so callers can treat
    /// "no result" and "null result" identically (matching the previous behavior).
    async fn eval_json(page: &Page, js: &str) -> ControllerResult<Value> {
        let result = page
            .evaluate_function(js)
            .await
            .map_err(|e| AgentError::ToolError(format!("JavaScript evaluation failed: {}", e)))?;
        Ok(result.value().cloned().unwrap_or(Value::Null))
    }

    /// Build a controller from a freshly connected/launched browser, reusing an
    /// existing page when one is available.
    async fn from_browser(
        browser: Browser,
        handler: Handler,
        connection_method: String,
    ) -> ControllerResult<Self> {
        let task = spawn_cdp_handler(handler);

        // Prefer an already-open page so we attach to what the user is looking at.
        let mut owns_page = false;
        let page = match browser.pages().await {
            Ok(pages) if !pages.is_empty() => {
                log::info!("Using existing page from browser");
                Some(pages[0].clone())
            }
            _ => match browser.new_page("about:blank").await {
                Ok(page) => {
                    owns_page = true;
                    Some(page)
                }
                Err(e) => {
                    log::warn!("Failed to create page: {}", e);
                    None
                }
            },
        };

        Ok(BrowserController {
            browser: Arc::new(Mutex::new(browser)),
            page: Arc::new(Mutex::new(page)),
            connection_method,
            handler_task: Arc::new(Mutex::new(Some(task))),
            owns_page: Arc::new(AtomicBool::new(owns_page)),
        })
    }

    /// Strategy 1: Try to connect to an existing browser instance via CDP
    async fn try_connect_to_existing_browser() -> ControllerResult<Self> {
        log::info!("Attempting to connect to existing browser via CDP...");

        // First check if Chrome is running and if remote debugging is enabled
        if !Self::is_chrome_running().await {
            log::info!("Chrome is not running, skipping CDP connection attempt");
            return Err(AgentError::ToolError("Chrome not running".to_string()));
        }

        if !Self::is_remote_debugging_enabled().await {
            log::info!("Chrome is running but remote debugging is not enabled");
            return Err(AgentError::ToolError(
                "Remote debugging not enabled".to_string(),
            ));
        }

        // Common CDP endpoints to try (using constants)
        let cdp_endpoints = chrome_debug_urls::get_all_urls();

        for endpoint in &cdp_endpoints {
            log::info!("Trying CDP endpoint: {}", endpoint);

            // Use a reasonable timeout for CDP connection attempts
            // `Browser::connect` accepts the http debug URL and resolves the
            // websocket endpoint via /json/version itself.
            match tokio::time::timeout(
                std::time::Duration::from_secs(
                    crate::constants::timeouts::BROWSER_CONNECTION_TIMEOUT_SECONDS,
                ), // Increased timeout for more reliable connection
                Browser::connect(endpoint.to_string()),
            )
            .await
            {
                Ok(Ok((browser, handler))) => {
                    log::info!("Successfully connected to existing browser at {}", endpoint);
                    return Self::from_browser(browser, handler, format!("CDP:{}", endpoint)).await;
                }
                Ok(Err(e)) => {
                    log::info!("CDP connection failed at {}: {}", endpoint, e);
                }
                Err(_) => {
                    log::info!("CDP connection timeout at {}", endpoint);
                }
            }
        }

        Err(AgentError::ToolError(
            "No existing browser instance found via CDP".to_string(),
        ))
    }

    /// Check if remote debugging is enabled on the running Chrome instance
    async fn is_remote_debugging_enabled() -> bool {
        // Try to make a simple HTTP request to the primary Chrome debugging port
        let debug_url = chrome_debug_urls::PRIMARY;
        let version_url = format!("{}/json/version", debug_url);

        match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            reqwest::get(&version_url),
        )
        .await
        {
            Ok(Ok(response)) => {
                if response.status().is_success() {
                    log::info!("Remote debugging is enabled on {}", debug_url);
                    return true;
                }
            }
            Ok(Err(e)) => {
                log::debug!("Failed to connect to remote debugging port: {}", e);
            }
            Err(_) => {
                log::debug!("Timeout connecting to remote debugging port");
            }
        }

        // Try alternative ports
        for endpoint in [
            chrome_debug_urls::ALTERNATIVE_1,
            chrome_debug_urls::ALTERNATIVE_2,
        ] {
            let version_url = format!("{}/json/version", endpoint);
            if let Ok(Ok(response)) = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                reqwest::get(&version_url),
            )
            .await
            {
                if response.status().is_success() {
                    log::info!("Remote debugging is enabled on {}", endpoint);
                    return true;
                }
            }
        }

        log::debug!("Remote debugging is not available on any standard ports");
        false
    }

    /// Strategy 2: Launch browser with persistent user profile
    async fn try_launch_with_user_profile() -> ControllerResult<Self> {
        log::info!("Attempting to launch browser with user profile...");

        // First, check if Chrome is already running - if so, try with temporary profile
        let chrome_running = Self::is_chrome_running().await;
        if chrome_running {
            log::info!("Chrome is already running, will try with temporary profile to avoid SingletonLock conflict");
            return Self::try_launch_with_temp_profile().await;
        }

        // Detect user profile directory based on OS and browser
        let user_data_dir = Self::detect_user_profile_directory()?;
        log::info!("Using user data directory: {:?}", user_data_dir);

        // Additional check: look for SingletonLock file directly
        let singleton_lock_path = format!("{}/SingletonLock", user_data_dir);
        if std::path::Path::new(&singleton_lock_path).exists() {
            log::warn!("SingletonLock file exists at: {}", singleton_lock_path);
            log::info!("Profile appears to be in use, switching to temporary profile strategy");
            return Self::try_launch_with_temp_profile().await;
        }

        // Detect browser executable
        let browser_info = Self::detect_browser_executable()?;
        log::info!("Using browser: {} at {:?}", browser_info.0, browser_info.1);

        let config = BrowserConfig::builder()
            .with_head()
            .user_data_dir(std::path::Path::new(&user_data_dir))
            .chrome_executable(&browser_info.1)
            .launch_timeout(Duration::from_secs(30))
            .args(vec![
                "--no-first-run",
                "--no-default-browser-check",
                // Prevent update checks slowing startup
                "--disable-component-update",
                // Enable remote debugging for future CDP connections
                "--remote-debugging-port=9222",
            ])
            .build()
            .map_err(|e| AgentError::ToolError(format!("Failed to build browser config: {}", e)))?;

        match Browser::launch(config).await {
            Ok((browser, handler)) => {
                log::info!("Successfully launched browser with user profile");
                Self::from_browser(browser, handler, format!("Persistent:{}", user_data_dir)).await
            }
            Err(e) => {
                log::warn!("Failed to launch with user profile: {}", e);
                // Check if this is a SingletonLock error and try temp profile
                if e.to_string().contains("SingletonLock")
                    || e.to_string().contains("profile directory")
                {
                    log::info!("Profile conflict detected - trying temporary profile strategy");
                    return Self::try_launch_with_temp_profile().await;
                }
                Err(AgentError::ToolError(format!(
                    "Persistent profile launch failed: {}",
                    e
                )))
            }
        }
    }

    /// Strategy 2b: Launch browser with temporary profile (when main profile is in use)
    async fn try_launch_with_temp_profile() -> ControllerResult<Self> {
        log::info!("Attempting to launch browser with temporary profile...");

        // Create a temporary directory for the browser profile
        let temp_dir = std::env::temp_dir();
        let temp_profile = temp_dir.join(format!("juno-browser-{}", std::process::id()));

        // Create the temporary profile directory
        if let Err(e) = std::fs::create_dir_all(&temp_profile) {
            log::warn!("Failed to create temporary profile directory: {}", e);
            return Err(AgentError::ToolError(format!(
                "Failed to create temp profile: {}",
                e
            )));
        }

        log::info!("Using temporary profile directory: {:?}", temp_profile);

        // Detect browser executable
        let browser_info = Self::detect_browser_executable()?;
        log::info!("Using browser: {} at {:?}", browser_info.0, browser_info.1);

        let config = BrowserConfig::builder()
            .with_head()
            .user_data_dir(&temp_profile)
            .chrome_executable(&browser_info.1)
            .launch_timeout(Duration::from_secs(30))
            .args(vec![
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-component-update",
                // Use different port to avoid conflicts
                "--remote-debugging-port=9223",
            ])
            .build()
            .map_err(|e| AgentError::ToolError(format!("Failed to build browser config: {}", e)))?;

        match Browser::launch(config).await {
            Ok((browser, handler)) => {
                log::info!("Successfully launched browser with temporary profile");
                Self::from_browser(
                    browser,
                    handler,
                    format!("TempProfile:{}", temp_profile.display()),
                )
                .await
            }
            Err(e) => {
                log::warn!("Failed to launch with temporary profile: {}", e);
                // Clean up the temporary directory if launch failed
                if let Err(cleanup_err) = std::fs::remove_dir_all(&temp_profile) {
                    log::warn!(
                        "Failed to clean up temporary profile directory: {}",
                        cleanup_err
                    );
                }
                Err(AgentError::ToolError(format!(
                    "Temporary profile launch failed: {}",
                    e
                )))
            }
        }
    }

    /// Check if Chrome is already running on this system
    async fn is_chrome_running() -> bool {
        #[cfg(target_os = "macos")]
        {
            // Use multiple methods to detect Chrome processes
            log::info!("Checking if Chrome is already running...");

            // Method 1: Use pgrep to check for Chrome processes
            let pgrep_output = tokio::process::Command::new("pgrep")
                .arg("-f")
                .arg("Google Chrome")
                .output()
                .await;

            if let Ok(output) = pgrep_output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let pgrep_running = !stdout.trim().is_empty();
                if pgrep_running {
                    log::info!("Chrome processes detected via pgrep: {}", stdout.trim());
                    return true;
                }
            } else {
                log::debug!("pgrep command failed, trying alternative detection");
            }

            // Method 2: Check for Chrome processes via ps (native approach, no osascript)
            let ps_output = tokio::process::Command::new("ps")
                .arg("-A")
                .arg("-o")
                .arg("comm")
                .output()
                .await;

            if let Ok(output) = ps_output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("Google Chrome") || stdout.contains("chrome") {
                    log::info!("Chrome detected via ps command");
                    return true;
                }
            } else {
                log::debug!("ps command failed, trying alternative detection");
            }

            // Method 3: Check for Chrome processes via pgrep as fallback
            let pgrep_output = tokio::process::Command::new("pgrep")
                .arg("-f")
                .arg("Google Chrome")
                .output()
                .await;

            if let Ok(output) = pgrep_output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() {
                    log::info!("Chrome detected via pgrep command");
                    return true;
                }
            }

            log::info!("No Chrome processes detected");
            false
        }

        #[cfg(target_os = "windows")]
        {
            log::info!("Checking if Chrome is already running...");

            let output = tokio::process::Command::new("tasklist")
                .arg("/FI")
                .arg("IMAGENAME eq chrome.exe")
                .output()
                .await;

            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let running = stdout.contains("chrome.exe");
                if running {
                    log::info!("Chrome processes detected on Windows");
                } else {
                    log::info!("No Chrome processes detected on Windows");
                }
                return running;
            }

            false
        }

        #[cfg(target_os = "linux")]
        {
            log::info!("Checking if Chrome is already running...");

            let output = tokio::process::Command::new("pgrep")
                .arg("-f")
                .arg("chrome")
                .output()
                .await;

            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let running = !stdout.trim().is_empty();
                if running {
                    log::info!("Chrome processes detected on Linux: {}", stdout.trim());
                } else {
                    log::info!("No Chrome processes detected on Linux");
                }
                return running;
            }

            false
        }
    }

    /// Strategy 3: Launch fresh browser instance (current behavior)
    async fn launch_fresh_instance() -> ControllerResult<Self> {
        log::info!("Launching fresh browser instance (fallback method)...");

        // --- Find Chromium Executable ---
        let executable_path: Option<PathBuf> = env::var("CHROMIUM_EXECUTABLE_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| {
                let common_paths = [
                    shell_commands::CHROME_BINARY_MACOS,
                    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                    "/Applications/Chromium.app/Contents/MacOS/Chromium",
                ];

                // Check each path and log whether it exists
                for path in &common_paths {
                    let path_buf = PathBuf::from(path);
                    if path_buf.exists() {
                        log::info!("Found browser at: {}", path);
                    } else {
                        log::debug!("Browser not found at: {}", path);
                    }
                }

                common_paths.iter().map(PathBuf::from).find(|p| p.exists())
            });

        // Print system information for debugging
        log::info!("Operating system: {}", std::env::consts::OS);
        log::info!(
            "Path environment: {:?}",
            env::var("PATH").unwrap_or_else(|_| "Not available".to_string())
        );

        // --- Build Launcher with Optional Path ---
        let exe_path = match &executable_path {
            Some(path) => {
                log::info!("Using browser executable found at: {:?}", path);
                path.clone()
            }
            None => {
                // If no path is found, return the specific error
                log::error!("Chromium executable not found. Set CHROMIUM_EXECUTABLE_PATH or install Chrome in a standard location.");
                return Err(AgentError::ToolError("Chromium executable not found. Set CHROMIUM_EXECUTABLE_PATH env var or ensure Chrome is installed in /Applications.".to_string()));
            }
        };

        // Configure the launcher with more resilient options and enable remote debugging
        let args = vec![
            "--no-first-run",
            "--no-default-browser-check",
            "--no-sandbox",
            // Enable remote debugging for future connections
            "--remote-debugging-port=9222",
            // Reduce security restrictions for automation
            "--disable-web-security",
            // Improve stability
            "--disable-features=VizDisplayCompositor",
        ];

        // Reduced timeout to 45 seconds for better user experience
        let build_config = |args: Vec<&str>| {
            BrowserConfig::builder()
                .with_head()
                .chrome_executable(&exe_path)
                .launch_timeout(Duration::from_secs(45))
                .args(args)
                .build()
                .map_err(|e| {
                    AgentError::ToolError(format!("Failed to build browser config: {}", e))
                })
        };

        log::info!("Launching browser with 45 second timeout and remote debugging enabled...");

        let (browser, handler) = match Browser::launch(build_config(args)?).await {
            Ok(pair) => {
                log::info!("Browser launched successfully.");
                pair
            }
            Err(e) => {
                // Try one more time with even fewer args
                log::warn!(
                    "First browser launch attempt failed: {}. Trying again with minimal args...",
                    e
                );

                // Try with absolute minimum arguments but keep remote debugging
                let minimal_args = vec!["--remote-debugging-port=9222"];

                log::info!("Retrying browser launch with minimal configuration...");
                Browser::launch(build_config(minimal_args)?)
                    .await
                    .map_err(|e| {
                        AgentError::ToolError(format!(
                            "Failed to launch browser (both attempts): {}",
                            e
                        ))
                    })?
            }
        };

        // The CDP handler must be pumped before any page work can succeed.
        let handler_task = spawn_cdp_handler(handler);

        // Create a test page to verify everything is working - retry multiple times if needed
        log::info!("Creating test page to verify browser setup...");

        let test_page = {
            let max_attempts = 3;
            let mut last_error = None;
            let mut test_page = None;

            for attempt in 1..=max_attempts {
                log::info!(
                    "Attempt {} of {} to create initial page",
                    attempt,
                    max_attempts
                );
                match browser.new_page("about:blank").await {
                    Ok(page) => {
                        test_page = Some(page);
                        break;
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to create initial page on attempt {}: {}",
                            attempt,
                            e
                        );
                        last_error = Some(e.to_string());
                        // Sleep between attempts
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    }
                }
            }

            match test_page {
                None => {
                    handler_task.abort();
                    return Err(AgentError::ToolError(format!(
                        "Failed to create initial page after {} attempts. Last error: {}",
                        max_attempts,
                        last_error.unwrap_or_else(|| "Unknown error".to_string())
                    )));
                }
                Some(page) => page,
            }
        };

        // Keep the test page open as our initial page
        let page = Arc::new(Mutex::new(Some(test_page)));
        log::info!("Browser successfully initialized with test page.");

        Ok(BrowserController {
            browser: Arc::new(Mutex::new(browser)),
            page,
            connection_method: "Fresh".to_string(),
            handler_task: Arc::new(Mutex::new(Some(handler_task))),
            // We launched this browser and opened this page; both are ours.
            owns_page: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Detect user profile directory based on OS and available browsers
    fn detect_user_profile_directory() -> ControllerResult<String> {
        #[cfg(target_os = "macos")]
        {
            let home = env::var("HOME").map_err(|_| {
                AgentError::ToolError("HOME environment variable not found".to_string())
            })?;

            // Try browsers in order of preference
            let browser_paths = [
                format!("{}/Library/Application Support/Google/Chrome", home),
                format!("{}/Library/Application Support/Microsoft Edge", home),
                format!(
                    "{}/Library/Application Support/BraveSoftware/Brave-Browser",
                    home
                ),
                format!("{}/Library/Application Support/Chromium", home),
            ];

            for path in &browser_paths {
                if PathBuf::from(path).exists() {
                    log::info!("Found user profile directory: {}", path);
                    return Ok(path.clone());
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = env::var("LOCALAPPDATA").map_err(|_| {
                AgentError::ToolError("LOCALAPPDATA environment variable not found".to_string())
            })?;

            let browser_paths = [
                format!("{}\\Google\\Chrome\\User Data", appdata),
                format!("{}\\Microsoft\\Edge\\User Data", appdata),
                format!("{}\\BraveSoftware\\Brave-Browser\\User Data", appdata),
            ];

            for path in &browser_paths {
                if PathBuf::from(path).exists() {
                    log::info!("Found user profile directory: {}", path);
                    return Ok(path.clone());
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let home = env::var("HOME").map_err(|_| {
                AgentError::ToolError("HOME environment variable not found".to_string())
            })?;

            let browser_paths = [
                format!("{}/.config/google-chrome", home),
                format!("{}/.config/microsoft-edge", home),
                format!("{}/.config/BraveSoftware/Brave-Browser", home),
                format!("{}/.config/chromium", home),
            ];

            for path in &browser_paths {
                if PathBuf::from(path).exists() {
                    log::info!("Found user profile directory: {}", path);
                    return Ok(path.clone());
                }
            }
        }

        Err(AgentError::ToolError(
            "No user browser profile directory found".to_string(),
        ))
    }

    /// Detect browser executable and return (channel, path)
    fn detect_browser_executable() -> ControllerResult<(String, PathBuf)> {
        #[cfg(target_os = "macos")]
        {
            let browsers = [
                ("chrome", shell_commands::CHROME_BINARY_MACOS),
                (
                    "msedge",
                    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                ),
                (
                    "chrome",
                    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                ),
                (
                    "chromium",
                    "/Applications/Chromium.app/Contents/MacOS/Chromium",
                ),
            ];

            for (channel, path) in &browsers {
                let path_buf = PathBuf::from(path);
                if path_buf.exists() {
                    return Ok((channel.to_string(), path_buf));
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let browsers = [
                (
                    "chrome",
                    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                ),
                (
                    "chrome",
                    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
                ),
                (
                    "msedge",
                    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                ),
                (
                    "chrome",
                    r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
                ),
            ];

            for (channel, path) in &browsers {
                let path_buf = PathBuf::from(path);
                if path_buf.exists() {
                    return Ok((channel.to_string(), path_buf));
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Try common installation paths
            let browsers = [
                ("chrome", "/usr/bin/google-chrome"),
                ("chrome", "/usr/bin/google-chrome-stable"),
                ("msedge", "/usr/bin/microsoft-edge"),
                ("chrome", "/usr/bin/brave-browser"),
                ("chromium", "/usr/bin/chromium"),
                ("chromium", "/usr/bin/chromium-browser"),
            ];

            for (channel, path) in &browsers {
                let path_buf = PathBuf::from(path);
                if path_buf.exists() {
                    return Ok((channel.to_string(), path_buf));
                }
            }
        }

        Err(AgentError::ToolError(
            "No supported browser executable found".to_string(),
        ))
    }

    /// Get connection method for debugging
    pub fn get_connection_method(&self) -> &str {
        &self.connection_method
    }

    /// Open URL with system's default handler (cross-platform)
    async fn open_url_with_system_handler(url: &str) -> ControllerResult<()> {
        log::info!("Opening URL with system handler: {}", url);

        #[cfg(target_os = "macos")]
        {
            let output = tokio::process::Command::new(system_url_commands::OPEN_URL_COMMAND)
                .arg(url)
                .output()
                .await
                .map_err(|e| {
                    AgentError::ToolError(format!("Failed to execute open command: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AgentError::ToolError(format!(
                    "Failed to open URL with system handler: {}",
                    stderr
                )));
            }
        }

        #[cfg(target_os = "windows")]
        {
            let output = tokio::process::Command::new(system_url_commands::OPEN_URL_COMMAND)
                .args(&system_url_commands::WINDOWS_START_ARGS)
                .arg(url)
                .output()
                .await
                .map_err(|e| {
                    AgentError::ToolError(format!("Failed to execute start command: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AgentError::ToolError(format!(
                    "Failed to open URL with system handler: {}",
                    stderr
                )));
            }
        }

        #[cfg(target_os = "linux")]
        {
            let output = tokio::process::Command::new(system_url_commands::OPEN_URL_COMMAND)
                .arg(url)
                .output()
                .await
                .map_err(|e| {
                    AgentError::ToolError(format!("Failed to execute xdg-open command: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AgentError::ToolError(format!(
                    "Failed to open URL with system handler: {}",
                    stderr
                )));
            }
        }

        log::info!("Successfully opened URL with system handler");
        Ok(())
    }

    // Helper to get or create a page, with enhanced error handling and recovery
    async fn ensure_page_exists(&self) -> ControllerResult<()> {
        // First attempt to work with the current page
        {
            let mut page_guard = self.page.lock().await;
            if page_guard.is_none() {
                log::info!("No active page found, will create a new one.");
            } else {
                // Check if the existing page is still valid
                if let Some(page) = page_guard.as_ref() {
                    // Try a simple JavaScript evaluation to check if page is still valid
                    match page.evaluate_function("() => true").await {
                        Ok(_) => {
                            log::debug!("Existing page is valid.");
                            return Ok(());
                        }
                        Err(e) => {
                            log::warn!(
                                "Existing page appears invalid: {}. Will create a new page.",
                                e
                            );
                            // Drop the stale handle; `Page::close` consumes self, and a
                            // dead target cannot be closed anyway.
                            *page_guard = None;
                        }
                    }
                }
            }
        } // Release the mutex guard here

        // At this point we need to create a new page
        // Multiple retry attempts with increasing backoff
        for attempt in 1..=3 {
            log::info!("Attempt {} of 3 to create new page", attempt);

            // Take the browser lock only for the call itself, never while holding
            // the page lock (see CLAUDE.md deadlock rules).
            let new_page = {
                let browser = self.browser.lock().await;
                browser.new_page("about:blank").await
            };

            match new_page {
                Ok(new_page) => {
                    let mut page_guard = self.page.lock().await;
                    *page_guard = Some(new_page);
                    // We opened this one, so cleanup is responsible for it.
                    self.owns_page.store(true, Ordering::SeqCst);
                    log::info!("New page created successfully on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Failed to create page on attempt {}: {}", attempt, e);

                    // Wait before retry with increasing backoff
                    let delay = 500 * attempt;
                    log::info!("Waiting {}ms before next attempt", delay);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                }
            }
        }

        // If we get here, all retries failed
        Err(AgentError::ToolError("Failed to create new page after multiple attempts. The browser may need to be restarted.".to_string()))
    }

    // --- Tool Implementation Methods ---

    pub async fn navigate(&self, args: &Value) -> ControllerResult<ToolResult> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| AgentError::ToolError("Missing 'url' argument".to_string()))?;
        let timeout_ms = args["timeout"]
            .as_u64()
            .unwrap_or(timeouts::DEFAULT_NAVIGATION_TIMEOUT_MS);

        log::info!("Navigating to: {}", url);

        // Check if this URL should be handled by the system instead of the browser
        if url_protocols::should_use_system_handler(url) {
            log::info!(
                "URL contains custom protocol or is not a web URL, opening with system handler: {}",
                url
            );

            // Open with system's default handler
            Self::open_url_with_system_handler(url).await?;

            let call_id = "nav_system_call".to_string();
            return Ok(ToolResult {
                call_id,
                output: serde_json::json!({
                    "status": "success",
                    "url": url,
                    "opened_with": "system_handler",
                    "message": "URL opened with system's default application"
                }),
            });
        }

        // Ensure a page exists, then get lock
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(|| {
            AgentError::ToolError(
                "Page lock could not be acquired after ensuring existence".to_string(),
            )
        })?;

        // Navigate with timeout. chromiumoxide has no per-call timeout, so we
        // impose one here to preserve the previous contract.
        let navigation = tokio::time::timeout(Duration::from_millis(timeout_ms), async {
            page.goto(url).await?;
            page.wait_for_navigation().await?;
            Ok::<(), chromiumoxide::error::CdpError>(())
        })
        .await
        .map_err(|_| {
            AgentError::ToolError(format!("Navigation timed out after {}ms", timeout_ms))
        })?;

        match navigation {
            Ok(_) => {
                log::info!("Navigation successful to: {}", url);

                // Get page title for a more helpful response
                let title = match page.get_title().await {
                    Ok(Some(t)) => t,
                    _ => "Unknown".to_string(),
                };

                let call_id = "nav_call".to_string();
                Ok(ToolResult {
                    call_id,
                    output: serde_json::json!({
                        "status": "success",
                        "url": url,
                        "title": title
                    }),
                })
            }
            Err(e) => {
                log::error!("Navigation failed for {}: {}", url, e);
                Err(AgentError::ToolError(format!("Navigation failed: {}", e)))
            }
        }
    }

    pub async fn get_current_url(&self, _args: &Value) -> ControllerResult<ToolResult> {
        // No need to ensure page exists if we just check Option
        let page_guard = self.page.lock().await;
        if let Some(page) = page_guard.as_ref() {
            let url = page
                .url()
                .await
                .map_err(|e| AgentError::ToolError(format!("Failed to get current URL: {}", e)))?
                .unwrap_or_default();
            log::info!("Current URL: {}", url);

            // Get page title too
            let title = match page.get_title().await {
                Ok(Some(t)) => t,
                _ => "Unknown".to_string(),
            };

            let call_id = "url_call".to_string();
            Ok(ToolResult {
                call_id,
                output: serde_json::json!({
                    "url": url,
                    "title": title
                }),
            })
        } else {
            log::warn!("get_current_url called with no active page.");
            Err(AgentError::ToolError(
                "No active browser page to get URL from".to_string(),
            ))
        }
    }

    // Implementation for extract_content
    pub async fn extract_content(&self, args: &Value) -> ControllerResult<ToolResult> {
        let selector = args["selector"].as_str().ok_or_else(|| {
            AgentError::ToolError(
                "Missing 'selector' argument for browser_extract_content".to_string(),
            )
        })?;
        let attribute = args["attribute"].as_str(); // Optional
        let property = args["property"].as_str(); // Optional
        let multiple = args["multiple"].as_bool().unwrap_or(false);

        if attribute.is_some() && property.is_some() {
            return Err(AgentError::ToolError(
                "Provide either 'attribute' or 'property' for browser_extract_content, not both"
                    .to_string(),
            ));
        }

        log::info!(
            "Extracting content with selector: {}, attribute: {:?}, property: {:?}, multiple: {}",
            selector,
            attribute,
            property,
            multiple
        );

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(|| {
            AgentError::ToolError("Page not available for content extraction".to_string())
        })?;

        // Expression that reads the requested value off an element bound to `el`.
        // `attribute` reads the static markup attribute; `property` reads the live
        // DOM property (`.value`, `.checked`, ...), which is the only way to see
        // state changed after parse (e.g. what `interact`'s `type` action wrote).
        let accessor = if let Some(attr) = attribute {
            format!(r#"el.getAttribute("{}")"#, attr.replace(r#"""#, r#"\""#))
        } else if let Some(prop) = property {
            format!(r#"el["{}"]"#, prop.replace(r#"""#, r#"\""#))
        } else {
            "el.textContent".to_string()
        };
        let escaped_selector = selector.replace(r#"""#, r#"\""#);

        // JavaScript approach keeps selector semantics consistent across engines
        let js_fn = if multiple {
            format!(
                r#"function() {{
                    const elements = Array.from(document.querySelectorAll("{escaped_selector}"));
                    return elements.map(el => {accessor});
                }}"#
            )
        } else {
            format!(
                r#"function() {{
                    const el = document.querySelector("{escaped_selector}");
                    return el ? {accessor} : null;
                }}"#
            )
        };

        // Execute JavaScript with type parameters that match expected arg & return types
        match Self::eval_json(page, &js_fn).await {
            Ok(result) => {
                log::info!("Content extraction successful for selector: {}", selector);
                let call_id = "extract_call".to_string();
                Ok(ToolResult {
                    call_id,
                    output: serde_json::json!({
                        "selector": selector,
                        "attribute": attribute,
                        "property": property,
                        "content": result,
                        "multiple": multiple
                    }),
                })
            }
            Err(e) => {
                log::error!("Content extraction failed for {}: {}", selector, e);
                Err(AgentError::ToolError(format!(
                    "Content extraction failed: {}",
                    e
                )))
            }
        }
    }

    // Implementation for interact
    pub async fn interact(&self, args: &Value) -> ControllerResult<ToolResult> {
        let action = args["action"].as_str().ok_or_else(|| {
            AgentError::ToolError("Missing 'action' argument for browser_interact".to_string())
        })?;

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(|| {
            AgentError::ToolError("Page not available for interaction".to_string())
        })?;

        match action {
            "click" => {
                let selector = args["selector"].as_str().ok_or_else(|| {
                    AgentError::ToolError(
                        "Missing 'selector' argument for click action".to_string(),
                    )
                })?;

                log::info!("Clicking on element: {}", selector);

                // Use JavaScript to perform the click
                let js_fn = format!(
                    r#"function() {{
                        const element = document.querySelector("{}");
                        if (!element) return false;
                        element.click();
                        return true;
                    }}"#,
                    selector.replace(r#"""#, r#"\""#)
                );

                // Add proper type annotations to evaluate
                match Self::eval_json(page, &js_fn).await {
                    Ok(result) => {
                        if result.as_bool().unwrap_or(false) {
                            // Wait a bit for any navigation/changes
                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                            let call_id = "click_call".to_string();
                            Ok(ToolResult {
                                call_id,
                                output: serde_json::json!({
                                    "status": "success",
                                    "action": "left_click",
                                    "selector": selector
                                }),
                            })
                        } else {
                            Err(AgentError::ToolError(format!(
                                "Element not found: {}",
                                selector
                            )))
                        }
                    }
                    Err(e) => Err(AgentError::ToolError(format!(
                        "Failed to click element: {}",
                        e
                    ))),
                }
            }
            "type" => {
                let selector = args["selector"].as_str().ok_or_else(|| {
                    AgentError::ToolError("Missing 'selector' argument for type action".to_string())
                })?;
                let value = args["value"].as_str().ok_or_else(|| {
                    AgentError::ToolError("Missing 'value' argument for type action".to_string())
                })?;

                log::info!("Typing '{}' into element: {}", value, selector);

                // Use JavaScript to fill the field
                let js_fn = format!(
                    r#"function() {{
                        const element = document.querySelector("{}");
                        if (!element) return false;

                        // Clear the field first
                        element.value = "";

                        // Then set the value and trigger events
                        element.value = "{}";
                        element.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        element.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return true;
                    }}"#,
                    selector.replace(r#"""#, r#"\""#),
                    value.replace(r#"""#, r#"\""#)
                );

                match Self::eval_json(page, &js_fn).await {
                    Ok(result) => {
                        if result.as_bool().unwrap_or(false) {
                            let call_id = "type_call".to_string();
                            Ok(ToolResult {
                                call_id,
                                output: serde_json::json!({
                                    "status": "success",
                                    "action": "type",
                                    "selector": selector,
                                    "value": value
                                }),
                            })
                        } else {
                            Err(AgentError::ToolError(format!(
                                "Element not found: {}",
                                selector
                            )))
                        }
                    }
                    Err(e) => Err(AgentError::ToolError(format!("Failed to type text: {}", e))),
                }
            }
            "select" => {
                let selector = args["selector"].as_str().ok_or_else(|| {
                    AgentError::ToolError(
                        "Missing 'selector' argument for select action".to_string(),
                    )
                })?;
                let value = args["value"].as_str().ok_or_else(|| {
                    AgentError::ToolError("Missing 'value' argument for select action".to_string())
                })?;

                log::info!("Selecting option '{}' in element: {}", value, selector);

                // Use JavaScript for selecting an option
                let js_fn = format!(
                    r#"function() {{
                        const element = document.querySelector("{}");
                        if (!element) return false;

                        // Set the value
                        element.value = "{}";

                        // Trigger change event
                        element.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return true;
                    }}"#,
                    selector.replace(r#"""#, r#"\""#),
                    value.replace(r#"""#, r#"\""#)
                );

                match Self::eval_json(page, &js_fn).await {
                    Ok(result) => {
                        if result.as_bool().unwrap_or(false) {
                            let call_id = "select_call".to_string();
                            Ok(ToolResult {
                                call_id,
                                output: serde_json::json!({
                                    "status": "success",
                                    "action": "select",
                                    "selector": selector,
                                    "value": value
                                }),
                            })
                        } else {
                            Err(AgentError::ToolError(format!(
                                "Element not found: {}",
                                selector
                            )))
                        }
                    }
                    Err(e) => Err(AgentError::ToolError(format!(
                        "Failed to select option: {}",
                        e
                    ))),
                }
            }
            "scroll" => {
                let direction = args["scroll_direction"].as_str().ok_or_else(|| {
                    AgentError::ToolError(
                        "Missing 'scroll_direction' argument for scroll action".to_string(),
                    )
                })?;
                let amount = args["scroll_amount_pixels"].as_i64().unwrap_or(500);

                log::info!("Scrolling {} by {} pixels", direction, amount);

                // Scroll using JavaScript
                let js_fn = match direction {
                    "down" => format!(
                        "function() {{ window.scrollBy(0, {}); return true; }}",
                        amount
                    ),
                    "up" => format!(
                        "function() {{ window.scrollBy(0, -{}); return true; }}",
                        amount
                    ),
                    _ => {
                        return Err(AgentError::ToolError(format!(
                            "Invalid scroll direction: {}",
                            direction
                        )))
                    }
                };

                match Self::eval_json(page, &js_fn).await {
                    Ok(_) => {
                        let call_id = "scroll_call".to_string();
                        Ok(ToolResult {
                            call_id,
                            output: serde_json::json!({
                                "status": "success",
                                "action": "scroll",
                                "direction": direction,
                                "amount": amount
                            }),
                        })
                    }
                    Err(e) => Err(AgentError::ToolError(format!(
                        "Failed to scroll page: {}",
                        e
                    ))),
                }
            }
            _ => Err(AgentError::ToolError(format!(
                "Unknown browser interaction action: {}",
                action
            ))),
        }
    }

    // Implementation for screenshot
    pub async fn screenshot(&self, args: &Value) -> ControllerResult<ToolResult> {
        let selector = args["selector"].as_str(); // Optional
        let full_page = args["full_page"].as_bool().unwrap_or(false);

        log::info!(
            "Taking screenshot. Selector: {:?}, Full page: {}",
            selector,
            full_page
        );

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(|| {
            AgentError::ToolError("Page not available for screenshot".to_string())
        })?;

        // chromiumoxide returns the encoded image directly, so there is no temp
        // file to create, read back, or clean up.
        let screenshot_params = if let Some(sel) = selector {
            // For element screenshot, capture element coordinates via JavaScript
            // then use clipping
            let js_fn = format!(
                r#"function() {{
                    const element = document.querySelector("{}");
                    if (!element) return null;

                    const rect = element.getBoundingClientRect();
                    return {{
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height
                    }};
                }}"#,
                sel.replace(r#"""#, r#"\""#)
            );

            // Get element position
            let element_rect = match Self::eval_json(page, &js_fn).await {
                Ok(rect) => {
                    if rect.is_null() {
                        return Err(AgentError::ToolError(format!(
                            "Element not found for screenshot: {}",
                            sel
                        )));
                    }
                    rect
                }
                Err(e) => {
                    return Err(AgentError::ToolError(format!(
                        "Error getting element position: {}",
                        e
                    )))
                }
            };

            // Get dimensions from element_rect and clip to them
            let x = element_rect["x"].as_f64().unwrap_or(0.0);
            let y = element_rect["y"].as_f64().unwrap_or(0.0);
            let width = element_rect["width"].as_f64().unwrap_or(100.0);
            let height = element_rect["height"].as_f64().unwrap_or(100.0);

            ScreenshotParams::builder()
                .clip(Viewport {
                    x,
                    y,
                    width,
                    height,
                    scale: 1.0,
                })
                .build()
        } else {
            // For full page or viewport screenshot
            ScreenshotParams::builder().full_page(full_page).build()
        };

        // Take the screenshot
        match page.screenshot(screenshot_params).await {
            Ok(image_data) => {
                let base64_data =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);

                let call_id = "screenshot_call".to_string();
                Ok(ToolResult {
                    call_id,
                    output: serde_json::json!({
                        "status": "success",
                        "base64": base64_data,
                        "selector": selector,
                        "full_page": full_page
                    }),
                })
            }
            Err(e) => Err(AgentError::ToolError(format!(
                "Failed to take screenshot: {}",
                e
            ))),
        }
    }

    // Ensure browser is closed gracefully
    pub async fn cleanup(&self) -> Result<(), AgentError> {
        log::info!("Cleaning up browser controller resources...");

        // A CDP attach means we borrowed a browser the user already had open,
        // and adopted one of their existing tabs. Closing either would destroy
        // work we do not own — `Browser.close` over CDP terminates the whole
        // application, every window and tab with it. Detach instead: drop our
        // handles and stop pumping events, leaving their session as we found it.
        // Browsers we launched ourselves are ours to shut down.
        let attached_to_user_browser = self.connection_method.starts_with("CDP:");

        // Close the page if it exists — but only if we opened it.
        {
            // Scope for page_guard
            let mut page_guard = self.page.lock().await; // Lock mutex
            if let Some(page) = page_guard.take() {
                if !self.owns_page.load(Ordering::SeqCst) {
                    log::info!("Adopted tab: leaving it open for the user.");
                } else if let Err(e) = page.close().await {
                    // Take ownership from Option — `Page::close` consumes self
                    log::error!("Failed to close browser page gracefully: {}", e);
                } else {
                    log::info!("Browser page closed.");
                }
            }
        } // MutexGuard is dropped here

        // Close the browser
        if attached_to_user_browser {
            log::info!("Attached session: leaving the user's browser running.");
        } else {
            let mut browser = self.browser.lock().await;
            if let Err(e) = browser.close().await {
                log::error!("Failed to close browser gracefully: {}", e);
            } else {
                log::info!("Browser instance closed.");
            }
        }

        // Stop pumping CDP events now that the connection is gone
        {
            let mut task_guard = self.handler_task.lock().await;
            if let Some(task) = task_guard.take() {
                task.abort();
                log::info!("CDP handler task stopped.");
            }
        }

        // Clean up temporary profile if this was a temporary profile launch
        if self.connection_method.starts_with("TempProfile:") {
            let temp_profile_path = self
                .connection_method
                .strip_prefix("TempProfile:")
                .unwrap_or("");
            if !temp_profile_path.is_empty() {
                log::info!(
                    "Cleaning up temporary profile directory: {}",
                    temp_profile_path
                );
                Self::remove_temp_profile_with_retry(temp_profile_path).await;
            }
        }

        Ok(())
    }

    /// Retries removing a temp profile directory with exponential backoff.
    /// The browser process may still hold file handles immediately after
    /// `browser.close()` returns, so a single removal attempt races with exit.
    async fn remove_temp_profile_with_retry(path: &str) {
        const MAX_RETRIES: u32 = 5;
        const INITIAL_DELAY_MS: u64 = 100;

        for attempt in 0..MAX_RETRIES {
            match std::fs::remove_dir_all(path) {
                Ok(()) => {
                    log::info!("Temporary profile directory cleaned up: {}", path);
                    return;
                }
                Err(e) if attempt + 1 < MAX_RETRIES => {
                    let delay = INITIAL_DELAY_MS << attempt; // 100, 200, 400, 800 ms
                    log::debug!(
                        "Temp profile removal attempt {} failed ({}), retrying in {}ms",
                        attempt + 1,
                        e,
                        delay
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to clean up temp profile {} after {} attempts: {}",
                        path,
                        MAX_RETRIES,
                        e
                    );
                }
            }
        }
    }

    /// Sweeps the system temp directory for orphaned `juno-browser-*` profile
    /// directories left by previous sessions that crashed before cleanup.
    pub async fn cleanup_orphaned_temp_profiles() {
        let temp_dir = std::env::temp_dir();
        let current_profile_name = format!("juno-browser-{}", std::process::id());

        let entries = match std::fs::read_dir(&temp_dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Could not read temp dir for orphaned profile sweep: {}", e);
                return;
            }
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            // Skip the current process's own profile and any non-Juno directories
            if name.starts_with("juno-browser-") && name != current_profile_name.as_str() {
                let path = entry.path();
                if path.is_dir() {
                    log::info!("Removing orphaned temp browser profile: {:?}", path);
                    if let Err(e) = std::fs::remove_dir_all(&path) {
                        log::warn!("Failed to remove orphaned profile {:?}: {}", path, e);
                    }
                }
            }
        }
    }
}

// Implement Drop to ensure cleanup happens if controller goes out of scope unexpectedly
impl Drop for BrowserController {
    fn drop(&mut self) {
        // This type is `Clone`, and every field is behind an `Arc`, so clones
        // share one browser. Without this guard the first clone to go out of
        // scope would tear down the browser that every surviving clone is still
        // using. Only the last handle standing is allowed to clean up.
        if Arc::strong_count(&self.browser) > 1 {
            return;
        }

        // See `cleanup()`: a browser we merely attached to belongs to the user.
        // Never close it or its tabs out from under them.
        let attached_to_user_browser = self.connection_method.starts_with("CDP:");

        // Check if Tokio runtime exists before attempting async cleanup
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let connection_method = self.connection_method.clone();
            let browser = self.browser.clone();
            let page = self.page.clone();
            let handler_task = self.handler_task.clone();
            let owns_page = self.owns_page.clone();

            // Schedule async cleanup in a detached task
            handle.spawn(async move {
                log::info!("BrowserController dropped, scheduling cleanup...");

                // Close the page if it exists
                {
                    let mut page_guard = page.lock().await;
                    if let Some(page) = page_guard.take() {
                        if !owns_page.load(Ordering::SeqCst) {
                            log::info!("Adopted tab: leaving it open for the user.");
                        } else if let Err(e) = page.close().await {
                            log::error!("Failed to close browser page in Drop: {}", e);
                        }
                    }
                }

                // Close the browser
                if attached_to_user_browser {
                    log::info!("Attached session: leaving the user's browser running.");
                } else {
                    let mut browser = browser.lock().await;
                    if let Err(e) = browser.close().await {
                        log::error!("Failed to close browser in Drop: {}", e);
                    }
                }

                // Stop the CDP event pump
                {
                    let mut task_guard = handler_task.lock().await;
                    if let Some(task) = task_guard.take() {
                        task.abort();
                    }
                }

                // Clean up temporary profile if needed
                if connection_method.starts_with("TempProfile:") {
                    if let Some(temp_path) = connection_method.strip_prefix("TempProfile:") {
                        if !temp_path.is_empty() {
                            BrowserController::remove_temp_profile_with_retry(temp_path).await;
                        }
                    }
                }

                log::info!("BrowserController cleanup completed");
            });
        } else {
            log::warn!("BrowserController dropped outside Tokio runtime - cleanup skipped");
            // Try to at least clean up temp profile synchronously
            if let Some(temp_path) = self.connection_method.strip_prefix("TempProfile:") {
                if !temp_path.is_empty() {
                    if let Err(e) = std::fs::remove_dir_all(temp_path) {
                        log::warn!("Failed to clean up temp profile in Drop: {}", e);
                    }
                }
            }
        }
    }
}

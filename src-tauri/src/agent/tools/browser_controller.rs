use base64;
use playwright::api::{Browser, BrowserContext, Page};
use playwright::Playwright;
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::agent::structs::{AgentError, ToolResult};
use crate::constants::{browser::{system_url_commands, url_protocols}, chrome_debug_urls, files::shell_commands, timeouts};

// Helper type alias for brevity
type ControllerResult<T> = Result<T, AgentError>;

// Timeout defaults
// const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 30000;

#[derive(Clone)]
pub struct BrowserController {
    // Store Playwright components
    _playwright: Arc<Playwright>, // Keep playwright instance alive
    browser: Arc<Browser>,
    context: Arc<BrowserContext>,
    // Store page in mutex for thread safety
    page: Arc<Mutex<Option<Page>>>,
    // Track connection method for debugging
    connection_method: String,
}

impl BrowserController {
    pub async fn new(playwright: Arc<Playwright>) -> ControllerResult<Self> {
        log::info!("BrowserController::new called - attempting optimized browser connection...");

        // Try three connection strategies in order of speed/preference

        // Strategy 1: Connect to existing browser instance via CDP (fastest - ~1-2 seconds)
        if let Ok(controller) = Self::try_connect_to_existing_browser(playwright.clone()).await {
            log::info!("Successfully connected to existing browser instance via CDP");
            return Ok(controller);
        }

        // Strategy 2: Launch with persistent user profile (fast - ~10-15 seconds)
        if let Ok(controller) = Self::try_launch_with_user_profile(playwright.clone()).await {
            log::info!("Successfully launched browser with user profile");
            return Ok(controller);
        }

        // Strategy 3: Fallback to fresh instance (current behavior - 90+ seconds)
        log::info!("Falling back to fresh browser instance...");
        Self::launch_fresh_instance(playwright).await
    }

    /// Strategy 1: Try to connect to an existing browser instance via CDP
    async fn try_connect_to_existing_browser(
        playwright: Arc<Playwright>,
    ) -> ControllerResult<Self> {
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
            match tokio::time::timeout(
                std::time::Duration::from_secs(
                    crate::constants::timeouts::BROWSER_CONNECTION_TIMEOUT_SECONDS,
                ), // Increased timeout for more reliable connection
                playwright
                    .chromium()
                    .connect_over_cdp_builder(endpoint)
                    .connect_over_cdp(),
            )
            .await
            {
                Ok(Ok(browser)) => {
                    log::info!("Successfully connected to existing browser at {}", endpoint);

                    // Create a new context since we can't clone existing ones
                    log::info!("Creating new context in existing browser");
                    let context = browser
                        .context_builder()
                        .accept_downloads(true)
                        .build()
                        .await
                        .map_err(|e| {
                            AgentError::ToolError(format!(
                                "Failed to create context in existing browser: {}",
                                e
                            ))
                        })?;

                    // Get existing page or create new one
                    let page = match context.pages() {
                        Ok(pages) if !pages.is_empty() => {
                            log::info!("Using existing page from browser");
                            Some(pages[0].clone())
                        }
                        _ => {
                            log::info!("Creating new page in existing browser");
                            match context.new_page().await {
                                Ok(page) => Some(page),
                                Err(e) => {
                                    log::warn!("Failed to create page in existing browser: {}", e);
                                    None
                                }
                            }
                        }
                    };

                    return Ok(BrowserController {
                        _playwright: playwright,
                        browser: Arc::new(browser),
                        context: Arc::new(context),
                        page: Arc::new(Mutex::new(page)),
                        connection_method: format!("CDP:{}", endpoint),
                    });
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
            match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                reqwest::get(&version_url),
            )
            .await
            {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        log::info!("Remote debugging is enabled on {}", endpoint);
                        return true;
                    }
                }
                _ => {} // Continue to next port
            }
        }

        log::debug!("Remote debugging is not available on any standard ports");
        false
    }

    /// Strategy 2: Launch browser with persistent user profile
    async fn try_launch_with_user_profile(playwright: Arc<Playwright>) -> ControllerResult<Self> {
        log::info!("Attempting to launch browser with user profile...");

        // First, check if Chrome is already running - if so, try with temporary profile
        let chrome_running = Self::is_chrome_running().await;
        if chrome_running {
            log::info!("Chrome is already running, will try with temporary profile to avoid SingletonLock conflict");
            return Self::try_launch_with_temp_profile(playwright).await;
        }

        // Detect user profile directory based on OS and browser
        let user_data_dir = Self::detect_user_profile_directory()?;
        log::info!("Using user data directory: {:?}", user_data_dir);

        // Additional check: look for SingletonLock file directly
        let singleton_lock_path = format!("{}/SingletonLock", user_data_dir);
        if std::path::Path::new(&singleton_lock_path).exists() {
            log::warn!("SingletonLock file exists at: {}", singleton_lock_path);
            log::info!("Profile appears to be in use, switching to temporary profile strategy");
            return Self::try_launch_with_temp_profile(playwright).await;
        }

        // Detect browser executable
        let browser_info = Self::detect_browser_executable()?;
        log::info!("Using browser: {} at {:?}", browser_info.0, browser_info.1);

        let chromium = playwright.chromium();

        // Use persistent_context_launcher for user profile access
        let user_data_path = std::path::Path::new(&user_data_dir);
        let args = vec![
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-component-update".to_string(), // Prevent update checks slowing startup
            "--remote-debugging-port=9222".to_string(), // Enable remote debugging for future CDP connections
        ];
        let launcher = chromium
            .persistent_context_launcher(user_data_path)
            .headless(false)
            .accept_downloads(true)
            .executable(&browser_info.1)
            .timeout(30000.0) // 30 second timeout
            .args(&args);

        // Note: Skip channel setting for broader browser compatibility

        let context_result = launcher.launch().await;

        match context_result {
            Ok(context) => {
                log::info!("Successfully launched browser with user profile");

                // Get browser from context - may return None for persistent contexts
                let browser = match context.browser() {
                    Ok(Some(browser)) => browser,
                    Ok(None) => {
                        return Err(AgentError::ToolError(
                            "No browser available from persistent context".to_string(),
                        ))
                    }
                    Err(e) => {
                        return Err(AgentError::ToolError(format!(
                            "Failed to get browser from persistent context: {}",
                            e
                        )))
                    }
                };

                // Get existing page or create new one
                let page = match context.pages() {
                    Ok(pages) if !pages.is_empty() => {
                        log::info!("Using existing page from persistent context");
                        Some(pages[0].clone())
                    }
                    _ => {
                        log::info!("Creating new page in persistent context");
                        match context.new_page().await {
                            Ok(page) => Some(page),
                            Err(e) => {
                                log::warn!("Failed to create page in persistent context: {}", e);
                                None
                            }
                        }
                    }
                };

                Ok(BrowserController {
                    _playwright: playwright,
                    browser: Arc::new(browser),
                    context: Arc::new(context),
                    page: Arc::new(Mutex::new(page)),
                    connection_method: format!("Persistent:{}", user_data_dir),
                })
            }
            Err(e) => {
                log::warn!("Failed to launch with user profile: {}", e);
                // Check if this is a SingletonLock error and try temp profile
                if e.to_string().contains("SingletonLock")
                    || e.to_string().contains("profile directory")
                {
                    log::info!("Profile conflict detected - trying temporary profile strategy");
                    return Self::try_launch_with_temp_profile(playwright).await;
                }
                Err(AgentError::ToolError(format!(
                    "Persistent profile launch failed: {}",
                    e
                )))
            }
        }
    }

    /// Strategy 2b: Launch browser with temporary profile (when main profile is in use)
    async fn try_launch_with_temp_profile(playwright: Arc<Playwright>) -> ControllerResult<Self> {
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

        let chromium = playwright.chromium();

        // Use persistent_context_launcher with temporary profile
        let args = vec![
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-component-update".to_string(),
            "--remote-debugging-port=9223".to_string(), // Use different port to avoid conflicts
        ];
        let launcher = chromium
            .persistent_context_launcher(&temp_profile)
            .headless(false)
            .accept_downloads(true)
            .executable(&browser_info.1)
            .timeout(30000.0) // 30 second timeout
            .args(&args);

        let context_result = launcher.launch().await;

        match context_result {
            Ok(context) => {
                log::info!("Successfully launched browser with temporary profile");

                // Get browser from context - may return None for persistent contexts
                let browser = match context.browser() {
                    Ok(Some(browser)) => browser,
                    Ok(None) => {
                        return Err(AgentError::ToolError(
                            "No browser available from temporary context".to_string(),
                        ))
                    }
                    Err(e) => {
                        return Err(AgentError::ToolError(format!(
                            "Failed to get browser from temporary context: {}",
                            e
                        )))
                    }
                };

                // Get existing page or create new one
                let page = match context.pages() {
                    Ok(pages) if !pages.is_empty() => {
                        log::info!("Using existing page from temporary context");
                        Some(pages[0].clone())
                    }
                    _ => {
                        log::info!("Creating new page in temporary context");
                        match context.new_page().await {
                            Ok(page) => Some(page),
                            Err(e) => {
                                log::warn!("Failed to create page in temporary context: {}", e);
                                None
                            }
                        }
                    }
                };

                Ok(BrowserController {
                    _playwright: playwright,
                    browser: Arc::new(browser),
                    context: Arc::new(context),
                    page: Arc::new(Mutex::new(page)),
                    connection_method: format!("TempProfile:{}", temp_profile.display()),
                })
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
    async fn launch_fresh_instance(playwright: Arc<Playwright>) -> ControllerResult<Self> {
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

        let chromium = playwright.chromium();

        // --- Build Launcher with Optional Path ---
        let mut launcher = chromium.launcher(); // Create mutable launcher

        if let Some(path) = &executable_path {
            log::info!("Using browser executable found at: {:?}", path);
            launcher = launcher.executable(path); // Pass the reference `path` directly
        } else {
            // If no path is found, return the specific error
            log::error!("Chromium executable not found. Set CHROMIUM_EXECUTABLE_PATH or install Chrome in a standard location.");
            return Err(AgentError::ToolError("Chromium executable not found. Set CHROMIUM_EXECUTABLE_PATH env var or ensure Chrome is installed in /Applications.".to_string()));
        }

        // Configure the launcher with more resilient options and enable remote debugging
        let args = vec![
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--no-sandbox".to_string(),
            "--remote-debugging-port=9222".to_string(), // Enable remote debugging for future connections
            "--disable-web-security".to_string(), // Reduce security restrictions for automation
            "--disable-features=VizDisplayCompositor".to_string(), // Improve stability
        ];

        launcher = launcher.headless(false).args(&args).timeout(45000.0); // Reduced timeout to 45 seconds for better user experience

        log::info!("Launching browser with 45 second timeout and remote debugging enabled...");

        let browser = match launcher.launch().await {
            Ok(browser) => {
                log::info!("Browser launched successfully.");
                browser
            }
            Err(e) => {
                // Try one more time with even fewer args
                log::warn!(
                    "First browser launch attempt failed: {}. Trying again with minimal args...",
                    e
                );

                // Reset launcher with minimal configuration
                let mut launcher = chromium.launcher();
                if let Some(path) = &executable_path {
                    launcher = launcher.executable(path);
                }

                // Try with absolute minimum arguments but keep remote debugging
                let minimal_args = vec!["--remote-debugging-port=9222".to_string()];

                launcher = launcher
                    .headless(false)
                    .args(&minimal_args)
                    .timeout(45000.0); // Consistent timeout with first attempt

                log::info!("Retrying browser launch with minimal configuration...");
                launcher.launch().await.map_err(|e| {
                    AgentError::ToolError(format!(
                        "Failed to launch browser (both attempts): {}",
                        e
                    ))
                })?
            }
        };

        log::info!(
            "Browser launched successfully. Browser version: {}",
            match browser.version() {
                Ok(version) => version,
                Err(_) => "unknown".to_string(),
            }
        );

        // Get existing contexts or create a new one
        let context = match browser.contexts() {
            Ok(contexts) if !contexts.is_empty() => {
                log::info!(
                    "Using existing browser context with {} contexts",
                    contexts.len()
                );
                // Create a new context since BrowserContext doesn't implement Clone
                browser
                    .context_builder()
                    .accept_downloads(true)
                    .build()
                    .await
                    .map_err(|e| {
                        AgentError::ToolError(format!(
                            "Failed to create context in existing browser: {}",
                            e
                        ))
                    })?
            }
            _ => {
                log::info!("Creating new context in existing browser");
                browser
                    .context_builder()
                    .accept_downloads(true)
                    .build()
                    .await
                    .map_err(|e| {
                        AgentError::ToolError(format!(
                            "Failed to create context in existing browser: {}",
                            e
                        ))
                    })?
            }
        };
        log::info!("Browser context created.");

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
                match context.new_page().await {
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

            if test_page.is_none() {
                return Err(AgentError::ToolError(format!(
                    "Failed to create initial page after {} attempts. Last error: {}",
                    max_attempts,
                    last_error.unwrap_or_else(|| "Unknown error".to_string())
                )));
            }

            test_page.unwrap()
        };

        // Try to navigate to a simple URL to verify browser works
        log::info!("Testing browser with navigation to about:blank...");
        match test_page
            .goto_builder("about:blank")
            .timeout(10000.0)
            .goto()
            .await
        {
            Ok(_) => log::info!("Browser test navigation successful."),
            Err(e) => {
                log::warn!("Browser test navigation failed: {}", e);
                // Continue anyway, but log the warning
            }
        }

        // Keep the test page open as our initial page
        let page = Arc::new(Mutex::new(Some(test_page)));
        log::info!("Browser successfully initialized with test page.");

        Ok(BrowserController {
            _playwright: playwright.clone(), // Store the Arc<Playwright>
            browser: Arc::new(browser),
            context: Arc::new(context),
            page,
            connection_method: "Fresh".to_string(),
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
        let mut retry_context = false;

        // First attempt to work with the current page
        {
            let mut page_guard = self.page.lock().await;
            if page_guard.is_none() {
                log::info!("No active page found, will create a new one.");
            } else {
                // Check if the existing page is still valid
                if let Some(page) = page_guard.as_ref() {
                    // Try a simple JavaScript evaluation to check if page is still valid
                    match page.evaluate::<Option<()>, bool>("() => true", None).await {
                        Ok(_) => {
                            log::debug!("Existing page is valid.");
                            return Ok(());
                        }
                        Err(e) => {
                            log::warn!(
                                "Existing page appears invalid: {}. Will create a new page.",
                                e
                            );
                            // Try to close the old page just in case
                            if let Err(ce) = page.close(None).await {
                                log::warn!("Error closing invalid page: {}", ce);
                            }

                            // Clear the invalid page reference
                            *page_guard = None;
                        }
                    }
                }
            }
        } // Release the mutex guard here

        // At this point we need to create a new page
        // Multiple retry attempts with increasing recovery actions
        for attempt in 1..=3 {
            log::info!("Attempt {} of 3 to create new page", attempt);

            if retry_context && attempt > 1 {
                // On later attempts, try to recreate the context first
                log::warn!("Attempting to recreate browser context as a recovery step");
                match self.browser.context_builder().build().await {
                    Ok(new_context) => {
                        // We successfully created a new context, but we can't replace the Arc-wrapped one
                        // Just log this situation - we'll still try to use the original context
                        log::info!(
                            "Created new context as a test, but continuing with original context"
                        );
                        // Close the test context as we can't use it
                        if let Err(e) = new_context.close().await {
                            log::warn!("Failed to close test context: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to recreate browser context: {}", e);
                        // If we can't even create a context, there may be deeper issues
                        if attempt == 3 {
                            return Err(AgentError::ToolError(format!(
                                "Browser appears to be in an unrecoverable state: {}",
                                e
                            )));
                        }
                    }
                }
            }

            // Try to create a new page
            match self.context.new_page().await {
                Ok(new_page) => {
                    let mut page_guard = self.page.lock().await;
                    *page_guard = Some(new_page);
                    log::info!("New page created successfully on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Failed to create page on attempt {}: {}", attempt, e);

                    // On first failure, set flag to try context recreation on next attempt
                    retry_context = true;

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
            log::info!("URL contains custom protocol or is not a web URL, opening with system handler: {}", url);

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

        // Navigate with timeout
        let goto_options = page.goto_builder(url).timeout(timeout_ms as f64);
        match goto_options.goto().await {
            Ok(_) => {
                log::info!("Navigation successful to: {}", url);

                // Get page title for a more helpful response
                let title = match page.title().await {
                    Ok(t) => t,
                    Err(_) => "Unknown".to_string(),
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
                .map_err(|e| AgentError::ToolError(format!("Failed to get current URL: {}", e)))?;
            log::info!("Current URL: {}", url);

            // Get page title too
            let title = match page.title().await {
                Ok(t) => t,
                Err(_) => "Unknown".to_string(),
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
        let multiple = args["multiple"].as_bool().unwrap_or(false);

        log::info!(
            "Extracting content with selector: {}, attribute: {:?}, multiple: {}",
            selector,
            attribute,
            multiple
        );

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(|| {
            AgentError::ToolError("Page not available for content extraction".to_string())
        })?;

        // JavaScript approach is more reliable across Playwright versions
        let js_fn = if multiple {
            // Multiple elements
            if let Some(attr) = attribute {
                format!(
                    r#"function() {{
                        const elements = Array.from(document.querySelectorAll("{}"));
                        return elements.map(el => el.getAttribute("{}"));
                    }}"#,
                    selector.replace(r#"""#, r#"\""#), // Escape quotes
                    attr.replace(r#"""#, r#"\""#)
                )
            } else {
                format!(
                    r#"function() {{
                        const elements = Array.from(document.querySelectorAll("{}"));
                        return elements.map(el => el.textContent);
                    }}"#,
                    selector.replace(r#"""#, r#"\""#)
                )
            }
        } else {
            // Single element
            if let Some(attr) = attribute {
                format!(
                    r#"function() {{
                        const element = document.querySelector("{}");
                        return element ? element.getAttribute("{}") : null;
                    }}"#,
                    selector.replace(r#"""#, r#"\""#),
                    attr.replace(r#"""#, r#"\""#)
                )
            } else {
                format!(
                    r#"function() {{
                        const element = document.querySelector("{}");
                        return element ? element.textContent : null;
                    }}"#,
                    selector.replace(r#"""#, r#"\""#)
                )
            }
        };

        // Execute JavaScript with type parameters that match expected arg & return types
        match page.evaluate::<Option<()>, Value>(&js_fn, None).await {
            Ok(result) => {
                log::info!("Content extraction successful for selector: {}", selector);
                let call_id = "extract_call".to_string();
                Ok(ToolResult {
                    call_id,
                    output: serde_json::json!({
                        "selector": selector,
                        "attribute": attribute,
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
                match page.evaluate::<Option<()>, Value>(&js_fn, None).await {
                    Ok(result) => {
                        if result.as_bool().unwrap_or(false) {
                            // Wait a bit for any navigation/changes
                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                            let call_id = "click_call".to_string();
                            Ok(ToolResult {
                                call_id,
                                output: serde_json::json!({
                                    "status": "success",
                                    "action": "click",
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

                match page.evaluate::<Option<()>, Value>(&js_fn, None).await {
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

                match page.evaluate::<Option<()>, Value>(&js_fn, None).await {
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

                match page.evaluate::<Option<()>, Value>(&js_fn, None).await {
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

        // Create a temporary file for the screenshot
        let temp_dir = tempfile::Builder::new()
            .prefix("juno_screenshot_")
            .tempdir()
            .map_err(|e| {
                AgentError::ToolError(format!("Failed to create temp directory: {}", e))
            })?;

        let screenshot_path = temp_dir.path().join("screenshot.png");

        // Configure screenshot options via builder
        let screenshot_builder = if let Some(sel) = selector {
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
            let element_rect = match page.evaluate::<Option<()>, Value>(&js_fn, None).await {
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

            // Fix the clip call to use a proper Rectangle struct
            // Get dimensions from element_rect
            let x = element_rect["x"].as_f64().unwrap_or(0.0);
            let y = element_rect["y"].as_f64().unwrap_or(0.0);
            let width = element_rect["width"].as_f64().unwrap_or(100.0);
            let height = element_rect["height"].as_f64().unwrap_or(100.0);

            // Configure builder with clipping using a proper FloatRect
            let clip_rect = playwright::api::FloatRect {
                x,
                y,
                width,
                height,
            };

            page.screenshot_builder()
                .path(screenshot_path.clone())
                .clip(clip_rect)
        } else {
            // For full page or viewport screenshot
            page.screenshot_builder()
                .path(screenshot_path.clone())
                .full_page(full_page)
        };

        // Take the screenshot
        match screenshot_builder.screenshot().await {
            Ok(_) => {
                // Read the file and convert to base64
                let image_data = std::fs::read(&screenshot_path).map_err(|e| {
                    AgentError::ToolError(format!("Failed to read screenshot file: {}", e))
                })?;

                let base64_data =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);

                // Clean up the temp directory (this removes the file too)
                if let Err(e) = temp_dir.close() {
                    log::warn!("Failed to clean up temp directory: {}", e);
                }

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

        // Close the page if it exists
        {
            // Scope for page_guard
            let mut page_guard = self.page.lock().await; // Lock mutex
            if let Some(page) = page_guard.take() {
                // Take ownership from Option
                if let Err(e) = page.close(None).await {
                    log::error!("Failed to close browser page gracefully: {}", e);
                } else {
                    log::info!("Browser page closed.");
                }
            }
        } // MutexGuard is dropped here

        // Close the context
        if let Err(e) = self.context.close().await {
            log::error!("Failed to close browser context gracefully: {}", e);
        } else {
            log::info!("Browser context closed.");
        }

        // Close the browser
        if let Err(e) = self.browser.close().await {
            log::error!("Failed to close browser gracefully: {}", e);
        } else {
            log::info!("Browser instance closed.");
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
                if let Err(e) = std::fs::remove_dir_all(temp_profile_path) {
                    log::warn!("Failed to clean up temporary profile directory: {}", e);
                } else {
                    log::info!("Temporary profile directory cleaned up successfully");
                }
            }
        }

        Ok(())
    }
}

// Implement Drop to ensure cleanup happens if controller goes out of scope unexpectedly
impl Drop for BrowserController {
    fn drop(&mut self) {
        // No automatic cleanup in Drop.
        // We'll only clean up explicitly when the app exits.
        log::debug!("BrowserController dropped, but browser instance is kept alive for reuse.");
    }
}

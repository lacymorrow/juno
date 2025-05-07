use playwright::Playwright;
use playwright::api::{Browser, BrowserContext, Page};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use base64;
use std::path::PathBuf;
use std::env;
// use tracing::{self};

use crate::agent::structs::{AgentError, ToolResult};

// Helper type alias for brevity
type ControllerResult<T> = Result<T, AgentError>;

// Timeout defaults
const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 30000;

#[derive(Clone)]
pub struct BrowserController {
    // Store Playwright components
    _playwright: Arc<Playwright>, // Keep playwright instance alive
    browser: Arc<Browser>,
    context: Arc<BrowserContext>,
    // Store page in mutex for thread safety
    page: Arc<Mutex<Option<Page>>>,
}

impl BrowserController {
    pub async fn new(playwright: Arc<Playwright>) -> ControllerResult<Self> {
        log::info!("BrowserController::new called with Playwright driver instance.");

        // --- Find Chromium Executable ---
        let executable_path: Option<PathBuf> = env::var("CHROMIUM_EXECUTABLE_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| {
                let common_paths = [
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
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

                common_paths.iter()
                    .map(PathBuf::from)
                    .find(|p| p.exists())
            });

        // Print system information for debugging
        log::info!("Operating system: {}", std::env::consts::OS);
        log::info!("Path environment: {:?}", env::var("PATH").unwrap_or_else(|_| "Not available".to_string()));

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

        // Configure the launcher with more resilient options - fewer options to reduce potential conflicts
        // On macOS, minimizing launch args can help with stability
        let args = vec![
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--no-sandbox".to_string(),
        ];

        launcher = launcher
            .headless(false)
            .args(&args)
            .timeout(90000.0); // Increase timeout to 90 seconds

        log::info!("Launching browser with 90 second timeout and simplified arguments...");

        let browser = match launcher.launch().await {
            Ok(browser) => {
                log::info!("Browser launched successfully.");
                browser
            },
            Err(e) => {
                // Try one more time with even fewer args
                log::warn!("First browser launch attempt failed: {}. Trying again with minimal args...", e);

                // Reset launcher with minimal configuration
                let mut launcher = chromium.launcher();
                if let Some(path) = &executable_path {
                    launcher = launcher.executable(path);
                }

                // Try with absolute minimum arguments
                launcher = launcher
                    .headless(false)
                    .timeout(90000.0);

                log::info!("Retrying browser launch with minimal configuration...");
                launcher.launch().await
                    .map_err(|e| AgentError::ToolError(format!("Failed to launch browser (both attempts): {}", e)))?
            }
        };

        log::info!("Browser launched successfully. Browser version: {}", match browser.version() {
            Ok(version) => version,
            Err(_) => "unknown".to_string(),
        });

        let context = browser.context_builder()
            .accept_downloads(true)
            .build()
            .await
            .map_err(|e| AgentError::ToolError(format!("Failed to create browser context: {}", e)))?;
        log::info!("Browser context created.");

        // Create a test page to verify everything is working - retry multiple times if needed
        log::info!("Creating test page to verify browser setup...");

        let test_page = {
            let max_attempts = 3;
            let mut last_error = None;
            let mut test_page = None;

            for attempt in 1..=max_attempts {
                log::info!("Attempt {} of {} to create initial page", attempt, max_attempts);
                match context.new_page().await {
                    Ok(page) => {
                        test_page = Some(page);
                        break;
                    },
                    Err(e) => {
                        log::warn!("Failed to create initial page on attempt {}: {}", attempt, e);
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
        match test_page.goto_builder("about:blank").timeout(10000.0).goto().await {
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
        })
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
                        },
                        Err(e) => {
                            log::warn!("Existing page appears invalid: {}. Will create a new page.", e);
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
                        log::info!("Created new context as a test, but continuing with original context");
                        // Close the test context as we can't use it
                        if let Err(e) = new_context.close().await {
                            log::warn!("Failed to close test context: {}", e);
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to recreate browser context: {}", e);
                        // If we can't even create a context, there may be deeper issues
                        if attempt == 3 {
                            return Err(AgentError::ToolError(format!("Browser appears to be in an unrecoverable state: {}", e)));
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
                },
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
        let url = args["url"].as_str().ok_or_else(|| AgentError::ToolError("Missing 'url' argument".to_string()))?;
        let timeout_ms = args["timeout"].as_u64().unwrap_or(DEFAULT_NAVIGATION_TIMEOUT_MS);

        log::info!("Navigating to: {}", url);

        // Ensure a page exists, then get lock
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(|| AgentError::ToolError("Page lock could not be acquired after ensuring existence".to_string()))?;

        // Navigate with timeout
        let goto_options = page.goto_builder(url).timeout(timeout_ms as f64);
        match goto_options.goto().await {
            Ok(_) => {
                 log::info!("Navigation successful to: {}", url);

                 // Get page title for a more helpful response
                 let title = match page.title().await {
                     Ok(t) => t,
                     Err(_) => "Unknown".to_string()
                 };

                 let call_id = "nav_call".to_string();
                 Ok(ToolResult {
                    call_id,
                    output: serde_json::json!({
                        "status": "success",
                        "url": url,
                        "title": title
                    })
                 })
            },
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
            let url = page.url().map_err(|e| AgentError::ToolError(format!("Failed to get current URL: {}", e)))?;
            log::info!("Current URL: {}", url);

            // Get page title too
            let title = match page.title().await {
                Ok(t) => t,
                Err(_) => "Unknown".to_string()
            };

            let call_id = "url_call".to_string();
            Ok(ToolResult {
                call_id,
                output: serde_json::json!({
                    "url": url,
                    "title": title
                })
            })
        } else {
            log::warn!("get_current_url called with no active page.");
            Err(AgentError::ToolError("No active browser page to get URL from".to_string()))
        }
    }

    // Implementation for extract_content
    pub async fn extract_content(&self, args: &Value) -> ControllerResult<ToolResult> {
        let selector = args["selector"].as_str().ok_or_else(|| AgentError::ToolError("Missing 'selector' argument for browser_extract_content".to_string()))?;
        let attribute = args["attribute"].as_str(); // Optional
        let multiple = args["multiple"].as_bool().unwrap_or(false);

        log::info!("Extracting content with selector: {}, attribute: {:?}, multiple: {}", selector, attribute, multiple);

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(||
            AgentError::ToolError("Page not available for content extraction".to_string()))?;

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
                    })
                })
            },
            Err(e) => {
                log::error!("Content extraction failed for {}: {}", selector, e);
                Err(AgentError::ToolError(format!("Content extraction failed: {}", e)))
            }
        }
    }

    // Implementation for interact
    pub async fn interact(&self, args: &Value) -> ControllerResult<ToolResult> {
        let action = args["action"].as_str().ok_or_else(||
            AgentError::ToolError("Missing 'action' argument for browser_interact".to_string()))?;

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(||
            AgentError::ToolError("Page not available for interaction".to_string()))?;

        match action {
            "click" => {
                let selector = args["selector"].as_str().ok_or_else(||
                    AgentError::ToolError("Missing 'selector' argument for click action".to_string()))?;

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
                                })
                            })
                        } else {
                            Err(AgentError::ToolError(format!("Element not found: {}", selector)))
                        }
                    },
                    Err(e) => Err(AgentError::ToolError(format!("Failed to click element: {}", e)))
                }
            },
            "type" => {
                let selector = args["selector"].as_str().ok_or_else(||
                    AgentError::ToolError("Missing 'selector' argument for type action".to_string()))?;
                let value = args["value"].as_str().ok_or_else(||
                    AgentError::ToolError("Missing 'value' argument for type action".to_string()))?;

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
                                })
                            })
                        } else {
                            Err(AgentError::ToolError(format!("Element not found: {}", selector)))
                        }
                    },
                    Err(e) => Err(AgentError::ToolError(format!("Failed to type text: {}", e)))
                }
            },
            "select" => {
                let selector = args["selector"].as_str().ok_or_else(||
                    AgentError::ToolError("Missing 'selector' argument for select action".to_string()))?;
                let value = args["value"].as_str().ok_or_else(||
                    AgentError::ToolError("Missing 'value' argument for select action".to_string()))?;

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
                                })
                            })
                        } else {
                            Err(AgentError::ToolError(format!("Element not found: {}", selector)))
                        }
                    },
                    Err(e) => Err(AgentError::ToolError(format!("Failed to select option: {}", e)))
                }
            },
            "scroll" => {
                let direction = args["scroll_direction"].as_str().ok_or_else(||
                    AgentError::ToolError("Missing 'scroll_direction' argument for scroll action".to_string()))?;
                let amount = args["scroll_amount_pixels"].as_i64().unwrap_or(500);

                log::info!("Scrolling {} by {} pixels", direction, amount);

                // Scroll using JavaScript
                let js_fn = match direction {
                    "down" => format!("function() {{ window.scrollBy(0, {}); return true; }}", amount),
                    "up" => format!("function() {{ window.scrollBy(0, -{}); return true; }}", amount),
                    _ => return Err(AgentError::ToolError(format!("Invalid scroll direction: {}", direction)))
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
                            })
                        })
                    },
                    Err(e) => Err(AgentError::ToolError(format!("Failed to scroll page: {}", e)))
                }
            },
            _ => Err(AgentError::ToolError(format!("Unknown browser interaction action: {}", action)))
        }
    }

    // Implementation for screenshot
    pub async fn screenshot(&self, args: &Value) -> ControllerResult<ToolResult> {
        let selector = args["selector"].as_str(); // Optional
        let full_page = args["full_page"].as_bool().unwrap_or(false);

        log::info!("Taking screenshot. Selector: {:?}, Full page: {}", selector, full_page);

        // Ensure a page exists
        self.ensure_page_exists().await?;
        let page_guard = self.page.lock().await;
        let page = page_guard.as_ref().ok_or_else(||
            AgentError::ToolError("Page not available for screenshot".to_string()))?;

        // Create a temporary file for the screenshot
        let temp_dir = tempfile::Builder::new().prefix("juno_screenshot_").tempdir()
            .map_err(|e| AgentError::ToolError(format!("Failed to create temp directory: {}", e)))?;

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
                        return Err(AgentError::ToolError(format!("Element not found for screenshot: {}", sel)));
                    }
                    rect
                },
                Err(e) => return Err(AgentError::ToolError(format!("Error getting element position: {}", e))),
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
                let image_data = std::fs::read(&screenshot_path)
                    .map_err(|e| AgentError::ToolError(format!("Failed to read screenshot file: {}", e)))?;

                let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);

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
                    })
                })
            },
            Err(e) => Err(AgentError::ToolError(format!("Failed to take screenshot: {}", e)))
        }
    }

    // Ensure browser is closed gracefully
    pub async fn cleanup(&self) -> Result<(), AgentError> {
        log::info!("Cleaning up browser controller resources...");

        // Close the page if it exists
        { // Scope for page_guard
            let mut page_guard = self.page.lock().await; // Lock mutex
            if let Some(page) = page_guard.take() { // Take ownership from Option
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

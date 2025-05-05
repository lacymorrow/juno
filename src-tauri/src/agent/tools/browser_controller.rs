use playwright::Playwright;
use playwright::api::{Browser, BrowserContext, Page};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use base64;
use std::path::{Path, PathBuf};
use std::env;
use thirtyfour::prelude::*;
use thirtyfour::WebDriver;
use tauri::Config;
use tracing::{info, error, warn};

use crate::agent::core::{AgentError, ToolResult};

// Helper type alias for brevity
type ControllerResult<T> = Result<T, AgentError>;

// Timeout defaults
const DEFAULT_NAVIGATION_TIMEOUT_MS: u64 = 30000;
const DEFAULT_ACTION_TIMEOUT_MS: u64 = 5000;

#[derive(Clone)]
pub struct BrowserController {
    // Store Playwright components
    playwright: Arc<Playwright>,
    browser: Arc<Browser>,
    context: Arc<BrowserContext>,
    // Store page in mutex for thread safety
    page: Arc<Mutex<Option<Page>>>,
}

impl BrowserController {
    pub async fn new() -> ControllerResult<Self> {
        log::info!("Initializing Playwright...");
        let playwright = Playwright::initialize().await
            .map_err(|e| AgentError::ToolError(format!("Failed to initialize Playwright: {}", e)))?;

        // --- Find Chromium Executable ---
        let executable_path: Option<PathBuf> = env::var("CHROMIUM_EXECUTABLE_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| {
                let common_paths = [
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                    // Add other potential paths if needed (e.g., Brave, Edge)
                    // "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
                    // "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                ];
                common_paths.iter()
                    .map(PathBuf::from)
                    .find(|p| p.exists())
            });

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
            // Note: We could alternatively try launching without the path, but it's better to be explicit
            // log::warn!("Chromium executable path not explicitly found. Attempting launch without path...");
        }

        let browser = launcher // Use the configured launcher
            // Try launching non-headless for debugging
            .headless(false) // Set other options like headless here
            .launch()
            .await
            .map_err(|e| AgentError::ToolError(format!("Failed to launch browser: {}", e)))?;
        log::info!("Browser launched successfully.");

        let context = browser.context_builder().build().await
            .map_err(|e| AgentError::ToolError(format!("Failed to create browser context: {}", e)))?;
        log::info!("Browser context created.");

        let page = Arc::new(Mutex::new(None));

        Ok(BrowserController {
            playwright: Arc::new(playwright),
            browser: Arc::new(browser),
            context: Arc::new(context),
            page,
        })
    }

    // Helper to get or create a page
    async fn ensure_page_exists(&self) -> ControllerResult<()> {
        let mut page_guard = self.page.lock().await;
        if page_guard.is_none() {
            log::info!("No active page found, creating a new one.");
            let new_page = self.context.new_page().await
                .map_err(|e| AgentError::ToolError(format!("Failed to create new page: {}", e)))?;
            *page_guard = Some(new_page);
            log::info!("New page created successfully.");
        } else {
             log::debug!("Existing page found.");
        }
        Ok(())
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
        log::info!("Closing browser...");

        // First close any open page
        {
            let mut page_guard = self.page.lock().await;
            if let Some(page) = page_guard.take() {
                if let Err(e) = page.close(Some(true)).await {
                    log::warn!("Failed to close page: {}", e);
                    // Continue with cleanup even if page close fails
                }
            }
        }

        // Then close the browser context
        if let Err(e) = self.context.close().await {
            log::warn!("Failed to close browser context: {}", e);
            // Continue with cleanup even if context close fails
        }

        // Finally close the browser
        if let Err(e) = self.browser.close().await {
            log::error!("Failed to close browser: {}", e);
            return Err(AgentError::ToolError(format!("Failed to close browser: {}", e)));
        }

        log::info!("Browser closed successfully");
        Ok(())
    }
}

// Add a shutdown hook to handle cleanup on application exit
impl Drop for BrowserController {
    fn drop(&mut self) {
        log::info!("BrowserController is being dropped, scheduling cleanup");

        // We can't run async code directly in drop, so let's spawn a task
        // This approach is not guaranteed to complete if the application exits abruptly
        let browser_clone = self.browser.clone();
        let context_clone = self.context.clone();

        tokio::spawn(async move {
            log::info!("Performing cleanup for dropped BrowserController");

            if let Err(e) = context_clone.close().await {
                log::warn!("Failed to close browser context during cleanup: {}", e);
            }

            if let Err(e) = browser_clone.close().await {
                log::error!("Failed to close browser during cleanup: {}", e);
            } else {
                log::info!("Browser cleanup completed successfully");
            }
        });
    }
}

use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};
use computer_use_ai_sdk::Desktop;

use crate::types::AppState;
use crate::handlers::utils::refresh_elements_and_attributes_after_action;
use crate::types::ListElementsAndAttributesResponse;

#[derive(Deserialize, Clone)]
pub struct OpenUrlRequest {
    pub url: String,
    pub browser: Option<String>,
}

#[derive(Serialize)]
pub struct OpenUrlResponse {
    pub success: bool,
    pub message: String,
}

// First, create a new response type that combines both results
#[derive(Serialize)]
pub struct OpenUrlWithElementsResponse {
    pub url: OpenUrlResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

pub async fn open_url_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OpenUrlRequest>,
) -> Result<JsonResponse<OpenUrlWithElementsResponse>, (StatusCode, JsonResponse<serde_json::Value>)> {
    info!("handling request to open url: {}", request.url);
    
    // Create Desktop automation instance
    let desktop = match Desktop::new(false, true) {
        Ok(desktop) => desktop,
        Err(err) => {
            error!("failed to initialize automation: {}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({"error": format!("failed to initialize automation: {}", err)})),
            ));
        }
    };

    // Open the URL
    let browser_ref = request.browser.as_deref();
    
    if let Some(browser) = browser_ref {
        debug!("opening url {} in specified browser: {}", request.url, browser);
    } else {
        debug!("opening url {} in system default browser", request.url);
    }
    
    match desktop.open_url(&request.url, browser_ref) {
        Ok(_) => {
            // Wait for browser to start/activate
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
            
            // Determine which browser to use for refreshing elements
            let browser_for_refresh: Option<String> = if let Some(browser) = &request.browser {
                // If user specified a browser, use that
                info!("using specified browser for refresh: {}", browser);
                
                // Map common browser names to possible variations
                let browser_search = match browser.as_str() {
                    "Google Chrome" => "Chrome",
                    "Microsoft Edge" => "Edge",
                    _ => browser.as_str(),
                };
                
                debug!("searching for browser as: {}", browser_search);
                
                if desktop.application(browser_search).is_ok() {
                    info!("found browser with name: {}", browser_search);
                    Some(browser_search.to_string())
                } else {
                    info!("could not find browser with name: {}", browser_search);
                    None
                }
            } else {
                // Try to detect which browser is running
                let likely_browsers = ["Arc", "Safari", "Chrome", "Firefox", "Edge", "Opera", "Brave"];
                let mut detected = None;
                
                for browser in likely_browsers.iter() {
                    match desktop.application(browser) {
                        Ok(_) => {
                            info!("detected browser for refresh: {}", browser);
                            detected = Some(browser.to_string());
                            break;
                        },
                        Err(_) => continue,
                    }
                }
                
                // If we couldn't detect a specific browser, we don't do element refresh
                if detected.is_none() {
                    info!("could not detect which browser was used - skipping element refresh");
                }
                
                detected
            };
            
            info!("successfully opened url: {}", request.url);
            
            // Create success response
            let url_response = OpenUrlResponse {
                success: true,
                message: if let Some(browser) = &browser_for_refresh {
                    format!("successfully opened URL: {} in browser: {}", request.url, browser)
                } else {
                    format!("successfully opened URL: {} in default browser (unknown)", request.url)
                },
            };
            
            // Only attempt to refresh elements if we know which browser to target
            let elements_response = if let Some(browser) = browser_for_refresh {
                refresh_elements_and_attributes_after_action(state, browser, 2000).await
            } else {
                // If we don't know which browser was used, don't try to refresh elements
                None
            };
            
            // Return combined response
            Ok(JsonResponse(OpenUrlWithElementsResponse {
                url: url_response,
                elements: elements_response,
            }))
        },
        Err(err) => {
            error!("failed to open url {}: {}", request.url, err);
            Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({"error": format!("failed to open URL: {}", err)})),
            ))
        },
    }
}
/*

curl -X POST http://localhost:8080/api/open-url \
  -H "Content-Type: application/json" \
  -d '{"url": "https://twitter.com"}' \
  | jq -r '"url opening:",
    "  success: \(.url.success)",
    "  message: \(.url.message)",
    "\nelements: \(if .elements then
      if .elements.elements then
        .elements.elements | map("\n  [\(.index)]: \(.role)\(if .text then " \"\(.text)\"" else "" end)") | join("")
      else
        "\n  no elements found"
      end
    else
      "\n  no elements info available"
    end)",
    "\nstats summary: \(if .elements then
      "\n  count: \(.elements.stats.count)",
      "  with_text_count: \(.elements.stats.with_text_count)",
      "  without_text_count: \(.elements.stats.without_text_count)",
      "  excluded_count: \(.elements.stats.excluded_count)",
      "  processing time: \(.elements.processing_time_seconds)s",
      "  cache_id: \(.elements.cache_info.cache_id)",
      "  expires_at: \(.elements.cache_info.expires_at)",
      "  element_count: \(.elements.cache_info.element_count)"
    else
      "\n  no stats available"
    end)"'
  
*/
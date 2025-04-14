use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use computer_use_ai_sdk::Desktop;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use chrono;
use serde_json::{self, json, Value};
use tracing::{error, info};
use uuid::Uuid;

use crate::types::*;
use crate::AppState;
use crate::types::ElementStatistics;
use crate::types::ListElementsAndAttributesResponse;

pub async fn list_elements_and_attributes_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ListInteractableElementsRequest>,
) -> Result<JsonResponse<ListElementsAndAttributesResponse>, (StatusCode, JsonResponse<serde_json::Value>)> {
    // Record start time at the beginning of the handler
    let start_time = std::time::Instant::now();
    
    info!("listing all elements and attributes for app: {}", request.app_name);
    
    // Create desktop automation engine
    let desktop = match Desktop::new(
        request.use_background_apps.unwrap_or(false),
        request.activate_app.unwrap_or(false),
    ) {
        Ok(d) => d,
        Err(e) => {
            error!("failed to initialize desktop automation: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({
                    "error": format!("failed to initialize desktop automation: {}", e)
                })),
            ));
        }
    };

    // Get application
    let app = match desktop.application(&request.app_name) {
        Ok(app) => app,
        Err(e) => {
            error!("application not found: {}", e);
            return Err((
                StatusCode::NOT_FOUND,
                JsonResponse(json!({
                    "error": format!("application not found: {}", e)
                })),
            ));
        }
    };

    // Get elements from the application
    let locator = match app.locator("") {
        Ok(locator) => locator,
        Err(e) => {
            error!("failed to get elements: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({
                    "error": format!("failed to get elements: {}", e)
                })),
            ));
        }
    };

    let elements = match locator.all() {
        Ok(elements) => elements,
        Err(e) => {
            error!("failed to get elements: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({
                    "error": format!("failed to get elements: {}", e)
                })),
            ));
        }
    };

    info!("found {} elements in {}", elements.len(), request.app_name);

    // Define non-interactable roles
    let non_interactable_roles = [
        "AXGroup", "AXStaticText", "AXUnknown", "AXSeparator", 
        "AXHeading", "AXLayoutArea", "AXHelpTag", "AXGrowArea"
    ];

    // Collect all elements with their filtered attributes
    let mut result_elements = Vec::new();
    let mut excluded_count = 0;
    let mut excluded_non_interactable_count = 0;
    let mut excluded_no_text_count = 0;
    
    for (i, element) in elements.iter().enumerate() {
        // Extract complete attributes from element
        let attrs = element.attributes();
        
        // Create a complete attributes object - removed id field
        let mut element_data = json!({
            "index": i,
            "role": attrs.role,
            // "id" field removed as requested
        });
        
        // Check if we have role description in properties and modify role field
        let role_without_ax = attrs.role.trim_start_matches("AX");
        
        if let Some(props) = attrs.properties.get("AXRoleDescription") {
            // First check if the property exists, then if it's a string value
            if let Some(role_desc) = props.as_ref().and_then(|v| v.as_str()) {
                // Only include role description if it's different from the role (after removing AX prefix)
                if !role_desc.eq_ignore_ascii_case(role_without_ax) {
                    element_data["role"] = Value::String(format!("{} ({})", attrs.role, role_desc));
                }
            }
        }
        
        // Collect all text content for the combined field
        let mut text_parts = Vec::new();
        
        // Collect value, label, description for text_parts
        if let Some(value) = &attrs.value {
            if !value.is_empty() {
                text_parts.push(value.clone());
            }
        }
        
        if let Some(label) = &attrs.label {
            if !label.is_empty() {
                text_parts.push(label.clone());
            }
        }
        
        if let Some(desc) = &attrs.description {
            if !desc.is_empty() {
                text_parts.push(desc.clone());
            }
        }
        
        // Add text values from properties if they exist
        for (key, value_opt) in &attrs.properties {
            // Skip properties that are likely to be non-human-readable
            if key.contains("Parent") || 
               key.contains("Children") || 
               key == "AXRoleDescription" || 
               key == "AXRole" || 
               key == "AXTopLevelUIElement" || 
               key == "AXWindow" {
                continue;
            }
            
            if let Some(value) = value_opt {
                if let Some(text_value) = value.as_str() {
                    if !text_value.is_empty() {
                        text_parts.push(text_value.to_string());
                    }
                }
            }
        }
        
        // Create the text field with all content
        let has_text = !text_parts.is_empty();
        if has_text {
            let combined_text = text_parts.join(" ");
            
            if i < 5 {  
                info!("element {}: text field created: '{}'", i, &combined_text);
            }
            
            element_data["text"] = Value::String(combined_text);
        }
        
        // Check if element is non-interactable based on its role
        let role = attrs.role.as_str();
        let is_non_interactable = non_interactable_roles.contains(&role);
        
        // Include element if it's either interactable OR has text
        if !is_non_interactable || has_text {
            // Add element to result
            result_elements.push(element_data);
        } else {
            // Count excluded elements
            excluded_count += 1;
            
            // Count by exclusion criteria
            if is_non_interactable {
                excluded_non_interactable_count += 1;
            }
            
            if !has_text {
                excluded_no_text_count += 1;
            }
        }
    }

    info!("excluded {} elements (non-interactable: {}, no text: {})", 
          excluded_count, excluded_non_interactable_count, excluded_no_text_count);

    // Apply max_elements limit if specified
    if let Some(max) = request.max_elements {
        if result_elements.len() > max {
            result_elements.truncate(max);
        }
    }

    // Generate element statistics
    let element_stats = generate_element_statistics(&result_elements, excluded_count, 
                                                   excluded_non_interactable_count, excluded_no_text_count);
    info!("generated statistics: {} different roles found", element_stats.top_roles.len());

    // Generate a cache ID and store elements in cache
    let cache_id = Uuid::new_v4().to_string();
    let cache_timestamp = Instant::now();
    let ttl_seconds: u64 = 30;

    {
        let mut cache = state.element_cache.lock().await;
        *cache = Some((elements.clone(), cache_timestamp, request.app_name.clone()));
    }

    // Create cache info for response
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::seconds(ttl_seconds as i64);

    let cache_info = ElementCacheInfo {
        cache_id: cache_id.clone(),
        timestamp: now.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
        element_count: elements.len(),
        ttl_seconds,
    };

    // Calculate elapsed time before returning response
    let elapsed_time = start_time.elapsed().as_secs_f64();
    let elapsed_formatted = format!("{:.2}", elapsed_time);
    
    info!("processed request in {} seconds", elapsed_formatted);
    
    Ok(JsonResponse(ListElementsAndAttributesResponse {
        elements: result_elements,
        cache_info,
        stats: element_stats,
        processing_time_seconds: elapsed_formatted,
    }))
}

// Function to generate statistics about the elements
fn generate_element_statistics(
    elements: &[serde_json::Value], 
    excluded_count: usize,
    excluded_non_interactable: usize,
    excluded_no_text: usize
) -> ElementStatistics {
    let mut roles_count: HashMap<String, u32> = HashMap::new();
    let mut property_counts: HashMap<String, u32> = HashMap::new();
    
    // Add counters for specific AX properties
    let mut ax_enabled_true = 0;
    let mut ax_enabled_false = 0;
    let mut ax_focused_true = 0;
    let mut ax_focused_false = 0;
    
    // Track elements with and without text
    let mut with_text_count = 0;
    let mut without_text_count = 0;
    
    for element in elements {
        // Count elements with/without text
        if element.get("text").is_some() {
            with_text_count += 1;
        } else {
            without_text_count += 1;
        }
        
        // Count by role
        if let Some(role) = element.get("role").and_then(|r| r.as_str()) {
            *roles_count.entry(role.to_string()).or_insert(0) += 1;
        }
        
        // Count elements with various properties (only track non-zero counts)
        if element.get("description").is_some() {
            *property_counts.entry("with_description".to_string()).or_insert(0) += 1;
        }
        
        if element.get("value").is_some() {
            *property_counts.entry("with_value".to_string()).or_insert(0) += 1;
        }
        
        if element.get("label").is_some() {
            *property_counts.entry("with_label".to_string()).or_insert(0) += 1;
        }
        
        if element.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false) {
            *property_counts.entry("enabled".to_string()).or_insert(0) += 1;
        }
        
        if element.get("focused").and_then(|f| f.as_bool()).unwrap_or(false) {
            *property_counts.entry("focused".to_string()).or_insert(0) += 1;
        }
        
        // Count elements with properties field
        if element.get("properties").is_some() {
            *property_counts.entry("with_properties".to_string()).or_insert(0) += 1;
            
            // Check for specific AX properties
            if let Some(props) = element.get("properties") {
                if let Some(enabled) = props.get("AXEnabled") {
                    if enabled.as_bool().unwrap_or(false) {
                        ax_enabled_true += 1;
                    } else {
                        ax_enabled_false += 1;
                    }
                }
                
                if let Some(focused) = props.get("AXFocused") {
                    if focused.as_bool().unwrap_or(false) {
                        ax_focused_true += 1;
                    } else {
                        ax_focused_false += 1;
                    }
                }
            }
        }
    }
    
    // Add AX property counts to the property_counts map
    if ax_enabled_true > 0 {
        property_counts.insert("AXEnabled_true".to_string(), ax_enabled_true);
    }
    if ax_enabled_false > 0 {
        property_counts.insert("AXEnabled_false".to_string(), ax_enabled_false);
    }
    if ax_focused_true > 0 {
        property_counts.insert("AXFocused_true".to_string(), ax_focused_true);
    }
    if ax_focused_false > 0 {
        property_counts.insert("AXFocused_false".to_string(), ax_focused_false);
    }
    
    // Sort roles by count (descending)
    let mut roles: Vec<(String, u32)> = roles_count.into_iter().collect();
    roles.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Limit to top roles (e.g., 5) for conciseness
    let top_limit = 5;
    let top_roles: HashMap<String, u32> = roles.into_iter()
        .take(top_limit)
        .collect();
    
    // Filter out zero counts from property_counts (already handled with conditionals)
    let properties = property_counts;
    
    ElementStatistics {
        count: elements.len(),
        excluded_count,
        excluded_non_interactable,
        excluded_no_text,
        with_text_count,
        without_text_count,
        top_roles,
        properties,
    }
}

/*
Test curl command provided you run the server, nicely formatted, provided you have jq, type in the app you want, e.g. Discord:

curl -X POST http://localhost:8080/api/list-elements-and-attributes \
  -H "Content-Type: application/json" \
  -d '{"app_name": "Whatsapp"}' \
  | jq -r '(.elements[] | "[\(.index)]: \(.role)\(if .text then " \"\(.text)\"" else "" end)"),
    "\n--- summary ---",
    "stats:",
    "  count: \(.stats.count)",
    "  excluded_count: \(.stats.excluded_count)",
    "  excluded_non_interactable: \(.stats.excluded_non_interactable)",
    "  excluded_no_text: \(.stats.excluded_no_text)",
    "  with_text_count: \(.stats.with_text_count)",
    "  without_text_count: \(.stats.without_text_count)",
    "  top_roles: \(.stats.top_roles | to_entries | map("    \(.key): \(.value)") | join("\n"))",
    "processing time: \(.processing_time_seconds)s",
    "cache info:",
    "  cache_id: \(.cache_info.cache_id)",
    "  timestamp: \(.cache_info.timestamp)",
    "  expires_at: \(.cache_info.expires_at)",
    "  element_count: \(.cache_info.element_count)",
    "  ttl_seconds: \(.cache_info.ttl_seconds)"'
*/
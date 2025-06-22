use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error};

// Define cache TTL constant
const CACHE_TTL_SECONDS: u64 = 30;

use crate::server::handlers::utils::refresh_elements_and_attributes_after_action;
use crate::server::types::{
    AppState, ListElementsAndAttributesResponse, TypeByIndexRequest,
    TypeByIndexResponse, TypeByIndexWithElementsResponse,
};

pub async fn type_by_index_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TypeByIndexRequest>,
) -> Result<
    JsonResponse<TypeByIndexWithElementsResponse>,
    (StatusCode, JsonResponse<serde_json::Value>),
> {
    // Get elements from cache
    let cache_guard = state.element_cache.lock().await;
    if let Some(cache) = cache_guard.as_ref() {
        if cache.timestamp.elapsed() < Duration::from_secs(CACHE_TTL_SECONDS) {
            if request.element_index < cache.elements.len() {
                let element_to_type = cache.elements[request.element_index].clone();
                let app_name_from_cache = cache.app_name.clone(); // Clone the String from cache
                drop(cache_guard); // Release the lock before async operations

                debug!(
                    "typing '{}' into element at index {} from cache",
                    request.text, request.element_index
                );
                match element_to_type.type_text(&request.text) {
                    Ok(_) => {
                        debug!("type text successful");
                        // Refresh elements after the action
                        let refreshed_elements = refresh_elements_and_attributes_with_cache(
                            state.clone(),
                            app_name_from_cache, // Use cloned String
                            500,
                        )
                        .await;

                        return Ok(JsonResponse(TypeByIndexWithElementsResponse {
                            type_action: TypeByIndexResponse {
                                success: true,
                                message: format!(
                                    "Typed '{}' into element at index {}",
                                    request.text, request.element_index
                                ),
                            },
                            elements: refreshed_elements,
                        }));
                    }
                    Err(e) => {
                        error!(
                            "failed to type '{}' into element at index {}: {}",
                            request.text, request.element_index, e
                        );
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to type text: {}", e)})),
                        ));
                    }
                }
            } else {
                // Index out of bounds for the cached elements
                error!(
                    "element index {} out of bounds for cached elements (count: {}), app: {}",
                    request.element_index,
                    cache.elements.len(),
                    cache.app_name
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error": format!("Element index {} out of bounds for cached elements of app '{}'. Try listing elements again.", request.element_index, cache.app_name)}),
                    ),
                ));
            }
        } else {
            debug!("cache miss or expired");
            drop(cache_guard);
        }
    } else {
        debug!("element cache is empty");
        drop(cache_guard);
    }

    // Cache miss logic...
    error!("cannot perform type by index without a valid element cache. list elements first.");
    Err((
        StatusCode::PRECONDITION_FAILED,
        Json(
            json!({"error": "Element cache miss or invalid. Please list elements for the target application first."}),
        ),
    ))
}

// Helper function
async fn refresh_elements_and_attributes_with_cache(
    state: Arc<AppState>,
    app_name: String, // Takes String
    wait_ms: u64,
) -> Option<ListElementsAndAttributesResponse> {
    tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;
    refresh_elements_and_attributes_after_action(state, app_name, 500).await // Pass String
}

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};

use crate::server::handlers::utils::refresh_elements_and_attributes_after_action;
use crate::server::types::{
    AppState, ClickByIndexRequest, ClickByIndexResponse, ClickByIndexWithElementsResponse,
    ListElementsAndAttributesResponse,
};

pub async fn click_by_index_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClickByIndexRequest>,
) -> Result<
    JsonResponse<ClickByIndexWithElementsResponse>,
    (StatusCode, JsonResponse<serde_json::Value>),
> {
    let cache_guard = state.element_cache.lock().await;
    if let Some(cache) = cache_guard.as_ref() {
        if cache.timestamp.elapsed() < std::time::Duration::from_secs(30) {
            if request.element_index < cache.elements.len() {
                let element_to_click = cache.elements[request.element_index].clone();
                let app_name_from_cache = cache.app_name.clone();
                drop(cache_guard);

                debug!(
                    "clicking element at index {} from cache",
                    request.element_index
                );
                match element_to_click.click() {
                    Ok(click_result) => {
                        debug!("click successful: {:?}", click_result);
                        let refreshed_elements = refresh_elements_and_attributes_with_cache(
                            state.clone(),
                            app_name_from_cache,
                            500,
                        )
                        .await;

                        return Ok(JsonResponse(ClickByIndexWithElementsResponse {
                            click: ClickByIndexResponse {
                                success: true,
                                message: format!(
                                    "Clicked element at index {}: Method={}, Details={}",
                                    request.element_index,
                                    click_result.method,
                                    click_result.details
                                ),
                                elements: None,
                            },
                            elements: refreshed_elements,
                        }));
                    }
                    Err(e) => {
                        error!(
                            "failed to click element at index {}: {}",
                            request.element_index, e
                        );
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            JsonResponse(
                                json!({"error": format!("Failed to click element: {}", e)}),
                            ),
                        ));
                    }
                }
            } else {
                error!(
                    "element index {} out of bounds for cached elements (count: {}), app: {}",
                    request.element_index,
                    cache.elements.len(),
                    cache.app_name
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    JsonResponse(
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

    error!("cannot perform click by index without a valid element cache. list elements first.");
    Err((
        StatusCode::PRECONDITION_FAILED,
        JsonResponse(
            json!({"error": "Element cache miss or invalid. Please list elements for the target application first."}),
        ),
    ))
}

async fn refresh_elements_and_attributes_with_cache(
    state: Arc<AppState>,
    app_name: String,
    wait_ms: u64,
) -> Option<ListElementsAndAttributesResponse> {
    tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;
    refresh_elements_and_attributes_after_action(state, app_name, 500).await
}

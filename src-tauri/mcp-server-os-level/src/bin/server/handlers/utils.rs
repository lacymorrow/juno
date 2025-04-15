use axum::extract::{Json, State};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

use crate::server::handlers::list_elements_and_attributes::list_elements_and_attributes_handler;
use crate::server::types::AppState;
use crate::server::types::ListElementsAndAttributesResponse;
use crate::server::types::ListInteractableElementsRequest;

pub async fn refresh_elements_and_attributes_after_action(
    state: Arc<AppState>,
    app_name: String,
    timeout_ms: u64,
) -> Option<ListElementsAndAttributesResponse> {
    debug!(
        "refreshing elements and attributes after action for app: {}",
        app_name
    );
    let start_time = Instant::now();
    let timeout_duration = Duration::from_millis(timeout_ms);

    // Loop until timeout or success
    while start_time.elapsed() < timeout_duration {
        // Create request for list_elements_and_attributes_handler
        let list_request = ListInteractableElementsRequest {
            app_name: app_name.clone(),
            max_elements: None,
            use_background_apps: Some(false),
            activate_app: Some(true),
        };

        // Call the handler
        match list_elements_and_attributes_handler(State(state.clone()), Json(list_request)).await {
            Ok(response) => {
                info!("successfully refreshed elements after action");
                return Some(response.0);
            }
            Err((status, err_json)) => {
                error!(
                    "error refreshing elements after action: status={}, error={:?}",
                    status, err_json
                );
                // Optionally, add a small delay before retrying
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    error!("timed out refreshing elements after action");
    None
}

use std::sync::Arc;
use axum::extract::{Json, State};
use tokio::time::Duration;
use tracing::{error, info};

use crate::types::*;
use crate::AppState;

use super::list_elements_and_attributes::list_elements_and_attributes_handler;


pub async fn refresh_elements_and_attributes_after_action(
    state: Arc<AppState>,
    app_name: String,
    delay_ms: u64,
) -> Option<ListElementsAndAttributesResponse> {
    // Add a small delay to allow UI to update
    info!("waiting for UI to update after action before listing elements and attributes");
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    
    // Create request for list elements and attributes
    let list_request = ListInteractableElementsRequest {
        app_name,
        max_elements: None,
        use_background_apps: Some(false),
        activate_app: Some(true),
    };
    
    // Call the handler to get fresh elements
    match list_elements_and_attributes_handler(State(state), Json(list_request)).await {
        Ok(response) => Some(response.0),
        Err(e) => {
            error!("failed to refresh elements and attributes after action: {:?}", e);
            None
        }
    }
}

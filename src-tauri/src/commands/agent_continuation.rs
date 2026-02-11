use crate::agent::core::AgentError;
use crate::constants::agent::config::CONTINUATION_REQUEST_TIMEOUT_SECONDS;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info, warn};
use crate::constants::events;

/// Request for agent continuation when max iterations reached
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationRequest {
    pub request_id: String,
    pub execution_id: String,
    pub current_step: u32,
    pub max_steps: u32,
    pub message: String,
    pub created_at: u64, // Unix timestamp in seconds
}

/// Response from user for continuation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationResponse {
    pub request_id: String,
    pub approved: bool,
    pub additional_steps: Option<u32>, // Optional: allow user to specify additional steps
}

/// Global continuation manager for handling iteration limit requests
pub struct ContinuationManager {
    /// Pending continuation requests
    pending_requests: Arc<Mutex<HashMap<String, ContinuationRequest>>>,
    /// Continuation responses (approved/denied)
    responses: Arc<Mutex<HashMap<String, ContinuationResponse>>>,
    /// Notify agents waiting for continuation decisions (using oneshot channels)
    continuation_notifiers:
        Arc<Mutex<HashMap<String, oneshot::Sender<Option<ContinuationResponse>>>>>,
}

#[allow(clippy::new_without_default)]
impl ContinuationManager {
    pub fn new() -> Self {
        Self {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            responses: Arc::new(Mutex::new(HashMap::new())),
            continuation_notifiers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Request continuation from user when max iterations reached.
    /// In headless mode, auto-denies continuation (returns None) since there is
    /// no GUI to present the dialog — prevents hanging on user input.
    pub async fn request_continuation(
        &self,
        execution_id: String,
        current_step: u32,
        max_steps: u32,
        app_handle: &AppHandle,
    ) -> Result<Option<ContinuationResponse>, AgentError> {
        // In headless mode, there is no user to approve continuation — auto-deny
        if crate::cli::headless::is_headless_mode() {
            info!(
                "Headless mode: auto-denying continuation for execution {} at step {}/{}",
                execution_id, current_step, max_steps
            );
            return Ok(None);
        }

        let request_id = uuid::Uuid::new_v4().to_string();
        let message = format!(
            "Agent has reached the maximum iteration limit ({} steps). Continue execution?",
            max_steps
        );

        let request = ContinuationRequest {
            request_id: request_id.clone(),
            execution_id: execution_id.clone(),
            current_step,
            max_steps,
            message: message.clone(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        // Create a oneshot channel for this specific request
        let (tx, rx) = oneshot::channel();

        // Store the request and the notifier
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), request.clone());
        }
        {
            let mut notifiers = self.continuation_notifiers.lock().await;
            notifiers.insert(request_id.clone(), tx);
        }

        // Emit event to frontend
        let event_data = serde_json::json!({
            "request_id": request_id,
            "execution_id": execution_id,
            "current_step": current_step,
            "max_steps": max_steps,
            "message": message
        });

        if let Err(e) = app_handle.emit(events::continuation::AGENT_REQUEST, event_data) {
            error!("Failed to emit agent-continuation-request event: {}", e);
            return Err(AgentError::Unknown(format!(
                "Failed to request continuation: {}",
                e
            )));
        }

        info!(
            "Requested continuation for execution {} at step {}/{}",
            execution_id, current_step, max_steps
        );

        // Wait for user response (with timeout)
        let timeout_duration = std::time::Duration::from_secs(CONTINUATION_REQUEST_TIMEOUT_SECONDS);
        match tokio::time::timeout(timeout_duration, rx).await {
            Ok(Ok(response)) => {
                // Clean up the request
                self.cleanup_request(&request_id).await;
                return Ok(response);
            }
            Ok(Err(_)) => {
                error!("Oneshot channel error while waiting for continuation");
            }
            Err(_) => {
                warn!(
                    "Timeout waiting for continuation response for execution {}",
                    execution_id
                );
                // Don't clean up immediately on timeout - allow a grace period for late responses
                // The request will be cleaned up when a response comes or when the manager is reset
            }
        }

        // For timeout case, return None but don't clean up yet (allow grace period)
        Ok(None)
    }

    /// Respond to a continuation request
    pub async fn respond_to_continuation(
        &self,
        request_id: String,
        approved: bool,
        additional_steps: Option<u32>,
    ) -> Result<(), String> {
        let response = ContinuationResponse {
            request_id: request_id.clone(),
            approved,
            additional_steps,
        };

        // Store the response
        {
            let mut responses = self.responses.lock().await;
            responses.insert(request_id.clone(), response.clone());
        }

        // Notify waiting agent
        {
            let mut notifiers = self.continuation_notifiers.lock().await;
            if let Some(tx) = notifiers.remove(&request_id) {
                if tx.send(Some(response.clone())).is_err() {
                    warn!("Failed to send continuation response: receiver dropped (agent may have timed out)");
                    // Don't return error - this is expected if agent timed out
                    // Clean up the orphaned request
                    self.cleanup_request(&request_id).await;
                }
            } else {
                warn!(
                    "No waiting agent found for continuation request: {} (may have timed out)",
                    request_id
                );
                // Clean up orphaned request
                self.cleanup_request(&request_id).await;
            }
        }

        info!(
            "Continuation response sent for request {}: approved={}, additional_steps={:?}",
            request_id, approved, additional_steps
        );
        Ok(())
    }

    /// Get pending continuation requests
    pub async fn get_pending_requests(&self) -> Vec<ContinuationRequest> {
        let pending = self.pending_requests.lock().await;
        pending.values().cloned().collect()
    }

    /// Clean up a completed request
    async fn cleanup_request(&self, request_id: &str) {
        {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(request_id);
        }
        {
            let mut responses = self.responses.lock().await;
            responses.remove(request_id);
        }
        {
            let mut notifiers = self.continuation_notifiers.lock().await;
            notifiers.remove(request_id);
        }
        debug!("Cleaned up continuation request {}", request_id);
    }
}

/// Global continuation manager instance
static CONTINUATION_MANAGER: std::sync::OnceLock<ContinuationManager> = std::sync::OnceLock::new();

/// Get or initialize the global continuation manager
pub fn get_continuation_manager() -> &'static ContinuationManager {
    CONTINUATION_MANAGER.get_or_init(ContinuationManager::new)
}

/// Request continuation when agent reaches max iterations
pub async fn request_agent_continuation(
    execution_id: String,
    current_step: u32,
    max_steps: u32,
    app_handle: &AppHandle,
) -> Result<Option<ContinuationResponse>, AgentError> {
    let manager = get_continuation_manager();
    manager
        .request_continuation(execution_id, current_step, max_steps, app_handle)
        .await
}

/// Tauri command: Respond to a continuation request
#[tauri::command]
pub async fn respond_to_agent_continuation(
    request_id: String,
    approved: bool,
    additional_steps: Option<u32>,
    app_handle: AppHandle,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    info!(
        "Received continuation response: request_id={}, approved={}, additional_steps={:?}",
        request_id, approved, additional_steps
    );

    let manager = get_continuation_manager();
    manager
        .respond_to_continuation(request_id.clone(), approved, additional_steps)
        .await?;

    // Emit event to notify other parts of the application
    let event_data = serde_json::json!({
        "request_id": request_id,
        "approved": approved,
        "additional_steps": additional_steps
    });

    if let Err(e) = app_handle.emit(events::continuation::AGENT_RESPONSE, event_data) {
        warn!("Failed to emit agent-continuation-response event: {}", e);
    }

    let message = if approved {
        "Agent continuation approved".to_string()
    } else {
        "Agent continuation denied".to_string()
    };

    Ok(message)
}

/// Tauri command: Get pending continuation requests
#[tauri::command]
pub async fn get_pending_continuation_requests(
    _state: State<'_, AppState>,
) -> Result<Vec<ContinuationRequest>, String> {
    let manager = get_continuation_manager();
    let requests = manager.get_pending_requests().await;

    info!("Retrieved {} pending continuation requests", requests.len());
    Ok(requests)
}

/// Tauri command: Check if there are any pending continuation requests
#[tauri::command]
pub async fn has_pending_continuation_requests(_state: State<'_, AppState>) -> Result<bool, String> {
    let manager = get_continuation_manager();
    let requests = manager.get_pending_requests().await;
    Ok(!requests.is_empty())
}

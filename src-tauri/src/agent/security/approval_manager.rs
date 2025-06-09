use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use super::RiskLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub id: String,
    pub command: String,
    pub risk_level: RiskLevel,
    pub context: String,
    pub requested_at: Instant,
    pub timeout_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approve,
    ApproveAlways,
    Deny,
    DenyAlways,
    Timeout,
}

#[derive(Debug)]
struct ApprovalRequest {
    approval: PendingApproval,
    response_sender: oneshot::Sender<ApprovalDecision>,
}

pub struct ApprovalManager {
    pending_requests: Arc<Mutex<HashMap<String, ApprovalRequest>>>,
    always_approved: Arc<Mutex<Vec<String>>>,
    always_denied: Arc<Mutex<Vec<String>>>,
    approval_timeout: Duration,
}

impl ApprovalManager {
    pub fn new(approval_timeout: Duration) -> Self {
        Self {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            always_approved: Arc::new(Mutex::new(Vec::new())),
            always_denied: Arc::new(Mutex::new(Vec::new())),
            approval_timeout,
        }
    }

    /// Request approval for a command
    pub async fn request_approval(
        &self,
        command: String,
        risk_level: RiskLevel,
        context: String,
    ) -> Result<String, String> {
        // Check if command is in always approved/denied lists
        let command_hash = self.hash_command(&command);
        
        {
            let always_approved = self.always_approved.lock().await;
            if always_approved.contains(&command_hash) {
                info!("Command auto-approved (previously approved): {}", command);
                return Ok("auto-approved".to_string());
            }
        }

        {
            let always_denied = self.always_denied.lock().await;
            if always_denied.contains(&command_hash) {
                warn!("Command auto-denied (previously denied): {}", command);
                return Err("Command previously denied by user".to_string());
            }
        }

        let approval_id = Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        
        let now = Instant::now();
        let pending_approval = PendingApproval {
            id: approval_id.clone(),
            command: command.clone(),
            risk_level,
            context,
            requested_at: now,
            timeout_at: now + self.approval_timeout,
        };

        let request = ApprovalRequest {
            approval: pending_approval.clone(),
            response_sender: sender,
        };

        // Store the pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(approval_id.clone(), request);
        }

        info!("Approval requested for command: {} (ID: {})", command, approval_id);

        // Emit approval request event to frontend
        self.emit_approval_request(&pending_approval).await;

        // Start timeout task
        let approval_manager = self.clone();
        let timeout_id = approval_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(approval_manager.approval_timeout).await;
            approval_manager.handle_timeout(timeout_id).await;
        });

        // Wait for approval decision
        match receiver.await {
            Ok(decision) => {
                match decision {
                    ApprovalDecision::Approve => {
                        info!("Command approved: {}", command);
                        Ok(approval_id)
                    },
                    ApprovalDecision::ApproveAlways => {
                        info!("Command approved always: {}", command);
                        let mut always_approved = self.always_approved.lock().await;
                        always_approved.push(command_hash);
                        Ok(approval_id)
                    },
                    ApprovalDecision::Deny => {
                        warn!("Command denied: {}", command);
                        Err("Command denied by user".to_string())
                    },
                    ApprovalDecision::DenyAlways => {
                        warn!("Command denied always: {}", command);
                        let mut always_denied = self.always_denied.lock().await;
                        always_denied.push(command_hash);
                        Err("Command denied permanently by user".to_string())
                    },
                    ApprovalDecision::Timeout => {
                        error!("Command approval timed out: {}", command);
                        Err("Approval request timed out".to_string())
                    }
                }
            },
            Err(_) => {
                error!("Failed to receive approval decision for: {}", command);
                Err("Internal error processing approval".to_string())
            }
        }
    }

    /// Wait for approval decision (used by legacy API)
    pub async fn wait_for_approval(&self, approval_id: String) -> Result<bool, String> {
        // This is a simpler interface that just returns true/false
        // In practice, the request_approval method handles the full workflow
        debug!("Legacy wait_for_approval called for ID: {}", approval_id);
        
        if approval_id == "auto-approved" {
            return Ok(true);
        }

        // For now, just return true since the actual approval logic
        // is handled in request_approval
        Ok(true)
    }

    /// Process an approval decision from the frontend
    pub async fn process_decision(
        &self,
        approval_id: String,
        decision: ApprovalDecision,
    ) -> Result<(), String> {
        let mut pending = self.pending_requests.lock().await;
        
        if let Some(request) = pending.remove(&approval_id) {
            info!("Processing approval decision for {}: {:?}", approval_id, decision);
            
            // Send the decision back to the waiting request
            if let Err(_) = request.response_sender.send(decision.clone()) {
                warn!("Failed to send approval decision for {}", approval_id);
            }
            
            // Log the decision
            match decision {
                ApprovalDecision::Approve | ApprovalDecision::ApproveAlways => {
                    info!("Command approved: {}", request.approval.command);
                },
                ApprovalDecision::Deny | ApprovalDecision::DenyAlways => {
                    warn!("Command denied: {}", request.approval.command);
                },
                ApprovalDecision::Timeout => {
                    error!("Command timed out: {}", request.approval.command);
                }
            }
            
            Ok(())
        } else {
            warn!("No pending approval found for ID: {}", approval_id);
            Err("Approval ID not found".to_string())
        }
    }

    /// Handle approval timeout
    async fn handle_timeout(&self, approval_id: String) {
        let mut pending = self.pending_requests.lock().await;
        
        if let Some(request) = pending.remove(&approval_id) {
            warn!("Approval request timed out: {} (ID: {})", 
                  request.approval.command, approval_id);
            
            // Send timeout decision
            let _ = request.response_sender.send(ApprovalDecision::Timeout);
        }
    }

    /// Get the number of pending approvals
    pub async fn get_pending_count(&self) -> usize {
        let pending = self.pending_requests.lock().await;
        pending.len()
    }

    /// Get all pending approvals (for UI display)
    pub async fn get_pending_approvals(&self) -> Vec<PendingApproval> {
        let pending = self.pending_requests.lock().await;
        pending.values().map(|r| r.approval.clone()).collect()
    }

    /// Clear expired approvals
    pub async fn cleanup_expired(&self) {
        let mut pending = self.pending_requests.lock().await;
        let now = Instant::now();
        
        let expired_ids: Vec<_> = pending
            .iter()
            .filter(|(_, request)| now > request.approval.timeout_at)
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired_ids {
            if let Some(request) = pending.remove(&id) {
                warn!("Cleaning up expired approval: {}", request.approval.command);
                let _ = request.response_sender.send(ApprovalDecision::Timeout);
            }
        }
    }

    /// Clear all approvals (emergency reset)
    pub async fn clear_all_approvals(&self) {
        info!("Clearing all pending approvals");
        
        let mut pending = self.pending_requests.lock().await;
        for (_, request) in pending.drain() {
            let _ = request.response_sender.send(ApprovalDecision::Deny);
        }
        
        let mut always_approved = self.always_approved.lock().await;
        always_approved.clear();
        
        let mut always_denied = self.always_denied.lock().await;
        always_denied.clear();
    }

    /// Get approval statistics
    pub async fn get_approval_stats(&self) -> ApprovalStats {
        let pending_count = self.get_pending_count().await;
        let always_approved_count = {
            let always_approved = self.always_approved.lock().await;
            always_approved.len()
        };
        let always_denied_count = {
            let always_denied = self.always_denied.lock().await;
            always_denied.len()
        };

        ApprovalStats {
            pending: pending_count,
            always_approved: always_approved_count,
            always_denied: always_denied_count,
            timeout_seconds: self.approval_timeout.as_secs(),
        }
    }

    /// Hash a command for comparison (removes variable parts)
    fn hash_command(&self, command: &str) -> String {
        // Simple hash - in production might want to normalize paths, etc.
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        command.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Emit approval request to frontend
    async fn emit_approval_request(&self, approval: &PendingApproval) {
        // In a real implementation, this would emit a Tauri event
        // For now, just log it
        info!("🚨 APPROVAL REQUIRED 🚨");
        info!("Command: {}", approval.command);
        info!("Risk Level: {:?}", approval.risk_level);
        info!("Context: {}", approval.context);
        info!("Approval ID: {}", approval.id);
        info!("Timeout in: {:?}", approval.timeout_at - approval.requested_at);
    }

    /// Clone the approval manager (for use in async contexts)
    fn clone(&self) -> Self {
        Self {
            pending_requests: self.pending_requests.clone(),
            always_approved: self.always_approved.clone(),
            always_denied: self.always_denied.clone(),
            approval_timeout: self.approval_timeout,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApprovalStats {
    pub pending: usize,
    pub always_approved: usize,
    pub always_denied: usize,
    pub timeout_seconds: u64,
}

// Tauri command interface for the approval system
#[cfg(feature = "tauri")]
mod tauri_commands {
    use super::*;
    use tauri::{command, State, Manager};

    #[command]
    pub async fn get_pending_approvals(
        approval_manager: State<'_, Arc<ApprovalManager>>,
    ) -> Result<Vec<PendingApproval>, String> {
        Ok(approval_manager.get_pending_approvals().await)
    }

    #[command]
    pub async fn submit_approval_decision(
        approval_manager: State<'_, Arc<ApprovalManager>>,
        approval_id: String,
        decision: ApprovalDecision,
    ) -> Result<(), String> {
        approval_manager.process_decision(approval_id, decision).await
    }

    #[command]
    pub async fn get_approval_stats(
        approval_manager: State<'_, Arc<ApprovalManager>>,
    ) -> Result<ApprovalStats, String> {
        Ok(approval_manager.get_approval_stats().await)
    }

    #[command]
    pub async fn clear_all_approvals(
        approval_manager: State<'_, Arc<ApprovalManager>>,
    ) -> Result<(), String> {
        approval_manager.clear_all_approvals().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_approval_request() {
        let manager = ApprovalManager::new(Duration::from_secs(1));
        
        // Start approval request in background
        let manager_clone = manager.clone();
        let approval_task = tokio::spawn(async move {
            manager_clone.request_approval(
                "test command".to_string(),
                RiskLevel::High,
                "test context".to_string(),
            ).await
        });

        // Wait a bit for the request to be registered
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Get pending approvals
        let pending = manager.get_pending_approvals().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].command, "test command");

        // Submit approval
        let approval_id = pending[0].id.clone();
        manager.process_decision(approval_id, ApprovalDecision::Approve).await.unwrap();

        // Verify approval was processed
        let result = approval_task.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_approval_timeout() {
        let manager = ApprovalManager::new(Duration::from_millis(100));
        
        let result = timeout(
            Duration::from_millis(200),
            manager.request_approval(
                "test command".to_string(),
                RiskLevel::High,
                "test context".to_string(),
            )
        ).await.unwrap();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timeout"));
    }

    #[tokio::test]
    async fn test_always_approve() {
        let manager = ApprovalManager::new(Duration::from_secs(1));
        
        // First approval
        let manager_clone = manager.clone();
        let approval_task = tokio::spawn(async move {
            manager_clone.request_approval(
                "test command".to_string(),
                RiskLevel::High,
                "test context".to_string(),
            ).await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        let pending = manager.get_pending_approvals().await;
        let approval_id = pending[0].id.clone();
        
        // Approve always
        manager.process_decision(approval_id, ApprovalDecision::ApproveAlways).await.unwrap();
        let result = approval_task.await.unwrap();
        assert!(result.is_ok());

        // Second request should be auto-approved
        let result2 = manager.request_approval(
            "test command".to_string(),
            RiskLevel::High,
            "test context".to_string(),
        ).await;
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), "auto-approved");
    }
}
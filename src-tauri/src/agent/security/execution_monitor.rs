use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandLogEntry {
    pub id: String,
    pub timestamp: SystemTime,
    pub tool_name: String,
    pub command: String,
    pub user_approved: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub execution_time: Duration,
    pub files_modified: Vec<String>,
    pub processes_spawned: Vec<u32>,
    pub network_activity: Vec<NetworkActivity>,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Starting,
    Running,
    Completed,
    Failed,
    Killed,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkActivity {
    pub connection_type: String,
    pub remote_address: String,
    pub port: u16,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug)]
struct ActiveMonitor {
    entry: CommandLogEntry,
    start_time: SystemTime,
}

pub struct ExecutionMonitor {
    active_monitors: Arc<Mutex<HashMap<String, ActiveMonitor>>>,
    completed_entries: Arc<Mutex<Vec<CommandLogEntry>>>,
    max_entries: usize,
}

impl ExecutionMonitor {
    pub fn new() -> Self {
        Self {
            active_monitors: Arc::new(Mutex::new(HashMap::new())),
            completed_entries: Arc::new(Mutex::new(Vec::new())),
            max_entries: 1000, // Keep last 1000 entries
        }
    }

    /// Start monitoring a command execution
    pub async fn start_monitoring(&self, command: &str, tool_name: &str) -> String {
        let monitor_id = Uuid::new_v4().to_string();
        let now = SystemTime::now();

        let entry = CommandLogEntry {
            id: monitor_id.clone(),
            timestamp: now,
            tool_name: tool_name.to_string(),
            command: command.to_string(),
            user_approved: false, // Will be updated if needed
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            execution_time: Duration::new(0, 0),
            files_modified: Vec::new(),
            processes_spawned: Vec::new(),
            network_activity: Vec::new(),
            status: ExecutionStatus::Starting,
        };

        let monitor = ActiveMonitor {
            entry,
            start_time: now,
        };

        {
            let mut active = self.active_monitors.lock().await;
            active.insert(monitor_id.clone(), monitor);
        }

        info!("Started monitoring command: {} (ID: {})", command, monitor_id);
        monitor_id
    }

    /// Update monitoring status
    pub async fn update_status(&self, monitor_id: &str, status: ExecutionStatus) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(monitor) = active.get_mut(monitor_id) {
            monitor.entry.status = status;
            debug!("Updated status for monitor {}: {:?}", monitor_id, monitor.entry.status);
            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }

    /// Record file modification
    pub async fn record_file_modification(&self, monitor_id: &str, file_path: String) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(monitor) = active.get_mut(monitor_id) {
            if !monitor.entry.files_modified.contains(&file_path) {
                monitor.entry.files_modified.push(file_path.clone());
                debug!("Recorded file modification for monitor {}: {}", monitor_id, file_path);
            }
            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }

    /// Record process spawn
    pub async fn record_process_spawn(&self, monitor_id: &str, process_id: u32) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(monitor) = active.get_mut(monitor_id) {
            monitor.entry.processes_spawned.push(process_id);
            debug!("Recorded process spawn for monitor {}: {}", monitor_id, process_id);
            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }

    /// Record network activity
    pub async fn record_network_activity(&self, monitor_id: &str, activity: NetworkActivity) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(monitor) = active.get_mut(monitor_id) {
            monitor.entry.network_activity.push(activity);
            debug!("Recorded network activity for monitor {}", monitor_id);
            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }

    /// Mark command as user approved
    pub async fn mark_user_approved(&self, monitor_id: &str) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(monitor) = active.get_mut(monitor_id) {
            monitor.entry.user_approved = true;
            debug!("Marked command as user approved: {}", monitor_id);
            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }

    /// Complete monitoring and log results
    pub async fn complete_monitoring(
        &self,
        monitor_id: &str,
        exit_code: Option<i32>,
        stdout: &str,
        stderr: &str,
        execution_time: Duration,
    ) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(mut monitor) = active.remove(monitor_id) {
            // Update the entry with final results
            monitor.entry.exit_code = exit_code;
            monitor.entry.stdout = stdout.to_string();
            monitor.entry.stderr = stderr.to_string();
            monitor.entry.execution_time = execution_time;
            monitor.entry.status = if exit_code.unwrap_or(0) == 0 {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            };

            // Log completion
            match monitor.entry.status {
                ExecutionStatus::Completed => {
                    info!(
                        "Command completed successfully: {} (ID: {}, time: {:?})",
                        monitor.entry.command, monitor_id, execution_time
                    );
                },
                ExecutionStatus::Failed => {
                    warn!(
                        "Command failed: {} (ID: {}, exit_code: {:?}, time: {:?})",
                        monitor.entry.command, monitor_id, exit_code, execution_time
                    );
                }
                _ => {}
            }

            // Log detailed information for high-risk commands
            if monitor.entry.user_approved || !monitor.entry.files_modified.is_empty() || !monitor.entry.processes_spawned.is_empty() {
                info!("Command execution details:");
                info!("  Command: {}", monitor.entry.command);
                info!("  User Approved: {}", monitor.entry.user_approved);
                info!("  Files Modified: {:?}", monitor.entry.files_modified);
                info!("  Processes Spawned: {:?}", monitor.entry.processes_spawned);
                info!("  Network Activity: {} connections", monitor.entry.network_activity.len());
            }

            // Store completed entry
            {
                let mut completed = self.completed_entries.lock().await;
                completed.push(monitor.entry);

                // Trim to max entries
                if completed.len() > self.max_entries {
                    let excess = completed.len() - self.max_entries;
                    completed.drain(0..excess);
                }
            }

            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }

    /// Get currently active monitors
    pub async fn get_active_monitors(&self) -> Vec<CommandLogEntry> {
        let active = self.active_monitors.lock().await;
        active.values().map(|m| m.entry.clone()).collect()
    }

    /// Get number of active monitors
    pub async fn get_active_count(&self) -> usize {
        let active = self.active_monitors.lock().await;
        active.len()
    }

    /// Get recent completed entries
    pub async fn get_recent_entries(&self, limit: usize) -> Vec<CommandLogEntry> {
        let completed = self.completed_entries.lock().await;
        let start = if completed.len() > limit {
            completed.len() - limit
        } else {
            0
        };
        completed[start..].to_vec()
    }

    /// Get entries by tool name
    pub async fn get_entries_by_tool(&self, tool_name: &str, limit: usize) -> Vec<CommandLogEntry> {
        let completed = self.completed_entries.lock().await;
        completed
            .iter()
            .rev()
            .filter(|entry| entry.tool_name == tool_name)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get entries by time range
    pub async fn get_entries_by_time_range(
        &self,
        start_time: SystemTime,
        end_time: SystemTime,
    ) -> Vec<CommandLogEntry> {
        let completed = self.completed_entries.lock().await;
        completed
            .iter()
            .filter(|entry| entry.timestamp >= start_time && entry.timestamp <= end_time)
            .cloned()
            .collect()
    }

    /// Get entries that modified files
    pub async fn get_entries_with_file_modifications(&self, limit: usize) -> Vec<CommandLogEntry> {
        let completed = self.completed_entries.lock().await;
        completed
            .iter()
            .rev()
            .filter(|entry| !entry.files_modified.is_empty())
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get entries with network activity
    pub async fn get_entries_with_network_activity(&self, limit: usize) -> Vec<CommandLogEntry> {
        let completed = self.completed_entries.lock().await;
        completed
            .iter()
            .rev()
            .filter(|entry| !entry.network_activity.is_empty())
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        let completed = self.completed_entries.lock().await;
        let active = self.active_monitors.lock().await;

        let total_commands = completed.len();
        let successful_commands = completed.iter().filter(|e| e.exit_code == Some(0)).count();
        let failed_commands = completed.iter().filter(|e| e.exit_code != Some(0) && e.exit_code.is_some()).count();
        let user_approved_commands = completed.iter().filter(|e| e.user_approved).count();
        let commands_with_file_changes = completed.iter().filter(|e| !e.files_modified.is_empty()).count();
        let commands_with_network_activity = completed.iter().filter(|e| !e.network_activity.is_empty()).count();

        let avg_execution_time = if !completed.is_empty() {
            let total_time: Duration = completed.iter().map(|e| e.execution_time).sum();
            total_time / completed.len() as u32
        } else {
            Duration::new(0, 0)
        };

        // Tool usage statistics
        let mut tool_usage = HashMap::new();
        for entry in completed.iter() {
            *tool_usage.entry(entry.tool_name.clone()).or_insert(0) += 1;
        }

        ExecutionStats {
            total_commands,
            successful_commands,
            failed_commands,
            active_commands: active.len(),
            user_approved_commands,
            commands_with_file_changes,
            commands_with_network_activity,
            avg_execution_time,
            tool_usage,
        }
    }

    /// Clear old entries (cleanup)
    pub async fn cleanup_old_entries(&self, older_than: SystemTime) {
        let mut completed = self.completed_entries.lock().await;
        completed.retain(|entry| entry.timestamp > older_than);
        info!("Cleaned up old command log entries");
    }

    /// Clear all entries
    pub async fn clear_all_entries(&self) {
        let mut completed = self.completed_entries.lock().await;
        let mut active = self.active_monitors.lock().await;
        
        completed.clear();
        active.clear();
        
        info!("Cleared all command log entries");
    }

    /// Kill an active command
    pub async fn kill_active_command(&self, monitor_id: &str) -> Result<(), String> {
        let mut active = self.active_monitors.lock().await;
        
        if let Some(monitor) = active.get_mut(monitor_id) {
            monitor.entry.status = ExecutionStatus::Killed;
            warn!("Marked command as killed: {} (ID: {})", monitor.entry.command, monitor_id);
            // Note: This doesn't actually kill the process - that would need OS-specific implementation
            Ok(())
        } else {
            Err(format!("Monitor not found: {}", monitor_id))
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ExecutionStats {
    pub total_commands: usize,
    pub successful_commands: usize,
    pub failed_commands: usize,
    pub active_commands: usize,
    pub user_approved_commands: usize,
    pub commands_with_file_changes: usize,
    pub commands_with_network_activity: usize,
    pub avg_execution_time: Duration,
    pub tool_usage: HashMap<String, usize>,
}

// Helper function to format duration for logging
impl CommandLogEntry {
    pub fn format_summary(&self) -> String {
        format!(
            "[{}] {} - {} (exit: {:?}, time: {:?})",
            self.tool_name,
            self.command,
            match self.status {
                ExecutionStatus::Completed => "✓",
                ExecutionStatus::Failed => "✗",
                ExecutionStatus::Running => "⏳",
                ExecutionStatus::Killed => "⚡",
                ExecutionStatus::TimedOut => "⏰",
                ExecutionStatus::Starting => "🚀",
            },
            self.exit_code,
            self.execution_time
        )
    }

    pub fn has_security_concerns(&self) -> bool {
        !self.files_modified.is_empty() 
            || !self.processes_spawned.is_empty() 
            || !self.network_activity.is_empty()
            || self.user_approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_basic_monitoring() {
        let monitor = ExecutionMonitor::new();
        
        let monitor_id = monitor.start_monitoring("test command", "test_tool").await;
        assert!(!monitor_id.is_empty());

        // Check active monitors
        let active = monitor.get_active_monitors().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].command, "test command");

        // Complete monitoring
        monitor.complete_monitoring(
            &monitor_id,
            Some(0),
            "test output",
            "",
            Duration::from_millis(100),
        ).await.unwrap();

        // Check completed entries
        let recent = monitor.get_recent_entries(10).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].command, "test command");
        assert_eq!(recent[0].exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_file_modification_tracking() {
        let monitor = ExecutionMonitor::new();
        
        let monitor_id = monitor.start_monitoring("test command", "test_tool").await;
        
        // Record file modifications
        monitor.record_file_modification(&monitor_id, "/path/to/file1.txt".to_string()).await.unwrap();
        monitor.record_file_modification(&monitor_id, "/path/to/file2.txt".to_string()).await.unwrap();

        // Complete monitoring
        monitor.complete_monitoring(&monitor_id, Some(0), "", "", Duration::from_millis(50)).await.unwrap();

        // Check entries with file modifications
        let entries = monitor.get_entries_with_file_modifications(10).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].files_modified.len(), 2);
        assert!(entries[0].files_modified.contains(&"/path/to/file1.txt".to_string()));
    }

    #[tokio::test]
    async fn test_execution_stats() {
        let monitor = ExecutionMonitor::new();
        
        // Add some test entries
        for i in 0..5 {
            let monitor_id = monitor.start_monitoring(&format!("command {}", i), "test_tool").await;
            monitor.complete_monitoring(&monitor_id, Some(0), "", "", Duration::from_millis(100)).await.unwrap();
        }

        // Add a failed command
        let monitor_id = monitor.start_monitoring("failed command", "test_tool").await;
        monitor.complete_monitoring(&monitor_id, Some(1), "", "error", Duration::from_millis(200)).await.unwrap();

        let stats = monitor.get_execution_stats().await;
        assert_eq!(stats.total_commands, 6);
        assert_eq!(stats.successful_commands, 5);
        assert_eq!(stats.failed_commands, 1);
        assert!(stats.tool_usage.contains_key("test_tool"));
        assert_eq!(stats.tool_usage["test_tool"], 6);
    }
}
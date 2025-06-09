use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug, error};
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, EventKind};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeEntry {
    pub timestamp: SystemTime,
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub size_change: i64,
    pub permissions_changed: bool,
    pub command_id: Option<String>, // Link to command that caused change
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Moved { from: PathBuf, to: PathBuf },
    PermissionsChanged,
    AttributesChanged,
}

pub struct FileMonitor {
    protected_paths: HashSet<PathBuf>,
    change_log: Arc<Mutex<Vec<FileChangeEntry>>>,
    active_watchers: Arc<Mutex<HashMap<PathBuf, RecommendedWatcher>>>,
    max_entries: usize,
    event_sender: mpsc::UnboundedSender<FileChangeEntry>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<FileChangeEntry>>>,
}

impl FileMonitor {
    pub fn new(protected_paths: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let protected_paths: HashSet<PathBuf> = protected_paths
            .iter()
            .map(|p| PathBuf::from(p))
            .collect();

        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        let monitor = Self {
            protected_paths,
            change_log: Arc::new(Mutex::new(Vec::new())),
            active_watchers: Arc::new(Mutex::new(HashMap::new())),
            max_entries: 10000,
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
        };

        // Start the event processing task
        monitor.start_event_processor();

        Ok(monitor)
    }

    /// Start monitoring a specific path
    pub async fn start_monitoring_path(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("Path does not exist: {}", path.display()));
        }

        let mut watchers = self.active_watchers.lock().await;
        if watchers.contains_key(path) {
            debug!("Path already being monitored: {}", path.display());
            return Ok(());
        }

        let sender = self.event_sender.clone();
        let path_buf = path.to_path_buf();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Some(change_entry) = Self::convert_event_to_change(event, &path_buf) {
                        if let Err(e) = sender.send(change_entry) {
                            error!("Failed to send file change event: {}", e);
                        }
                    }
                },
                Err(e) => error!("File watcher error: {}", e),
            }
        }).map_err(|e| format!("Failed to create file watcher: {}", e))?;

        watcher.watch(path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to start watching path: {}", e))?;

        watchers.insert(path.to_path_buf(), watcher);
        info!("Started monitoring path: {}", path.display());

        Ok(())
    }

    /// Stop monitoring a specific path
    pub async fn stop_monitoring_path(&self, path: &Path) -> Result<(), String> {
        let mut watchers = self.active_watchers.lock().await;
        if let Some(mut watcher) = watchers.remove(path) {
            watcher.unwatch(path)
                .map_err(|e| format!("Failed to stop watching path: {}", e))?;
            info!("Stopped monitoring path: {}", path.display());
        }
        Ok(())
    }

    /// Start monitoring all protected paths
    pub async fn start_monitoring_protected_paths(&self) -> Result<(), String> {
        for path in &self.protected_paths {
            if path.exists() {
                if let Err(e) = self.start_monitoring_path(path).await {
                    warn!("Failed to monitor protected path {}: {}", path.display(), e);
                }
            } else {
                debug!("Protected path does not exist: {}", path.display());
            }
        }
        Ok(())
    }

    /// Record a file change manually (for commands that we know modify files)
    pub async fn record_file_change(
        &self,
        path: PathBuf,
        change_type: FileChangeType,
        command_id: Option<String>,
    ) {
        let change_entry = FileChangeEntry {
            timestamp: SystemTime::now(),
            path: path.clone(),
            change_type,
            before_hash: None, // Would need to be calculated separately
            after_hash: None,  // Would need to be calculated separately
            size_change: 0,    // Would need to be calculated
            permissions_changed: false, // Would need to be detected
            command_id,
        };

        self.add_change_entry(change_entry).await;
        debug!("Manually recorded file change: {}", path.display());
    }

    /// Get file changes since a specific time
    pub async fn get_changes_since(&self, since: SystemTime) -> Vec<FileChangeEntry> {
        let change_log = self.change_log.lock().await;
        change_log
            .iter()
            .filter(|entry| entry.timestamp >= since)
            .cloned()
            .collect()
    }

    /// Get recent file changes
    pub async fn get_recent_changes(&self, limit: usize) -> Vec<FileChangeEntry> {
        let change_log = self.change_log.lock().await;
        let start = if change_log.len() > limit {
            change_log.len() - limit
        } else {
            0
        };
        change_log[start..].to_vec()
    }

    /// Get changes by command ID
    pub async fn get_changes_by_command(&self, command_id: &str) -> Vec<FileChangeEntry> {
        let change_log = self.change_log.lock().await;
        change_log
            .iter()
            .filter(|entry| entry.command_id.as_ref() == Some(command_id))
            .cloned()
            .collect()
    }

    /// Get changes in protected paths
    pub async fn get_protected_path_changes(&self, limit: usize) -> Vec<FileChangeEntry> {
        let change_log = self.change_log.lock().await;
        change_log
            .iter()
            .rev()
            .filter(|entry| self.is_protected_path(&entry.path))
            .take(limit)
            .cloned()
            .collect()
    }

    /// Check if a path is protected
    pub fn is_protected_path(&self, path: &Path) -> bool {
        self.protected_paths.iter().any(|protected| {
            path.starts_with(protected)
        })
    }

    /// Get file monitoring statistics
    pub async fn get_monitoring_stats(&self) -> FileMonitoringStats {
        let change_log = self.change_log.lock().await;
        let watchers = self.active_watchers.lock().await;

        let total_changes = change_log.len();
        let protected_changes = change_log
            .iter()
            .filter(|entry| self.is_protected_path(&entry.path))
            .count();

        let changes_today = change_log
            .iter()
            .filter(|entry| {
                SystemTime::now()
                    .duration_since(entry.timestamp)
                    .unwrap_or(Duration::from_secs(0))
                    < Duration::from_secs(86400)
            })
            .count();

        // Count by change type
        let mut change_types = HashMap::new();
        for entry in change_log.iter() {
            let type_name = match &entry.change_type {
                FileChangeType::Created => "created",
                FileChangeType::Modified => "modified",
                FileChangeType::Deleted => "deleted",
                FileChangeType::Moved { .. } => "moved",
                FileChangeType::PermissionsChanged => "permissions",
                FileChangeType::AttributesChanged => "attributes",
            };
            *change_types.entry(type_name.to_string()).or_insert(0) += 1;
        }

        FileMonitoringStats {
            total_changes,
            protected_changes,
            changes_today,
            active_watchers: watchers.len(),
            protected_paths_count: self.protected_paths.len(),
            change_types,
        }
    }

    /// Clear old change entries
    pub async fn cleanup_old_entries(&self, older_than: SystemTime) {
        let mut change_log = self.change_log.lock().await;
        change_log.retain(|entry| entry.timestamp > older_than);
        info!("Cleaned up old file change entries");
    }

    /// Clear all change entries
    pub async fn clear_all_entries(&self) {
        let mut change_log = self.change_log.lock().await;
        change_log.clear();
        info!("Cleared all file change entries");
    }

    /// Add protected path
    pub async fn add_protected_path(&mut self, path: PathBuf) -> Result<(), String> {
        self.protected_paths.insert(path.clone());
        if path.exists() {
            self.start_monitoring_path(&path).await?;
        }
        info!("Added protected path: {}", path.display());
        Ok(())
    }

    /// Remove protected path
    pub async fn remove_protected_path(&self, path: &Path) -> Result<(), String> {
        self.stop_monitoring_path(path).await?;
        info!("Removed protected path: {}", path.display());
        Ok(())
    }

    /// Convert notify event to file change entry
    fn convert_event_to_change(event: Event, base_path: &Path) -> Option<FileChangeEntry> {
        let paths = event.paths;
        if paths.is_empty() {
            return None;
        }

        let path = paths[0].clone();
        let change_type = match event.kind {
            EventKind::Create(_) => FileChangeType::Created,
            EventKind::Modify(_) => FileChangeType::Modified,
            EventKind::Remove(_) => FileChangeType::Deleted,
            EventKind::Access(_) => return None, // Skip access events
            EventKind::Other => FileChangeType::AttributesChanged,
            _ => return None,
        };

        Some(FileChangeEntry {
            timestamp: SystemTime::now(),
            path,
            change_type,
            before_hash: None,
            after_hash: None,
            size_change: 0,
            permissions_changed: false,
            command_id: None,
        })
    }

    /// Start the event processor task
    fn start_event_processor(&self) {
        let change_log = self.change_log.clone();
        let max_entries = self.max_entries;
        let receiver = self.event_receiver.clone();

        tokio::spawn(async move {
            let mut receiver = receiver.lock().await;
            while let Some(change_entry) = receiver.recv().await {
                let mut log = change_log.lock().await;
                log.push(change_entry);

                // Trim to max entries
                if log.len() > max_entries {
                    let excess = log.len() - max_entries;
                    log.drain(0..excess);
                }
            }
        });
    }

    /// Add a change entry to the log
    async fn add_change_entry(&self, entry: FileChangeEntry) {
        if let Err(_) = self.event_sender.send(entry) {
            error!("Failed to send file change entry to processor");
        }
    }

    /// Get file hash (for detecting content changes)
    async fn get_file_hash(&self, path: &Path) -> Option<String> {
        if let Ok(contents) = tokio::fs::read(path).await {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            
            let mut hasher = DefaultHasher::new();
            contents.hash(&mut hasher);
            Some(format!("{:x}", hasher.finish()))
        } else {
            None
        }
    }

    /// Generate a detailed file change report
    pub async fn generate_change_report(&self, since: SystemTime) -> FileChangeReport {
        let changes = self.get_changes_since(since).await;
        
        let mut files_created = Vec::new();
        let mut files_modified = Vec::new();
        let mut files_deleted = Vec::new();
        let mut files_moved = Vec::new();
        let mut protected_files_affected = Vec::new();

        for change in &changes {
            match &change.change_type {
                FileChangeType::Created => files_created.push(change.path.clone()),
                FileChangeType::Modified => files_modified.push(change.path.clone()),
                FileChangeType::Deleted => files_deleted.push(change.path.clone()),
                FileChangeType::Moved { from, to } => {
                    files_moved.push((from.clone(), to.clone()));
                },
                _ => {}
            }

            if self.is_protected_path(&change.path) {
                protected_files_affected.push(change.clone());
            }
        }

        FileChangeReport {
            time_range_start: since,
            time_range_end: SystemTime::now(),
            total_changes: changes.len(),
            files_created,
            files_modified,
            files_deleted,
            files_moved,
            protected_files_affected,
            changes_by_command: self.group_changes_by_command(&changes),
        }
    }

    /// Group changes by command ID
    fn group_changes_by_command(&self, changes: &[FileChangeEntry]) -> HashMap<String, Vec<FileChangeEntry>> {
        let mut grouped = HashMap::new();
        for change in changes {
            if let Some(command_id) = &change.command_id {
                grouped.entry(command_id.clone())
                    .or_insert_with(Vec::new)
                    .push(change.clone());
            }
        }
        grouped
    }
}

#[derive(Debug, Serialize)]
pub struct FileMonitoringStats {
    pub total_changes: usize,
    pub protected_changes: usize,
    pub changes_today: usize,
    pub active_watchers: usize,
    pub protected_paths_count: usize,
    pub change_types: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct FileChangeReport {
    pub time_range_start: SystemTime,
    pub time_range_end: SystemTime,
    pub total_changes: usize,
    pub files_created: Vec<PathBuf>,
    pub files_modified: Vec<PathBuf>,
    pub files_deleted: Vec<PathBuf>,
    pub files_moved: Vec<(PathBuf, PathBuf)>,
    pub protected_files_affected: Vec<FileChangeEntry>,
    pub changes_by_command: HashMap<String, Vec<FileChangeEntry>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_monitor_creation() {
        let temp_dir = TempDir::new().unwrap();
        let protected_paths = vec![temp_dir.path().to_string_lossy().to_string()];
        
        let monitor = FileMonitor::new(&protected_paths).unwrap();
        assert!(monitor.is_protected_path(temp_dir.path()));
    }

    #[tokio::test]
    async fn test_manual_file_change_recording() {
        let temp_dir = TempDir::new().unwrap();
        let protected_paths = vec![temp_dir.path().to_string_lossy().to_string()];
        let monitor = FileMonitor::new(&protected_paths).unwrap();

        let test_file = temp_dir.path().join("test.txt");
        monitor.record_file_change(
            test_file.clone(),
            FileChangeType::Created,
            Some("test-command-123".to_string()),
        ).await;

        // Give the event processor time to handle the event
        tokio::time::sleep(Duration::from_millis(10)).await;

        let changes = monitor.get_recent_changes(10).await;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, test_file);
        assert_eq!(changes[0].command_id, Some("test-command-123".to_string()));
    }

    #[tokio::test]
    async fn test_protected_path_detection() {
        let temp_dir = TempDir::new().unwrap();
        let protected_paths = vec![temp_dir.path().to_string_lossy().to_string()];
        let monitor = FileMonitor::new(&protected_paths).unwrap();

        let protected_file = temp_dir.path().join("protected.txt");
        let unprotected_file = PathBuf::from("/tmp/unprotected.txt");

        assert!(monitor.is_protected_path(&protected_file));
        assert!(!monitor.is_protected_path(&unprotected_file));
    }

    #[tokio::test]
    async fn test_file_monitoring_stats() {
        let temp_dir = TempDir::new().unwrap();
        let protected_paths = vec![temp_dir.path().to_string_lossy().to_string()];
        let monitor = FileMonitor::new(&protected_paths).unwrap();

        // Record some changes
        let test_file = temp_dir.path().join("test.txt");
        monitor.record_file_change(test_file.clone(), FileChangeType::Created, None).await;
        monitor.record_file_change(test_file.clone(), FileChangeType::Modified, None).await;

        // Give the event processor time
        tokio::time::sleep(Duration::from_millis(10)).await;

        let stats = monitor.get_monitoring_stats().await;
        assert_eq!(stats.total_changes, 2);
        assert_eq!(stats.protected_changes, 2);
        assert!(stats.change_types.contains_key("created"));
        assert!(stats.change_types.contains_key("modified"));
    }
}
// Workspace Management - Reproducible automation environments
// Provides CUA-like environment consistency without VMs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub root_path: PathBuf,
    pub state: WorkspaceState,
    pub snapshots: Vec<WorkspaceSnapshot>,
    pub environment: EnvironmentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub filesystem_state: FilesystemState,
    pub application_states: HashMap<String, ApplicationState>,
    pub browser_profiles: HashMap<String, BrowserProfile>,
    pub system_settings: SystemSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemState {
    pub tracked_files: HashMap<PathBuf, FileMetadata>,
    pub virtual_filesystem: HashMap<PathBuf, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub hash: String,
    pub permissions: u32,
    pub modified: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationState {
    pub name: String,
    pub version: String,
    pub config_files: HashMap<PathBuf, Vec<u8>>,
    pub preferences: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub browser_type: String,
    pub profile_path: PathBuf,
    pub cookies: Vec<Cookie>,
    pub local_storage: HashMap<String, String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub domain: String,
    pub name: String,
    pub value: String,
    pub path: String,
    pub expires: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    pub display_resolution: (u32, u32),
    pub timezone: String,
    pub locale: String,
    pub environment_variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    pub id: String,
    pub timestamp: std::time::SystemTime,
    pub description: String,
    pub state: WorkspaceState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub base_image: String, // Reference configuration, not a VM image
    pub installed_tools: Vec<String>,
    pub python_packages: Vec<String>,
    pub node_modules: Vec<String>,
    pub system_packages: Vec<String>,
}

impl Workspace {
    pub async fn new(id: &str) -> Result<Self> {
        let root_path = Self::get_workspace_root(id)?;
        fs::create_dir_all(&root_path).await?;
        
        let state = WorkspaceState::default();
        let environment = EnvironmentConfig::default();
        
        Ok(Self {
            id: id.to_string(),
            root_path,
            state,
            snapshots: Vec::new(),
            environment,
        })
    }
    
    pub async fn from_template(template: &WorkspaceTemplate) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut workspace = Self::new(&id).await?;
        
        // Apply template
        workspace.environment = template.environment.clone();
        workspace.state.system_settings = template.system_settings.clone();
        
        // Set up virtual filesystem
        for (path, content) in &template.initial_files {
            workspace.write_file(path, content).await?;
        }
        
        Ok(workspace)
    }
    
    pub async fn snapshot(&mut self, description: &str) -> Result<String> {
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        let snapshot = WorkspaceSnapshot {
            id: snapshot_id.clone(),
            timestamp: std::time::SystemTime::now(),
            description: description.to_string(),
            state: self.state.clone(),
        };
        
        // Save snapshot to disk
        let snapshot_path = self.root_path.join("snapshots").join(&snapshot_id);
        fs::create_dir_all(&snapshot_path).await?;
        
        let snapshot_data = serde_json::to_vec(&snapshot)?;
        fs::write(snapshot_path.join("snapshot.json"), snapshot_data).await?;
        
        self.snapshots.push(snapshot);
        Ok(snapshot_id)
    }
    
    pub async fn restore(&mut self, snapshot_id: &str) -> Result<()> {
        let snapshot = self.snapshots.iter()
            .find(|s| s.id == snapshot_id)
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found"))?;
        
        self.state = snapshot.state.clone();
        
        // Restore filesystem state
        for (path, metadata) in &self.state.filesystem_state.tracked_files {
            // Restore file if it exists in virtual filesystem
            if let Some(content) = self.state.filesystem_state.virtual_filesystem.get(path) {
                self.write_file(path, content).await?;
            }
        }
        
        Ok(())
    }
    
    pub async fn write_file(&mut self, path: &Path, content: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path)?;
        
        // Write to actual filesystem if not in strict mode
        fs::write(&full_path, content).await?;
        
        // Track in virtual filesystem
        self.state.filesystem_state.virtual_filesystem.insert(
            path.to_path_buf(),
            content.to_vec(),
        );
        
        // Update metadata
        let metadata = FileMetadata {
            hash: Self::hash_content(content),
            permissions: 0o644,
            modified: std::time::SystemTime::now(),
        };
        self.state.filesystem_state.tracked_files.insert(
            path.to_path_buf(),
            metadata,
        );
        
        Ok(())
    }
    
    pub async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        // First check virtual filesystem
        if let Some(content) = self.state.filesystem_state.virtual_filesystem.get(path) {
            return Ok(content.clone());
        }
        
        // Fall back to actual filesystem
        let full_path = self.resolve_path(path)?;
        Ok(fs::read(full_path).await?)
    }
    
    fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(self.root_path.join(path))
        }
    }
    
    fn get_workspace_root(id: &str) -> Result<PathBuf> {
        let base_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine local data directory"))?;
        Ok(base_dir.join("juno").join("workspaces").join(id))
    }
    
    fn hash_content(content: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }
    
    pub async fn export(&self) -> Result<WorkspaceExport> {
        Ok(WorkspaceExport {
            id: self.id.clone(),
            state: self.state.clone(),
            environment: self.environment.clone(),
            created_at: std::time::SystemTime::now(),
        })
    }
    
    pub async fn import(export: &WorkspaceExport) -> Result<Self> {
        let mut workspace = Self::new(&export.id).await?;
        workspace.state = export.state.clone();
        workspace.environment = export.environment.clone();
        
        // Restore all files
        for (path, content) in &workspace.state.filesystem_state.virtual_filesystem {
            workspace.write_file(path, content).await?;
        }
        
        Ok(workspace)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTemplate {
    pub name: String,
    pub description: String,
    pub environment: EnvironmentConfig,
    pub system_settings: SystemSettings,
    pub initial_files: HashMap<PathBuf, Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceExport {
    pub id: String,
    pub state: WorkspaceState,
    pub environment: EnvironmentConfig,
    pub created_at: std::time::SystemTime,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            filesystem_state: FilesystemState {
                tracked_files: HashMap::new(),
                virtual_filesystem: HashMap::new(),
            },
            application_states: HashMap::new(),
            browser_profiles: HashMap::new(),
            system_settings: SystemSettings {
                display_resolution: (1920, 1080),
                timezone: "UTC".to_string(),
                locale: "en_US".to_string(),
                environment_variables: HashMap::new(),
            },
        }
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            base_image: "juno-base".to_string(),
            installed_tools: vec![],
            python_packages: vec![],
            node_modules: vec![],
            system_packages: vec![],
        }
    }
}

// Workspace Manager for handling multiple workspaces
pub struct WorkspaceManager {
    workspaces: HashMap<String, Workspace>,
    active_workspace: Option<String>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            active_workspace: None,
        }
    }
    
    pub async fn create_workspace(&mut self, template: Option<WorkspaceTemplate>) -> Result<String> {
        let workspace = if let Some(tmpl) = template {
            Workspace::from_template(&tmpl).await?
        } else {
            Workspace::new(&uuid::Uuid::new_v4().to_string()).await?
        };
        
        let id = workspace.id.clone();
        self.workspaces.insert(id.clone(), workspace);
        Ok(id)
    }
    
    pub fn switch_workspace(&mut self, id: &str) -> Result<()> {
        if self.workspaces.contains_key(id) {
            self.active_workspace = Some(id.to_string());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Workspace {} not found", id))
        }
    }
    
    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.active_workspace.as_ref()
            .and_then(|id| self.workspaces.get(id))
    }
    
    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.active_workspace.as_ref()
            .and_then(|id| self.workspaces.get_mut(id))
    }
}
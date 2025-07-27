/// Resource management utilities for preventing resource leaks
/// Provides RAII patterns and lifecycle management for expensive resources

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{info, warn, error, debug};

/// Resource wrapper that ensures cleanup on drop
pub struct ManagedResource<T> {
    resource: Option<T>,
    cleanup: Option<Box<dyn FnOnce(T) + Send + 'static>>,
    name: String,
    created_at: Instant,
}

impl<T> ManagedResource<T> {
    /// Create a new managed resource with automatic cleanup
    pub fn new(resource: T, name: String, cleanup: impl FnOnce(T) + Send + 'static) -> Self {
        Self {
            resource: Some(resource),
            cleanup: Some(Box::new(cleanup)),
            name,
            created_at: Instant::now(),
        }
    }

    /// Take the resource, disabling automatic cleanup
    pub fn take(mut self) -> T {
        self.cleanup = None;
        self.resource.take().expect("Resource already taken")
    }

    /// Get a reference to the resource
    pub fn get(&self) -> Option<&T> {
        self.resource.as_ref()
    }

    /// Get a mutable reference to the resource
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.resource.as_mut()
    }

    /// Get the age of the resource
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

impl<T> Drop for ManagedResource<T> {
    fn drop(&mut self) {
        if let (Some(resource), Some(cleanup)) = (self.resource.take(), self.cleanup.take()) {
            debug!("Cleaning up resource: {}", self.name);
            cleanup(resource);
        }
    }
}

/// Pool for managing reusable resources with automatic cleanup
pub struct ResourcePool<T: Send + 'static> {
    resources: Arc<Mutex<Vec<ManagedResource<T>>>>,
    max_size: usize,
    max_age: Duration,
    name: String,
}

impl<T: Send + 'static> ResourcePool<T> {
    pub fn new(name: String, max_size: usize, max_age: Duration) -> Self {
        Self {
            resources: Arc::new(Mutex::new(Vec::new())),
            max_size,
            max_age,
            name,
        }
    }

    /// Add a resource to the pool
    pub async fn add(
        &self,
        resource: T,
        cleanup: impl FnOnce(T) + Send + 'static,
    ) -> Result<(), T> {
        let mut pool = self.resources.lock().await;
        
        // Clean up old resources
        pool.retain(|r| r.age() < self.max_age);
        
        if pool.len() >= self.max_size {
            warn!("Resource pool {} is full, rejecting resource", self.name);
            return Err(resource);
        }

        let managed = ManagedResource::new(
            resource,
            format!("{}_resource_{}", self.name, pool.len()),
            cleanup,
        );
        
        pool.push(managed);
        info!("Added resource to pool {}, size: {}", self.name, pool.len());
        Ok(())
    }

    /// Get a resource from the pool
    pub async fn get(&self) -> Option<T> {
        let mut pool = self.resources.lock().await;
        
        // Clean up old resources first
        pool.retain(|r| r.age() < self.max_age);
        
        if let Some(managed) = pool.pop() {
            info!("Retrieved resource from pool {}, remaining: {}", self.name, pool.len());
            Some(managed.take())
        } else {
            debug!("No resources available in pool {}", self.name);
            None
        }
    }

    /// Clear all resources in the pool
    pub async fn clear(&self) {
        let mut pool = self.resources.lock().await;
        let count = pool.len();
        pool.clear(); // Drop will clean up each resource
        info!("Cleared {} resources from pool {}", count, self.name);
    }

    /// Get the current size of the pool
    pub async fn size(&self) -> usize {
        let pool = self.resources.lock().await;
        pool.len()
    }
}

/// Manager for browser controller lifecycle
pub struct BrowserControllerManager {
    pool: ResourcePool<Arc<crate::agent::tools::browser_controller::BrowserController>>,
    active_controllers: Arc<RwLock<HashMap<String, Arc<crate::agent::tools::browser_controller::BrowserController>>>>,
}

impl BrowserControllerManager {
    pub fn new() -> Self {
        Self {
            pool: ResourcePool::new(
                "browser_controllers".to_string(),
                3, // Keep max 3 idle browsers
                Duration::from_secs(300), // 5 minute max idle time
            ),
            active_controllers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a browser controller
    pub async fn get_or_create(
        &self,
        id: String,
        playwright: Arc<playwright::Playwright>,
    ) -> Result<Arc<crate::agent::tools::browser_controller::BrowserController>, String> {
        // Check if we have an active controller
        {
            let active = self.active_controllers.read().await;
            if let Some(controller) = active.get(&id) {
                debug!("Reusing active browser controller: {}", id);
                return Ok(controller.clone());
            }
        }

        // Try to get from pool
        if let Some(controller) = self.pool.get().await {
            let mut active = self.active_controllers.write().await;
            active.insert(id.clone(), controller.clone());
            info!("Retrieved browser controller from pool: {}", id);
            return Ok(controller);
        }

        // Create new controller
        info!("Creating new browser controller: {}", id);
        let controller = crate::agent::tools::browser_controller::BrowserController::new(playwright)
            .await
            .map_err(|e| format!("Failed to create browser controller: {}", e))?;
        
        let controller = Arc::new(controller);
        let mut active = self.active_controllers.write().await;
        active.insert(id, controller.clone());
        
        Ok(controller)
    }

    /// Release a browser controller back to the pool
    pub async fn release(&self, id: String) -> Result<(), String> {
        let controller = {
            let mut active = self.active_controllers.write().await;
            active.remove(&id)
        };

        if let Some(controller) = controller {
            let _controller_clone = controller.clone();
            
            // Try to add to pool for reuse
            let result = self.pool.add(
                controller,
                move |ctrl| {
                    // Cleanup function that runs when controller is dropped from pool
                    let ctrl_clone = ctrl.clone();
                    tokio::spawn(async move {
                        if let Err(e) = ctrl_clone.cleanup().await {
                            error!("Failed to cleanup browser controller: {}", e);
                        }
                    });
                },
            ).await;

            match result {
                Ok(()) => {
                    info!("Released browser controller to pool: {}", id);
                    Ok(())
                }
                Err(controller) => {
                    // Pool was full, cleanup immediately
                    warn!("Pool full, cleaning up browser controller immediately: {}", id);
                    if let Err(e) = controller.cleanup().await {
                        error!("Failed to cleanup browser controller: {}", e);
                    }
                    Ok(())
                }
            }
        } else {
            debug!("Browser controller not found: {}", id);
            Ok(())
        }
    }

    /// Cleanup all browser controllers
    pub async fn cleanup_all(&self) {
        info!("Cleaning up all browser controllers");
        
        // Clear active controllers
        let active_controllers: Vec<_> = {
            let mut active = self.active_controllers.write().await;
            active.drain().map(|(_, v)| v).collect()
        };

        for controller in active_controllers {
            if let Err(e) = controller.cleanup().await {
                error!("Failed to cleanup active browser controller: {}", e);
            }
        }

        // Clear pool (will trigger cleanup for each)
        self.pool.clear().await;
    }
}

/// macOS autorelease pool wrapper for proper memory management
#[cfg(target_os = "macos")]
pub struct AutoreleasePool {
    pool: *mut objc::runtime::Object,
}

#[cfg(target_os = "macos")]
impl AutoreleasePool {
    pub fn new() -> Self {
        use objc::{class, msg_send, sel, sel_impl};
        
        unsafe {
            let pool: *mut objc::runtime::Object = msg_send![class!(NSAutoreleasePool), new];
            Self { pool }
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for AutoreleasePool {
    fn drop(&mut self) {
        use objc::{msg_send, sel, sel_impl};
        
        unsafe {
            let _: () = msg_send![self.pool, drain];
        }
    }
}

/// Run a closure within an autorelease pool (macOS only)
#[cfg(target_os = "macos")]
pub fn with_autorelease_pool<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _pool = AutoreleasePool::new();
    f()
}

#[cfg(not(target_os = "macos"))]
pub fn with_autorelease_pool<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

/// Global resource manager instance
static RESOURCE_MANAGER: std::sync::OnceLock<ResourceManager> = std::sync::OnceLock::new();

/// Central resource manager for the application
pub struct ResourceManager {
    browser_manager: BrowserControllerManager,
    temp_files: Arc<Mutex<Vec<std::path::PathBuf>>>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            browser_manager: BrowserControllerManager::new(),
            temp_files: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get the global resource manager instance
    pub fn global() -> &'static ResourceManager {
        RESOURCE_MANAGER.get_or_init(|| ResourceManager::new())
    }

    /// Get the browser controller manager
    pub fn browser_manager(&self) -> &BrowserControllerManager {
        &self.browser_manager
    }

    /// Register a temporary file for cleanup
    pub async fn register_temp_file(&self, path: std::path::PathBuf) {
        let mut files = self.temp_files.lock().await;
        files.push(path);
    }

    /// Clean up all temporary files
    pub async fn cleanup_temp_files(&self) {
        let mut files = self.temp_files.lock().await;
        for path in files.drain(..) {
            if let Err(e) = std::fs::remove_file(&path) {
                if path.exists() {
                    warn!("Failed to remove temporary file {:?}: {}", path, e);
                }
            } else {
                debug!("Removed temporary file: {:?}", path);
            }
        }
    }

    /// Perform full cleanup of all managed resources
    pub async fn cleanup_all(&self) {
        info!("Performing full resource cleanup");
        
        // Cleanup browsers
        self.browser_manager.cleanup_all().await;
        
        // Cleanup temp files
        self.cleanup_temp_files().await;
        
        info!("Resource cleanup completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_managed_resource_cleanup() {
        let cleanup_called = Arc::new(Mutex::new(false));
        let cleanup_called_clone = cleanup_called.clone();
        
        {
            let _resource = ManagedResource::new(
                42,
                "test_resource".to_string(),
                move |_val| {
                    let cleanup_called = cleanup_called_clone.clone();
                    tokio::spawn(async move {
                        let mut called = cleanup_called.lock().await;
                        *called = true;
                    });
                },
            );
        } // Resource dropped here
        
        // Give async cleanup time to run
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let called = cleanup_called.lock().await;
        assert!(*called, "Cleanup should have been called");
    }

    #[tokio::test]
    async fn test_resource_pool() {
        let pool = ResourcePool::new("test_pool".to_string(), 2, Duration::from_secs(60));
        
        // Add resources
        assert!(pool.add(1, |_| {}).await.is_ok());
        assert!(pool.add(2, |_| {}).await.is_ok());
        assert!(pool.add(3, |_| {}).await.is_err()); // Pool full
        
        assert_eq!(pool.size().await, 2);
        
        // Get resources
        assert_eq!(pool.get().await, Some(2));
        assert_eq!(pool.get().await, Some(1));
        assert_eq!(pool.get().await, None); // Pool empty
    }
}
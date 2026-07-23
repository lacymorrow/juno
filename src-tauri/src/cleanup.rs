//! Application cleanup module
//! Handles resource cleanup on application exit

use crate::utils::resource_manager::ResourceManager;
use crate::state::AppState;
use tracing::{info, error};
use tauri::{Listener, Manager};

/// Initialize cleanup handlers for the application
pub fn init_cleanup_handlers(app_handle: tauri::AppHandle) {
    // Register cleanup on app exit
    let app_handle_clone = app_handle.clone();
    
    // Handle window close event
    app_handle.listen("tauri://destroyed", move |_| {
        info!("Application window destroyed, initiating cleanup...");
        let handle = app_handle_clone.clone();
        
        // Spawn cleanup in a blocking task since we're exiting anyway
        std::thread::spawn(move || {
            tauri::async_runtime::block_on(async {
                cleanup_application(&handle).await;
            });
        });
    });
    
    // Setup ctrl+c handler for CLI mode
    let app_handle_ctrl_c = app_handle.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        info!("Received interrupt signal, cleaning up...");
        let handle = app_handle_ctrl_c.clone();

        std::thread::spawn(move || {
            tauri::async_runtime::block_on(async {
                cleanup_application(&handle).await;
            });
            // Use Tauri's managed exit instead of std::process::exit
            handle.exit(0);
        });
    }) {
        error!("Failed to set Ctrl-C handler: {}", e);
    }
}

/// Perform full application cleanup
pub async fn cleanup_application(app_handle: &tauri::AppHandle) {
    info!("Starting application cleanup...");

    // Restore cursor scale before anything else — this must happen even on crash paths
    crate::cursor_scale::force_restore_cursor_scale();

    // Get AppState
    if let Some(app_state) = app_handle.try_state::<AppState>() {
        // Cancel any ongoing agent execution
        app_state.signal_cancel();
        
        // Clean up browser controllers
        if let Ok(mut controller_guard) = app_state.browser_controller.try_lock() {
            if let Some(controller) = controller_guard.take() {
                info!("Cleaning up browser controller from AppState...");
                if let Err(e) = controller.cleanup().await {
                    error!("Failed to cleanup browser controller: {}", e);
                }
            }
        }
        
        // Note: the CDP browser connection is cleaned up when AppState is dropped
        // TTS provider is part of AudioSettings and doesn't need explicit cleanup
    }
    
    // Clean up global resources
    let resource_manager = ResourceManager::global();
    resource_manager.cleanup_all().await;
    
    // Clean up temporary directories
    cleanup_temp_directories();
    
    info!("Application cleanup completed");
}

/// Clean up temporary directories created by the application
fn cleanup_temp_directories() {
    // Clean up Juno temp directories
    if let Ok(temp_dir) = std::env::temp_dir().read_dir() {
        for entry in temp_dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("juno_") || name.starts_with("juno-") {
                    if let Ok(metadata) = entry.metadata() {
                        // Only delete if older than 1 hour (in case multiple instances are running)
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed.as_secs() > 3600 {
                                    info!("Cleaning up old temp directory: {:?}", entry.path());
                                    let _ = std::fs::remove_dir_all(entry.path());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
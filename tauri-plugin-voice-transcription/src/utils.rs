use tauri::{Runtime, Manager};
use std::path::PathBuf;

/// Resolve model path to an absolute path using production-ready path resolution
pub fn resolve_model_path<R: Runtime>(app: &tauri::AppHandle<R>, model_path: &str) -> String {
    tracing::info!("Starting model path resolution for: '{}'", model_path);
    let path = PathBuf::from(model_path);

    // If it's already absolute, use as-is
    if path.is_absolute() {
        if path.exists() {
            tracing::info!("Using absolute model path: {}", path.display());
            return model_path.to_string();
        } else {
            tracing::warn!("Absolute model path does not exist: {}", path.display());
        }
    }

    // Strategy 1: Try bundled resources (production apps)
    tracing::info!("Strategy 1: Checking bundled resources...");

    // First try the direct resource resolution
    if let Ok(resource_path) = app.path().resolve(model_path, tauri::path::BaseDirectory::Resource) {
        tracing::info!("  Resource path resolved to: {}", resource_path.display());
        if resource_path.exists() {
            tracing::info!("Found model in bundled resources: {}", resource_path.display());
            return resource_path.to_string_lossy().to_string();
        } else {
            tracing::info!("  Resource path does not exist");
        }
    } else {
        tracing::info!("  Failed to resolve resource path");
    }

    // Try the _up_ directory pattern used by other bundled resources in production
    if let Ok(resource_dir) = app.path().resource_dir() {
        tracing::info!("  Resource directory: {:?}", resource_dir);

        // Modern bundled paths in production builds (_up_ directory)
        let bundled_paths = [
            // _up_ paths for production builds
            resource_dir.join("_up_").join("models").join(model_path),
            resource_dir.join("_up_").join(model_path),
            // Actual bundled path from our resources configuration
            resource_dir.join("_up_").join("tauri-plugin-voice-transcription").join("models").join("ggml-tiny.en.bin"),
            resource_dir.join("_up_").join("tauri-plugin-voice-transcription").join(model_path),
            // Standard resource paths
            resource_dir.join("models").join(model_path),
            resource_dir.join(model_path),
            // Additional paths for development and production compatibility
            std::path::PathBuf::from("models").join(model_path),
            std::path::PathBuf::from(model_path),
        ];

        for test_path in bundled_paths.iter() {
            tracing::info!("  Checking bundled path: {:?}", test_path);
            if test_path.exists() {
                tracing::info!("Found model in bundled resources: {:?}", test_path);
                return test_path.to_string_lossy().to_string();
            } else {
                tracing::info!("  Bundled path does not exist");
            }
        }
    } else {
        tracing::warn!("  Failed to get resource directory");
    }

    // Strategy 2: Try app data directory (user-installed models)
    if let Ok(app_dir) = app.path().app_data_dir() {
        let app_model_path = app_dir.join(model_path);
        if app_model_path.exists() {
            tracing::info!("Found model in app data dir: {}", app_model_path.display());
            return app_model_path.to_string_lossy().to_string();
        }
    }

    // Strategy 3: Try app local data directory
    if let Ok(local_dir) = app.path().app_local_data_dir() {
        let local_model_path = local_dir.join(model_path);
        if local_model_path.exists() {
            tracing::info!("Found model in app local data dir: {}", local_model_path.display());
            return local_model_path.to_string_lossy().to_string();
        }
    }

    // Strategy 4: Development mode - look for models in plugin directory structure
    if cfg!(debug_assertions) {
        tracing::info!("Strategy 4: Development mode path checking...");
        // Try to find the plugin's models directory in development
        if let Ok(cwd) = std::env::current_dir() {
            tracing::info!("  Current working directory: {}", cwd.display());
            // Look for tauri-plugin-voice-transcription/models and other common dev locations
            let dev_model_paths = [
                cwd.join("tauri-plugin-voice-transcription").join(model_path),
                cwd.join(model_path),
            ];

            for dev_path in &dev_model_paths {
                tracing::info!("  Checking development path: {}", dev_path.display());
                if dev_path.exists() {
                    tracing::info!("Found model in development path: {}", dev_path.display());
                    return dev_path.canonicalize()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| dev_path.to_string_lossy().to_string());
                } else {
                    tracing::info!("  Development path does not exist");
                }
            }
        } else {
            tracing::warn!("  Failed to get current working directory");
        }
    }

    // Strategy 5: macOS App Bundle - Check in Resources directory
    #[cfg(target_os = "macos")]
    {
        tracing::info!("Strategy 5: Checking macOS app bundle...");
        if let Ok(exe_path) = std::env::current_exe() {
            tracing::info!("  Current executable: {}", exe_path.display());
            
            // For macOS app bundles: executable is at Contents/MacOS/binary
            // Resources are at Contents/Resources/
            if let Some(macos_dir) = exe_path.parent() {
                if let Some(contents_dir) = macos_dir.parent() {
                    let resources_dir = contents_dir.join("Resources");
                    tracing::info!("  Checking Resources directory: {}", resources_dir.display());
                    
                    let bundle_paths = vec![
                        resources_dir.join(model_path),
                        resources_dir.join("models").join(model_path),
                        resources_dir.join("_up_").join(model_path),
                        resources_dir.join("_up_").join("models").join(model_path),
                        // Add the actual bundled path we found
                        resources_dir.join("_up_").join("tauri-plugin-voice-transcription").join(model_path),
                        resources_dir.join("_up_").join("tauri-plugin-voice-transcription").join("models").join("ggml-tiny.en.bin"),
                    ];
                    
                    for bundle_path in bundle_paths {
                        tracing::info!("  Checking bundle path: {}", bundle_path.display());
                        if bundle_path.exists() {
                            tracing::info!("Found model in macOS bundle: {}", bundle_path.display());
                            return bundle_path.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }
    }

    // Strategy 6: Look relative to current working directory
    let cwd_model_path = PathBuf::from(model_path);
    if cwd_model_path.exists() {
        tracing::info!("Found model in current working directory: {}", cwd_model_path.display());
        return cwd_model_path.canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| model_path.to_string());
    }

    // Strategy 7: Final fallback - return original path (will likely fail, but preserves error handling)
    tracing::warn!("Model file '{}' not found in any standard location. Locations checked:", model_path);
    tracing::warn!("  - Bundled resources: {}", model_path);
    tracing::warn!("  - Bundled resources (_up_ pattern): _up_/{}", model_path);
    tracing::warn!("  - App data directory: [app_data]/{}", model_path);
    tracing::warn!("  - App local data directory: [local_data]/{}", model_path);
    if cfg!(debug_assertions) {
        tracing::warn!("  - Development paths: ./tauri-plugin-voice-transcription/{} and ./{}", model_path, model_path);
    }
    tracing::warn!("  - Current working directory: ./{}", model_path);

    tracing::error!("Returning original model path '{}' as fallback (will likely fail)", model_path);
    model_path.to_string()
}

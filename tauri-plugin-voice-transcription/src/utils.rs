use tauri::{Runtime, Manager};
use std::path::PathBuf;

/// Resolve model path to an absolute path
pub fn resolve_model_path<R: Runtime>(app: &tauri::AppHandle<R>, model_path: &str) -> String {
    let path = PathBuf::from(model_path);

    // If it's already absolute, use as-is
    if path.is_absolute() {
        return model_path.to_string();
    }

    // Try to resolve relative to app directory first
    if let Ok(app_dir) = app.path().app_data_dir() {
        let app_model_path = app_dir.join(model_path);
        if app_model_path.exists() {
            tracing::info!("Found model in app data dir: {}", app_model_path.display());
            return app_model_path.to_string_lossy().to_string();
        }
    }

    // Try to resolve relative to app local data directory
    if let Ok(local_dir) = app.path().app_local_data_dir() {
        let local_model_path = local_dir.join(model_path);
        if local_model_path.exists() {
            tracing::info!("Found model in app local data dir: {}", local_model_path.display());
            return local_model_path.to_string_lossy().to_string();
        }
    }

    // Try to resolve relative to the current executable directory (for dev mode)
    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(exe_parent) = exe_dir.parent() {
            // In development, go up to project root
            let dev_model_path = exe_parent.join("../../../").join(model_path);
            if dev_model_path.exists() {
                tracing::info!("Found model in dev project root: {}", dev_model_path.display());
                return dev_model_path.canonicalize()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| dev_model_path.to_string_lossy().to_string());
            }
        }
    }

    // Try the current working directory as fallback
    let cwd_model_path = PathBuf::from(model_path);
    if cwd_model_path.exists() {
        tracing::info!("Found model in current working directory: {}", cwd_model_path.display());
        return cwd_model_path.canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| model_path.to_string());
    }

    tracing::warn!("Model file not found in any location, using original path: {}", model_path);
    model_path.to_string()
}

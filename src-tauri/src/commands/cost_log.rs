//! Switching per-turn cost logging on, and getting the log back out as CSV.
//!
//! The log itself is [`crate::cost_log`]. This is the thin Tauri surface over
//! it: three commands and the startup sync that tells the logger what the
//! store says.
//!
//! There is no UI for any of this. Cost logging is an instrument for sizing
//! the trial credit and the Pro allowance (LAC-4124), not a feature, so it is
//! reached from the console or from a headless run with `JUNO_COST_LOG=1`.

use tauri_plugin_store::StoreExt;

use crate::constants::settings::{defaults, store_keys, SETTINGS_STORE_FILE};
use crate::cost_log;

/// What the store says about cost logging.
fn stored_enabled(app: &tauri::AppHandle) -> bool {
    app.store(SETTINGS_STORE_FILE)
        .ok()
        .and_then(|store| store.get(store_keys::COST_LOGGING_ENABLED))
        .and_then(|value| value.as_bool())
        .unwrap_or(defaults::COST_LOGGING_ENABLED)
}

/// Tell the logger what the store says.
///
/// Called once at startup. The logger keeps the answer in an atomic because
/// it is read on every model request from code that has no `AppHandle` —
/// `juno-cua` and the batch runner have no store to read at all.
pub fn sync_from_store(app: &tauri::AppHandle) {
    let enabled = stored_enabled(app);
    cost_log::set_enabled(enabled);
    if enabled {
        match cost_log::log_path() {
            Some(path) => tracing::info!(
                target: cost_log::TARGET,
                "[CostLog] recording per-turn cost to {}",
                path.display()
            ),
            None => tracing::warn!(
                target: cost_log::TARGET,
                "[CostLog] on, but this system has no cache directory to write to"
            ),
        }
    }
}

/// Whether per-turn cost logging is on.
#[tauri::command]
pub async fn get_cost_logging_enabled(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(stored_enabled(&app_handle))
}

/// Turn per-turn cost logging on or off. Applies to the next model request.
#[tauri::command]
pub async fn set_cost_logging_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let store = app_handle
        .store(SETTINGS_STORE_FILE)
        .map_err(|e| format!("Failed to access settings store: {e}"))?;
    store.set(
        store_keys::COST_LOGGING_ENABLED,
        serde_json::Value::Bool(enabled),
    );
    store
        .save()
        .map_err(|e| format!("Failed to save settings store: {e}"))?;
    cost_log::set_enabled(enabled);
    tracing::info!(
        target: cost_log::TARGET,
        "[CostLog] per-turn cost logging {}",
        if enabled { "on" } else { "off" }
    );
    Ok(())
}

/// What an export produced.
#[derive(Debug, serde::Serialize)]
pub struct CostLogExport {
    /// Where the CSV was written.
    pub path: String,
    /// How many turns it holds. Zero with cost logging never switched on is
    /// the expected answer, not a failure.
    pub rows: usize,
}

/// Write the cost log out as CSV, next to the NDJSON it came from.
///
/// Reading rather than streaming: the file is one short line per model
/// request, so even a long measurement run is a few megabytes.
#[tauri::command]
pub async fn export_cost_log_csv(path: Option<String>) -> Result<CostLogExport, String> {
    let records = cost_log::read_log()?;
    let destination = match path {
        Some(path) => std::path::PathBuf::from(path),
        None => cost_log::log_path()
            .map(|p| p.with_extension("csv"))
            .ok_or_else(|| "No cache directory on this system".to_string())?,
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    std::fs::write(&destination, cost_log::to_csv(&records))
        .map_err(|e| format!("Failed to write {}: {}", destination.display(), e))?;
    Ok(CostLogExport {
        path: destination.to_string_lossy().to_string(),
        rows: records.len(),
    })
}

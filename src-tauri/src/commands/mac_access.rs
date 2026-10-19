//! The bridge for the Mac app cards that need System Settings.
//!
//! One command, no logic of its own: the answer is `mac_apps::preflight`'s.

/// Can this running Juno read the Messages history right now? Silent, no dialog.
#[tauri::command]
pub async fn full_disk_access_granted() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(
        crate::agent::tools::mac_apps::preflight::full_disk_access_granted,
    )
    .await
    .map_err(|e| format!("Could not check Full Disk Access: {e}"))
}

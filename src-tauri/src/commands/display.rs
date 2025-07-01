//! Display Commands - spawn/update/close lightweight display windows
//!
//! SECURITY NOTES:
//!  * Only specific `kind` values are permitted (image, widget, html, url).
//!  * Payload size is limited to prevent memory abuse.
//!  * HTML payload is sanitized via `ammonia` to strip scripts and dangerous tags.
//!
//! Windows are stored in AppState for lookup. The payload is sent to the window via
//! the `display://update` event channel so the frontend can update its contents.

use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, WebviewUrl};
use tracing::{debug, error, info};

/// Maximum payload size we accept (bytes) to prevent memory/DoS abuse
const MAX_PAYLOAD_SIZE: usize = 5 * 1024 * 1024; // 5 MB

/// Allowed kinds for display windows
const ALLOWED_KINDS: &[&str] = &["image", "widget", "html", "url"];

#[derive(Clone, Deserialize)]
pub struct DisplaySpawnArgs {
    pub id: String,
    pub kind: String,
    pub payload: Value,
    pub title: Option<String>,
    pub position: Option<[i32; 2]>,
    pub size: Option<[i32; 2]>,
}

#[derive(Clone, Deserialize)]
pub struct DisplayUpdateArgs {
    pub id: String,
    pub payload: Value,
}

#[derive(Clone, Deserialize)]
pub struct DisplayCloseArgs {
    pub id: String,
}

/// Internal helper to validate kind and payload size
fn validate_kind_and_payload(kind: &str, payload: &Value) -> Result<(), String> {
    if !ALLOWED_KINDS.contains(&kind) {
        return Err(format!("Unsupported display kind: {}", kind));
    }

    // Rough payload size check when serialized to string
    let size = payload.to_string().len();
    if size > MAX_PAYLOAD_SIZE {
        return Err("Display payload too large (>5MB)".into());
    }
    Ok(())
}

/// Sanitize HTML payload if needed.
fn sanitize_payload(kind: &str, payload: &Value) -> Result<Value, String> {
    if kind == "html" {
        let raw = payload
            .as_str()
            .ok_or("HTML payload must be a string")?;
        let clean = ammonia::Builder::default()
            .link_rel(None)
            .clean(raw)
            .to_string();
        Ok(Value::String(clean))
    } else {
        Ok(payload.clone())
    }
}

/// Map window IDs to windows. Stored inside tauri AppState.
#[derive(Default)]
pub struct DisplayWindowManager {
    windows: dashmap::DashMap<String, WebviewWindow>,
}

impl DisplayWindowManager {
    pub fn insert(&self, id: &str, window: WebviewWindow) {
        self.windows.insert(id.to_string(), window);
    }

    pub fn get(&self, id: &str) -> Option<WebviewWindow> {
        self.windows.get(id).map(|r| r.value().clone())
    }

    pub fn remove(&self, id: &str) {
        self.windows.remove(id);
    }
}

pub fn get_display_manager(app_handle: &AppHandle) -> Arc<DisplayWindowManager> {
    app_handle
        .state::<Arc<DisplayWindowManager>>()
        .clone()
}

/// Spawn a display window
#[tauri::command]
pub async fn display_spawn(app_handle: AppHandle, args: DisplaySpawnArgs) -> Result<(), String> {
    debug!("display_spawn called: {:?}", args.id);

    validate_kind_and_payload(&args.kind, &args.payload)?;
    let sanitized_payload = sanitize_payload(&args.kind, &args.payload)?;

    let manager = get_display_manager(&app_handle);

    if manager.get(&args.id).is_some() {
        return Err(format!("Display with id '{}' already exists", args.id));
    }

    // Build window
    let label = format!("display_{}", &args.id);
    let mut builder = WebviewWindowBuilder::new(&app_handle, label.clone(), WebviewUrl::App("/display.html".into()))
        .title(args.title.clone().unwrap_or_else(|| args.id.clone()))
        .resizable(true);

    if let Some(size) = args.size {
        builder = builder.inner_size(size[0] as f64, size[1] as f64);
    }

    if let Some(pos) = args.position {
        builder = builder.position(pos[0] as f64, pos[1] as f64);
    }

    let window = builder.build().map_err(|e| format!("Failed to create window: {}", e))?;

    // Store window in manager
    manager.insert(&args.id, window.clone());

    // Emit initial payload to window
    window
        .emit("display://init", &serde_json::json!({
            "id": args.id,
            "kind": args.kind,
            "payload": sanitized_payload,
        }))
        .map_err(|e| format!("Emit failed: {}", e))?;

    info!("Display window '{}' spawned", args.id);
    Ok(())
}

/// Update an existing display window
#[tauri::command]
pub async fn display_update(app_handle: AppHandle, args: DisplayUpdateArgs) -> Result<(), String> {
    debug!("display_update called: {:?}", args.id);

    let manager = get_display_manager(&app_handle);
    let window = manager
        .get(&args.id)
        .ok_or_else(|| format!("Display with id '{}' not found", args.id))?;

    // Validate payload size
    if args.payload.to_string().len() > MAX_PAYLOAD_SIZE {
        return Err("Display payload too large (>5MB)".into());
    }

    let sanitized_payload = sanitize_payload("html", &args.payload)?; // sanitize if html

    window
        .emit("display://update", &serde_json::json!({
            "id": args.id,
            "payload": sanitized_payload,
        }))
        .map_err(|e| format!("Emit failed: {}", e))?;

    Ok(())
}

/// Close and destroy a display window
#[tauri::command]
pub async fn display_close(app_handle: AppHandle, args: DisplayCloseArgs) -> Result<(), String> {
    debug!("display_close called: {:?}", args.id);

    let manager = get_display_manager(&app_handle);
    let window = manager
        .get(&args.id)
        .ok_or_else(|| format!("Display with id '{}' not found", args.id))?;

    window.close().map_err(|e| format!("Failed to close window: {}", e))?;
    manager.remove(&args.id);

    info!("Display window '{}' closed", args.id);
    Ok(())
}
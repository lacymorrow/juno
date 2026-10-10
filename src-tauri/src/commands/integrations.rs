//! Connected apps: the commands behind the connect card, the Tools section's
//! "Connected apps" group, and the Advanced "Use my own Composio account"
//! toggle (LAC-4210, internal beta).
//!
//! The agent never reaches any of this. A person pressing a button is the
//! only caller: the toggle installs or removes the one Composio MCP server
//! and runs MCP OAuth in their browser, and the connect card drives one app's
//! own consent flow. Phase 2 replaces the person's Composio account with
//! Juno's gateway (LAC-4128) without touching these surfaces.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_store::StoreExt;
use tracing::{info, warn};

use crate::agent::tools::composio::{self, ConnectIntent, ConnectedApp};
use crate::agent::tools::mcp_oauth;
use crate::constants::events;
use crate::state::AppState;

/// The store file holding the connected-app records. Records, not secrets:
/// tokens live with Composio, and Juno's own OAuth tokens live in the
/// Keychain.
const STORE_FILE: &str = "integrations.json";
const CONNECTED_APPS_KEY: &str = "connected_apps";

/// Everything the two UI surfaces need, in one read.
#[derive(Debug, Clone, Serialize)]
pub struct IntegrationsStatus {
    /// The Advanced toggle: the Composio MCP server is installed.
    pub enabled: bool,
    /// OAuth to the Composio endpoint has completed.
    pub authorized: bool,
    /// Apps recorded as connected; the Tools group renders only when this is
    /// non-empty.
    pub connected_apps: Vec<ConnectedApp>,
}

#[tauri::command]
pub async fn get_integrations_status(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<IntegrationsStatus, String> {
    let config = find_composio_config(&state).await;
    let authorized = match &config {
        Some(config) => mcp_oauth::is_authorized(&config.id).await,
        None => false,
    };
    Ok(IntegrationsStatus {
        enabled: config.is_some(),
        authorized,
        connected_apps: read_connected_apps(&app_handle),
    })
}

/// The Advanced toggle. Enabling installs the Composio MCP server and runs
/// the browser OAuth consent before returning; disabling removes the server,
/// forgets the tokens, and clears the app records.
#[tauri::command]
pub async fn set_byo_composio_enabled(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<IntegrationsStatus, String> {
    let existing = find_composio_config(&state).await;

    if enabled {
        let config = match existing {
            Some(config) => config,
            None => {
                let config = composio::server_config();
                super::mcp::add_mcp_server(app_handle.clone(), state.clone(), config.clone())
                    .await?;
                config
            }
        };
        if !mcp_oauth::is_authorized(&config.id).await {
            mcp_oauth::authorize_interactive(&config.id, composio::MCP_URL).await?;
        }
        // The first start may have failed its 401 before consent; start again
        // now that tokens exist, and pull the tool catalog in.
        let mcp_manager = state.get_mcp_manager().await;
        {
            let manager_guard = mcp_manager.lock().await;
            if let Err(e) = manager_guard.start_server(&config.id).await {
                warn!("Composio server start after consent: {}", e);
            }
        }
        state.sync_mcp_tools().await?;
    } else if let Some(config) = existing {
        super::mcp::remove_mcp_server(app_handle.clone(), state.clone(), config.id.clone()).await?;
        mcp_oauth::forget(&config.id).await?;
        write_connected_apps(&app_handle, &[])?;
    }

    emit_connections_changed(&app_handle);
    get_integrations_status(app_handle, state).await
}

/// The connect card's one button. Asks Composio to initiate the app's
/// connection, opens the provider's own consent screen in the browser, waits
/// for Composio to confirm, and records the app. The caller retries the
/// person's original request after this returns.
#[tauri::command]
pub async fn connect_integration_app(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    toolkit_slug: String,
) -> Result<ConnectedApp, String> {
    let toolkit_slug = toolkit_slug.to_lowercase();
    let initiate = call_connections_tool(
        &state,
        "COMPOSIO_MANAGE_CONNECTIONS",
        &toolkit_slug,
        ConnectIntent::Initiate,
    )
    .await?;

    // An already-connected app comes back with no consent link; that is
    // success, not an error.
    if let Some(consent_url) = composio::first_url(&initiate) {
        info!("Opening consent for {}: {}", toolkit_slug, consent_url);
        open::that(&consent_url).map_err(|e| format!("Could not open the browser: {e}"))?;

        // Composio's own wait meta-tool blocks until the consent lands.
        if let Err(e) = call_connections_tool(
            &state,
            "COMPOSIO_WAIT_FOR_CONNECTION",
            &toolkit_slug,
            ConnectIntent::List,
        )
        .await
        {
            warn!("Wait for {} connection reported: {}", toolkit_slug, e);
        }
    }

    let app = ConnectedApp {
        toolkit_slug: toolkit_slug.clone(),
        app_name: composio::toolkit_display(&toolkit_slug),
        account: find_account(&initiate),
        connected_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    let mut apps = read_connected_apps(&app_handle);
    apps.retain(|existing| existing.toolkit_slug != app.toolkit_slug);
    apps.push(app.clone());
    write_connected_apps(&app_handle, &apps)?;
    emit_connections_changed(&app_handle);
    Ok(app)
}

/// The Disconnect button on a Connected apps row.
#[tauri::command]
pub async fn disconnect_integration_app(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    toolkit_slug: String,
) -> Result<(), String> {
    let toolkit_slug = toolkit_slug.to_lowercase();
    if let Err(e) = call_connections_tool(
        &state,
        "COMPOSIO_MANAGE_CONNECTIONS",
        &toolkit_slug,
        ConnectIntent::Disconnect,
    )
    .await
    {
        // The record still goes: a row whose Disconnect does nothing is worse
        // than re-running the connect flow later.
        warn!("Composio disconnect for {} reported: {}", toolkit_slug, e);
    }

    let mut apps = read_connected_apps(&app_handle);
    apps.retain(|existing| existing.toolkit_slug != toolkit_slug);
    write_connected_apps(&app_handle, &apps)?;
    emit_connections_changed(&app_handle);
    Ok(())
}

// --- helpers ---

async fn find_composio_config(
    state: &State<'_, AppState>,
) -> Option<crate::agent::tools::mcp_integration::MCPServerConfig> {
    let tool_config = state.get_tool_config_manager().await;
    let config_guard = tool_config.lock().await;
    config_guard
        .get_mcp_servers()
        .into_iter()
        .find(|config| config.name == composio::SERVER_NAME)
}

/// Call one of Composio's connection meta-tools with arguments built from its
/// *discovered* schema, so a vendor schema drift degrades to a readable
/// validation error instead of silently wrong arguments.
async fn call_connections_tool(
    state: &State<'_, AppState>,
    tool: &str,
    toolkit_slug: &str,
    intent: ConnectIntent,
) -> Result<Value, String> {
    let mcp_manager = state.get_mcp_manager().await;
    let manager_guard = mcp_manager.lock().await;

    // The wait tool's exact name is matched loosely (singular/plural has
    // drifted in the wild); the manage tool is exact.
    let wanted_prefix = composio::registered_tool_name(tool);
    let tools = manager_guard.get_all_tools().await;
    let tool_info = tools
        .iter()
        .find(|info| info.tool_definition.name == wanted_prefix)
        .or_else(|| {
            tools
                .iter()
                .find(|info| info.tool_definition.name.starts_with(&wanted_prefix))
        })
        .ok_or_else(|| {
            format!("Composio is not connected (no {tool} tool). Turn the integration on first.")
        })?;

    let args = composio::manage_connections_args(
        &tool_info.tool_definition.input_schema,
        toolkit_slug,
        intent,
    );
    let name = tool_info.tool_definition.name.clone();
    let result = manager_guard
        .execute_tool(&name, args, uuid::Uuid::new_v4().to_string())
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.output)
}

/// An account label when the initiate result carries one. Best effort: the
/// row renders fine without it.
fn find_account(value: &Value) -> Option<String> {
    const ACCOUNT_KEYS: &[&str] = &["account", "email", "user_email", "connected_account"];
    match value {
        Value::Object(map) => {
            for key in ACCOUNT_KEYS {
                if let Some(Value::String(account)) = map.get(*key) {
                    if !account.is_empty() {
                        return Some(account.clone());
                    }
                }
            }
            map.values().find_map(find_account)
        }
        Value::Array(items) => items.iter().find_map(find_account),
        _ => None,
    }
}

fn read_connected_apps(app_handle: &AppHandle) -> Vec<ConnectedApp> {
    let Ok(store) = app_handle.store(STORE_FILE) else {
        return Vec::new();
    };
    store
        .get(CONNECTED_APPS_KEY)
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn write_connected_apps(app_handle: &AppHandle, apps: &[ConnectedApp]) -> Result<(), String> {
    let store = app_handle
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open integrations store: {e}"))?;
    let value =
        serde_json::to_value(apps).map_err(|e| format!("Failed to serialize app records: {e}"))?;
    store.set(CONNECTED_APPS_KEY, value);
    store
        .save()
        .map_err(|e| format!("Failed to save integrations store: {e}"))
}

fn emit_connections_changed(app_handle: &AppHandle) {
    if let Err(e) = app_handle.emit(events::integrations::CONNECTIONS_CHANGED, ()) {
        warn!("Failed to emit connections-changed: {}", e);
    }
}

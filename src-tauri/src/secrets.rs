//! Secrets live in the macOS Keychain, never in settings JSON.
//!
//! Two kinds of integration secrets exist today: static header values for an
//! HTTP MCP server (`x-consumer-api-key` and the like) and the OAuth token
//! set `mcp_oauth` keeps per server. Both would otherwise end up in the Tauri
//! store, which is a world-readable JSON file in Application Support; a
//! refresh token for someone's Gmail does not belong there (LAC-4210).
//!
//! The Security.framework calls are blocking C, so every function here hops
//! through `spawn_blocking` rather than stalling the async runtime.

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

/// Keychain service for MCP header values referenced as `keychain:{account}`.
pub const MCP_HEADER_SERVICE: &str = "com.juno.desktop.mcp-headers";
/// Keychain service for per-server MCP OAuth token sets.
pub const MCP_OAUTH_SERVICE: &str = "com.juno.desktop.mcp-oauth";

/// The marker an `MCPServerConfig` header value uses to say "the real value
/// is in the Keychain under this account".
pub const KEYCHAIN_VALUE_PREFIX: &str = "keychain:";

/// Read a secret. `Ok(None)` is "not stored", which is an ordinary state and
/// not an error.
pub async fn get(service: &'static str, account: &str) -> Result<Option<String>, String> {
    let account = account.to_string();
    tokio::task::spawn_blocking(move || match get_generic_password(service, &account) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "Keychain item is not valid UTF-8".to_string()),
        // errSecItemNotFound is the "nothing stored" answer; everything else
        // (locked keychain, denied access) is worth surfacing.
        Err(e) if e.code() == security_framework_sys::base::errSecItemNotFound => Ok(None),
        Err(e) => Err(format!("Keychain read failed: {e}")),
    })
    .await
    .map_err(|e| format!("Keychain task failed: {e}"))?
}

/// Write a secret, replacing whatever was stored.
pub async fn set(service: &'static str, account: &str, value: &str) -> Result<(), String> {
    let account = account.to_string();
    let value = value.as_bytes().to_vec();
    tokio::task::spawn_blocking(move || {
        set_generic_password(service, &account, &value)
            .map_err(|e| format!("Keychain write failed: {e}"))
    })
    .await
    .map_err(|e| format!("Keychain task failed: {e}"))?
}

/// Delete a secret. Deleting something absent is success: the goal state is
/// "not stored" and it already holds.
pub async fn delete(service: &'static str, account: &str) -> Result<(), String> {
    let account = account.to_string();
    tokio::task::spawn_blocking(move || match delete_generic_password(service, &account) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == security_framework_sys::base::errSecItemNotFound => Ok(()),
        Err(e) => Err(format!("Keychain delete failed: {e}")),
    })
    .await
    .map_err(|e| format!("Keychain task failed: {e}"))?
}

/// Resolve an MCP header value: a literal passes through, and a
/// `keychain:{account}` reference is read from [`MCP_HEADER_SERVICE`]. A
/// dangling reference is an error rather than an empty header, because an
/// empty credential header produces a 401 that reads like the server's fault.
pub async fn resolve_header_value(value: &str) -> Result<String, String> {
    match value.strip_prefix(KEYCHAIN_VALUE_PREFIX) {
        None => Ok(value.to_string()),
        Some(account) => get(MCP_HEADER_SERVICE, account)
            .await?
            .ok_or_else(|| format!("No Keychain item for header reference '{account}'")),
    }
}

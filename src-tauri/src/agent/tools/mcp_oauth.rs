//! A generic OAuth client for remote MCP servers.
//!
//! Any streamable HTTP MCP server that answers 401 with RFC 9728
//! protected-resource metadata can be connected with no server-specific code:
//! discovery, dynamic client registration (RFC 7591), PKCE S256, a loopback
//! redirect on 127.0.0.1, token exchange, and refresh on 401. Composio
//! (`connect.composio.dev/mcp`) is the first user, but nothing in this file
//! knows that (LAC-4210).
//!
//! Tokens, the client id and the token endpoint are stored together as one
//! JSON blob in the macOS Keychain under the MCP server's id
//! (`secrets::MCP_OAUTH_SERVICE`), so a refresh needs no re-discovery and no
//! secret ever sits in settings JSON.
//!
//! The agent never reaches this module. Connecting is a person pressing a
//! button in Settings or on a connect card; both land on
//! [`authorize_interactive`] via a Tauri command.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex as TokioMutex;
use tracing::{info, warn};

use crate::secrets;

/// How long the person gets to finish the browser consent step.
const CONSENT_TIMEOUT: Duration = Duration::from_secs(300);
/// Timeout for every metadata/registration/token HTTP call.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Tokens are refreshed this long before their stated expiry, so a token is
/// never sent in its final seconds.
const EXPIRY_MARGIN_SECS: u64 = 60;

/// Serializes refreshes. Two concurrent 401s otherwise race the token
/// endpoint, and a server that rotates refresh tokens would invalidate the
/// winner with the loser.
static REFRESH_LOCK: TokioMutex<()> = TokioMutex::const_new(());

/// Everything a later refresh needs, persisted as one Keychain item per
/// server id.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredOAuth {
    client_id: String,
    token_endpoint: String,
    /// The MCP endpoint, sent as the RFC 8707 `resource` indicator.
    resource: String,
    access_token: String,
    refresh_token: Option<String>,
    /// Unix seconds after which the access token is treated as expired.
    expires_at: u64,
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

async fn load(server_id: &str) -> Result<Option<StoredOAuth>, String> {
    match secrets::get(secrets::MCP_OAUTH_SERVICE, server_id).await? {
        None => Ok(None),
        Some(blob) => serde_json::from_str(&blob)
            .map(Some)
            .map_err(|e| format!("Stored OAuth state is unreadable: {e}")),
    }
}

async fn store(server_id: &str, state: &StoredOAuth) -> Result<(), String> {
    let blob =
        serde_json::to_string(state).map_err(|e| format!("Failed to serialize tokens: {e}"))?;
    secrets::set(secrets::MCP_OAUTH_SERVICE, server_id, &blob).await
}

/// Whether this server has a stored token set at all.
pub async fn is_authorized(server_id: &str) -> bool {
    matches!(load(server_id).await, Ok(Some(_)))
}

/// Forget a server's tokens. Used by disconnect and by "start over".
pub async fn forget(server_id: &str) -> Result<(), String> {
    secrets::delete(secrets::MCP_OAUTH_SERVICE, server_id).await
}

/// The access token to send right now, refreshed first when it is at or past
/// its expiry margin. `None` means "not connected", which the transport
/// surfaces as its 401 path.
pub async fn access_token(server_id: &str) -> Option<String> {
    let stored = load(server_id).await.ok()??;
    if now_epoch_secs() < stored.expires_at {
        return Some(stored.access_token);
    }
    match refresh_tokens(server_id).await {
        Ok(token) => Some(token),
        Err(e) => {
            warn!("MCP OAuth refresh for '{server_id}' failed: {e}");
            // Let the expired token travel: the server's 401 carries better
            // information than silently sending nothing.
            Some(stored.access_token)
        }
    }
}

/// Exchange the refresh token for a new access token and persist the result.
pub async fn refresh_tokens(server_id: &str) -> Result<String, String> {
    let _serialized = REFRESH_LOCK.lock().await;

    let stored = load(server_id)
        .await?
        .ok_or_else(|| "No stored OAuth state; connect this server first".to_string())?;
    let refresh_token = stored
        .refresh_token
        .clone()
        .ok_or_else(|| "The server issued no refresh token; reconnect it".to_string())?;

    let client = http_client()?;
    let response = client
        .post(&stored.token_endpoint)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", stored.client_id.as_str()),
            ("resource", stored.resource.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Token refresh request failed: {e}"))?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Token refresh response unreadable: {e}"))?;
    if !status.is_success() {
        return Err(format!("Token refresh returned {status}: {body}"));
    }

    let updated = apply_token_response(stored, &body)?;
    store(server_id, &updated).await?;
    info!("Refreshed MCP OAuth tokens for '{server_id}'");
    Ok(updated.access_token)
}

/// Fold a token-endpoint response into the stored state. A missing refresh
/// token keeps the old one (rotation is optional in the spec).
fn apply_token_response(mut stored: StoredOAuth, body: &Value) -> Result<StoredOAuth, String> {
    let access = body
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Token response carried no access_token: {body}"))?;
    stored.access_token = access.to_string();
    if let Some(rotated) = body.get("refresh_token").and_then(Value::as_str) {
        stored.refresh_token = Some(rotated.to_string());
    }
    let expires_in = body
        .get("expires_in")
        .and_then(Value::as_u64)
        .unwrap_or(3600);
    stored.expires_at = now_epoch_secs() + expires_in.saturating_sub(EXPIRY_MARGIN_SECS);
    Ok(stored)
}

/// The authorization server's metadata, reduced to the three endpoints this
/// client uses.
struct AuthServerMetadata {
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: Option<String>,
    scopes_supported: Vec<String>,
}

/// RFC 9728 + RFC 8414 discovery, starting from nothing but the MCP URL.
async fn discover(mcp_url: &str) -> Result<AuthServerMetadata, String> {
    let client = http_client()?;

    // Step 1: provoke the 401 and read where the protected-resource metadata
    // lives. The fallback is the well-known path on the MCP URL's origin.
    let probe = client
        .post(mcp_url)
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc": "2.0", "id": 0, "method": "ping"}))
        .send()
        .await
        .map_err(|e| format!("Could not reach {mcp_url}: {e}"))?;

    let advertised = probe
        .headers()
        .get("www-authenticate")
        .and_then(|v| v.to_str().ok())
        .and_then(extract_resource_metadata_url);

    let origin = url_origin(mcp_url)?;
    let resource_metadata_url =
        advertised.unwrap_or_else(|| format!("{origin}/.well-known/oauth-protected-resource"));

    let resource_metadata: Value = client
        .get(&resource_metadata_url)
        .send()
        .await
        .map_err(|e| format!("Could not fetch resource metadata: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Resource metadata unreadable: {e}"))?;

    let auth_server = resource_metadata
        .get("authorization_servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(Value::as_str)
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_else(|| origin.clone());

    // Step 2: the authorization server's own metadata.
    let metadata_url = format!("{auth_server}/.well-known/oauth-authorization-server");
    let metadata: Value = client
        .get(&metadata_url)
        .send()
        .await
        .map_err(|e| format!("Could not fetch auth server metadata: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Auth server metadata unreadable: {e}"))?;

    let endpoint = |key: &str| -> Result<String, String> {
        metadata
            .get(key)
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Auth server metadata is missing {key}"))
    };

    Ok(AuthServerMetadata {
        authorization_endpoint: endpoint("authorization_endpoint")?,
        token_endpoint: endpoint("token_endpoint")?,
        registration_endpoint: metadata
            .get("registration_endpoint")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        scopes_supported: metadata
            .get("scopes_supported")
            .and_then(Value::as_array)
            .map(|scopes| {
                scopes
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Pull the `resource_metadata="..."` URL out of a WWW-Authenticate header.
fn extract_resource_metadata_url(header: &str) -> Option<String> {
    let (_, tail) = header.split_once("resource_metadata=")?;
    let tail = tail.trim_start_matches('"');
    let end = tail.find('"').unwrap_or(tail.len());
    let url = tail.get(..end)?.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// `https://host[:port]` of a URL, no path.
fn url_origin(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw).map_err(|e| format!("Invalid MCP URL {raw:?}: {e}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| format!("MCP URL {raw:?} has no host"))?;
    let origin = match parsed.port() {
        Some(port) => format!("{}://{}:{}", parsed.scheme(), host, port),
        None => format!("{}://{}", parsed.scheme(), host),
    };
    Ok(origin)
}

/// PKCE verifier: 64 characters of uuid hex, which is inside RFC 7636's
/// unreserved character set and carries 244 bits of OS randomness.
fn pkce_verifier() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// What the loopback listener hands back from the redirect.
struct CallbackResult {
    code: String,
    state: String,
}

/// Run the whole interactive flow for one server: discovery, registration,
/// browser consent on the system default browser, loopback callback, token
/// exchange, Keychain. Returns once the tokens are stored.
///
/// Every run registers a fresh client: registration is free, the loopback
/// port is ephemeral so the redirect URI differs each time anyway, and a
/// stored client id is then only ever used for the one thing it stays valid
/// for, refreshing.
pub async fn authorize_interactive(server_id: &str, mcp_url: &str) -> Result<(), String> {
    let metadata = discover(mcp_url).await?;
    let client = http_client()?;

    // Loopback listener before registration, because the redirect URI has to
    // be in the registration request.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("Could not open a loopback port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Loopback port unreadable: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let registration_endpoint = metadata
        .registration_endpoint
        .as_deref()
        .ok_or_else(|| "The auth server offers no dynamic client registration".to_string())?;
    let registration: Value = client
        .post(registration_endpoint)
        .json(&json!({
            "client_name": "Juno",
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        }))
        .send()
        .await
        .map_err(|e| format!("Client registration failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Registration response unreadable: {e}"))?;
    let client_id = registration
        .get("client_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Registration returned no client_id: {registration}"))?
        .to_string();

    let verifier = pkce_verifier();
    let challenge = pkce_challenge(&verifier);
    let state = uuid::Uuid::new_v4().to_string();

    let mut authorize_url = url::Url::parse(&metadata.authorization_endpoint)
        .map_err(|e| format!("Bad authorization endpoint: {e}"))?;
    {
        let mut query = authorize_url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", &client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state)
            .append_pair("resource", mcp_url);
        if !metadata.scopes_supported.is_empty() {
            query.append_pair("scope", &metadata.scopes_supported.join(" "));
        }
    }

    info!("Opening browser for MCP OAuth consent ({server_id})");
    open::that(authorize_url.as_str())
        .map_err(|e| format!("Could not open the browser for consent: {e}"))?;

    let callback = tokio::time::timeout(CONSENT_TIMEOUT, wait_for_callback(listener))
        .await
        .map_err(|_| "Nobody finished the browser consent in time".to_string())??;

    if callback.state != state {
        return Err("The consent redirect carried the wrong state; dropping it".to_string());
    }

    let response = client
        .post(&metadata.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", callback.code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", client_id.as_str()),
            ("code_verifier", verifier.as_str()),
            ("resource", mcp_url),
        ])
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {e}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Token response unreadable: {e}"))?;
    if !status.is_success() {
        return Err(format!("Token exchange returned {status}: {body}"));
    }

    let stored = apply_token_response(
        StoredOAuth {
            client_id,
            token_endpoint: metadata.token_endpoint,
            resource: mcp_url.to_string(),
            access_token: String::new(),
            refresh_token: None,
            expires_at: 0,
        },
        &body,
    )?;
    store(server_id, &stored).await?;
    info!("MCP OAuth connected for '{server_id}'");
    Ok(())
}

/// Accept connections until one carries the authorization code. The response
/// is a sentence, not a page: the person's next glance is back at Juno.
async fn wait_for_callback(listener: tokio::net::TcpListener) -> Result<CallbackResult, String> {
    use tokio::io::AsyncReadExt;

    loop {
        let (mut socket, _) = listener
            .accept()
            .await
            .map_err(|e| format!("Loopback accept failed: {e}"))?;

        let mut buffer = vec![0u8; 8192];
        let read = socket.read(&mut buffer).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..read]);

        // "GET /callback?code=...&state=... HTTP/1.1"
        let target = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("");

        let parsed = url::Url::parse(&format!("http://127.0.0.1{target}")).ok();
        let query = |key: &str| -> Option<String> {
            parsed.as_ref().and_then(|u| {
                u.query_pairs()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.into_owned())
            })
        };

        let (body, result) = match (query("code"), query("state"), query("error")) {
            (Some(code), Some(state), _) => (
                "You're connected. You can close this tab.",
                Some(CallbackResult { code, state }),
            ),
            (_, _, Some(error)) => {
                let _ = respond(&mut socket, "The connection was declined.").await;
                return Err(format!("Consent was declined: {error}"));
            }
            _ => ("Waiting for the sign-in to finish…", None),
        };

        let _ = respond(&mut socket, body).await;
        if let Some(result) = result {
            return Ok(result);
        }
    }

    async fn respond(
        socket: &mut tokio::net::TcpStream,
        message: &str,
    ) -> Result<(), std::io::Error> {
        use tokio::io::AsyncWriteExt;
        let body = format!(
            "<!doctype html><meta charset=\"utf-8\"><title>Juno</title>\
             <body style=\"font-family: -apple-system, sans-serif; display: grid; \
             place-items: center; height: 100vh; margin: 0;\"><p>{message}</p></body>"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await?;
        socket.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_the_rfc_7636_s256_of_the_verifier() {
        // The worked example from RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );

        let generated = pkce_verifier();
        assert_eq!(generated.len(), 64);
        assert!(generated.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn the_resource_metadata_url_is_read_from_www_authenticate() {
        let header = "Bearer error=\"unauthorized\", \
             resource_metadata=\"https://connect.composio.dev/.well-known/oauth-protected-resource\"";
        assert_eq!(
            extract_resource_metadata_url(header).as_deref(),
            Some("https://connect.composio.dev/.well-known/oauth-protected-resource")
        );
        assert_eq!(extract_resource_metadata_url("Bearer error=\"x\""), None);
    }

    #[test]
    fn token_responses_fold_into_stored_state() {
        let stored = StoredOAuth {
            client_id: "c".into(),
            token_endpoint: "https://login.example/oauth2/token".into(),
            resource: "https://example/mcp".into(),
            access_token: "old".into(),
            refresh_token: Some("keep-me".into()),
            expires_at: 0,
        };

        // Rotation is optional: no refresh_token in the response keeps the
        // old one, so a refresh can never strand the connection.
        let updated = apply_token_response(
            stored.clone(),
            &serde_json::json!({"access_token": "new", "expires_in": 120}),
        )
        .expect("folds");
        assert_eq!(updated.access_token, "new");
        assert_eq!(updated.refresh_token.as_deref(), Some("keep-me"));
        assert!(updated.expires_at > now_epoch_secs());

        let rotated = apply_token_response(
            stored,
            &serde_json::json!({"access_token": "new", "refresh_token": "next"}),
        )
        .expect("folds");
        assert_eq!(rotated.refresh_token.as_deref(), Some("next"));

        // No access token is an error, not a silently emptied credential.
        assert!(
            apply_token_response(rotated, &serde_json::json!({"token_type": "Bearer"})).is_err()
        );
    }

    #[test]
    fn origins_drop_paths_and_keep_ports() {
        assert_eq!(
            url_origin("https://connect.composio.dev/mcp").as_deref(),
            Ok("https://connect.composio.dev")
        );
        assert_eq!(
            url_origin("http://127.0.0.1:8123/x/y").as_deref(),
            Ok("http://127.0.0.1:8123")
        );
        assert!(url_origin("not a url").is_err());
    }
}

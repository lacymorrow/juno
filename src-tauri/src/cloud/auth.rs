use super::config::CloudConfig;
use super::types::{AuthResponse, CloudError, DeviceRegistration};
use crate::utils::current_timestamp_secs;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable, non-reversing identifier for an API key: the first 8 bytes of
/// SHA-256(api_key), hex-encoded. Lets the server select which key to verify
/// an auth proof with, without the payload ever disclosing the key itself
/// (security audit 2026-02-08, item #7).
pub fn api_key_id(api_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(api_key.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// Canonical string covered by the WebSocket auth proof signature.
/// Versioned so the wire format can evolve without ambiguity.
pub fn auth_proof_data(device_id: &str, nonce: &str, timestamp: u64) -> String {
    format!("juno-auth-v2|{}|{}|{}", device_id, nonce, timestamp)
}

/// Device authentication credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCredentials {
    pub device_id: String,
    pub api_key: String,
    pub token: Option<String>,
    pub expires_at: Option<u64>,
}

/// Device authentication handler
#[derive(Debug, Clone)]
pub struct DeviceAuth {
    credentials: Option<CloudCredentials>,
    config: CloudConfig,
}

impl DeviceAuth {
    /// Create new device authentication instance
    pub fn new(config: CloudConfig) -> Self {
        Self {
            credentials: None,
            config,
        }
    }

    /// Generate a new device ID
    pub fn generate_device_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Create device registration payload
    pub fn create_registration(&self) -> Result<DeviceRegistration, CloudError> {
        let device_id = self
            .config
            .device_id
            .clone()
            .unwrap_or_else(Self::generate_device_id);

        let api_key =
            self.config.api_key.clone().ok_or_else(|| {
                CloudError::AuthenticationFailed("No API key configured".to_string())
            })?;

        let capabilities = self.get_device_capabilities();

        Ok(DeviceRegistration {
            device_id,
            device_name: self.config.device_name.clone(),
            api_key,
            platform: self.get_platform(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities,
            user_id: None, // Will be set by cloud platform
        })
    }

    /// Set credentials after successful authentication
    pub fn set_credentials(&mut self, credentials: CloudCredentials) {
        self.credentials = Some(credentials);
    }

    /// Get current credentials
    pub fn get_credentials(&self) -> Option<&CloudCredentials> {
        self.credentials.as_ref()
    }

    /// Check if authentication is valid
    pub fn is_authenticated(&self) -> bool {
        if let Some(creds) = &self.credentials {
            if let Some(expires_at) = creds.expires_at {
                let now = current_timestamp_secs();
                return now < expires_at;
            }
            return true; // No expiration set, assume valid
        }
        false
    }

    /// Get authentication token for requests
    pub fn get_auth_token(&self) -> Option<String> {
        self.credentials
            .as_ref()
            .and_then(|creds| creds.token.clone())
    }

    /// Create authentication message for WebSocket.
    ///
    /// The payload never contains the raw API key (security audit
    /// 2026-02-08, item #7). Instead it carries a non-disclosing proof of
    /// possession: a `key_id` (one-way fingerprint of the key) plus an
    /// HMAC-SHA256 signature over the device id, a fresh connection nonce,
    /// and the timestamp, keyed by the API key. The server looks the key up
    /// by `key_id` and verifies the signature in constant time (the same
    /// scheme as [`Self::verify_signature`]).
    pub fn create_auth_message(&self) -> Result<serde_json::Value, CloudError> {
        let creds = self.credentials.as_ref().ok_or_else(|| {
            CloudError::AuthenticationFailed("No credentials available".to_string())
        })?;

        let timestamp = current_timestamp_secs();
        let nonce = Uuid::new_v4().to_string();
        let proof_data = auth_proof_data(&creds.device_id, &nonce, timestamp);
        let signature = self.create_signature(&proof_data)?;

        let auth_data = serde_json::json!({
            "device_id": creds.device_id,
            "key_id": api_key_id(&creds.api_key),
            "nonce": nonce,
            "signature": signature,
            "token": creds.token,
            "timestamp": timestamp,
            "platform": self.get_platform(),
            "version": env!("CARGO_PKG_VERSION")
        });

        Ok(auth_data)
    }

    /// Validate authentication response from cloud
    pub fn validate_auth_response(&mut self, response: AuthResponse) -> Result<(), CloudError> {
        if !response.success {
            return Err(CloudError::AuthenticationFailed(
                response
                    .error
                    .unwrap_or_else(|| "Authentication failed".to_string()),
            ));
        }

        let token = response
            .token
            .ok_or_else(|| CloudError::AuthenticationFailed("No token in response".to_string()))?;

        let device_id = response.device_id.ok_or_else(|| {
            CloudError::AuthenticationFailed("No device ID in response".to_string())
        })?;

        let api_key =
            self.config.api_key.clone().ok_or_else(|| {
                CloudError::AuthenticationFailed("No API key configured".to_string())
            })?;

        self.credentials = Some(CloudCredentials {
            device_id,
            api_key,
            token: Some(token),
            expires_at: response.expires_at,
        });

        Ok(())
    }

    /// Get device capabilities
    fn get_device_capabilities(&self) -> Vec<String> {
        let mut capabilities = vec![
            "text_processing".to_string(),
            "voice_transcription".to_string(),
            "screenshot_capture".to_string(),
            "system_automation".to_string(),
            "file_operations".to_string(),
            "web_browsing".to_string(),
        ];

        // Add platform-specific capabilities
        #[cfg(target_os = "macos")]
        {
            capabilities.extend_from_slice(&[
                "macos_automation".to_string(),
                "accessibility_api".to_string(),
                "applescript".to_string(),
            ]);
        }

        #[cfg(target_os = "windows")]
        {
            capabilities
                .extend_from_slice(&["windows_automation".to_string(), "win32_api".to_string()]);
        }

        #[cfg(target_os = "linux")]
        {
            capabilities
                .extend_from_slice(&["x11_automation".to_string(), "gtk_integration".to_string()]);
        }

        capabilities
    }

    /// Get current platform
    fn get_platform(&self) -> String {
        #[cfg(target_os = "macos")]
        return "macOS".to_string();

        #[cfg(target_os = "windows")]
        return "Windows".to_string();

        #[cfg(target_os = "linux")]
        return "Linux".to_string();

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return "Unknown".to_string();
    }

    /// Create signature for command validation
    pub fn create_signature(&self, data: &str) -> Result<String, CloudError> {
        let creds = self
            .credentials
            .as_ref()
            .ok_or_else(|| CloudError::SecurityError("No credentials for signing".to_string()))?;

        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(creds.api_key.as_bytes())
            .map_err(|e| CloudError::SecurityError(format!("Failed to create HMAC: {}", e)))?;

        mac.update(data.as_bytes());
        let result = mac.finalize();

        Ok(general_purpose::STANDARD.encode(result.into_bytes()))
    }

    /// Verify signature from cloud.
    /// Uses HMAC's built-in constant-time verification (via the `subtle` crate
    /// under the hood) so signature comparison cannot leak timing information.
    pub fn verify_signature(&self, data: &str, signature: &str) -> Result<bool, CloudError> {
        let creds = self
            .credentials
            .as_ref()
            .ok_or_else(|| CloudError::SecurityError("No credentials for signing".to_string()))?;

        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        // A signature that is not valid base64 can never match; treat as invalid.
        let signature_bytes = match general_purpose::STANDARD.decode(signature) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(false),
        };

        let mut mac = HmacSha256::new_from_slice(creds.api_key.as_bytes())
            .map_err(|e| CloudError::SecurityError(format!("Failed to create HMAC: {}", e)))?;
        mac.update(data.as_bytes());

        Ok(mac.verify_slice(&signature_bytes).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_with_credentials(api_key: &str) -> DeviceAuth {
        let mut auth = DeviceAuth::new(CloudConfig::default());
        auth.set_credentials(CloudCredentials {
            device_id: "test-device".to_string(),
            api_key: api_key.to_string(),
            token: None,
            expires_at: None,
        });
        auth
    }

    #[test]
    fn verify_signature_accepts_valid_signature() {
        let auth = auth_with_credentials("test-key");
        let data = "payload with unicode: héllo 🚀";
        let signature = auth.create_signature(data).expect("signing should work");
        assert!(auth
            .verify_signature(data, &signature)
            .expect("verify should not error"));
    }

    #[test]
    fn verify_signature_rejects_tampered_data() {
        let auth = auth_with_credentials("test-key");
        let signature = auth
            .create_signature("original data")
            .expect("signing should work");
        assert!(!auth
            .verify_signature("tampered data", &signature)
            .expect("verify should not error"));
    }

    #[test]
    fn verify_signature_rejects_wrong_key() {
        let signer = auth_with_credentials("key-a");
        let verifier = auth_with_credentials("key-b");
        let data = "payload";
        let signature = signer.create_signature(data).expect("signing should work");
        assert!(!verifier
            .verify_signature(data, &signature)
            .expect("verify should not error"));
    }

    #[test]
    fn verify_signature_rejects_invalid_base64() {
        let auth = auth_with_credentials("test-key");
        assert!(!auth
            .verify_signature("payload", "not-valid-base64!!!")
            .expect("verify should not error"));
    }

    // --- Auth payload carries an HMAC proof, never the raw key (#7) ---

    #[test]
    fn auth_message_does_not_contain_api_key() {
        let auth = auth_with_credentials("super-secret-api-key");
        let message = auth
            .create_auth_message()
            .expect("auth message should build");

        assert!(message.get("api_key").is_none(), "raw api_key field found");
        let serialized = message.to_string();
        assert!(
            !serialized.contains("super-secret-api-key"),
            "raw api key leaked into the auth payload"
        );
    }

    #[test]
    fn auth_message_proof_verifies_with_the_key() {
        let auth = auth_with_credentials("test-key");
        let message = auth
            .create_auth_message()
            .expect("auth message should build");

        let device_id = message["device_id"].as_str().expect("device_id present");
        let nonce = message["nonce"].as_str().expect("nonce present");
        let timestamp = message["timestamp"].as_u64().expect("timestamp present");
        let signature = message["signature"].as_str().expect("signature present");

        let proof_data = auth_proof_data(device_id, nonce, timestamp);
        assert!(auth
            .verify_signature(&proof_data, signature)
            .expect("verify should not error"));

        // A tampered nonce must not verify
        let tampered = auth_proof_data(device_id, "other-nonce", timestamp);
        assert!(!auth
            .verify_signature(&tampered, signature)
            .expect("verify should not error"));

        // A different key must not verify
        let other = auth_with_credentials("other-key");
        assert!(!other
            .verify_signature(&proof_data, signature)
            .expect("verify should not error"));
    }

    #[test]
    fn api_key_id_is_stable_and_non_disclosing() {
        let id_a = api_key_id("secret-key-a");
        assert_eq!(id_a, api_key_id("secret-key-a"), "key id must be stable");
        assert_ne!(id_a, api_key_id("secret-key-b"));
        assert_eq!(id_a.len(), 16, "8 bytes hex-encoded");
        assert!(!id_a.contains("secret-key-a"));
    }

    #[test]
    fn auth_message_nonce_is_fresh_per_message() {
        let auth = auth_with_credentials("test-key");
        let first = auth.create_auth_message().expect("first message");
        let second = auth.create_auth_message().expect("second message");
        assert_ne!(first["nonce"], second["nonce"]);
    }

    // --- validate_auth_response enforces success + token + device id (#24) ---

    fn auth_response(success: bool, token: Option<&str>, device_id: Option<&str>) -> AuthResponse {
        AuthResponse {
            success,
            token: token.map(str::to_string),
            device_id: device_id.map(str::to_string),
            permissions: None,
            expires_at: None,
            error: None,
        }
    }

    fn auth_with_config_key(api_key: &str) -> DeviceAuth {
        let config = CloudConfig {
            api_key: Some(api_key.to_string()),
            ..CloudConfig::default()
        };
        DeviceAuth::new(config)
    }

    #[test]
    fn validate_auth_response_accepts_valid_response() {
        let mut auth = auth_with_config_key("test-key");
        let response = auth_response(true, Some("session-token"), Some("device-1"));
        assert!(auth.validate_auth_response(response).is_ok());

        let creds = auth.get_credentials().expect("credentials stored");
        assert_eq!(creds.device_id, "device-1");
        assert_eq!(creds.token.as_deref(), Some("session-token"));
        assert!(auth.is_authenticated());
    }

    #[test]
    fn validate_auth_response_rejects_failure() {
        let mut auth = auth_with_config_key("test-key");
        let response = auth_response(false, Some("token"), Some("device-1"));
        assert!(auth.validate_auth_response(response).is_err());
        assert!(auth.get_credentials().is_none());
    }

    #[test]
    fn validate_auth_response_rejects_missing_token_or_device_id() {
        let mut auth = auth_with_config_key("test-key");
        assert!(auth
            .validate_auth_response(auth_response(true, None, Some("device-1")))
            .is_err());
        assert!(auth
            .validate_auth_response(auth_response(true, Some("token"), None))
            .is_err());
        assert!(auth.get_credentials().is_none());
    }
}

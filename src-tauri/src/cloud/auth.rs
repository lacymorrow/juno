use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use super::types::{CloudError, DeviceRegistration, AuthResponse};
use super::config::CloudConfig;

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
        let device_id = self.config.device_id
            .clone()
            .unwrap_or_else(|| Self::generate_device_id());
        
        let api_key = self.config.api_key
            .clone()
            .ok_or_else(|| CloudError::AuthenticationFailed("No API key configured".to_string()))?;
        
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
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
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
    
    /// Create authentication message for WebSocket
    pub fn create_auth_message(&self) -> Result<serde_json::Value, CloudError> {
        let creds = self.credentials
            .as_ref()
            .ok_or_else(|| CloudError::AuthenticationFailed("No credentials available".to_string()))?;
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let auth_data = serde_json::json!({
            "device_id": creds.device_id,
            "api_key": creds.api_key,
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
                response.error.unwrap_or_else(|| "Authentication failed".to_string())
            ));
        }
        
        let token = response.token
            .ok_or_else(|| CloudError::AuthenticationFailed("No token in response".to_string()))?;
        
        let device_id = response.device_id
            .ok_or_else(|| CloudError::AuthenticationFailed("No device ID in response".to_string()))?;
        
        let api_key = self.config.api_key
            .clone()
            .ok_or_else(|| CloudError::AuthenticationFailed("No API key configured".to_string()))?;
        
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
            capabilities.extend_from_slice(&[
                "windows_automation".to_string(),
                "win32_api".to_string(),
            ]);
        }
        
        #[cfg(target_os = "linux")]
        {
            capabilities.extend_from_slice(&[
                "x11_automation".to_string(),
                "gtk_integration".to_string(),
            ]);
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
        let creds = self.credentials
            .as_ref()
            .ok_or_else(|| CloudError::SecurityError("No credentials for signing".to_string()))?;
        
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        
        let mut mac = HmacSha256::new_from_slice(creds.api_key.as_bytes())
            .map_err(|e| CloudError::SecurityError(format!("Failed to create HMAC: {}", e)))?;
        
        mac.update(data.as_bytes());
        let result = mac.finalize();
        
        Ok(base64::encode(result.into_bytes()))
    }
    
    /// Verify signature from cloud
    pub fn verify_signature(&self, data: &str, signature: &str) -> Result<bool, CloudError> {
        let expected_signature = self.create_signature(data)?;
        Ok(expected_signature == signature)
    }
}
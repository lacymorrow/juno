use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use super::types::{CloudError, CloudCommand, CloudCommandType};
use super::config::{CloudConfig, SecurityLevel};
use super::auth::DeviceAuth;

/// Security levels for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationSecurity {
    Safe,      // Always allow
    Sensitive, // Require confirmation in medium/high security
    Dangerous, // Only allow in low security or with explicit permission
    Forbidden, // Never allow
}

/// Cloud security handler
#[derive(Debug, Clone)]
pub struct CloudSecurity {
    config: CloudConfig,
    auth: DeviceAuth,
}

impl CloudSecurity {
    /// Create new security handler
    pub fn new(config: CloudConfig, auth: DeviceAuth) -> Self {
        Self { config, auth }
    }
    
    /// Validate incoming command
    pub fn validate_command(&self, command: &CloudCommand) -> Result<(), CloudError> {
        // Check timestamp to prevent replay attacks
        self.validate_timestamp(command.timestamp)?;
        
        // Verify command signature if present
        if let Some(signature) = &command.signature {
            let command_data = serde_json::to_string(command)?;
            if !self.auth.verify_signature(&command_data, signature)? {
                return Err(CloudError::SecurityError("Invalid command signature".to_string()));
            }
        }
        
        // Check if command type is allowed
        self.validate_command_type(&command.command_type)?;
        
        // Validate specific command parameters
        self.validate_command_payload(command)?;
        
        Ok(())
    }
    
    /// Validate command timestamp
    fn validate_timestamp(&self, timestamp: u64) -> Result<(), CloudError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let time_diff = if now > timestamp {
            now - timestamp
        } else {
            timestamp - now
        };
        
        // Allow 5 minutes of clock skew
        if time_diff > 300 {
            return Err(CloudError::SecurityError("Command timestamp too old or too far in future".to_string()));
        }
        
        Ok(())
    }
    
    /// Validate command type
    fn validate_command_type(&self, command_type: &CloudCommandType) -> Result<(), CloudError> {
        let command_str = self.command_type_to_string(command_type);
        
        if !self.config.is_command_allowed(&command_str) {
            return Err(CloudError::SecurityError(format!("Command type '{}' is not allowed", command_str)));
        }
        
        let security_level = self.get_command_security_level(command_type);
        
        match (&self.config.security_level, &security_level) {
            (SecurityLevel::High, OperationSecurity::Dangerous) => {
                return Err(CloudError::SecurityError("Dangerous commands not allowed in high security mode".to_string()));
            },
            (SecurityLevel::High, OperationSecurity::Forbidden) => {
                return Err(CloudError::SecurityError("Forbidden command".to_string()));
            },
            (SecurityLevel::Medium, OperationSecurity::Forbidden) => {
                return Err(CloudError::SecurityError("Forbidden command".to_string()));
            },
            (_, OperationSecurity::Forbidden) => {
                return Err(CloudError::SecurityError("Forbidden command".to_string()));
            },
            _ => Ok(()),
        }
    }
    
    /// Validate command payload
    fn validate_command_payload(&self, command: &CloudCommand) -> Result<(), CloudError> {
        match command.command_type {
            CloudCommandType::VoiceQuery | CloudCommandType::TextQuery => {
                if command.payload.query.is_none() && command.payload.audio_base64.is_none() {
                    return Err(CloudError::ValidationFailed("Query commands require either text or audio".to_string()));
                }
                
                // Validate query length
                if let Some(query) = &command.payload.query {
                    if query.len() > 10000 {
                        return Err(CloudError::ValidationFailed("Query text too long".to_string()));
                    }
                }
                
                // Validate audio data
                if let Some(audio) = &command.payload.audio_base64 {
                    if audio.len() > 10_000_000 { // ~7.5MB base64 encoded
                        return Err(CloudError::ValidationFailed("Audio data too large".to_string()));
                    }
                }
            },
            CloudCommandType::SystemCommand => {
                // System commands require additional validation
                if let Some(params) = &command.payload.parameters {
                    if params.contains_key("destructive") {
                        return Err(CloudError::SecurityError("Destructive system commands not allowed".to_string()));
                    }
                }
            },
            CloudCommandType::ConfigUpdate => {
                // Configuration updates need special handling
                if command.payload.config.is_none() {
                    return Err(CloudError::ValidationFailed("Config update requires config data".to_string()));
                }
            },
            _ => {
                // Other commands are generally safe
            }
        }
        
        Ok(())
    }
    
    /// Get security level for command type
    fn get_command_security_level(&self, command_type: &CloudCommandType) -> OperationSecurity {
        match command_type {
            CloudCommandType::VoiceQuery => OperationSecurity::Safe,
            CloudCommandType::TextQuery => OperationSecurity::Safe,
            CloudCommandType::StatusRequest => OperationSecurity::Safe,
            CloudCommandType::Screenshot => OperationSecurity::Sensitive,
            CloudCommandType::SystemCommand => OperationSecurity::Dangerous,
            CloudCommandType::ConfigUpdate => OperationSecurity::Sensitive,
        }
    }
    
    /// Convert command type to string
    fn command_type_to_string(&self, command_type: &CloudCommandType) -> String {
        match command_type {
            CloudCommandType::VoiceQuery => "voice_query".to_string(),
            CloudCommandType::TextQuery => "text_query".to_string(),
            CloudCommandType::SystemCommand => "system_command".to_string(),
            CloudCommandType::StatusRequest => "status_request".to_string(),
            CloudCommandType::Screenshot => "screenshot".to_string(),
            CloudCommandType::ConfigUpdate => "config_update".to_string(),
        }
    }
    
    /// Check if command requires user confirmation
    pub fn requires_confirmation(&self, command: &CloudCommand) -> bool {
        let security_level = self.get_command_security_level(&command.command_type);
        
        match (&self.config.security_level, &security_level) {
            (SecurityLevel::Medium, OperationSecurity::Sensitive) => true,
            (SecurityLevel::Medium, OperationSecurity::Dangerous) => true,
            (SecurityLevel::High, OperationSecurity::Sensitive) => true,
            _ => false,
        }
    }
    
    /// Sanitize command payload for logging
    pub fn sanitize_for_logging(&self, command: &CloudCommand) -> CloudCommand {
        let mut sanitized = command.clone();
        
        // Remove sensitive data from logs
        if let Some(_) = &sanitized.payload.audio_base64 {
            sanitized.payload.audio_base64 = Some("[AUDIO_DATA_REDACTED]".to_string());
        }
        
        // Truncate long queries
        if let Some(query) = &sanitized.payload.query {
            if query.len() > 200 {
                sanitized.payload.query = Some(format!("{}...[TRUNCATED]", &query[..200]));
            }
        }
        
        // Remove signature for logging
        sanitized.signature = Some("[SIGNATURE_REDACTED]".to_string());
        
        sanitized
    }
    
    /// Create audit log entry
    pub fn create_audit_log(&self, command: &CloudCommand, result: &Result<(), CloudError>) -> AuditLogEntry {
        AuditLogEntry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            command_id: command.id.clone(),
            command_type: self.command_type_to_string(&command.command_type),
            device_id: self.auth.get_credentials()
                .map(|c| c.device_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            success: result.is_ok(),
            error_message: result.as_ref().err().map(|e| e.to_string()),
            security_level: format!("{:?}", self.config.security_level),
        }
    }
    
    /// Rate limiting check
    pub fn check_rate_limit(&self, _command_type: &CloudCommandType) -> Result<(), CloudError> {
        // TODO: Implement rate limiting based on command type and time window
        // For now, always allow
        Ok(())
    }
}

/// Audit log entry for security monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: u64,
    pub command_id: String,
    pub command_type: String,
    pub device_id: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub security_level: String,
}

/// Security policy for specific commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub command_type: String,
    pub security_level: OperationSecurity,
    pub requires_confirmation: bool,
    pub rate_limit: Option<RateLimit>,
    pub additional_checks: Vec<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub max_requests: u32,
    pub time_window_seconds: u64,
    pub burst_allowance: Option<u32>,
}
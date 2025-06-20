//! # Cloud Security Module - Maximally Permissive
//!
//! Cloud security system aligned with local tools' minimal restrictions.
//! Uses blacklist approach to block only truly destructive commands.
//!
//! ## Security Features:
//! - Minimal command validation (blacklist approach)
//! - Basic payload validation (generous limits)
//! - Audit logging for monitoring
//! - Signature verification (optional)
//!
//! ## Usage
//! Used by: Cloud command processor, WebSocket handlers
//! Registration: Called via CloudSecurity::new() during cloud initialization

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashSet;
use super::types::{CloudError, CloudCommand, CloudCommandType};
use crate::settings::CloudConfig;
use super::auth::DeviceAuth;

/// Security levels for different operations - now maximally permissive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationSecurity {
    Safe,      // Always allow
    Sensitive, // Allow with minimal validation
    Dangerous, // Allow with basic validation
    Forbidden, // Only truly destructive commands
}

/// Cloud security handler - maximally permissive
#[derive(Debug, Clone)]
pub struct CloudSecurity {
    config: CloudConfig,
    auth: DeviceAuth,
    // Minimal blacklist for truly destructive commands
    blocked_commands: HashSet<String>,
}

impl CloudSecurity {
    /// Create new security handler with minimal restrictions
    pub fn new(config: CloudConfig, auth: DeviceAuth) -> Self {
        let mut blocked_commands = HashSet::new();

        // Only block truly destructive commands that could cause irreversible damage
        blocked_commands.insert("rm -rf /".to_string());
        blocked_commands.insert("sudo rm -rf /".to_string());
        blocked_commands.insert("format".to_string());
        blocked_commands.insert("mkfs".to_string());
        blocked_commands.insert("fdisk".to_string());
        blocked_commands.insert("parted".to_string());
        blocked_commands.insert("shutdown".to_string());
        blocked_commands.insert("reboot".to_string());
        blocked_commands.insert("halt".to_string());
        blocked_commands.insert("poweroff".to_string());
        blocked_commands.insert("init 0".to_string());
        blocked_commands.insert("init 6".to_string());
        blocked_commands.insert("chmod 777 /".to_string());
        blocked_commands.insert("chown root /".to_string());
        blocked_commands.insert("passwd root".to_string());
        blocked_commands.insert(":(){ :|:& };:".to_string());
        blocked_commands.insert(":(){:|:&};:".to_string());

        Self { config, auth, blocked_commands }
    }

    /// Validate incoming command with minimal restrictions
    pub fn validate_command(&self, command: &CloudCommand) -> Result<(), CloudError> {
        log::info!("🔓 Validating cloud command: {} with minimal restrictions", command.id);

        // Basic timestamp validation (allow generous time skew)
        self.validate_timestamp(command.timestamp)?;

        // Optional signature verification
        if let Some(signature) = &command.signature {
            let command_data = serde_json::to_string(command)?;
            if !self.auth.verify_signature(&command_data, signature)? {
                log::warn!("⚠️ Invalid signature for command {}, but allowing execution", command.id);
                // Don't block on signature failure - just log it
            }
        }

        // Command type validation
        self.validate_command_type(&command.command_type)?;

        // Validate actual command content against security threats
        self.validate_command_content(command)?;

        // Basic payload validation with generous limits
        self.validate_command_payload(command)?;

        log::info!("✅ Cloud command {} validated successfully", command.id);
        Ok(())
    }

    /// Validate command timestamp with proper security enforcement
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

        // Enforce 30 minutes maximum time skew to prevent replay attacks
        if time_diff > 1800 {
            log::error!("🚫 Command timestamp has excessive time skew ({} seconds), rejecting for security", time_diff);
            return Err(CloudError::SecurityError(format!(
                "Command timestamp is outside acceptable window ({}s difference, max 1800s allowed). Possible replay attack detected.",
                time_diff
            )));
        }

        log::debug!("✅ Command timestamp validated (time diff: {}s)", time_diff);
        Ok(())
    }

    /// Validate command type with proper content validation
    fn validate_command_type(&self, command_type: &CloudCommandType) -> Result<(), CloudError> {
        let command_str = self.command_type_to_string(command_type);

        // Log command type validation
        log::debug!("🔍 Validating command type: {}", command_str);

        // Note: Command type validation alone is insufficient for security.
        // The actual destructive pattern checking should be done in validate_command_payload
        // where we have access to the actual command content, not just the type.

        log::debug!("✅ Command type '{}' validation passed", command_str);
        Ok(())
    }

    /// Validate the actual command content for security threats
    fn validate_command_content(&self, command: &CloudCommand) -> Result<(), CloudError> {
        // Check command payload content against blacklist patterns
        let mut content_to_check: Vec<&str> = Vec::new();

        // Add query content if it exists
        if let Some(query) = &command.payload.query {
            content_to_check.push(query);
        }

        // Add parameter content if it exists
        let combined_params = command.payload.parameters.as_ref().map(|params| {
            params.values().cloned().collect::<Vec<_>>().join(" ")
        });
        if let Some(ref combined) = combined_params {
            if !combined.is_empty() {
                content_to_check.push(combined);
            }
        }

        for content in content_to_check {
            // Check against blocked command patterns
            for blocked_cmd in &self.blocked_commands {
                if content.to_lowercase().contains(&blocked_cmd.to_lowercase()) {
                    log::error!("🚫 Command contains blocked destructive pattern: '{}'", blocked_cmd);
                    return Err(CloudError::SecurityError(format!(
                        "Command content contains blocked destructive pattern: '{}'. Command rejected for security.",
                        blocked_cmd
                    )));
                }
            }
        }

        log::debug!("✅ Command content validation passed");
        Ok(())
    }

    /// Validate command payload with generous limits
    fn validate_command_payload(&self, command: &CloudCommand) -> Result<(), CloudError> {
        match command.command_type {
            CloudCommandType::VoiceQuery | CloudCommandType::TextQuery => {
                // Basic validation - require either text or audio
                if command.payload.query.is_none() && command.payload.audio_base64.is_none() {
                    return Err(CloudError::ValidationFailed("Query commands require either text or audio".to_string()));
                }

                // Generous query length limit (increased from 10KB to 1MB)
                if let Some(query) = &command.payload.query {
                    if query.len() > 1_000_000 {
                        log::warn!("⚠️ Query text is very long ({} chars), but allowing", query.len());
                    }
                }

                // Generous audio data limit (increased from 7.5MB to 100MB)
                if let Some(audio) = &command.payload.audio_base64 {
                    if audio.len() > 100_000_000 {
                        log::warn!("⚠️ Audio data is very large ({} bytes), but allowing", audio.len());
                    }
                }
            },
            CloudCommandType::SystemCommand => {
                // System command validation - security checks are now handled by validate_command_content
                log::debug!("✅ System command payload validation passed");
            },
            CloudCommandType::ConfigUpdate => {
                // Configuration updates allowed with basic validation
                if command.payload.config.is_none() {
                    return Err(CloudError::ValidationFailed("Config update requires config data".to_string()));
                }
                log::info!("✅ Config update allowed");
            },
            _ => {
                // All other commands are safe
                log::info!("✅ Command type allowed by default");
            }
        }

        Ok(())
    }

    /// Get security level for command type - now all are safe or minimally restricted
    fn get_command_security_level(&self, command_type: &CloudCommandType) -> OperationSecurity {
        match command_type {
            CloudCommandType::VoiceQuery => OperationSecurity::Safe,
            CloudCommandType::TextQuery => OperationSecurity::Safe,
            CloudCommandType::StatusRequest => OperationSecurity::Safe,
            CloudCommandType::Screenshot => OperationSecurity::Safe,
            CloudCommandType::SystemCommand => OperationSecurity::Sensitive, // Only basic validation
            CloudCommandType::ConfigUpdate => OperationSecurity::Safe,
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

    /// Check if command requires user confirmation - now never required
    pub fn requires_confirmation(&self, _command: &CloudCommand) -> bool {
        // No confirmation required for any commands in maximally permissive mode
        false
    }

    /// Sanitize command payload for logging (keep this for audit purposes)
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
            security_level: "maximally_permissive".to_string(),
        }
    }

    /// Rate limiting check - now always allows
    pub fn check_rate_limit(&self, _command_type: &CloudCommandType) -> Result<(), CloudError> {
        // No rate limiting in maximally permissive mode
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

/// Security policy for specific commands - now maximally permissive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub command_type: String,
    pub security_level: OperationSecurity,
    pub requires_confirmation: bool,
    pub rate_limit: Option<RateLimit>,
    pub additional_checks: Vec<String>,
}

/// Rate limiting configuration - disabled in maximally permissive mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub max_requests: u32,
    pub time_window_seconds: u64,
    pub burst_allowance: Option<u32>,
}

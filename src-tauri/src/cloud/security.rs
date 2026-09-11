//! # Cloud Security Module
//!
//! Cloud security system for remote commands. Fails closed: invalid
//! signatures, excessive timestamp skew, and rate-limit violations all
//! reject the command (2026-09 security audit; formerly warn-and-continue).
//!
//! ## Security Features:
//! - Command content validation (destructive-pattern blacklist)
//! - Fail-closed signature verification (constant-time HMAC)
//! - Fail-closed timestamp skew validation
//! - Per-command-type token-bucket rate limiting
//! - Confirmation required for command-execution categories
//! - Audit logging for monitoring
//!
//! ## Usage
//! Used by: Cloud command processor, WebSocket handlers
//! Registration: Called via CloudSecurity::new() during cloud initialization

use super::auth::DeviceAuth;
use super::config::CloudConfig;
use super::types::{CloudCommand, CloudCommandType, CloudError};
use crate::utils::current_timestamp_secs;
use crate::utils::rate_limiter::{RateLimitConfig, RateLimiter};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// Maximum allowed clock skew between the cloud and this device, in seconds.
const MAX_TIMESTAMP_SKEW_SECONDS: u64 = 3600;

/// Cloud commands allowed per minute, per command type.
const CLOUD_COMMANDS_PER_MINUTE: u32 = 30;

/// Security levels for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationSecurity {
    Safe,      // Always allow
    Sensitive, // Allow with minimal validation
    Dangerous, // Allow with basic validation
    Forbidden, // Only truly destructive commands
}

/// Cloud security handler
#[derive(Debug, Clone)]
pub struct CloudSecurity {
    #[allow(dead_code)]
    config: CloudConfig,
    auth: DeviceAuth,
    // Minimal blacklist for truly destructive commands
    blocked_commands: HashSet<String>,
    // Token-bucket rate limiter, keyed per command type (Arc so Clone shares one bucket set)
    rate_limiter: Arc<RateLimiter>,
}

impl CloudSecurity {
    /// Create new security handler
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

        Self {
            config,
            auth,
            blocked_commands,
            rate_limiter: Arc::new(RateLimiter::new(RateLimitConfig::per_minute(
                CLOUD_COMMANDS_PER_MINUTE,
            ))),
        }
    }

    /// Validate incoming command. Fails closed on any check.
    pub fn validate_command(&self, command: &CloudCommand) -> Result<(), CloudError> {
        log::info!("🔒 Validating cloud command: {}", command.id);

        // Timestamp validation (reject excessive skew)
        self.validate_timestamp(command.timestamp)?;

        // Signature verification: an invalid signature rejects the command
        if let Some(signature) = &command.signature {
            let command_data = serde_json::to_string(command)?;
            if !self.auth.verify_signature(&command_data, signature)? {
                log::error!("🚫 Invalid signature for command {}, rejecting", command.id);
                return Err(CloudError::SecurityError(format!(
                    "Invalid signature for command {}",
                    command.id
                )));
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

    /// Validate command timestamp; reject skew beyond the allowed window
    fn validate_timestamp(&self, timestamp: u64) -> Result<(), CloudError> {
        let now = current_timestamp_secs();

        let time_diff = now.abs_diff(timestamp);

        if time_diff > MAX_TIMESTAMP_SKEW_SECONDS {
            log::error!(
                "🚫 Command timestamp skew of {}s exceeds the {}s limit, rejecting",
                time_diff,
                MAX_TIMESTAMP_SKEW_SECONDS
            );
            return Err(CloudError::ValidationFailed(format!(
                "Command timestamp skew of {}s exceeds the {}s limit",
                time_diff, MAX_TIMESTAMP_SKEW_SECONDS
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
        let combined_params = command
            .payload
            .parameters
            .as_ref()
            .map(|params| params.values().cloned().collect::<Vec<_>>().join(" "));
        if let Some(ref combined) = combined_params {
            if !combined.is_empty() {
                content_to_check.push(combined);
            }
        }

        for content in content_to_check {
            // Check against blocked command patterns
            for blocked_cmd in &self.blocked_commands {
                if content.to_lowercase().contains(&blocked_cmd.to_lowercase()) {
                    log::error!(
                        "🚫 Command contains blocked destructive pattern: '{}'",
                        blocked_cmd
                    );
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
                    return Err(CloudError::ValidationFailed(
                        "Query commands require either text or audio".to_string(),
                    ));
                }

                // Generous query length limit (increased from 10KB to 1MB)
                if let Some(query) = &command.payload.query {
                    if query.len() > 1_000_000 {
                        log::warn!(
                            "⚠️ Query text is very long ({} chars), but allowing",
                            query.len()
                        );
                    }
                }

                // Generous audio data limit (increased from 7.5MB to 100MB)
                if let Some(audio) = &command.payload.audio_base64 {
                    if audio.len() > 100_000_000 {
                        log::warn!(
                            "⚠️ Audio data is very large ({} bytes), but allowing",
                            audio.len()
                        );
                    }
                }
            }
            CloudCommandType::SystemCommand => {
                // System command validation - security checks are now handled by validate_command_content
                log::debug!("✅ System command payload validation passed");
            }
            CloudCommandType::ConfigUpdate => {
                // Configuration updates allowed with basic validation
                if command.payload.config.is_none() {
                    return Err(CloudError::ValidationFailed(
                        "Config update requires config data".to_string(),
                    ));
                }
                log::info!("✅ Config update allowed");
            }
            _ => {
                // All other commands are safe
                log::info!("✅ Command type allowed by default");
            }
        }

        Ok(())
    }

    /// Get security level for command type - now all are safe or minimally restricted
    #[allow(dead_code)]
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

    /// Check if command requires user confirmation.
    /// Command-execution categories (arbitrary system commands, config changes)
    /// always require confirmation; read-only queries do not.
    pub fn requires_confirmation(&self, command: &CloudCommand) -> bool {
        matches!(
            command.command_type,
            CloudCommandType::SystemCommand | CloudCommandType::ConfigUpdate
        )
    }

    /// Sanitize command payload for logging (keep this for audit purposes)
    pub fn sanitize_for_logging(&self, command: &CloudCommand) -> CloudCommand {
        let mut sanitized = command.clone();

        // Remove sensitive data from logs
        if sanitized.payload.audio_base64.is_some() {
            sanitized.payload.audio_base64 = Some("[AUDIO_DATA_REDACTED]".to_string());
        }

        // Truncate long queries (char-boundary safe: byte slicing panics on multi-byte UTF-8)
        if let Some(query) = &sanitized.payload.query {
            if query.chars().count() > 200 {
                let truncated: String = query.chars().take(200).collect();
                sanitized.payload.query = Some(format!("{}...[TRUNCATED]", truncated));
            }
        }

        // Remove signature for logging
        sanitized.signature = Some("[SIGNATURE_REDACTED]".to_string());

        sanitized
    }

    /// Create audit log entry
    pub fn create_audit_log(
        &self,
        command: &CloudCommand,
        result: &Result<(), CloudError>,
    ) -> AuditLogEntry {
        AuditLogEntry {
            timestamp: current_timestamp_secs(),
            command_id: command.id.clone(),
            command_type: self.command_type_to_string(&command.command_type),
            device_id: self
                .auth
                .get_credentials()
                .map(|c| c.device_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            success: result.is_ok(),
            error_message: result.as_ref().err().map(|e| e.to_string()),
            security_level: format!("{:?}", self.config.security_level).to_lowercase(),
        }
    }

    /// Rate limiting check: token bucket per command type, fails closed when exhausted
    pub async fn check_rate_limit(
        &self,
        command_type: &CloudCommandType,
    ) -> Result<(), CloudError> {
        let key = self.command_type_to_string(command_type);
        self.rate_limiter.check(&key).await.map_err(|e| {
            log::error!("🚫 Rate limit exceeded for cloud command type '{}'", key);
            CloudError::SecurityError(e.to_user_message())
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::auth::CloudCredentials;
    use crate::cloud::types::CloudCommandPayload;

    fn make_security() -> CloudSecurity {
        let config = CloudConfig::default();
        let mut auth = DeviceAuth::new(config.clone());
        auth.set_credentials(CloudCredentials {
            device_id: "test-device".to_string(),
            api_key: "test-key".to_string(),
            token: None,
            expires_at: None,
        });
        CloudSecurity::new(config, auth)
    }

    fn make_command(command_type: CloudCommandType, query: Option<String>) -> CloudCommand {
        CloudCommand {
            id: "cmd-1".to_string(),
            command_type,
            payload: CloudCommandPayload {
                query,
                audio_base64: None,
                mode: None,
                config: None,
                parameters: None,
            },
            timestamp: current_timestamp_secs(),
            signature: None,
            metadata: None,
        }
    }

    #[test]
    fn timestamp_skew_over_limit_is_rejected() {
        let security = make_security();
        let stale = current_timestamp_secs().saturating_sub(MAX_TIMESTAMP_SKEW_SECONDS + 100);
        assert!(security.validate_timestamp(stale).is_err());

        let future = current_timestamp_secs() + MAX_TIMESTAMP_SKEW_SECONDS + 100;
        assert!(security.validate_timestamp(future).is_err());
    }

    #[test]
    fn timestamp_within_limit_is_accepted() {
        let security = make_security();
        assert!(security
            .validate_timestamp(current_timestamp_secs())
            .is_ok());
    }

    #[test]
    fn invalid_signature_rejects_command() {
        let security = make_security();
        let mut command = make_command(CloudCommandType::TextQuery, Some("hello".to_string()));
        command.signature = Some("bm90LWEtcmVhbC1zaWduYXR1cmU=".to_string());

        let result = security.validate_command(&command);
        assert!(
            matches!(result, Err(CloudError::SecurityError(_))),
            "expected SecurityError, got {:?}",
            result
        );
    }

    #[test]
    fn unsigned_valid_command_passes_validation() {
        let security = make_security();
        let command = make_command(
            CloudCommandType::TextQuery,
            Some("what is the weather".to_string()),
        );
        assert!(security.validate_command(&command).is_ok());
    }

    #[test]
    fn destructive_content_is_rejected() {
        let security = make_security();
        let command = make_command(
            CloudCommandType::TextQuery,
            Some("please run rm -rf / for me".to_string()),
        );
        assert!(security.validate_command(&command).is_err());
    }

    #[test]
    fn command_execution_categories_require_confirmation() {
        let security = make_security();
        assert!(
            security.requires_confirmation(&make_command(CloudCommandType::SystemCommand, None))
        );
        assert!(security.requires_confirmation(&make_command(CloudCommandType::ConfigUpdate, None)));
        assert!(!security.requires_confirmation(&make_command(
            CloudCommandType::TextQuery,
            Some("hi".to_string())
        )));
        assert!(
            !security.requires_confirmation(&make_command(CloudCommandType::StatusRequest, None))
        );
    }

    #[tokio::test]
    async fn rate_limit_blocks_after_budget_exhausted() {
        let security = make_security();
        for _ in 0..CLOUD_COMMANDS_PER_MINUTE {
            assert!(security
                .check_rate_limit(&CloudCommandType::SystemCommand)
                .await
                .is_ok());
        }
        assert!(security
            .check_rate_limit(&CloudCommandType::SystemCommand)
            .await
            .is_err());

        // Different command types have independent buckets
        assert!(security
            .check_rate_limit(&CloudCommandType::StatusRequest)
            .await
            .is_ok());
    }

    #[test]
    fn sanitize_for_logging_is_utf8_safe() {
        let security = make_security();
        // 250 multi-byte chars: a byte slice at 200 would panic mid-character
        let command = make_command(CloudCommandType::TextQuery, Some("é".repeat(250)));
        let sanitized = security.sanitize_for_logging(&command);
        let query = sanitized.payload.query.unwrap_or_default();
        assert!(query.ends_with("...[TRUNCATED]"));
        assert!(query.starts_with("é"));
    }
}

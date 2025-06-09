use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{warn, debug, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLimits {
    pub max_commands_per_minute: u32,
    pub max_dangerous_commands_per_hour: u32,
    pub max_file_operations_per_minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLimits {
    pub max_commands_per_minute: u32,
    pub max_execution_time: Duration,
    pub cooldown_after_failure: Duration,
}

#[derive(Debug)]
struct CommandCounter {
    count: u32,
    window_start: SystemTime,
    recent_commands: VecDeque<CommandInstance>,
}

#[derive(Debug, Clone)]
struct CommandInstance {
    timestamp: SystemTime,
    command: String,
    was_dangerous: bool,
    was_file_operation: bool,
    failed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AbusePattern {
    RapidCommandExecution { commands_per_second: f32 },
    RepeatedFailedCommands { failures_in_window: u32 },
    SuspiciousCommandSequence { pattern: Vec<String> },
    ExcessiveResourceUsage { cpu_percent: f32, memory_mb: u64 },
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub max_commands_per_minute: u32,
    pub max_dangerous_commands_per_hour: u32,
    pub violation_cooldown: Duration,
    pub max_violations_before_lockout: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_commands_per_minute: 60,
            max_dangerous_commands_per_hour: 10,
            violation_cooldown: Duration::from_secs(300), // 5 minutes
            max_violations_before_lockout: 3,
        }
    }
}

/// Rate limit violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    pub timestamp: SystemTime,
    pub command: String,
    pub risk_level: super::RiskLevel,
}

/// Rate limit violation details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitViolation {
    pub timestamp: SystemTime,
    pub tool_name: String,
    pub command: String,
    pub violation_type: ViolationType,
    pub current_rate: f32,
    pub limit: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    GlobalCommandRate,
    DangerousCommandRate,
    FileOperationRate,
    ToolSpecificRate,
    AbusePattern(AbusePattern),
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    global_limits: GlobalLimits,
    tool_limits: HashMap<String, ToolLimits>,
    command_counters: Arc<Mutex<HashMap<String, CommandCounter>>>,
    global_counter: Arc<Mutex<CommandCounter>>,
    dangerous_counter: Arc<Mutex<CommandCounter>>,
    file_operation_counter: Arc<Mutex<CommandCounter>>,
    violations: Arc<Mutex<Vec<RateLimitViolation>>>,
    max_violations: usize,
}

impl Default for GlobalLimits {
    fn default() -> Self {
        Self {
            max_commands_per_minute: 60,
            max_dangerous_commands_per_hour: 10,
            max_file_operations_per_minute: 30,
        }
    }
}

impl Default for ToolLimits {
    fn default() -> Self {
        Self {
            max_commands_per_minute: 20,
            max_execution_time: Duration::from_secs(300), // 5 minutes
            cooldown_after_failure: Duration::from_secs(5),
        }
    }
}

impl CommandCounter {
    fn new() -> Self {
        Self {
            count: 0,
            window_start: SystemTime::now(),
            recent_commands: VecDeque::new(),
        }
    }

    fn add_command(&mut self, command: String, is_dangerous: bool, is_file_operation: bool) {
        let now = SystemTime::now();
        
        // Clean old commands (older than 1 hour)
        while let Some(front) = self.recent_commands.front() {
            if let Ok(duration) = now.duration_since(front.timestamp) {
                if duration > Duration::from_secs(3600) {
                    self.recent_commands.pop_front();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Add new command
        self.recent_commands.push_back(CommandInstance {
            timestamp: now,
            command,
            was_dangerous: is_dangerous,
            was_file_operation: is_file_operation,
            failed: false,
        });

        self.count += 1;
    }

    fn mark_last_failed(&mut self) {
        if let Some(last) = self.recent_commands.back_mut() {
            last.failed = true;
        }
    }

    fn get_rate_per_minute(&self) -> f32 {
        let now = SystemTime::now();
        let one_minute_ago = now - Duration::from_secs(60);
        
        let recent_count = self.recent_commands
            .iter()
            .filter(|cmd| cmd.timestamp > one_minute_ago)
            .count();
        
        recent_count as f32
    }

    fn get_dangerous_rate_per_hour(&self) -> f32 {
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);
        
        let dangerous_count = self.recent_commands
            .iter()
            .filter(|cmd| cmd.timestamp > one_hour_ago && cmd.was_dangerous)
            .count();
        
        dangerous_count as f32
    }

    fn get_file_operation_rate_per_minute(&self) -> f32 {
        let now = SystemTime::now();
        let one_minute_ago = now - Duration::from_secs(60);
        
        let file_op_count = self.recent_commands
            .iter()
            .filter(|cmd| cmd.timestamp > one_minute_ago && cmd.was_file_operation)
            .count();
        
        file_op_count as f32
    }

    fn get_failed_commands_in_window(&self, window: Duration) -> u32 {
        let now = SystemTime::now();
        let window_start = now - window;
        
        self.recent_commands
            .iter()
            .filter(|cmd| cmd.timestamp > window_start && cmd.failed)
            .count() as u32
    }

    fn detect_rapid_execution(&self) -> Option<f32> {
        let now = SystemTime::now();
        let recent_commands: Vec<_> = self.recent_commands
            .iter()
            .filter(|cmd| {
                if let Ok(duration) = now.duration_since(cmd.timestamp) {
                    duration < Duration::from_secs(10)
                } else {
                    false
                }
            })
            .collect();

        if recent_commands.len() >= 3 {
            if let Some(first) = recent_commands.first() {
                if let Some(last) = recent_commands.last() {
                    if let Ok(time_span) = first.timestamp.duration_since(last.timestamp) {
                        if time_span.as_secs_f32() > 0.0 {
                            let rate = recent_commands.len() as f32 / time_span.as_secs_f32();
                            if rate > 2.0 { // More than 2 commands per second
                                return Some(rate);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

impl RateLimiter {
    pub fn new(global_limits: GlobalLimits) -> Self {
        Self {
            global_limits,
            tool_limits: HashMap::new(),
            command_counters: Arc::new(Mutex::new(HashMap::new())),
            global_counter: Arc::new(Mutex::new(CommandCounter::new())),
            dangerous_counter: Arc::new(Mutex::new(CommandCounter::new())),
            file_operation_counter: Arc::new(Mutex::new(CommandCounter::new())),
            violations: Arc::new(Mutex::new(Vec::new())),
            max_violations: 1000,
        }
    }

    /// Check if a command is allowed based on rate limits
    pub async fn check_rate_limit(&self, tool_name: &str, command: &str) -> Result<bool, String> {
        let is_dangerous = self.is_dangerous_command(command);
        let is_file_operation = self.is_file_operation_command(command);

        // Check global rate limits
        if !self.check_global_limits(command, is_dangerous, is_file_operation).await? {
            return Ok(false);
        }

        // Check tool-specific limits
        if !self.check_tool_limits(tool_name, command).await? {
            return Ok(false);
        }

        // Check for abuse patterns
        if !self.check_abuse_patterns(tool_name, command).await? {
            return Ok(false);
        }

        // Record the command
        self.record_command(tool_name, command, is_dangerous, is_file_operation).await;

        Ok(true)
    }

    /// Record a command execution
    async fn record_command(&self, tool_name: &str, command: &str, is_dangerous: bool, is_file_operation: bool) {
        // Record in global counter
        {
            let mut global = self.global_counter.lock().await;
            global.add_command(command.to_string(), is_dangerous, is_file_operation);
        }

        // Record in tool-specific counter
        {
            let mut counters = self.command_counters.lock().await;
            let counter = counters.entry(tool_name.to_string()).or_insert_with(CommandCounter::new);
            counter.add_command(command.to_string(), is_dangerous, is_file_operation);
        }

        debug!("Recorded command execution: {} -> {}", tool_name, command);
    }

    /// Check global rate limits
    async fn check_global_limits(&self, command: &str, is_dangerous: bool, is_file_operation: bool) -> Result<bool, String> {
        let global = self.global_counter.lock().await;
        
        // Check global command rate
        let global_rate = global.get_rate_per_minute();
        if global_rate >= self.global_limits.max_commands_per_minute as f32 {
            self.record_violation(
                "global".to_string(),
                command.to_string(),
                ViolationType::GlobalCommandRate,
                global_rate,
                self.global_limits.max_commands_per_minute as f32,
            ).await;
            return Err("Global command rate limit exceeded".to_string());
        }

        // Check dangerous command rate
        if is_dangerous {
            let dangerous_rate = global.get_dangerous_rate_per_hour();
            if dangerous_rate >= self.global_limits.max_dangerous_commands_per_hour as f32 {
                self.record_violation(
                    "global".to_string(),
                    command.to_string(),
                    ViolationType::DangerousCommandRate,
                    dangerous_rate,
                    self.global_limits.max_dangerous_commands_per_hour as f32,
                ).await;
                return Err("Dangerous command rate limit exceeded".to_string());
            }
        }

        // Check file operation rate
        if is_file_operation {
            let file_rate = global.get_file_operation_rate_per_minute();
            if file_rate >= self.global_limits.max_file_operations_per_minute as f32 {
                self.record_violation(
                    "global".to_string(),
                    command.to_string(),
                    ViolationType::FileOperationRate,
                    file_rate,
                    self.global_limits.max_file_operations_per_minute as f32,
                ).await;
                return Err("File operation rate limit exceeded".to_string());
            }
        }

        Ok(true)
    }

    /// Check tool-specific rate limits
    async fn check_tool_limits(&self, tool_name: &str, command: &str) -> Result<bool, String> {
        let tool_limits = self.tool_limits.get(tool_name)
            .cloned()
            .unwrap_or_default();

        let counters = self.command_counters.lock().await;
        if let Some(counter) = counters.get(tool_name) {
            let tool_rate = counter.get_rate_per_minute();
            if tool_rate >= tool_limits.max_commands_per_minute as f32 {
                self.record_violation(
                    tool_name.to_string(),
                    command.to_string(),
                    ViolationType::ToolSpecificRate,
                    tool_rate,
                    tool_limits.max_commands_per_minute as f32,
                ).await;
                return Err(format!("Tool {} rate limit exceeded", tool_name));
            }
        }

        Ok(true)
    }

    /// Check for abuse patterns
    async fn check_abuse_patterns(&self, tool_name: &str, command: &str) -> Result<bool, String> {
        let counters = self.command_counters.lock().await;
        if let Some(counter) = counters.get(tool_name) {
            // Check for rapid execution
            if let Some(rate) = counter.detect_rapid_execution() {
                let pattern = AbusePattern::RapidCommandExecution { commands_per_second: rate };
                self.record_violation(
                    tool_name.to_string(),
                    command.to_string(),
                    ViolationType::AbusePattern(pattern),
                    rate,
                    2.0, // Threshold
                ).await;
                warn!("Rapid command execution detected: {} commands/second", rate);
                return Err("Rapid command execution detected".to_string());
            }

            // Check for repeated failures
            let failed_count = counter.get_failed_commands_in_window(Duration::from_secs(300)); // 5 minutes
            if failed_count >= 5 {
                let pattern = AbusePattern::RepeatedFailedCommands { failures_in_window: failed_count };
                self.record_violation(
                    tool_name.to_string(),
                    command.to_string(),
                    ViolationType::AbusePattern(pattern),
                    failed_count as f32,
                    5.0,
                ).await;
                warn!("Repeated failed commands detected: {} failures", failed_count);
                return Err("Too many failed commands recently".to_string());
            }
        }

        Ok(true)
    }

    /// Record a rate limit violation
    async fn record_violation(
        &self,
        tool_name: String,
        command: String,
        violation_type: ViolationType,
        current_rate: f32,
        limit: f32,
    ) {
        let violation = RateLimitViolation {
            timestamp: SystemTime::now(),
            tool_name,
            command,
            violation_type,
            current_rate,
            limit,
        };

        let mut violations = self.violations.lock().await;
        violations.push(violation);

        // Trim old violations
        if violations.len() > self.max_violations {
            let excess = violations.len() - self.max_violations;
            violations.drain(0..excess);
        }
    }

    /// Mark the last command as failed
    pub async fn mark_command_failed(&self, tool_name: &str) {
        let mut counters = self.command_counters.lock().await;
        if let Some(counter) = counters.get_mut(tool_name) {
            counter.mark_last_failed();
        }

        let mut global = self.global_counter.lock().await;
        global.mark_last_failed();
    }

    /// Get recent violations
    pub async fn get_recent_violations(&self) -> usize {
        let violations = self.violations.lock().await;
        let one_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        violations
            .iter()
            .filter(|v| v.timestamp > one_hour_ago)
            .count()
    }

    /// Get rate limit statistics
    pub async fn get_rate_stats(&self) -> RateStats {
        let global = self.global_counter.lock().await;
        let global_rate = global.get_rate_per_minute();
        let dangerous_rate = global.get_dangerous_rate_per_hour();
        let file_operation_rate = global.get_file_operation_rate_per_minute();

        let counters = self.command_counters.lock().await;
        let mut tool_rates = HashMap::new();
        for (tool, counter) in counters.iter() {
            tool_rates.insert(tool.clone(), counter.get_rate_per_minute());
        }

        let violations_count = self.get_recent_violations().await;

        RateStats {
            global_command_rate: global_rate,
            dangerous_command_rate: dangerous_rate,
            file_operation_rate,
            tool_rates,
            recent_violations: violations_count,
            global_limits: self.global_limits.clone(),
        }
    }

    /// Set tool-specific limits
    pub fn set_tool_limits(&mut self, tool_name: String, limits: ToolLimits) {
        self.tool_limits.insert(tool_name, limits);
    }

    /// Check if command is dangerous (simplified heuristic)
    fn is_dangerous_command(&self, command: &str) -> bool {
        let dangerous_keywords = ["sudo", "rm -rf", "chmod 777", "dd if=", "format", "del /f"];
        let command_lower = command.to_lowercase();
        dangerous_keywords.iter().any(|keyword| command_lower.contains(keyword))
    }

    /// Check if command is a file operation
    fn is_file_operation_command(&self, command: &str) -> bool {
        let file_keywords = ["rm", "mv", "cp", "mkdir", "rmdir", "touch", "chmod", "chown", "del", "copy", "move"];
        let command_lower = command.to_lowercase();
        file_keywords.iter().any(|keyword| command_lower.starts_with(keyword))
    }

    /// Reset all counters (for testing or emergency)
    pub async fn reset_all_counters(&self) {
        let mut counters = self.command_counters.lock().await;
        counters.clear();

        let mut global = self.global_counter.lock().await;
        *global = CommandCounter::new();

        let mut violations = self.violations.lock().await;
        violations.clear();

        warn!("All rate limit counters have been reset");
    }
}

#[derive(Debug, Serialize)]
pub struct RateStats {
    pub global_command_rate: f32,
    pub dangerous_command_rate: f32,
    pub file_operation_rate: f32,
    pub tool_rates: HashMap<String, f32>,
    pub recent_violations: usize,
    pub global_limits: GlobalLimits,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_global_rate_limit() {
        let limits = GlobalLimits {
            max_commands_per_minute: 2,
            max_dangerous_commands_per_hour: 1,
            max_file_operations_per_minute: 1,
        };
        let limiter = RateLimiter::new(limits);

        // First two commands should pass
        assert!(limiter.check_rate_limit("test_tool", "echo hello").await.unwrap());
        assert!(limiter.check_rate_limit("test_tool", "echo world").await.unwrap());

        // Third command should fail
        assert!(!limiter.check_rate_limit("test_tool", "echo fail").await.unwrap());
    }

    #[tokio::test]
    async fn test_dangerous_command_detection() {
        let limiter = RateLimiter::new(GlobalLimits::default());

        assert!(limiter.is_dangerous_command("sudo rm -rf /"));
        assert!(limiter.is_dangerous_command("chmod 777 /etc"));
        assert!(!limiter.is_dangerous_command("ls -la"));
        assert!(!limiter.is_dangerous_command("echo hello"));
    }

    #[tokio::test]
    async fn test_file_operation_detection() {
        let limiter = RateLimiter::new(GlobalLimits::default());

        assert!(limiter.is_file_operation_command("rm file.txt"));
        assert!(limiter.is_file_operation_command("mv file1 file2"));
        assert!(limiter.is_file_operation_command("chmod 755 script.sh"));
        assert!(!limiter.is_file_operation_command("echo hello"));
        assert!(!limiter.is_file_operation_command("ps aux"));
    }

    #[tokio::test]
    async fn test_abuse_pattern_detection() {
        let limiter = RateLimiter::new(GlobalLimits::default());

        // Rapidly execute commands to trigger abuse detection
        for i in 0..5 {
            let result = limiter.check_rate_limit("test_tool", &format!("echo {}", i)).await;
            if i < 3 {
                assert!(result.unwrap());
            }
            // Commands after the first few might be blocked due to rapid execution
        }
    }
}
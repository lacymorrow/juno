# Self-Awareness Agent Security Enhancement Plan

## Executive Summary

The current self-awareness agent implementation poses significant security risks due to unrestricted command execution, lack of visibility, and absence of guard rails. This plan outlines comprehensive security enhancements to protect the system while maintaining agent functionality.

## Current Security Risks Identified

### Critical Vulnerabilities
1. **Unrestricted Command Execution**: Tools like `bash`, `run_terminal_command`, and `build_self` can execute arbitrary system commands
2. **No Command Blacklisting**: Dangerous commands like `rm -rf /`, `sudo rm -rf /*`, `format C:`, etc. can be executed
3. **No Approval Mechanism**: All commands run automatically without user consent
4. **Limited Visibility**: Minimal logging of what commands are being executed
5. **No File Change Tracking**: No visibility into what files are being modified
6. **No Sandboxing**: Full system access without restrictions
7. **No Rate Limiting**: Commands can be executed rapidly without throttling

### Affected Tools
- `src-tauri/src/agent/tools/self_awareness_tools.rs` - `build_self` tool
- `src-tauri/src/agent/tools/basic_tools.rs` - `run_terminal_command` tool
- `src-tauri/src/agent/tools/anthropic_computer_use.rs` - `bash` tool
- `src-tauri/src/commands/shell.rs` - `dev_bash_command` persistent shells
- `src-tauri/src/agent/tools/desktop_tools.rs` - System automation tools
- `src-tauri/src/cloud/commands.rs` - Remote command execution

## Security Enhancement Implementation Plan

### Phase 1: Command Security Framework

#### 1.1 Command Blacklist System
**File**: `src-tauri/src/agent/security/command_validator.rs`

Create a comprehensive command validation system:

```rust
pub struct CommandValidator {
    blacklisted_commands: Vec<Regex>,
    dangerous_patterns: Vec<DangerousPattern>,
    approval_required_patterns: Vec<Regex>,
}

pub struct DangerousPattern {
    pattern: Regex,
    risk_level: RiskLevel,
    description: String,
}

pub enum RiskLevel {
    Critical,    // Requires explicit approval
    High,        // Requires approval with warning
    Medium,      // Logs and warns but allows
    Low,         // Logs only
}
```

**Blacklisted Commands**:
- `rm -rf /` (and variations)
- `sudo rm -rf /*`
- `format C:` (Windows)
- `del /f /s /q C:\*` (Windows)
- `dd if=/dev/zero of=/dev/...`
- `sudo chmod -R 777 /`
- `sudo chown -R root:root /`
- Commands containing `sudo` + destructive operations
- Network-based attacks (`curl ... | bash`, `wget ... | sh`)
- Package manager destructive operations (`npm uninstall -g *`)

#### 1.2 User Approval System
**File**: `src-tauri/src/agent/security/approval_manager.rs`

```rust
pub struct ApprovalManager {
    pending_approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    approval_timeout: Duration,
}

pub struct PendingApproval {
    id: String,
    command: String,
    risk_level: RiskLevel,
    context: String,
    requested_at: Instant,
    approved: Option<bool>,
}
```

**Approval UI Components**:
- Modal dialog showing command details
- Risk level indicators
- Command breakdown and explanation
- "Allow Once", "Allow Always", "Deny" options
- Timeout handling (auto-deny after 30 seconds)

### Phase 2: Enhanced Visibility and Logging

#### 2.1 Command Execution Monitor
**File**: `src-tauri/src/agent/security/execution_monitor.rs`

```rust
pub struct ExecutionMonitor {
    command_log: Arc<Mutex<Vec<CommandLogEntry>>>,
    file_watcher: Arc<Mutex<Option<FileWatcher>>>,
    system_state_snapshots: Arc<Mutex<Vec<SystemSnapshot>>>,
}

pub struct CommandLogEntry {
    id: String,
    timestamp: SystemTime,
    tool_name: String,
    command: String,
    user_approved: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    execution_time: Duration,
    files_modified: Vec<String>,
    processes_spawned: Vec<u32>,
    network_activity: Vec<NetworkActivity>,
}
```

#### 2.2 Real-time Command Display
**Frontend**: Enhanced dev tools with live command monitoring

```typescript
interface CommandMonitor {
  activeCommands: ActiveCommand[];
  commandHistory: CommandLogEntry[];
  riskAlerts: SecurityAlert[];
}

interface ActiveCommand {
  id: string;
  tool: string;
  command: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  startTime: Date;
  estimatedDuration?: number;
}
```

### Phase 3: File System Protection

#### 3.1 File Change Tracking
**File**: `src-tauri/src/agent/security/file_monitor.rs`

```rust
pub struct FileMonitor {
    watched_paths: HashSet<PathBuf>,
    change_log: Arc<Mutex<Vec<FileChangeEntry>>>,
    watcher: RecommendedWatcher,
}

pub struct FileChangeEntry {
    timestamp: SystemTime,
    path: PathBuf,
    change_type: FileChangeType,
    before_hash: Option<String>,
    after_hash: Option<String>,
    size_change: i64,
    permissions_changed: bool,
    command_id: Option<String>, // Link to command that caused change
}

pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Moved { from: PathBuf, to: PathBuf },
    PermissionsChanged,
}
```

#### 3.2 Protected Directories
**Configuration**: Define critical system directories that require elevated approval:

```rust
pub const PROTECTED_PATHS: &[&str] = &[
    "/System",           // macOS system
    "/usr/bin",          // System binaries
    "/etc",              // System configuration
    "/boot",             // Boot directory
    "C:\\Windows",       // Windows system
    "C:\\Program Files", // Windows programs
    "/Applications",     // macOS applications
];
```

#### 3.3 Code Diff Display
**Frontend**: Real-time diff viewer for modified files

```typescript
interface FileDiff {
  filePath: string;
  changeType: 'created' | 'modified' | 'deleted';
  beforeContent?: string;
  afterContent?: string;
  diff: DiffLine[];
  commandId: string;
}

interface DiffLine {
  lineNumber: number;
  type: 'added' | 'removed' | 'unchanged';
  content: string;
}
```

### Phase 4: Sandboxing and Isolation

#### 4.1 Virtual Environment Detection
**File**: `src-tauri/src/agent/security/sandbox_detector.rs`

```rust
pub struct SandboxDetector;

impl SandboxDetector {
    pub fn is_running_in_container() -> bool { ... }
    pub fn is_running_in_vm() -> bool { ... }
    pub fn get_sandbox_restrictions() -> SandboxProfile { ... }
    pub fn recommend_sandbox_setup() -> Vec<SandboxRecommendation> { ... }
}
```

#### 4.2 Restricted Execution Context
**Implementation**: Create isolated execution environments for dangerous commands:

```rust
pub struct RestrictedExecutor {
    allowed_binaries: HashSet<String>,
    blocked_paths: HashSet<PathBuf>,
    resource_limits: ResourceLimits,
    network_restrictions: NetworkRestrictions,
}

pub struct ResourceLimits {
    max_memory: u64,
    max_cpu_percent: f32,
    max_execution_time: Duration,
    max_file_descriptors: u32,
}
```

### Phase 5: Rate Limiting and Abuse Prevention

#### 5.1 Command Rate Limiting
**File**: `src-tauri/src/agent/security/rate_limiter.rs`

```rust
pub struct CommandRateLimiter {
    command_counts: Arc<Mutex<HashMap<String, CommandCounter>>>,
    global_limits: GlobalLimits,
    tool_specific_limits: HashMap<String, ToolLimits>,
}

pub struct CommandCounter {
    count: u32,
    window_start: Instant,
    recent_commands: VecDeque<Instant>,
}

pub struct GlobalLimits {
    max_commands_per_minute: u32,
    max_dangerous_commands_per_hour: u32,
    max_file_operations_per_minute: u32,
}
```

#### 5.2 Abuse Detection Patterns
```rust
pub enum AbusePattern {
    RapidCommandExecution { commands_per_second: f32 },
    RepeatedFailedCommands { failures_in_window: u32 },
    SuspiciousCommandSequence { pattern: Vec<String> },
    ExcessiveResourceUsage { cpu_percent: f32, memory_mb: u64 },
}
```

### Phase 6: Enhanced Self-Awareness Security

#### 6.1 Secure Build System
**File**: `src-tauri/src/agent/tools/secure_self_awareness_tools.rs`

```rust
pub struct SecureBuildSystem {
    validator: CommandValidator,
    monitor: ExecutionMonitor,
    approval_manager: ApprovalManager,
}

impl SecureBuildSystem {
    pub async fn secure_build_self(&self, input: Value) -> Result<Value, String> {
        let target = input["target"].as_str().unwrap_or("dev");
        let manifest_path = input["manifest_path"].as_str().unwrap_or("src-tauri/Cargo.toml");
        
        // Validate target and path
        self.validate_build_target(target)?;
        self.validate_manifest_path(manifest_path)?;
        
        // Check if approval is required
        let build_command = format!("cargo build --manifest-path {}", manifest_path);
        if self.validator.requires_approval(&build_command) {
            let approval_id = self.approval_manager.request_approval(
                build_command.clone(),
                RiskLevel::Medium,
                "Self-build operation requested by agent"
            ).await?;
            
            // Wait for user approval
            self.approval_manager.wait_for_approval(approval_id).await?;
        }
        
        // Execute with monitoring
        self.monitor.execute_monitored_command(&build_command).await
    }
}
```

### Phase 7: User Interface Enhancements

#### 7.1 Security Dashboard
**Frontend**: `src/components/devtools/SecurityDashboard.tsx`

```typescript
interface SecurityDashboard {
  commandActivity: LiveCommandActivity;
  riskAlerts: SecurityAlert[];
  fileChanges: FileChangeLog[];
  systemHealth: SystemHealthMetrics;
  approvalQueue: PendingApproval[];
}
```

#### 7.2 Command Approval Modal
**Frontend**: `src/components/devtools/CommandApprovalModal.tsx`

```typescript
interface CommandApprovalProps {
  command: string;
  riskLevel: RiskLevel;
  context: string;
  estimatedImpact: ImpactAssessment;
  onApprove: (decision: ApprovalDecision) => void;
}
```

#### 7.3 File Diff Viewer
**Frontend**: `src/components/devtools/FileDiffViewer.tsx`

```typescript
interface FileDiffViewerProps {
  filePath: string;
  beforeContent: string;
  afterContent: string;
  commandContext: CommandContext;
}
```

## Implementation Priority

### Phase 1 (Critical - Week 1)
1. Command blacklist system
2. Basic approval mechanism
3. Command logging enhancement

### Phase 2 (High - Week 2)
1. Real-time command monitoring UI
2. File change tracking
3. Enhanced visibility dashboard

### Phase 3 (Medium - Week 3)
1. Sandboxing detection and recommendations
2. Rate limiting implementation
3. Abuse pattern detection

### Phase 4 (Low - Week 4)
1. Advanced diff viewing
2. Performance optimizations
3. Additional security hardening

## Configuration and Settings

### Security Configuration File
**File**: `src-tauri/security-config.toml`

```toml
[security]
enabled = true
development_mode_restrictions = true

[command_validation]
enable_blacklist = true
require_approval_for_sudo = true
require_approval_for_destructive = true
auto_deny_critical_commands = true

[rate_limiting]
max_commands_per_minute = 60
max_dangerous_commands_per_hour = 10
enable_abuse_detection = true

[file_monitoring]
enable_change_tracking = true
protected_paths = ["/System", "/usr", "/etc"]
auto_backup_before_changes = true

[approval_system]
timeout_seconds = 30
require_explicit_approval = true
log_all_decisions = true
```

## Testing Strategy

### Security Test Suite
1. **Command Injection Tests**: Verify blacklist effectiveness
2. **Approval Bypass Tests**: Ensure approval cannot be circumvented
3. **Rate Limiting Tests**: Verify rate limits work correctly
4. **File Protection Tests**: Ensure protected directories are secured
5. **UI Security Tests**: Test approval modal security

### Integration Tests
1. End-to-end command execution with approval
2. File change detection accuracy
3. Real-time monitoring performance
4. Security dashboard functionality

## Migration Plan

### Backward Compatibility
1. Existing tools will continue to work with security enhancements
2. Configuration options to disable security for development
3. Gradual rollout of restrictions

### Developer Experience
1. Clear error messages when commands are blocked
2. Easy approval process for legitimate commands
3. Comprehensive logging for debugging

## Monitoring and Alerts

### Security Events
1. **Critical Command Attempts**: Immediate alert
2. **Repeated Approval Denials**: Potential abuse
3. **Unusual File Access Patterns**: Suspicious activity
4. **Rate Limit Violations**: Possible automated attacks

### Metrics Collection
1. Command execution statistics
2. Approval rates and patterns
3. File change frequency
4. Security violation counts

## Future Enhancements

### Advanced Features
1. **Machine Learning**: Anomaly detection for command patterns
2. **Behavioral Analysis**: User interaction pattern analysis
3. **Threat Intelligence**: Integration with security databases
4. **Automatic Sandboxing**: Dynamic isolation for risky commands

### Enterprise Features
1. **Centralized Security Policies**: Organization-wide rules
2. **Audit Logging**: Compliance-ready logs
3. **Integration APIs**: Security tool integration
4. **Advanced Reporting**: Security dashboards and reports

This comprehensive security enhancement plan addresses all major vulnerabilities while maintaining the agent's functionality and providing excellent visibility into system operations.
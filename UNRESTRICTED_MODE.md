# Juno Unrestricted Mode - Full System Control

## Overview
Juno now includes an **Unrestricted Mode** that provides complete, unlimited control over the computer system. When enabled, Juno can perform any system operation without restrictions, including:

- Execute any system commands with full privileges
- Access and modify any file on the system
- Control network settings and firewall rules
- Kill or manipulate any process
- Execute commands with administrator/sudo privileges
- Modify system configuration files
- Access protected system areas

## ⚠️ WARNING
**UNRESTRICTED MODE GRANTS FULL SYSTEM ACCESS**
- All security restrictions are disabled
- Juno can modify critical system files
- System damage is possible if used incorrectly
- Only enable when absolutely necessary
- Always disable after completing required tasks

## Implementation Components

### 1. Core Unrestricted Module (`src-tauri/src/commands/computer/unrestricted_computer.rs`)
- `UnrestrictedComputer` struct for system operations
- `UnrestrictedConfig` for configuration management
- Direct system command execution
- File system operations without restrictions
- Network control operations
- Process manipulation capabilities

### 2. Command Interface (`src-tauri/src/commands/unrestricted.rs`)
Tauri commands for controlling unrestricted mode:
- `enable_unrestricted_mode()` - Activate full system access
- `disable_unrestricted_mode()` - Return to normal security
- `get_unrestricted_status()` - Check current mode status
- `update_unrestricted_config()` - Modify configuration
- `execute_unrestricted()` - Execute system operations
- `emergency_shutdown()` - Immediately disable all unrestricted access

### 3. State Management (`src-tauri/src/state.rs`)
Added to AppState:
- `unrestricted_mode: Arc<StdMutex<bool>>` - Mode status flag
- `unrestricted_config: Arc<StdMutex<UnrestrictedConfig>>` - Configuration
- Helper methods for mode control

## Available Operations

### System Commands
```javascript
// Execute any system command
await invoke('execute_unrestricted', {
  operation: 'system_command',
  parameters: {
    command: 'ls',
    args: ['-la', '/System']
  }
});
```

### File Operations
```javascript
// Read any file
await invoke('execute_unrestricted', {
  operation: 'file_operation',
  parameters: {
    path: '/etc/passwd',
    // operation type determined by parameters
  }
});

// Write to system file
await invoke('execute_unrestricted', {
  operation: 'file_operation',
  parameters: {
    path: '/etc/hosts',
    write_data: btoa('127.0.0.1 localhost')
  }
});

// Delete file/directory
await invoke('execute_unrestricted', {
  operation: 'file_operation',
  parameters: {
    path: '/path/to/delete',
    delete: true
  }
});
```

### Administrator Commands
```javascript
// Execute with sudo/admin privileges
await invoke('execute_unrestricted', {
  operation: 'admin_command',
  parameters: {
    command: 'systemctl',
    args: ['restart', 'nginx']
  }
});
```

## Usage Examples

### Enable Unrestricted Mode
```javascript
// Enable full system access
const status = await invoke('enable_unrestricted_mode');
console.log(status.warning); // ⚠️ UNRESTRICTED MODE ACTIVE

// Perform system operations...

// Disable when done
await invoke('disable_unrestricted_mode');
```

### Emergency Shutdown
```javascript
// In case of issues, immediately disable all access
await invoke('emergency_shutdown');
```

### Check Status
```javascript
const status = await invoke('get_unrestricted_status');
if (status.enabled) {
  console.log('Unrestricted mode is active');
  console.log('Config:', status.config);
}
```

## Configuration Options

```javascript
const config = {
  bypass_all_permissions: true,      // Bypass all permission checks
  allow_system_modifications: true,  // Allow modifying system files
  allow_kernel_access: true,         // Allow kernel-level operations
  allow_driver_installation: true,   // Allow driver installations
  allow_firmware_access: true,       // Allow firmware modifications
  disable_all_sandboxing: true,     // Disable all sandboxes
  full_admin_privileges: true        // Full administrator access
};

await invoke('update_unrestricted_config', { config });
```

## Security Considerations

1. **Default State**: Unrestricted mode is DISABLED by default
2. **Explicit Activation**: Must be explicitly enabled via command
3. **Logging**: All unrestricted operations are logged
4. **Emergency Stop**: Emergency shutdown available at all times
5. **Configuration Control**: Fine-grained control over capabilities

## Platform-Specific Features

### macOS
- Direct Core Graphics access
- Accessibility API bypass
- System Integrity Protection awareness
- Kernel extension loading (if SIP disabled)

### Windows
- UAC bypass capabilities
- Registry modification
- Service control
- Driver installation

### Linux
- Direct syscall execution
- Kernel module loading
- iptables manipulation
- systemd control

## Integration with Existing Features

Unrestricted mode enhances existing Juno capabilities:
- **Computer Use API**: Removes all restrictions on automation
- **File Operations**: Access any file without path validation
- **Shell Commands**: Execute any command without whitelist
- **Browser Control**: Full access to browser internals
- **System Settings**: Modify any system configuration

## Best Practices

1. **Minimal Usage**: Only enable when absolutely necessary
2. **Specific Operations**: Enable, perform operation, disable immediately
3. **Logging**: Monitor all operations performed in unrestricted mode
4. **Testing**: Test operations in safe environment first
5. **Backup**: Always backup critical files before modifications

## Future Enhancements

- [ ] Audit trail for all unrestricted operations
- [ ] Time-limited unrestricted sessions
- [ ] Granular permission controls per operation
- [ ] Rollback capabilities for system changes
- [ ] Integration with system restore points

## Conclusion

Juno's Unrestricted Mode provides complete system control when needed, while maintaining safety through:
- Default-disabled state
- Explicit activation requirement
- Emergency shutdown capability
- Comprehensive logging
- Clear warning messages

This ensures Juno can perform any required system operation while maintaining user awareness of the elevated privileges.
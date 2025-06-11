# MCP Server Startup Error Analysis

## Error Summary

**Error Message**: `Failed to start MCP server 9bf5e83e-dbbe-4371-bd14-b16e567260f5: Failed to parse response JSON: EOF while parsing a value at line 1 column 0`

**Error Type**: JSON parsing failure during MCP server initialization

## Root Cause Analysis

The error "EOF while parsing a value at line 1 column 0" indicates that:

1. **Empty Response**: The MCP server process is not outputting any JSON response
2. **Process Crash**: The server process exits immediately after starting
3. **Invalid Command**: The command or executable path is incorrect
4. **Missing Dependencies**: Required dependencies or environment variables are missing
5. **Protocol Mismatch**: The server doesn't implement the expected MCP protocol

## Enhanced Diagnostic Features

### 1. **Improved Error Handling** ✅ IMPLEMENTED
- **stderr Monitoring**: Captures error messages from MCP server processes
- **Process Status Checking**: Detects if servers crash immediately
- **Detailed Logging**: Enhanced debug information for startup sequence
- **Better Error Messages**: More descriptive error messages with context

### 2. **New Diagnostic Commands** ✅ IMPLEMENTED
- `get_mcp_diagnostics()`: Comprehensive status report for all MCP servers
- `test_mcp_server_connection()`: Test server configuration without permanent setup
- `restart_mcp_server_with_diagnostics()`: Restart with enhanced logging

### 3. **Diagnostic Information Captured**
- Server configuration (command, args, working directory, environment)
- Process status and exit codes
- stderr output from server processes
- Startup timing and timeout information
- Tool discovery status

## Common Causes and Solutions

### 1. **Incorrect Command Path**
```json
{
  "problem": "Command not found or invalid path",
  "solution": "Verify the executable exists and is accessible",
  "example": {
    "wrong": {"command": "nonexistent-server"},
    "correct": {"command": "/usr/local/bin/mcp-server"}
  }
}
```

### 2. **Missing Dependencies**
```json
{
  "problem": "Server requires Python/Node.js/other runtime",
  "solution": "Install required runtime and dependencies",
  "check": "Verify runtime is in PATH and accessible"
}
```

### 3. **Permission Issues**
```json
{
  "problem": "Executable doesn't have permission to run",
  "solution": "Make file executable: chmod +x /path/to/server",
  "verification": "Test command manually in terminal"
}
```

### 4. **Environment Variables**
```json
{
  "problem": "Server needs specific environment variables",
  "solution": "Add required variables to server configuration",
  "example": {
    "environment_variables": {
      "API_KEY": "your-api-key",
      "CONFIG_PATH": "/path/to/config"
    }
  }
}
```

### 5. **Working Directory**
```json
{
  "problem": "Server needs to run from specific directory",
  "solution": "Set working_directory in server config",
  "example": {
    "working_directory": "/path/to/server/directory"
  }
}
```

## Diagnostic Steps

### Step 1: Check Server Configuration
Use the new diagnostic command to get detailed server information:
```typescript
const diagnostics = await invoke('get_mcp_diagnostics');
console.log('Server diagnostics:', diagnostics);
```

### Step 2: Test Server Manually
Before adding to Juno, test the server command manually:
```bash
# Test the exact command that's failing
/path/to/your/mcp-server --args

# Check if it responds to MCP protocol
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}' | /path/to/your/mcp-server
```

### Step 3: Use Test Connection
Test server configuration without permanent setup:
```typescript
const result = await invoke('test_mcp_server_connection', {
  config: {
    name: "Test Server",
    command: "/path/to/server",
    args: ["--stdio"],
    // ... other config
  }
});
console.log('Test result:', result);
```

### Step 4: Check stderr Output
The enhanced MCP integration now captures stderr output. Check the logs for error messages from the server process.

### Step 5: Restart with Diagnostics
Use the diagnostic restart command for enhanced logging:
```typescript
const result = await invoke('restart_mcp_server_with_diagnostics', {
  server_id: "your-server-id"
});
console.log('Restart result:', result);
```

## Common MCP Server Examples

### 1. **Python MCP Server**
```json
{
  "name": "Python MCP Server",
  "command": "python3",
  "args": ["-m", "mcp_server"],
  "working_directory": "/path/to/server",
  "environment_variables": {
    "PYTHONPATH": "/path/to/server"
  }
}
```

### 2. **Node.js MCP Server**
```json
{
  "name": "Node MCP Server",
  "command": "node",
  "args": ["server.js"],
  "working_directory": "/path/to/server",
  "environment_variables": {
    "NODE_ENV": "production"
  }
}
```

### 3. **Binary MCP Server**
```json
{
  "name": "Binary MCP Server",
  "command": "/usr/local/bin/mcp-server",
  "args": ["--stdio", "--config", "config.json"],
  "working_directory": "/opt/mcp-server"
}
```

## Debugging Checklist

- [ ] **Command exists**: `which /path/to/command` or `ls -la /path/to/command`
- [ ] **Executable permissions**: `chmod +x /path/to/command` if needed
- [ ] **Dependencies installed**: Check for Python, Node.js, libraries
- [ ] **Environment variables**: Verify required API keys and config paths
- [ ] **Working directory**: Ensure server can find its configuration files
- [ ] **Manual test**: Run command manually to see actual error messages
- [ ] **JSON output**: Verify server outputs valid JSON responses
- [ ] **Protocol compliance**: Check server implements MCP protocol correctly

## Prevention Strategies

1. **Test Before Adding**: Always use `test_mcp_server_connection` before permanent setup
2. **Monitor stderr**: Check logs for server error messages
3. **Validate Configuration**: Ensure all paths and environment variables are correct
4. **Documentation**: Keep MCP server documentation for reference
5. **Version Compatibility**: Ensure MCP protocol version compatibility

## Resolution Outcome

With the enhanced error handling and diagnostic features implemented:

1. **Better Error Messages**: More descriptive errors with context
2. **stderr Capture**: Server error messages are now visible in logs
3. **Process Monitoring**: Detect immediate crashes and exit codes
4. **Diagnostic Commands**: Tools to troubleshoot server configurations
5. **Testing Framework**: Safe way to test configurations before deployment

The error you encountered should now provide much more detailed information about what went wrong with the MCP server startup, making it easier to diagnose and fix the issue.
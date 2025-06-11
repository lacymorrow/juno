# Custom MCP Server Integration

This document outlines the comprehensive custom MCP (Model Context Protocol) server integration that has been implemented in the Juno AI agent system.

## Overview

The custom MCP integration allows users to add external MCP servers to extend the agent's capabilities with custom tools. This system provides a complete infrastructure for:

- Managing MCP server configurations
- Starting/stopping MCP servers dynamically
- Discovering and registering tools from MCP servers
- Executing MCP tools alongside built-in tools
- Persistent configuration storage

## Architecture

### Core Components

1. **MCP Integration Module** (`src-tauri/src/agent/tools/mcp_integration.rs`)
   - `MCPServerConfig`: Configuration for external MCP servers
   - `MCPServerConnection`: Active connection management
   - `MCPManager`: Central manager for all MCP servers
   - `MCPToolInfo`: Information about discovered tools

2. **Tool Configuration System** (`src-tauri/src/agent/tools/tool_config.rs`)
   - Extended to support MCP server configurations
   - Persistent storage for server settings
   - Tool enable/disable management

3. **State Management** (`src-tauri/src/state.rs`)
   - Integrated MCP manager into AppState
   - Automatic server initialization
   - Tool synchronization

4. **Tool Provider Integration** (`src-tauri/src/agent/implementations/tool_provider.rs`)
   - Extended LocalToolProvider with MCP support
   - Seamless execution of MCP tools alongside local tools
   - Tool namespacing to prevent conflicts

5. **Tauri Commands** (`src-tauri/src/commands/mcp.rs`)
   - Complete API for frontend integration
   - Server management commands
   - Tool discovery and status monitoring

## Features

### MCP Server Management

- **Add/Remove Servers**: Configure external MCP servers with commands, arguments, and environment variables
- **Start/Stop Control**: Dynamic control over server lifecycle
- **Status Monitoring**: Real-time status tracking for all servers
- **Auto-start Support**: Automatic server startup on application launch
- **Connection Testing**: Test server connectivity before adding permanently

### Tool Integration

- **Dynamic Discovery**: Automatically discover tools from connected servers
- **Tool Namespacing**: Prevent conflicts by prefixing tools with server names
- **Configuration Persistence**: Save tool enable/disable settings
- **Category Management**: MCP tools appear in dedicated "MCP Tools" category
- **Seamless Execution**: MCP tools execute transparently alongside built-in tools

### Configuration Management

- **Persistent Storage**: All configurations saved to `tool_config.json`
- **Environment Variables**: Support for server-specific environment variables
- **Working Directories**: Configurable working directories for servers
- **Timeout Management**: Configurable timeouts for server communications
- **Retry Logic**: Automatic retry for failed server connections

## API Reference

### Tauri Commands

#### Server Management

```typescript
// Add a new MCP server
await invoke('add_mcp_server', {
  config: {
    name: 'my-server',
    command: 'python',
    args: ['-m', 'my_mcp_server'],
    description: 'My custom MCP server',
    enabled: true,
    auto_start: true,
    timeout_seconds: 30,
    environment_variables: {
      'API_KEY': 'your-api-key'
    }
  }
});

// Remove an MCP server
await invoke('remove_mcp_server', { serverId: 'server-id' });

// Start/stop servers
await invoke('start_mcp_server', { serverId: 'server-id' });
await invoke('stop_mcp_server', { serverId: 'server-id' });

// Get server configurations and statuses
const servers = await invoke('get_mcp_servers');
const statuses = await invoke('get_mcp_server_statuses');
```

#### Tool Management

```typescript
// Get all MCP tools
const tools = await invoke('get_mcp_tools');

// Test server connection
const toolNames = await invoke('test_mcp_server_connection', {
  config: serverConfig
});

// Initialize all servers
await invoke('initialize_mcp_servers');
```

### Configuration Structure

```rust
struct MCPServerConfig {
    id: String,                    // Unique identifier
    name: String,                  // Display name
    description: Option<String>,   // Optional description
    command: String,               // Executable command
    args: Vec<String>,            // Command arguments
    working_directory: Option<PathBuf>, // Working directory
    environment_variables: HashMap<String, String>, // Environment vars
    enabled: bool,                 // Whether server is enabled
    auto_start: bool,             // Auto-start on app launch
    timeout_seconds: u64,         // Connection timeout
    max_retries: u32,             // Max retry attempts
}
```

## Usage Examples

### Adding a Python MCP Server

```rust
let config = MCPServerConfig::new(
    "python-tools".to_string(),
    "python".to_string(),
    vec!["-m", "my_mcp_package"].iter().map(|s| s.to_string()).collect(),
)
.with_description("Python-based MCP tools for data processing".to_string())
.with_working_directory(PathBuf::from("/path/to/server"))
.with_environment_variable("PYTHONPATH".to_string(), "/custom/path".to_string());
```

### Adding a Node.js MCP Server

```rust
let config = MCPServerConfig::new(
    "node-server".to_string(),
    "node".to_string(),
    vec!["server.js", "--stdio"].iter().map(|s| s.to_string()).collect(),
)
.with_description("Node.js MCP server with web APIs".to_string())
.with_environment_variable("NODE_ENV".to_string(), "production".to_string());
```

### Adding a Rust MCP Server

```rust
let config = MCPServerConfig::new(
    "rust-tools".to_string(),
    "./target/release/my-mcp-server".to_string(),
    vec!["--stdio"].iter().map(|s| s.to_string()).collect(),
)
.with_description("High-performance Rust-based tools".to_string());
```

## Protocol Implementation

### JSON-RPC Communication

The integration follows the MCP specification using JSON-RPC 2.0 over STDIO:

1. **Initialization**: Send `initialize` request with client capabilities
2. **Tool Discovery**: Send `listTools` request to discover available tools
3. **Tool Execution**: Send `callTool` requests to execute specific tools

### Message Format

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "callTool",
  "params": {
    "name": "tool_name",
    "arguments": { "arg1": "value1" }
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "output": "Tool execution result"
  }
}
```

## Error Handling

### Connection Errors
- Automatic retry with exponential backoff
- Graceful degradation when servers are unavailable
- Clear error messages for debugging

### Tool Execution Errors
- Proper error propagation to the agent
- Timeout handling for long-running tools
- Resource cleanup on failures

### Configuration Errors
- Validation of server configurations
- Clear error messages for invalid settings
- Recovery from corrupted configuration files

## Security Considerations

### Process Isolation
- Each MCP server runs in its own process
- Limited communication through STDIO only
- Automatic cleanup of terminated processes

### Environment Variables
- Secure storage of sensitive configuration
- Environment variable validation
- No exposure of internal system variables

### Command Execution
- Validation of executable paths
- Argument sanitization
- Working directory restrictions

## Performance Optimization

### Connection Pooling
- Reuse of established server connections
- Lazy initialization of servers
- Connection health monitoring

### Tool Caching
- Cache of discovered tools
- Efficient tool lookup
- Automatic cache invalidation

### Resource Management
- Memory-efficient server communication
- Timeout-based resource cleanup
- Configurable resource limits

## Integration Points

### Agent System Integration
- Seamless integration with existing tool system
- Automatic tool registration and discovery
- Consistent tool execution interface

### UI Integration
- Settings panel for server management
- Real-time status indicators
- Tool enable/disable controls

### Configuration Persistence
- JSON-based configuration storage
- Automatic backup and recovery
- Version migration support

## Future Enhancements

### Planned Features
- Server auto-discovery mechanisms
- Tool dependency management
- Performance monitoring and metrics
- Server marketplace integration
- Advanced security policies

### Extensibility
- Plugin system for custom protocols
- Support for additional communication methods
- Custom tool registration APIs
- Advanced configuration templates

## Troubleshooting

### Common Issues
1. **Server fails to start**: Check command path and permissions
2. **Tools not discovered**: Verify server implements MCP protocol correctly
3. **Connection timeouts**: Increase timeout settings or check server performance
4. **Permission errors**: Ensure proper file system permissions for server executable

### Debug Mode
Enable debug logging to troubleshoot MCP server issues:
```rust
RUST_LOG=debug cargo run
```

### Log Analysis
- Server startup/shutdown events
- Tool discovery and registration
- Execution success/failure metrics
- Performance timing information

## Examples and Templates

### Basic MCP Server Template (Python)

```python
#!/usr/bin/env python3
import json
import sys
from typing import Dict, Any

class MCPServer:
    def __init__(self):
        self.tools = {
            "hello": {
                "name": "hello",
                "description": "Say hello with a custom message",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    },
                    "required": ["name"]
                }
            }
        }

    def handle_initialize(self, params: Dict[str, Any]) -> Dict[str, Any]:
        return {
            "capabilities": {
                "tools": {"listChanged": True}
            }
        }

    def handle_list_tools(self) -> Dict[str, Any]:
        return {"tools": list(self.tools.values())}

    def handle_call_tool(self, params: Dict[str, Any]) -> Dict[str, Any]:
        tool_name = params["name"]
        arguments = params.get("arguments", {})
        
        if tool_name == "hello":
            name = arguments.get("name", "World")
            return {"result": f"Hello, {name}!"}
        
        raise ValueError(f"Unknown tool: {tool_name}")

    def run(self):
        while True:
            try:
                line = input()
                request = json.loads(line)
                
                method = request["method"]
                params = request.get("params", {})
                request_id = request["id"]
                
                if method == "initialize":
                    result = self.handle_initialize(params)
                elif method == "listTools":
                    result = self.handle_list_tools()
                elif method == "callTool":
                    result = self.handle_call_tool(params)
                else:
                    raise ValueError(f"Unknown method: {method}")
                
                response = {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": result
                }
                
                print(json.dumps(response))
                sys.stdout.flush()
                
            except EOFError:
                break
            except Exception as e:
                error_response = {
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32000,
                        "message": str(e)
                    }
                }
                print(json.dumps(error_response))
                sys.stdout.flush()

if __name__ == "__main__":
    server = MCPServer()
    server.run()
```

This comprehensive MCP integration provides a robust foundation for extending the Juno AI agent with custom tools and capabilities through external MCP servers.
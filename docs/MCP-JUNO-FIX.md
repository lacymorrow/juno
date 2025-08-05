# MCP (Model Context Protocol) Fix for Juno

## Problem Summary

MCP capabilities are not working in Juno even though they work with Claude. The issue is that MCP servers need to be properly configured in Juno's settings.

## Root Cause

1. **No MCP servers configured by default**: Unlike Claude which may have pre-configured MCP servers, Juno requires manual configuration
2. **Configuration location**: MCP servers are stored in `tool_config.json` in the Application Support directory
3. **Initialization timing**: MCP servers are initialized in the background after app startup

## Solution

### Quick Fix

1. Open Juno
2. Go to **Settings → Network**
3. In the **MCP Server JSON** field, paste this configuration:

```json
{
  "everything-test": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-everything"],
    "description": "Test MCP server with everything capabilities"
  }
}
```

4. Click **"Add Server"**
5. Make sure the server toggle is **enabled**
6. The server status should change to **"Connected"**

### Full Configuration (Recommended)

For a complete MCP setup with multiple capabilities, use this configuration:

```json
{
  "everything": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-everything"],
    "description": "General purpose MCP server"
  },
  "filesystem": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-filesystem", "/Users/your-username"],
    "description": "File system operations"
  },
  "memory": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-memory"],
    "description": "Knowledge graph memory"
  },
  "sequential-thinking": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-sequential-thinking"],
    "description": "Problem solving and planning"
  }
}
```

## Technical Details

### Configuration Files

Juno stores MCP configuration in:
- **macOS**: `~/Library/Application Support/com.juno.app/tool_config.json`
- **Linux**: `~/.config/com.juno.app/tool_config.json`
- **Windows**: `%APPDATA%/com.juno.app/tool_config.json`

### MCP Server Structure

Each MCP server configuration includes:
- `command`: The command to run (usually "npx")
- `args`: Command arguments (the MCP package name and any parameters)
- `description`: Human-readable description
- `enabled`: Whether the server is active
- `auto_start`: Whether to start automatically
- `timeout_seconds`: Connection timeout
- `max_retries`: Retry attempts on failure

### Initialization Process

1. On app startup, `initialize_mcp_state` is called
2. MCP servers are loaded from `tool_config.json`
3. Enabled servers with `auto_start: true` are started in parallel
4. Each server spawns a Node.js process via `npx`
5. Juno establishes JSON-RPC communication with the server
6. Available tools are discovered and registered

## Troubleshooting

### Server won't connect

1. **Check Node.js installation**:
   ```bash
   node --version
   npm --version
   ```

2. **Test MCP package directly**:
   ```bash
   npx @modelcontextprotocol/server-everything --help
   ```

3. **Check Juno logs** for error messages

4. **Restart Juno** after adding servers

### Common Issues

- **404 npm errors**: The MCP package doesn't exist. Use verified packages listed above
- **Timeout errors**: Increase `timeout_seconds` in the configuration
- **Permission errors**: Ensure Juno has necessary permissions for file/system access

## Diagnostic Scripts

Two diagnostic scripts are available in `/scripts/`:

1. **test-mcp-diagnostics.js**: Tests MCP environment and configuration
2. **fix-mcp-config.js**: Provides ready-to-use MCP configurations

Run them with:
```bash
node scripts/test-mcp-diagnostics.js
node scripts/fix-mcp-config.js
```

## Verified Working MCP Servers

### No API Key Required
- `@modelcontextprotocol/server-everything` - General purpose testing
- `@modelcontextprotocol/server-filesystem` - File system operations
- `@modelcontextprotocol/server-memory` - Knowledge graph
- `@modelcontextprotocol/server-sequential-thinking` - Problem solving

### API Key Required
- `@modelcontextprotocol/server-brave-search` - Web search (BRAVE_SEARCH_API_KEY)
- `@modelcontextprotocol/server-google-maps` - Maps (GOOGLE_MAPS_API_KEY)
- `@modelcontextprotocol/server-postgres` - Database (POSTGRES_CONNECTION_STRING)

## References

- [MCP Documentation](https://modelcontextprotocol.io/)
- [Juno MCP Implementation](../src-tauri/src/agent/tools/mcp_integration.rs)
- [MCP Server Guide](./MCP-SERVER-GUIDE.md)
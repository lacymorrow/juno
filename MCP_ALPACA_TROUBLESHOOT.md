# MCP Alpaca Server Troubleshooting Guide

## Issue
The Alpaca MCP server is configured in Juno but not showing as connected.

## Root Cause
FastMCP was not installed in your Python virtual environment. I've installed it for you, so the server should now work.

## Steps to Verify MCP is Working in Juno

### 1. Start Juno in Development Mode
```bash
bun run tauri dev
```

### 2. Check MCP Server Status
1. Open Juno
2. Go to Settings (⌘,)
3. Navigate to Tools → MCP
4. Look for your "alpaca" server in the list

### 3. If Server Shows as "Disconnected" or "Error"

#### Option A: Restart the Server
1. Click the "Restart" button next to the alpaca server
2. Wait a few seconds for it to connect
3. The status should change to "Connected"

#### Option B: Re-enable the Server
1. Toggle the server off
2. Wait 2 seconds
3. Toggle it back on

### 4. Check Available Tools
Once connected, you should see Alpaca trading tools listed below the server. These include:
- `get_account_info` - Get account balances and status
- `get_positions` - View current positions
- `get_stock_quote` - Get real-time quotes
- `place_stock_order` - Place trades
- And many more...

### 5. Test the Tools
In Juno's main interface:
1. Press Alt+D to activate agent mode
2. Try a command like "Show me my Alpaca account info"
3. The agent should use the MCP tools to fetch your account data

## Troubleshooting Commands

If you still have issues, you can use these Tauri commands in the developer console:

```javascript
// Get MCP diagnostics
await window.__TAURI__.invoke('get_mcp_diagnostics')

// Test MCP server connection
await window.__TAURI__.invoke('test_mcp_server_connection', {
  config: {
    id: "mcp-alpaca",
    name: "alpaca",
    command: "/Users/lacymorrow/repo/alpaca-mcp-server/myvenv/bin/python",
    args: ["/Users/lacymorrow/repo/alpaca-mcp-server/alpaca_mcp_server.py"],
    working_directory: "/Users/lacymorrow/repo/alpaca-mcp-server",
    environment_variables: {
      ALPACA_API_KEY: "YOUR_KEY",
      ALPACA_SECRET_KEY: "YOUR_SECRET",
      ALPACA_PAPER_TRADE: "False"
    },
    enabled: true,
    auto_start: true,
    timeout_seconds: 30,
    max_retries: 3
  }
})

// Restart with diagnostics
await window.__TAURI__.invoke('restart_mcp_server_with_diagnostics', {
  server_id: "mcp-alpaca"
})
```

## Common Issues and Solutions

### Server Won't Start
- **Check Python Path**: Ensure `/Users/lacymorrow/repo/alpaca-mcp-server/myvenv/bin/python` exists
- **Check Script Path**: Ensure `/Users/lacymorrow/repo/alpaca-mcp-server/alpaca_mcp_server.py` exists
- **Check Logs**: Look in the Juno console for error messages

### Server Starts but No Tools
- This might indicate the MCP protocol communication is failing
- Check that the server is outputting proper JSON-RPC responses
- Try the test connection command above

### Permission Errors
- Ensure the Python script is readable
- Check that environment variables are being passed correctly

## Working MCP Server Requirements

For an MCP server to work with Juno, it must:
1. Implement the MCP protocol (JSON-RPC over stdio)
2. Handle these methods:
   - `initialize` - Server initialization
   - `tools/list` - List available tools
   - `tools/call` - Execute tool calls
3. Communicate via stdin/stdout
4. Return proper JSON-RPC formatted responses

Your Alpaca server uses FastMCP which handles all of this automatically.

## Next Steps

1. Restart Juno if it's currently running
2. The server should auto-start and connect
3. You'll see the Alpaca tools available in the MCP section
4. Start using voice commands or text to interact with your Alpaca account!

## Note on Security

Your API keys are stored in the app_settings.json file. Make sure to:
- Never commit this file to version control
- Keep your API keys secure
- Use paper trading for testing
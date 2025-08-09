# MCP Alpaca Server Connection Issue - Summary

## Issue
Your Alpaca MCP server works in Claude Desktop but not in Juno.

## Root Cause Found
The protocol version mismatch between Juno and your MCP server:
- **Juno was sending**: `"protocolVersion": "2025-03-26"`
- **Your server expects**: `"protocolVersion": "2024-11-05"`

## Fix Applied
Changed the protocol version in `/Users/lacymorrow/repo/juno/src-tauri/src/agent/tools/mcp_integration.rs` line 326 from `"2025-03-26"` to `"2024-11-05"`.

## Next Steps
1. **Restart Juno** to apply the changes:
   ```bash
   # If currently running, stop it with Ctrl+C
   bun run tauri dev
   ```

2. **Check MCP Connection**:
   - Go to Settings → Tools → MCP
   - Your "alpaca" server should now show as "Connected"
   - You should see all the Alpaca trading tools listed

3. **Test the Integration**:
   - Use Alt+D to activate agent mode
   - Try: "Show me my Alpaca account info"
   - The agent should now successfully use the MCP tools

## Why This Happened
The MCP protocol version `"2024-11-05"` is the current stable version that most MCP servers (including yours) implement. Juno was using a future/development version `"2025-03-26"` which your server doesn't recognize, causing it to reject the initialization request.

## Additional Notes
- The FastMCP installation I did earlier was helpful but wasn't the root cause
- Your MCP server configuration in Juno is correct
- No changes were needed to your working Alpaca MCP server
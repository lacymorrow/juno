# MCP Alpaca Server Fix

## Issue Summary

The MCP functionality wasn't working in Juno because the configured "alpaca" server is not actually an MCP server.

### Root Cause

The file `/Users/lacymorrow/repo/alpaca-mcp-server/alpaca_mcp_server.py` is just a regular Python script that uses the Alpaca trading API. It does NOT implement the MCP (Model Context Protocol), which requires:

1. JSON-RPC protocol implementation
2. MCP-specific methods (`initialize`, `tools/list`, `tools/call`)
3. Communication via stdin/stdout

### Solution Applied

1. **Removed the non-working alpaca configuration** from `app_settings.json`
2. **Added a working MCP server** ("everything" server) to verify MCP functionality works

### Next Steps

If you want to use Alpaca functionality via MCP, you have three options:

#### Option 1: Find an existing Alpaca MCP server
Search for a proper MCP server implementation that wraps Alpaca functionality.

#### Option 2: Create an MCP wrapper for your Alpaca script
Use the official Python MCP SDK to create a proper MCP server:

```bash
pip install mcp
```

Then create a new file `alpaca_mcp_wrapper.py`:

```python
import asyncio
from mcp.server import Server
from mcp.server.stdio import stdio_server

# Import your alpaca functionality
from alpaca_mcp_server import *

app = Server("alpaca-mcp")

@app.tool()
async def get_account_info():
    """Get Alpaca account information"""
    # Implement using your existing alpaca code
    pass

@app.tool()
async def place_order(symbol: str, qty: int, side: str):
    """Place an order via Alpaca"""
    # Implement using your existing alpaca code
    pass

# Add more tools as needed

async def main():
    async with stdio_server() as (read_stream, write_stream):
        await app.run(read_stream, write_stream)

if __name__ == "__main__":
    asyncio.run(main())
```

#### Option 3: Use existing working MCP servers
Juno now has the "everything" MCP server configured which provides various testing tools. You can add other working servers like:

- `@modelcontextprotocol/server-filesystem` - File operations
- `@modelcontextprotocol/server-memory` - Persistent memory
- `@modelcontextprotocol/server-sequential-thinking` - Problem solving

## Verification

To verify MCP is now working:

1. Restart Juno
2. Check the MCP tools section in settings
3. You should see tools from the "everything" server available

## Important Note

When configuring MCP servers in Juno, ensure they are actual MCP servers that implement the protocol, not just regular scripts. Valid MCP servers typically:

- Are npm packages starting with `@modelcontextprotocol/`
- Or are Python scripts that use the MCP SDK
- Communicate via JSON-RPC over stdin/stdout
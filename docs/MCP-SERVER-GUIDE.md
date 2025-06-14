# MCP Server Configuration Guide

This guide explains the MCP (Model Context Protocol) server configuration issues in Juno and provides solutions.

## The Problem

Many default MCP servers in the initial Juno configuration reference npm packages that don't exist, causing 404 errors during startup. This results in log messages like:

```
npm error 404 Not Found - GET https://registry.npmjs.org/@modelcontextprotocol%2fserver-time
npm error 404 Not Found - GET https://registry.npmjs.org/@modelcontextprotocol%2fserver-git
npm error 404 Not Found - GET https://registry.npmjs.org/mcp-server-sqlite
```

## The Solution

We've updated the default configuration to use only verified, working MCP servers.

## Working MCP Servers

### Core Servers (Confirmed Working)

1. **@modelcontextprotocol/server-everything**
   - Comprehensive testing server with all MCP features
   - Perfect for testing and development
   ```bash
   npx @modelcontextprotocol/server-everything
   ```

2. **@modelcontextprotocol/server-filesystem**
   - Secure file operations
   ```bash
   npx @modelcontextprotocol/server-filesystem /Users
   ```

3. **@modelcontextprotocol/server-memory**
   - Persistent knowledge graph
   ```bash
   npx @modelcontextprotocol/server-memory
   ```

4. **@modelcontextprotocol/server-sequential-thinking**
   - Problem-solving capabilities
   ```bash
   npx @modelcontextprotocol/server-sequential-thinking
   ```

### Integration Servers (Require API Keys)

5. **@modelcontextprotocol/server-brave-search**
   - Web search capabilities
   - Requires: `BRAVE_SEARCH_API_KEY`
   ```bash
   npx @modelcontextprotocol/server-brave-search
   ```

6. **@modelcontextprotocol/server-google-maps**
   - Location and mapping services
   - Requires: `GOOGLE_MAPS_API_KEY`
   ```bash
   npx @modelcontextprotocol/server-google-maps
   ```

7. **@modelcontextprotocol/server-postgres**
   - PostgreSQL database integration
   - Requires: `POSTGRES_CONNECTION_STRING`
   ```bash
   npx @modelcontextprotocol/server-postgres
   ```

## Removed Non-Working Servers

These servers were removed because they don't exist on npm:

- ❌ `@modelcontextprotocol/server-time`
- ❌ `@modelcontextprotocol/server-git` 
- ❌ `@modelcontextprotocol/server-fetch`
- ❌ `mcp-server-sqlite`
- ❌ `calculator-mcp`
- ❌ `mcp-weather`

## Configuration Examples

### Minimal Working Configuration
```json
{
  "mcpServers": {
    "everything": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-everything"]
    },
    "filesystem": {
      "command": "npx", 
      "args": ["@modelcontextprotocol/server-filesystem", "/Users"]
    }
  }
}
```

### Enhanced Configuration with API Keys
```json
{
  "mcpServers": {
    "everything": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-everything"]
    },
    "filesystem": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-filesystem", "/Users"]
    },
    "brave-search": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_SEARCH_API_KEY": "your-api-key-here"
      }
    },
    "postgres": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-postgres"],
      "env": {
        "POSTGRES_CONNECTION_STRING": "postgresql://user:pass@host:port/db"
      }
    }
  }
}
```

## Alternative Servers

If you need functionality similar to the removed servers, try these community alternatives:

### For SQLite functionality:
- Custom local server using Python/Node.js
- Use postgres server with local PostgreSQL

### For Weather functionality:
- Use brave-search to find weather information
- Custom weather server using OpenWeatherMap API

### For Git functionality:
- Use filesystem server to access git repositories
- Custom git server implementation

### For Time functionality:
- Use sequential-thinking server for time-based reasoning
- Built-in system time functions

## Troubleshooting

### If servers still fail to start:

1. **Check Node.js installation:**
   ```bash
   node --version
   npm --version
   ```

2. **Clear npm cache:**
   ```bash
   npm cache clean --force
   ```

3. **Test individual servers:**
   ```bash
   npx @modelcontextprotocol/server-everything --help
   ```

4. **Check network connectivity:**
   - Ensure access to npmjs.org
   - Check firewall settings

### Common Error Messages:

- **"404 Not Found"**: Package doesn't exist - use working alternatives
- **"ENOTFOUND"**: Network issue - check internet connection
- **"Permission denied"**: File system permissions - check path access
- **"Module not found"**: Missing dependencies - try npm install

## Getting API Keys

### Brave Search API
1. Go to https://brave.com/search/api/
2. Sign up for an account
3. Get your API key from the dashboard

### Google Maps API
1. Go to https://console.cloud.google.com/
2. Create a new project or select existing
3. Enable Maps JavaScript API
4. Create credentials (API Key)

### PostgreSQL
- For local: `postgresql://user:password@localhost:5432/database`
- For cloud: Get connection string from your provider

## Best Practices

1. **Start Simple**: Begin with just the "everything" server
2. **Add Gradually**: Add one server at a time to test functionality
3. **Use Environment Variables**: Store API keys securely
4. **Monitor Logs**: Check server startup logs for issues
5. **Test Individually**: Test each server independently before integration

## Future Considerations

The MCP ecosystem is rapidly evolving. For the latest servers:

1. Check the official repository: https://github.com/modelcontextprotocol/servers
2. Browse npm packages: https://www.npmjs.com/search?q=mcp
3. Follow MCP documentation: https://modelcontextprotocol.io/

## Support

If you encounter issues:

1. Check this guide first
2. Review the Juno logs
3. Test servers independently
4. Check the MCP official documentation
5. File issues with specific error messages and configurations

## Summary

The MCP server configuration has been updated to use only verified, working packages. This eliminates the 404 errors and provides a stable foundation for extending Juno's capabilities. Start with the basic configuration and gradually add more servers as needed.
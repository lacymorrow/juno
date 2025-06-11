# WebSocket Test Server

A Node.js WebSocket server for testing Juno's cloud connectivity features.

## Quick Start

```bash
# Navigate to the test server directory
cd websocket-test

# Start the server (installs dependencies automatically)
./start-server.sh
```

## Server Details

- **Location**: `./websocket-test/`
- **Port**: 8080 (default)
- **WebSocket Endpoint**: `ws://localhost:8080`
- **Health Check**: `http://localhost:8080/health`

## Testing with Juno

1. **Start the test server**:
   ```bash
   cd websocket-test && ./start-server.sh
   ```

2. **Configure Juno**:
   - Open Juno's DevTools panel
   - Go to Cloud Test Panel
   - Set server URL to: `ws://localhost:8080`
   - Enable cloud connectivity

3. **Run tests**:
   - Use the "Quick Test" button for basic connectivity
   - Try different command types in the testing tab
   - Test remote commands in the remote tab

## Supported Features

- ✅ WebSocket connection handling
- ✅ Authentication simulation
- ✅ Command processing (text_query, voice_query, system_command, etc.)
- ✅ Heartbeat system
- ✅ Error handling
- ✅ Real-time logging
- ✅ Health monitoring

## Test Commands

The server handles all Juno command types:

### Text Query
```json
{
  "command_type": "text_query",
  "payload": {
    "query": "Hello from Juno"
  }
}
```

### System Command
```json
{
  "command_type": "system_command", 
  "payload": {
    "action": "screenshot"
  }
}
```

### Voice Query
```json
{
  "command_type": "voice_query",
  "payload": {
    "audio_base64": "..."
  }
}
```

## Development

The test server simulates a production cloud backend, allowing you to:

- Test WebSocket connectivity
- Verify command formatting
- Debug authentication flows
- Test error handling
- Monitor connection stability

For detailed documentation, see `./websocket-test/README.md`.

## Integration

This server integrates with:

- Juno's Cloud Test Panel (`src/components/devtools/CloudTestPanel.tsx`)
- Production Cloud Connector (`src-tauri/src/cloud/connector.rs`)
- Cloud command types (`src-tauri/src/cloud/types.rs`)

## Files

- `websocket-test/server.js` - Main WebSocket server
- `websocket-test/test-client.js` - Test client for validation
- `websocket-test/start-server.sh` - Startup script
- `websocket-test/package.json` - Dependencies 

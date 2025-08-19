# Juno WebSocket Test Server

Note: For deployment, use the consolidated `remote-control-server/` directory which provides a minimal, deployment-ready server with Docker configs. This `websocket-test/` folder remains for local experiments and ad-hoc testing scripts.

A Node.js WebSocket server for testing Juno AI's cloud connectivity and remote command functionality.

## Features

- **WebSocket Server**: Full WebSocket implementation compatible with Juno's cloud protocol
- **Command Processing**: Handles all Juno command types (voice_query, text_query, system_command, etc.)
- **Authentication Simulation**: Simulates device authentication and token management
- **Heartbeat System**: Automatic heartbeat monitoring and response
- **Real-time Logging**: Detailed connection and message logging
- **Health Endpoints**: HTTP endpoints for status monitoring

## Supported Command Types

- `voice_query` - Voice-based queries with simulated audio processing
- `text_query` - Text-based queries with echo responses
- `system_command` - System automation commands (screenshot, click, type, etc.)
- `status_request` - Server status and health information
- `screenshot` - Simulated screenshot capture
- `config_update` - Configuration update handling

## Quick Start

### 1. Install Dependencies

```bash
# Copy the package configuration
cp websocket-server-package.json package-ws.json

# Install dependencies
npm install --package-lock-only ws uuid nodemon
```

### 2. Start the Server

```bash
# Run the server
node websocket-test-server.js

# Or with auto-reload during development
npx nodemon websocket-test-server.js
```

### 3. Server Information

- **WebSocket Endpoint**: `ws://localhost:8080/ws`
- **Health Endpoint**: `http://localhost:8080/health`
- **Status Endpoint**: `http://localhost:8080/status`

## Testing with Juno

### 1. Update Juno Cloud Configuration

In the Juno app's Cloud Test Panel:

1. Set server URL to: `ws://localhost:8080/ws`
2. Enable cloud connectivity
3. Start the cloud connector

### 2. Run Tests

Use the Cloud Test Panel to:

- **Quick Test**: Run basic connectivity tests
- **WebSocket Test**: Test raw WebSocket connections
- **Command Test**: Send various command types
- **Remote Commands**: Test system automation commands
- **Test Suite**: Run comprehensive tests

### 3. Example Test Commands

#### Text Query
```json
{
  "query": "Hello from Juno AI"
}
```

#### System Command (Screenshot)
```json
{
  "action": "screenshot"
}
```

#### System Command (Click)
```json
{
  "action": "click",
  "x": 100,
  "y": 200
}
```

#### System Command (Type)
```json
{
  "action": "type",
  "text": "Hello World"
}
```

## Message Protocol

### WebSocket Message Format
```json
{
  "type": "command|response|status|heartbeat|auth|error",
  "data": { ... },
  "timestamp": 1234567890
}
```

### Command Format
```json
{
  "id": "uuid-v4",
  "command_type": "text_query|voice_query|system_command|...",
  "payload": {
    "query": "optional query string",
    "action": "optional action type",
    "parameters": { ... }
  },
  "timestamp": 1234567890
}
```

### Response Format
```json
{
  "command_id": "uuid-v4",
  "status": "success|error|in_progress|cancelled",
  "data": {
    "text": "response text",
    "metadata": { ... }
  },
  "timestamp": 1234567890
}
```

## Server Logs

The server provides detailed logging:

```
🚀 Juno WebSocket Test Server running on port 8080
📡 WebSocket endpoint: ws://localhost:8080
🔧 Ready to test cloud connectivity and commands

Client connected: abc123-def456 from ::1
Handling command: text_query (cmd-789)
Sent response for command cmd-789
```

## Environment Variables

- `PORT`: Server port (default: 8080)

## Troubleshooting

### Connection Issues

1. **Firewall**: Ensure port 8080 is not blocked
2. **Port Conflicts**: Change PORT environment variable if needed
3. **WebSocket Support**: Verify client supports WebSocket protocol

### Authentication Issues

1. **Device ID**: Server auto-generates device IDs for testing
2. **Tokens**: All authentication requests are automatically approved

### Command Processing Issues

1. **Message Format**: Ensure JSON is properly formatted
2. **Command Types**: Use exact command type strings (snake_case)
3. **Payload Structure**: Follow the expected payload format for each command type

## Integration with Juno

This server simulates the cloud backend that Juno connects to. It:

1. **Accepts WebSocket connections** from Juno devices
2. **Authenticates devices** automatically for testing
3. **Processes commands** sent from Juno
4. **Sends responses** back to Juno
5. **Maintains heartbeat** to keep connections alive

## Development

### Adding New Command Types

1. Add command type to `CloudCommandType` object
2. Add handling logic in `handleCloudCommand` function
3. Update documentation and tests

### Modifying Response Format

1. Update `createDeviceResponse` function
2. Ensure compatibility with Juno's expected format
3. Test with actual Juno client

### Custom Port

```bash
PORT=9000 node websocket-test-server.js
```

## License

MIT License - Part of the Juno AI project. 

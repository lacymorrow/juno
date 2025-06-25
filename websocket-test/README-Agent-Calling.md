# Juno AI Agent Calling Guide

This guide shows you how to call your Juno AI agent using both WebSocket and HTTP approaches.

## 🎯 **Quick Start**

### WebSocket Approach (Recommended)

```bash
node call-agent.js "Your question here"
```

### Examples

```bash
# Default query
node call-agent.js

# Custom query
node call-agent.js "What's the weather like today?"

# Coding help
node call-agent.js "Help me debug this JavaScript function"

# Complex query
node call-agent.js "Can you analyze this CSV data and create a summary report?"
```

## 🔧 **Configuration**

Your credentials (already configured in the scripts):

- **API Key**: `eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0`
- **HMAC Secret**: `7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244`
- **Server**: `wss://juno-cloud-backend.fly.dev/ws`

## 📡 **WebSocket Protocol**

### Authentication Flow

1. Connect to WebSocket
2. Send HMAC-signed auth message
3. Receive auth confirmation
4. Send agent query
5. Receive response

### Message Format

```javascript
// Authentication
{
  "type": "auth",
  "data": {
    "api_key": "your-api-key",
    "timestamp": 1234567890,
    "signature": "hmac-signature",
    "method": "POST",
    "path": "/ws/auth",
    "body": ""
  }
}

// Agent Query
{
  "type": "command",
  "data": {
    "id": "unique-uuid",
    "command_type": "text_query",
    "payload": {
      "query": "Your question here"
    },
    "timestamp": 1234567890
  }
}

// Response
{
  "type": "response",
  "data": {
    "command_id": "unique-uuid",
    "status": "success",
    "data": {
      "text": "AI agent response here",
      "agent_state": {
        "status": "completed"
      }
    }
  }
}
```

## 🌐 **HTTP/cURL Approach**

### Available Endpoints

Based on the server code, these HTTP endpoints are available:

```bash
# Health check
curl https://juno-cloud-backend.fly.dev/health

# Device registration
curl -X POST https://juno-cloud-backend.fly.dev/api/register \
  -H "Content-Type: application/json" \
  -d '{
    "device_name": "My Device",
    "device_type": "desktop",
    "user_email": "test@example.com"
  }'

# Authentication
curl -X POST https://juno-cloud-backend.fly.dev/api/auth \
  -H "Content-Type: application/json" \
  -d '{
    "api_key": "your-api-key",
    "timestamp": 1234567890,
    "signature": "hmac-signature"
  }'
```

**Note**: There doesn't appear to be a direct HTTP endpoint for agent queries - the server is designed primarily for WebSocket communication.

## 🔐 **HMAC Signature Generation**

### JavaScript/Node.js

```javascript
const crypto = require('crypto');

function generateHmacSignature(method, path, body, timestamp, hmacSecret) {
    const payload = `${method}:${path}:${body || ''}:${timestamp}`;
    return crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
}
```

### Bash (with openssl)

```bash
PAYLOAD="POST:/ws/auth::1234567890"
SIGNATURE=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$HMAC_SECRET" -binary | xxd -p -c 256)
```

### Python

```python
import hmac
import hashlib

def generate_hmac_signature(method, path, body, timestamp, hmac_secret):
    payload = f"{method}:{path}:{body or ''}:{timestamp}"
    return hmac.new(
        hmac_secret.encode(), 
        payload.encode(), 
        hashlib.sha256
    ).hexdigest()
```

## 🛠 **Command Types**

Your Juno backend supports these command types:

- **`text_query`**: Send text to the AI agent
- **`voice_query`**: Send voice data to the AI agent  
- **`screenshot`**: Request a screenshot
- **`system_command`**: Execute system commands
- **`status_request`**: Get server status
- **`config_update`**: Update configuration

## 📊 **Response Types**

Responses include these status values:

- **`in_progress`**: Command is being processed
- **`success`**: Command completed successfully
- **`error`**: Command failed
- **`cancelled`**: Command was cancelled

## 🚨 **Rate Limiting**

The server has rate limiting enabled:

- You'll get `RATE_LIMIT_EXCEEDED` errors if you send too many requests
- Wait for the cooldown period before retrying

## 🎯 **Complete Working Example**

```bash
#!/bin/bash
# Save this as 'query-agent.sh' and make executable

node -e "
const WebSocket = require('ws');
const crypto = require('crypto');
const { v4: uuidv4 } = require('uuid');

const query = process.argv[1] || 'Hello AI!';
const API_KEY = 'eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0';
const HMAC_SECRET = '7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244';

function generateHmacSignature(method, path, body, timestamp, hmacSecret) {
    const payload = \`\${method}:\${path}:\${body || ''}:\${timestamp}\`;
    return crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
}

const ws = new WebSocket('wss://juno-cloud-backend.fly.dev/ws');

ws.on('open', () => {
    const timestamp = Math.floor(Date.now() / 1000);
    const signature = generateHmacSignature('POST', '/ws/auth', '', timestamp, HMAC_SECRET);
    
    ws.send(JSON.stringify({
        type: 'auth',
        data: { api_key: API_KEY, timestamp, signature, method: 'POST', path: '/ws/auth', body: '' }
    }));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    if (msg.type === 'auth' && msg.data.success) {
        ws.send(JSON.stringify({
            type: 'command',
            data: {
                id: uuidv4(),
                command_type: 'text_query',
                payload: { query },
                timestamp: Math.floor(Date.now() / 1000)
            }
        }));
    } else if (msg.type === 'response' && msg.data.status === 'success') {
        console.log('AI Response:', msg.data.data.text);
        ws.close();
    }
});
" "$1"
```

## 🎉 **Success!**

Your Juno AI cloud backend is fully operational and ready to receive agent queries!

Use the `call-agent.js` script for the most reliable experience.

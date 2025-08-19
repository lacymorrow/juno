# Juno Cloud Backend Server

Note: If you need a minimal, deployment-ready remote control server without authentication/DB, use `remote-control-server/`. This `backend-server/` is the production backend with full features (auth, DB, metrics, rate limit, etc.).

A production-ready Node.js WebSocket backend for the Juno AI Computer Use Agent, providing authentication, cloud control, and premium features.

## ✅ Production Status

**DEPLOYED**: <https://juno-cloud-backend.fly.dev> (Fly.io)

- Status: ✅ Healthy and operational
- Region: Atlanta (atl)
- Health endpoint: `https://juno-cloud-backend.fly.dev/health`

## 🌟 Features

### Core Functionality

- **WebSocket Server**: Real-time bidirectional communication with Tauri clients
- **Device Authentication**: HMAC-signed API key authentication with JWT sessions
- **Command Processing**: Full support for Anthropic Computer Use commands
- **Health Monitoring**: Comprehensive system health checks and metrics
- **Security**: Rate limiting, CORS, validation, and audit logging

### Supported Commands

- `voice_query` - Voice-to-text processing and AI responses
- `text_query` - Direct text-based AI queries
- `system_command` - System automation commands
- `screenshot` - Screen capture requests
- `status_request` - System status inquiries
- `config_update` - Configuration changes

### Security Features

- HMAC signature validation for all requests
- JWT token-based session management
- Rate limiting with configurable thresholds
- Comprehensive audit logging
- Input validation and sanitization
- CORS protection

## 📁 Project Structure

```
backend-server/
├── src/
│   ├── auth/
│   │   └── AuthService.js          # Device authentication & JWT management
│   ├── database/
│   │   └── Database.js             # SQLite database operations
│   ├── middleware/
│   │   └── rateLimiter.js          # Request rate limiting
│   ├── services/
│   │   ├── CommandProcessor.js     # Command handling logic
│   │   └── HealthCheck.js          # System health monitoring
│   ├── utils/
│   │   ├── logger.js               # Winston logging configuration
│   │   └── validation.js           # Input validation utilities
│   ├── websocket/
│   │   └── WebSocketManager.js     # WebSocket connection management
│   └── server.js                   # Main server entry point
├── data/                           # SQLite database storage
├── logs/                           # Application logs
├── Dockerfile                      # Container configuration
├── docker-compose.yml              # Docker Compose setup
├── UNRAID_DEPLOYMENT.md           # Unraid deployment guide
└── README.md                       # This file
```

## 🚀 Quick Start

### Prerequisites

- Node.js 20 or higher
- npm or yarn package manager

### Installation

1. **Install dependencies:**

   ```bash
   npm install
   ```

2. **Configure environment:**

   ```bash
   cp env.example .env
   # Edit .env with your configuration
   ```

3. **Create required directories:**

   ```bash
   mkdir -p data logs
   ```

4. **Start the server:**

   ```bash
   npm start
   ```

The server will be available at:

- **HTTP/WebSocket**: `http://localhost:8080`
- **WebSocket Endpoint**: `ws://localhost:8080/ws`
- **Health Check**: `http://localhost:8080/health`
- **Metrics**: `http://localhost:8080/metrics`

## 🔧 Configuration

### Environment Variables

Key configuration options in `.env`:

```bash
# Server Settings
NODE_ENV=production
PORT=8080
HOST=0.0.0.0

# Authentication
JWT_SECRET=your-super-secure-jwt-secret
HMAC_SECRET=your-hmac-secret-for-device-auth

# Database
DB_PATH=./data/juno.db

# Security
CORS_ORIGIN=*
RATE_LIMIT_MAX_REQUESTS=100

# Logging
LOG_LEVEL=info
LOG_FILE=./logs/server.log
```

## 📡 API Endpoints

### Device Registration

```bash
POST /api/register
Content-Type: application/json

{
  "device_name": "MacBook Pro",
  "device_type": "desktop",
  "platform": "macos"
}
```

**Response:**

```json
{
  "success": true,
  "device_id": "uuid",
  "api_key": "hex_string",
  "hmac_secret": "hex_string",
  "message": "Device registered successfully"
}
```

### Device Authentication

```bash
POST /api/auth
Content-Type: application/json

{
  "api_key": "your_api_key",
  "timestamp": 1749496859,
  "signature": "hmac_signature",
  "method": "POST",
  "path": "/api/auth"
}
```

**Response:**

```json
{
  "success": true,
  "token": "jwt_token",
  "device_id": "uuid",
  "device_name": "device_name",
  "permissions": ["text_processing", "voice_transcription", ...],
  "expires_at": 1749583264,
  "session_id": "uuid"
}
```

## 🔌 WebSocket Protocol

### Connection

Connect to `ws://localhost:8080/ws`

### Authentication Flow

1. **Connect** to WebSocket endpoint
2. **Receive** welcome message with client ID
3. **Send** authentication message with JWT token
4. **Receive** authentication confirmation
5. **Exchange** command messages

### Message Format

All WebSocket messages follow this structure:

```json
{
  "type": "message_type",
  "data": { /* message payload */ },
  "timestamp": 1749496859
}
```

### Message Types

#### Client → Server

- `authenticate` - Authenticate with JWT token
- `voice_query` - Process voice input
- `text_query` - Process text input
- `system_command` - Execute system command
- `screenshot` - Request screenshot
- `status_request` - Get system status
- `config_update` - Update configuration
- `heartbeat` - Respond to server ping

#### Server → Client

- `status` - Welcome message with capabilities
- `authenticated` - Authentication successful
- `heartbeat` - Keep-alive ping (every 30s)
- `command_response` - Response to commands
- `error` - Error messages

## 🐳 Docker Deployment

### Using Docker Compose

```bash
docker-compose up -d
```

### Manual Docker Build

```bash
docker build -t juno-cloud-backend .
docker run -p 8080:8080 -v $(pwd)/data:/app/data juno-cloud-backend
```

## 📊 Monitoring

### Health Check

```bash
curl http://localhost:8080/health | jq .
```

### Metrics

```bash
curl http://localhost:8080/metrics | jq .
```

### Logs

```bash
tail -f logs/server.log
```

## 🔒 Security

### HMAC Authentication

All API requests must include HMAC signatures:

1. **Create payload**: `METHOD:PATH:BODY:TIMESTAMP`
2. **Generate signature**: `HMAC-SHA256(payload, hmac_secret)`
3. **Include in request**: `{"api_key": "...", "timestamp": 123, "signature": "..."}`

### Rate Limiting

- **Default**: 100 requests per 15 minutes per IP
- **Configurable**: Via `RATE_LIMIT_*` environment variables
- **Backend**: Memory-based (Redis optional for scaling)

### Audit Logging

All authentication attempts and command executions are logged to the database with:

- Device ID and user information
- Timestamp and IP address
- Action details and success/failure status
- Request metadata

## 🚀 Deployment to Unraid

### Deployment Documentation

- **[FLY_DEPLOYMENT_RULES.md](FLY_DEPLOYMENT_RULES.md)** - Complete Fly.io deployment rules and guidelines
- **[QUICK_DEPLOY.md](QUICK_DEPLOY.md)** - Fast deployment guide for multiple platforms
- **[CLOUD_DEPLOYMENT.md](CLOUD_DEPLOYMENT.md)** - Comprehensive cloud deployment options
- **[UNRAID_DEPLOYMENT.md](UNRAID_DEPLOYMENT.md)** - Unraid container deployment instructions

For production deployments, see [FLY_DEPLOYMENT_RULES.md](FLY_DEPLOYMENT_RULES.md) for complete rules and guidelines.

- Environment setup
- Port forwarding
- SSL/reverse proxy setup

## 🛠️ Development

### Running in Development Mode

```bash
# Set NODE_ENV=development in .env
npm run dev
```

### Testing Authentication

```javascript
// Generate HMAC signature
const crypto = require('crypto');
const payload = `POST:/api/auth::${timestamp}`;
const signature = crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
```

### Database Schema

The server automatically creates SQLite tables for:

- `users` - User accounts
- `devices` - Registered devices
- `sessions` - Active JWT sessions
- `commands` - Command execution history
- `audit_logs` - Security audit trail
- `subscriptions` - Premium feature tracking

## 📝 License

MIT License - see LICENSE file for details.

## 🤝 Integration with Juno AI

This backend server is designed to work seamlessly with the Juno AI Computer Use Agent:

1. **Tauri Client**: Connects via WebSocket for real-time control
2. **Authentication**: Automatic device registration and session management
3. **Command Processing**: Handles all Anthropic Computer Use action types
4. **Cloud Features**: Premium functionality gating and user management

For the complete Juno AI system, see the main repository documentation.

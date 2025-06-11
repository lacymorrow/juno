# 🌐 Juno App ↔ Fly.io Backend Connection Guide

## ✅ Connection Setup Complete

Your Juno AI Computer Use Agent is now fully connected to your production Fly.io backend!

---

## 🔧 Configuration Changes Made

### 1. **Backend URLs Updated**

**Files Modified:**

- `src-tauri/src/cloud/config.rs` - Default server URL
- `src-tauri/src/constants.rs` - Cloud server constant
- `CLOUD_CONTROL_AUTH_MECHANISM_ANALYSIS.md` - Documentation reference

**Changed From:**

```
wss://juno-cloud.shipkit.io/ws
```

**Changed To:**

```
wss://juno-cloud-backend.fly.dev/ws
```

### 2. **Production Backend Details**

| Component | URL | Status |
|-----------|-----|---------|
| **Main Backend** | <https://juno-cloud-backend.fly.dev> | ✅ Running |
| **Health Endpoint** | <https://juno-cloud-backend.fly.dev/health> | ✅ Healthy |
| **Metrics Endpoint** | <https://juno-cloud-backend.fly.dev/metrics> | ✅ Active |
| **API Registration** | <https://juno-cloud-backend.fly.dev/api/register> | ✅ Working |
| **WebSocket Endpoint** | wss://juno-cloud-backend.fly.dev/ws | ✅ Ready |

---

## 🚀 Testing Your Connection

### 1. **Quick Connection Test**

Run the automated test script:

```bash
./test-cloud-connection.sh
```

### 2. **Manual Backend Testing**

Test individual endpoints:

```bash
# Health check
curl https://juno-cloud-backend.fly.dev/health

# Device registration
curl -X POST -H "Content-Type: application/json" \
  -d '{"device_name":"test","device_type":"desktop","platform":"macos"}' \
  https://juno-cloud-backend.fly.dev/api/register
```

### 3. **App Testing**

**Start the app with cloud debugging:**

```bash
RUST_LOG=debug bun run tauri dev
```

**Test connection in the app:**

1. Open **Dev Tools** → **Cloud Test Panel**
2. Check connection status
3. Test WebSocket connection
4. Verify device registration

---

## 🔌 How the Connection Works

### 1. **Architecture Overview**

```
┌─────────────────┐    WebSocket     ┌─────────────────────┐
│   Juno App      │◄────────────────►│  Fly.io Backend     │
│  (Tauri/Rust)   │   TLS/WSS        │  (Node.js/Express)  │
│                 │                  │                     │
│ • Cloud Client  │                  │ • WebSocket Server  │
│ • Auth System   │                  │ • Authentication    │
│ • Tool Executor │                  │ • Command Router    │
└─────────────────┘                  └─────────────────────┘
```

### 2. **Connection Flow**

1. **App Startup**: Cloud client initializes with Fly.io URL
2. **WebSocket Connect**: Secure connection to `wss://juno-cloud-backend.fly.dev/ws`
3. **Authentication**: Device registration with API key
4. **Command Channel**: Real-time bidirectional communication
5. **Tool Execution**: Remote computer use commands

### 3. **Key Components**

**App Side (Tauri):**

- `src-tauri/src/cloud/client.rs` - WebSocket client
- `src-tauri/src/cloud/connector.rs` - Production connector
- `src-tauri/src/cloud/auth.rs` - Device authentication
- `src-tauri/src/cloud/commands.rs` - Command processing

**Backend Side (Fly.io):**

- `backend-server/src/websocket/WebSocketManager.js` - Connection handling
- `backend-server/src/auth/AuthService.js` - Authentication
- `backend-server/src/services/CommandProcessor.js` - Command execution

---

## 🛠️ Development Workflow

### 1. **Local Development**

```bash
# Start app with debug logging
RUST_LOG=debug bun run tauri dev

# Monitor backend logs
flyctl logs --app juno-cloud-backend --follow
```

### 2. **Testing Changes**

```bash
# Compile Rust changes (REQUIRED after any Rust changes)
cargo check --manifest-path src-tauri/Cargo.toml

# Test connection
./test-cloud-connection.sh

# Run app tests
bun run test
```

### 3. **Backend Updates**

When updating the backend:

```bash
cd backend-server
flyctl deploy
./check-deployment.sh  # Verify deployment
```

---

## 🔒 Security Features

### 1. **Transport Security**

- **TLS/WSS**: All communication encrypted in transit
- **Certificate Validation**: Automatic SSL certificate verification
- **Secure WebSocket**: WSS protocol prevents MITM attacks

### 2. **Authentication**

- **Device Registration**: Unique device IDs and API keys
- **JWT Tokens**: Stateless authentication with expiration
- **HMAC Validation**: Message integrity verification

### 3. **Command Security**

- **Whitelist System**: Only approved commands allowed
- **Security Levels**: Low/Medium/High command validation
- **Audit Logging**: All commands logged with metadata

---

## 🐛 Troubleshooting

### 1. **Connection Issues**

**Symptoms**: Connection timeouts, WebSocket errors
**Solutions**:

```bash
# Check backend health
curl https://juno-cloud-backend.fly.dev/health

# Verify WebSocket connectivity
# (Install websocat: brew install websocat)
websocat wss://juno-cloud-backend.fly.dev/ws

# Check app logs
RUST_LOG=debug bun run tauri dev
```

### 2. **Authentication Issues**

**Symptoms**: Device registration failures, auth errors
**Solutions**:

- Check API keys in app state
- Verify device name uniqueness
- Clear app data and re-register

### 3. **Command Execution Issues**

**Symptoms**: Commands not executing, timeout errors
**Solutions**:

- Check security level settings
- Verify command is in allowed list
- Check backend command processor logs

---

## 📋 App Configuration

### 1. **Cloud Settings Location**

Configuration is stored in:

- **Tauri State**: In-memory during app runtime
- **Config Files**: Persistent storage in app data directory
- **Environment**: Debug/production mode detection

### 2. **Default Configuration**

```rust
CloudConfig {
    enabled: false,  // Manually enable in app
    server_url: "wss://juno-cloud-backend.fly.dev/ws",
    device_name: "Juno-{hostname}",
    auto_connect: true,
    security_level: Medium,
    heartbeat_interval: 60,  // seconds
    reconnect_interval: 30,  // seconds
}
```

### 3. **Enabling Cloud Features**

In the app:

1. Open **Settings** → **Cloud Control**
2. Enable **Cloud Connectivity**
3. Configure **Device Name**
4. Set **Security Level**
5. Click **Connect**

---

## 🌟 Next Steps

### 1. **Immediate Actions**

- [ ] Test the connection using the Dev Tools Cloud Panel
- [ ] Enable cloud features in app settings
- [ ] Verify remote command execution
- [ ] Set up any desired security constraints

### 2. **Optional Enhancements**

- [ ] Set up monitoring and alerting for the backend
- [ ] Configure premium features if needed
- [ ] Set up development vs production environments
- [ ] Add custom command workflows

### 3. **Scaling Considerations**

- [ ] Monitor backend resource usage
- [ ] Set up database backups
- [ ] Configure rate limiting based on usage
- [ ] Consider multi-region deployment for global users

---

## 📞 Support Resources

### **Backend (Fly.io)**

- **Status**: `flyctl status --app juno-cloud-backend`
- **Logs**: `flyctl logs --app juno-cloud-backend`
- **Scale**: `flyctl scale --app juno-cloud-backend`

### **App (Local)**

- **Debug Mode**: `RUST_LOG=debug bun run tauri dev`
- **Cloud Panel**: Dev Tools → Cloud Test Panel
- **Connection Test**: `./test-cloud-connection.sh`

### **Documentation**

- **Backend Rules**: [backend-server/FLY_DEPLOYMENT_RULES.md](backend-server/FLY_DEPLOYMENT_RULES.md)
- **Backend Status**: [backend-server/DEPLOYMENT_SUMMARY.md](backend-server/DEPLOYMENT_SUMMARY.md)
- **App Architecture**: [ARCHITECTURE.md](ARCHITECTURE.md)

---

## 🎉 Success

Your Juno AI Computer Use Agent is now connected to a production-grade cloud backend on Fly.io. The connection is secure, scalable, and ready for real-world use.

**Key Benefits:**

- ✅ **Remote Control**: Control your computer from anywhere
- ✅ **Cloud Commands**: Execute AI-powered computer use remotely
- ✅ **Secure Communication**: End-to-end encrypted WebSocket connection
- ✅ **Production Ready**: Deployed on enterprise-grade infrastructure
- ✅ **Scalable**: Can handle multiple devices and concurrent connections

**Start using your cloud-connected Juno app today!** 🚀

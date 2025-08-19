# Juno Remote Control Server (Deployment-Ready)

A minimal WebSocket server to remotely control Juno via the native cloud connector protocol. It consolidates the examples into a single, clean directory for deployment.

## Features
- Command processing: `text_query`, `voice_query`, `system_command`, `status_request`, `screenshot`, `config_update`
- Heartbeats and status messages
- Simple auth (accept-all, token echo) for testing
- Minimal dependencies; Node 18+

## Quick Start
1. Install
```bash
cd remote-control-server
npm install
```
2. Run
```bash
npm run start # or: PORT=9000 npm run start
```
Configure Juno to connect to `ws://localhost:8080` from the Cloud Test Panel.

## Docker
```bash
docker build -t juno-remote-control .
docker run -p 8080:8080 juno-remote-control
```

## Environment
- PORT (default: 8080)

## Notes
- For production features (auth, DB, rate limits, metrics), see `backend-server/`.
- Message protocol mirrors the format used by Juno's native cloud connector.

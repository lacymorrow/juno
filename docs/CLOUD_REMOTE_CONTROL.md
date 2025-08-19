# Cloud Remote Control – Servers, Auth, and Deployment

This document consolidates how to run and deploy Juno’s cloud remote control servers.

## Servers Overview

- backend-server/ (Production)
  - WebSocket: `wss://<host>/ws`
  - Auth: Device HMAC/JWT (built-in) + optional GitHub OAuth (user session)
  - Storage: SQLite (with Fly.io volume)
  - Health: `GET /health`
  - Use this for production deployments.

- remote-control-server/ (Minimal)
  - WebSocket: `ws://<host>/ws`
  - Auth: none (intentionally minimal)
  - Health: `GET /health`
  - Use for internal testing only.

## Client Configuration

- Juno app URL format: `ws://<host>/ws` or `wss://<host>/ws` (note `/ws` path)
- Health URL mapping is derived: `/ws` → `/health`

## Production Auth

- Device HMAC/JWT (backend-server)
  - Register device via `/api/register`, authenticate via `/api/auth`
  - WebSocket messages use the session after auth

- GitHub OAuth (optional, backend-server)
  - `GET /auth/github/login` → redirect to GitHub
  - `GET /auth/github/callback` → exchanges code, issues `user_session` JWT
  - Env vars required:
    - `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`
    - `GITHUB_CALLBACK_URL` (e.g., `https://<host>/auth/github/callback`)
    - `GITHUB_POST_LOGIN_REDIRECT_URL` (optional)

## Deploy: Fly.io (backend-server)

Requirements: `flyctl`, `FLY_API_TOKEN`

1) Volume

```bash
flyctl volumes create data_volume --size 1 --region <region> --app <app>
```

2) Secrets (required)

```bash
flyctl secrets set \
  JWT_SECRET=<32+ chars> \
  HMAC_SECRET=<32+ chars> \
  --app <app>
```

3) Optional OAuth

```bash
flyctl secrets set \
  GITHUB_CLIENT_ID=<id> \
  GITHUB_CLIENT_SECRET=<secret> \
  GITHUB_CALLBACK_URL=https://<app>.fly.dev/auth/github/callback \
  GITHUB_POST_LOGIN_REDIRECT_URL=<https-url> \
  --app <app>
```

4) Deploy

```bash
cd backend-server
flyctl deploy --remote-only --app <app>
```

5) Verify

```bash
curl https://<app>.fly.dev/health
# Expect status: healthy and websocket endpoint reported
```

## Deploy: Minimal Remote Control Server

```bash
cd remote-control-server
npm install
PORT=8080 npm start
# or docker
docker build -t juno-remote-control .
docker run -p 8080:8080 juno-remote-control
```

## CI/CD (GitHub Actions)

- Workflow: `.github/workflows/deploy-backend-server.yml`
- Set repo secrets: `FLY_API_TOKEN`, `FLY_APP_NAME`, `JWT_SECRET`, `HMAC_SECRET` (and OAuth secrets if used)
- Push to `main` to deploy `backend-server/`

## Notes

- Use `backend-server/` for any external or production traffic; it includes auth, rate limits, health, metrics, and audit logging.
- Use `remote-control-server/` for quick internal testing only; no auth or persistence.

'use strict';

const express = require('express');
const session = require('express-session');
const passport = require('passport');
const { Strategy: GitHubStrategy } = require('passport-github2');
const helmet = require('helmet');
const { WebSocketServer } = require('ws');
const http = require('http');
const crypto = require('crypto');
const { v4: uuidv4 } = require('uuid');

const auth = require('./auth');
const devices = require('./devices');

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------
const PORT = parseInt(process.env.PORT || '3000', 10);
const SESSION_SECRET = process.env.SESSION_SECRET || crypto.randomBytes(32).toString('hex');
const GITHUB_CLIENT_ID = process.env.GITHUB_CLIENT_ID;
const GITHUB_CLIENT_SECRET = process.env.GITHUB_CLIENT_SECRET;
const BASE_URL = process.env.BASE_URL || `http://localhost:${PORT}`;

if (!GITHUB_CLIENT_ID || !GITHUB_CLIENT_SECRET) {
  console.error('ERROR: GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET must be set');
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Express app
// ---------------------------------------------------------------------------
const app = express();

app.use(helmet({
  contentSecurityPolicy: {
    directives: {
      defaultSrc: ["'self'"],
      scriptSrc: ["'self'", "'unsafe-inline'"],
      styleSrc: ["'self'", "'unsafe-inline'"],
    },
  },
}));
app.use(express.json());
app.use(express.urlencoded({ extended: false }));

// Session store — SQLite in production keeps sessions across restarts
let sessionStore;
try {
  const SQLiteStore = require('connect-sqlite3')(session);
  sessionStore = new SQLiteStore({ db: 'sessions.sqlite', dir: process.env.DATA_DIR || '.' });
} catch {
  // Fallback to in-memory (dev / first boot before npm install)
  sessionStore = undefined;
}

app.use(session({
  store: sessionStore,
  secret: SESSION_SECRET,
  resave: false,
  saveUninitialized: false,
  cookie: {
    secure: process.env.NODE_ENV === 'production',
    httpOnly: true,
    maxAge: 7 * 24 * 60 * 60 * 1000, // 7 days
  },
}));

// ---------------------------------------------------------------------------
// Passport / GitHub OAuth
// ---------------------------------------------------------------------------
passport.use(new GitHubStrategy(
  {
    clientID: GITHUB_CLIENT_ID,
    clientSecret: GITHUB_CLIENT_SECRET,
    callbackURL: `${BASE_URL}/auth/github/callback`,
    scope: ['read:user'],
  },
  (_accessToken, _refreshToken, profile, done) => done(null, profile),
));

passport.serializeUser((user, done) => done(null, { id: user.id, username: user.username }));
passport.deserializeUser((user, done) => done(null, user));

app.use(passport.initialize());
app.use(passport.session());

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------
function requireAuth(req, res, next) {
  if (req.isAuthenticated()) return next();
  res.redirect('/login');
}

// ---------------------------------------------------------------------------
// HTTP routes
// ---------------------------------------------------------------------------
app.get('/', requireAuth, (req, res) => {
  const connected = devices.list();
  res.json({
    user: req.user,
    devices: connected,
  });
});

app.get('/login', (req, res) => {
  res.send(`<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Juno Remote Control</title></head>
<body>
  <h1>Juno Remote Control</h1>
  <a href="/auth/github">Sign in with GitHub</a>
</body>
</html>`);
});

app.get('/auth/github', passport.authenticate('github'));

app.get(
  '/auth/github/callback',
  passport.authenticate('github', { failureRedirect: '/login' }),
  (req, res) => res.redirect('/'),
);

app.post('/logout', requireAuth, (req, res, next) => {
  req.logout((err) => {
    if (err) return next(err);
    res.redirect('/login');
  });
});

// List connected devices
app.get('/api/devices', requireAuth, (_req, res) => {
  res.json(devices.list());
});

// Send a command to a specific Juno device
app.post('/api/devices/:deviceId/command', requireAuth, (req, res) => {
  const { deviceId } = req.params;
  const { command, params } = req.body;

  if (!command) {
    return res.status(400).json({ error: 'command is required' });
  }

  const ok = devices.sendCommand(deviceId, {
    id: uuidv4(),
    command,
    params: params || {},
    sender: req.user.username,
  });

  if (!ok) {
    return res.status(404).json({ error: 'Device not found or not connected' });
  }

  res.json({ ok: true });
});

// ---------------------------------------------------------------------------
// HTTP server + WebSocket server
// ---------------------------------------------------------------------------
const server = http.createServer(app);

// The WebSocket server handles Juno device connections (authenticated via HMAC API key)
const wss = new WebSocketServer({ server, path: '/ws' });

wss.on('connection', (ws, req) => {
  const deviceId = auth.authenticateDevice(req);
  if (!deviceId) {
    ws.close(4001, 'Unauthorized');
    return;
  }

  devices.register(deviceId, ws);

  ws.on('message', (data) => {
    try {
      const msg = JSON.parse(data.toString());
      devices.handleMessage(deviceId, msg);
    } catch {
      // ignore malformed messages
    }
  });

  ws.on('close', () => devices.unregister(deviceId));
  ws.on('error', () => devices.unregister(deviceId));

  // Acknowledge successful connection
  ws.send(JSON.stringify({ type: 'connected', deviceId }));
});

server.listen(PORT, () => {
  console.log(`Juno Remote Control Server listening on port ${PORT}`);
});

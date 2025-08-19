const WebSocket = require('ws');
const http = require('http');
const { v4: uuidv4 } = require('uuid');

// Basic, deployment-ready WebSocket remote control server
// Inspired by existing examples under websocket-test/

const server = http.createServer();
const wss = new WebSocket.Server({ server, path: '/ws' });

const clients = new Map(); // clientId -> { ws, info }

const MessageType = {
  Command: 'command',
  Response: 'response',
  Status: 'status',
  Heartbeat: 'heartbeat',
  Auth: 'auth',
  Error: 'error',
};

const CloudCommandType = {
  VoiceQuery: 'voice_query',
  TextQuery: 'text_query',
  SystemCommand: 'system_command',
  StatusRequest: 'status_request',
  Screenshot: 'screenshot',
  ConfigUpdate: 'config_update',
};

const ResponseStatus = {
  Success: 'success',
  Error: 'error',
  InProgress: 'in_progress',
  Cancelled: 'cancelled',
};

function createMessage(type, data) {
  return { type, data, timestamp: Math.floor(Date.now() / 1000) };
}

function createResponse(commandId, status, data, error = null) {
  return { command_id: commandId, status, data, timestamp: Math.floor(Date.now() / 1000), error };
}

function handleCommand(command, ws) {
  let data = {
    text: null,
    audio_base64: null,
    screenshot_base64: null,
    agent_state: null,
    progress: null,
    metadata: null,
  };

  switch (command.command_type) {
    case CloudCommandType.StatusRequest:
      data.text = 'Server is running and healthy';
      data.metadata = { server_status: 'healthy', uptime: process.uptime() };
      break;
    case CloudCommandType.TextQuery: {
      const query = command.payload?.query || 'No query provided';
      data.text = `Echo: ${query}`;
      data.metadata = { original_query: query, processed_at: new Date().toISOString() };
      break;
    }
    case CloudCommandType.VoiceQuery:
      data.text = 'Voice query processed (simulated)';
      break;
    case CloudCommandType.Screenshot:
      data.text = 'Screenshot captured (simulated)';
      break;
    case CloudCommandType.SystemCommand: {
      const action = command.payload?.action || command.payload?.parameters?.action;
      data.text = `System command executed: ${action}`;
      data.metadata = { action, simulated: true };
      break;
    }
    case CloudCommandType.ConfigUpdate:
      data.text = 'Configuration updated successfully';
      data.metadata = { updated_at: new Date().toISOString() };
      break;
    default:
      data.text = `Unknown command type: ${command.command_type}`;
  }

  const response = createResponse(command.id, ResponseStatus.Success, data);
  try {
    ws.send(JSON.stringify(createMessage(MessageType.Response, response)));
  } catch (e) {
    console.error('Failed to send response:', e.message);
  }
}

function sendHeartbeat(ws, clientId) {
  const hb = createMessage(MessageType.Heartbeat, { server_time: Date.now(), client_id: clientId, status: 'alive' });
  try {
    ws.send(JSON.stringify(hb));
  } catch (e) {
    console.error('Failed to send heartbeat:', e.message);
  }
}

wss.on('connection', (ws, req) => {
  const clientId = uuidv4();
  const info = {
    id: clientId,
    connected_at: Date.now(),
    ip: req.socket.remoteAddress,
    user_agent: req.headers['user-agent'] || 'Unknown',
  };
  clients.set(clientId, { ws, info });

  ws.send(
    JSON.stringify(
      createMessage(MessageType.Status, {
        message: 'Connected to Juno Remote Control Server',
        client_id: clientId,
        server_capabilities: ['command_processing', 'heartbeat', 'authentication'],
      }),
    ),
  );

  const heartbeatInterval = setInterval(() => {
    if (ws.readyState === WebSocket.OPEN) sendHeartbeat(ws, clientId);
    else clearInterval(heartbeatInterval);
  }, 30000);

  ws.on('message', (raw) => {
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch (e) {
      ws.send(JSON.stringify(createMessage(MessageType.Error, { error: 'INVALID_JSON' })));
      return;
    }

    switch (msg.type) {
      case MessageType.Command:
        if (msg.data?.command_type) handleCommand(msg.data, ws);
        else ws.send(JSON.stringify(createMessage(MessageType.Error, { error: 'INVALID_COMMAND' })));
        break;
      case MessageType.Auth:
        ws.send(
          JSON.stringify(
            createMessage(MessageType.Auth, {
              success: true,
              token: `token_${clientId}_${Date.now()}`,
              device_id: msg.data?.device_id || `device_${clientId}`,
              permissions: ['text_processing', 'voice_transcription', 'screenshot_capture', 'system_automation'],
              expires_at: Math.floor(Date.now() / 1000) + 3600,
            }),
          ),
        );
        break;
      case MessageType.Heartbeat:
        ws.send(JSON.stringify(createMessage(MessageType.Heartbeat, { echo: true, server_time: Date.now(), client_id: clientId })));
        break;
      case MessageType.Status:
        ws.send(
          JSON.stringify(
            createMessage(MessageType.Status, {
              server_status: 'healthy',
              connected_clients: clients.size,
              uptime: process.uptime(),
            }),
          ),
        );
        break;
      default:
        ws.send(JSON.stringify(createMessage(MessageType.Response, { echo: msg, timestamp: Date.now() })));
    }
  });

  ws.on('close', () => {
    clients.delete(clientId);
    clearInterval(heartbeatInterval);
  });

  ws.on('error', () => {
    clients.delete(clientId);
    clearInterval(heartbeatInterval);
  });
});

process.on('SIGTERM', () => {
  wss.close(() => {
    server.close(() => process.exit(0));
  });
});

process.on('SIGINT', () => {
  wss.close(() => {
    server.close(() => process.exit(0));
  });
});

const PORT = process.env.PORT || 8080;
server.listen(PORT, () => {
  console.log(`Juno Remote Control Server listening on :${PORT}`);
});

// Basic health/status endpoints
server.on('request', (req, res) => {
  if (req.url === '/health' || req.url === '/status') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(
      JSON.stringify({
        status: 'healthy',
        uptime: process.uptime(),
        connected_clients: clients.size,
        websocket_endpoint: `ws://localhost:${PORT}/ws`,
        timestamp: new Date().toISOString(),
      }),
    );
  }
});



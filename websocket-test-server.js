const WebSocket = require('ws');
const http = require('http');
const { v4: uuidv4 } = require('uuid');

// Create HTTP server
const server = http.createServer();
const wss = new WebSocket.Server({ server });

// Connected clients
const clients = new Map();

// Message types from the Rust code
const MessageType = {
    Command: 'command',
    Response: 'response',
    Status: 'status',
    Heartbeat: 'heartbeat',
    Auth: 'auth',
    Error: 'error'
};

// Command types
const CloudCommandType = {
    VoiceQuery: 'voice_query',
    TextQuery: 'text_query',
    SystemCommand: 'system_command',
    StatusRequest: 'status_request',
    Screenshot: 'screenshot',
    ConfigUpdate: 'config_update'
};

// Response status
const ResponseStatus = {
    Success: 'success',
    Error: 'error',
    InProgress: 'in_progress',
    Cancelled: 'cancelled'
};

function createWebSocketMessage(type, data) {
    return {
        type: type,
        data: data,
        timestamp: Math.floor(Date.now() / 1000)
    };
}

function createDeviceResponse(commandId, status, data, error = null) {
    return {
        command_id: commandId,
        status: status,
        data: data,
        timestamp: Math.floor(Date.now() / 1000),
        error: error
    };
}

function handleCloudCommand(command, ws) {
    console.log(`Handling command: ${command.command_type} (${command.id})`);

    let responseData = {
        text: null,
        audio_base64: null,
        screenshot_base64: null,
        agent_state: null,
        progress: null,
        metadata: null
    };

    switch (command.command_type) {
        case CloudCommandType.StatusRequest:
            responseData.text = "Server is running and healthy";
            responseData.metadata = {
                server_status: "healthy",
                uptime: process.uptime(),
                connected_clients: clients.size
            };
            break;

        case CloudCommandType.TextQuery:
            const query = command.payload?.query || "No query provided";
            responseData.text = `Echo: ${query}`;
            responseData.metadata = {
                original_query: query,
                processed_at: new Date().toISOString()
            };
            break;

        case CloudCommandType.VoiceQuery:
            responseData.text = "Voice query processed (simulated)";
            responseData.audio_base64 = "simulated_audio_response_base64";
            break;

        case CloudCommandType.Screenshot:
            responseData.text = "Screenshot captured (simulated)";
            responseData.screenshot_base64 = "simulated_screenshot_base64_data";
            break;

        case CloudCommandType.SystemCommand:
            const action = command.payload?.action || command.payload?.parameters?.action;
            responseData.text = `System command executed: ${action}`;
            responseData.metadata = {
                action: action,
                simulated: true
            };
            break;

        case CloudCommandType.ConfigUpdate:
            responseData.text = "Configuration updated successfully";
            responseData.metadata = {
                updated_at: new Date().toISOString()
            };
            break;

        default:
            responseData.text = `Unknown command type: ${command.command_type}`;
            break;
    }

    const response = createDeviceResponse(
        command.id,
        ResponseStatus.Success,
        responseData
    );

    const wsMessage = createWebSocketMessage(MessageType.Response, response);

    try {
        ws.send(JSON.stringify(wsMessage));
        console.log(`Sent response for command ${command.id}`);
    } catch (error) {
        console.error(`Failed to send response: ${error.message}`);
    }
}

function sendHeartbeat(ws, clientId) {
    const heartbeat = createWebSocketMessage(MessageType.Heartbeat, {
        server_time: Date.now(),
        client_id: clientId,
        status: "alive"
    });

    try {
        ws.send(JSON.stringify(heartbeat));
    } catch (error) {
        console.error(`Failed to send heartbeat to ${clientId}: ${error.message}`);
    }
}

function handleAuthentication(authData, ws, clientId) {
    console.log(`Authentication request from ${clientId}:`, authData);

    // Simulate authentication success
    const authResponse = {
        success: true,
        token: `token_${clientId}_${Date.now()}`,
        device_id: authData.device_id || `device_${clientId}`,
        permissions: [
            "text_processing",
            "voice_transcription",
            "screenshot_capture",
            "system_automation",
            "file_operations",
            "web_browsing"
        ],
        expires_at: Math.floor(Date.now() / 1000) + 3600 // 1 hour
    };

    const wsMessage = createWebSocketMessage(MessageType.Auth, authResponse);
    ws.send(JSON.stringify(wsMessage));
    console.log(`Authentication successful for ${clientId}`);
}

wss.on('connection', (ws, req) => {
    const clientId = uuidv4();
    const clientInfo = {
        id: clientId,
        connected_at: Date.now(),
        ip: req.socket.remoteAddress,
        user_agent: req.headers['user-agent'] || 'Unknown'
    };

    clients.set(clientId, { ws, info: clientInfo });
    console.log(`Client connected: ${clientId} from ${clientInfo.ip}`);
    console.log(`Total clients: ${clients.size}`);

    // Send welcome message
    const welcome = createWebSocketMessage(MessageType.Status, {
        message: "Connected to Juno WebSocket Test Server",
        client_id: clientId,
        server_capabilities: [
            "command_processing",
            "heartbeat",
            "authentication",
            "echo_testing"
        ]
    });
    ws.send(JSON.stringify(welcome));

    // Set up heartbeat interval
    const heartbeatInterval = setInterval(() => {
        if (ws.readyState === WebSocket.OPEN) {
            sendHeartbeat(ws, clientId);
        } else {
            clearInterval(heartbeatInterval);
        }
    }, 30000); // Every 30 seconds

    ws.on('message', (data) => {
        try {
            const message = JSON.parse(data.toString());
            console.log(`Received from ${clientId}:`, message.type || 'unknown type');

            switch (message.type) {
                case MessageType.Command:
                    if (message.data && message.data.command_type) {
                        handleCloudCommand(message.data, ws);
                    } else {
                        console.error('Invalid command format');
                    }
                    break;

                case MessageType.Auth:
                    handleAuthentication(message.data, ws, clientId);
                    break;

                case MessageType.Heartbeat:
                    // Echo heartbeat
                    const heartbeatResponse = createWebSocketMessage(MessageType.Heartbeat, {
                        echo: true,
                        server_time: Date.now(),
                        client_id: clientId
                    });
                    ws.send(JSON.stringify(heartbeatResponse));
                    break;

                case MessageType.Status:
                    const statusResponse = createWebSocketMessage(MessageType.Status, {
                        server_status: "healthy",
                        connected_clients: clients.size,
                        uptime: process.uptime(),
                        memory_usage: process.memoryUsage()
                    });
                    ws.send(JSON.stringify(statusResponse));
                    break;

                default:
                    // Echo any other messages (for basic testing)
                    const echo = createWebSocketMessage(MessageType.Response, {
                        echo: message,
                        timestamp: Date.now()
                    });
                    ws.send(JSON.stringify(echo));
                    break;
            }
        } catch (error) {
            console.error(`Error processing message from ${clientId}:`, error.message);
            const errorResponse = createWebSocketMessage(MessageType.Error, {
                message: `Invalid message format: ${error.message}`,
                client_id: clientId
            });
            ws.send(JSON.stringify(errorResponse));
        }
    });

    ws.on('close', () => {
        clients.delete(clientId);
        clearInterval(heartbeatInterval);
        console.log(`Client disconnected: ${clientId}`);
        console.log(`Total clients: ${clients.size}`);
    });

    ws.on('error', (error) => {
        console.error(`WebSocket error for ${clientId}:`, error.message);
        clients.delete(clientId);
        clearInterval(heartbeatInterval);
    });
});

// Handle server shutdown gracefully
process.on('SIGTERM', () => {
    console.log('Shutting down WebSocket server...');
    wss.close(() => {
        server.close(() => {
            console.log('Server closed');
            process.exit(0);
        });
    });
});

process.on('SIGINT', () => {
    console.log('\nShutting down WebSocket server...');
    wss.close(() => {
        server.close(() => {
            console.log('Server closed');
            process.exit(0);
        });
    });
});

// Start server
const PORT = process.env.PORT || 8080;
server.listen(PORT, () => {
    console.log(`🚀 Juno WebSocket Test Server running on port ${PORT}`);
    console.log(`📡 WebSocket endpoint: ws://localhost:${PORT}`);
    console.log(`🔧 Ready to test cloud connectivity and commands`);
    console.log('\nSupported command types:');
    Object.values(CloudCommandType).forEach(type => {
        console.log(`  - ${type}`);
    });
    console.log('\nPress Ctrl+C to stop the server');
});

// Status endpoint for health checks
server.on('request', (req, res) => {
    if (req.url === '/health' || req.url === '/status') {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
            status: 'healthy',
            uptime: process.uptime(),
            connected_clients: clients.size,
            websocket_endpoint: `ws://localhost:${PORT}`,
            timestamp: new Date().toISOString()
        }));
    } else {
        res.writeHead(404, { 'Content-Type': 'text/plain' });
        res.end('Juno WebSocket Test Server - Use WebSocket connection or /health endpoint');
    }
});

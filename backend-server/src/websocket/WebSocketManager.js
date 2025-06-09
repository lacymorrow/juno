import { v4 as uuidv4 } from 'uuid';
import { WebSocketRateLimiter } from '../middleware/rateLimiter.js';
import { logCommand, logger, logWebSocketConnection } from '../utils/logger.js';

// Message types that match your Tauri client
const MessageType = {
    Command: 'command',
    Response: 'response',
    Status: 'status',
    Heartbeat: 'heartbeat',
    Auth: 'auth',
    Error: 'error'
};

// Command types from your Tauri implementation
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

export class WebSocketManager {
    constructor(wss, authService, commandProcessor, database) {
        this.wss = wss;
        this.authService = authService;
        this.commandProcessor = commandProcessor;
        this.database = database;

        // Connected clients
        this.clients = new Map(); // clientId -> { ws, device, heartbeatInterval }
        this.deviceConnections = new Map(); // deviceId -> Set<clientId>

        // Rate limiting
        this.rateLimiter = new WebSocketRateLimiter();

        // Statistics
        this.stats = {
            totalConnections: 0,
            activeConnections: 0,
            messagesProcessed: 0,
            commandsExecuted: 0,
            errors: 0
        };

        this.setupWebSocketServer();
        this.startCleanupJob();
    }

    setupWebSocketServer() {
        this.wss.on('connection', (ws, request) => {
            this.handleConnection(ws, request);
        });

        logger.info('WebSocket server event handlers configured');
    }

    async handleConnection(ws, request) {
        const clientId = uuidv4();
        const clientInfo = {
            id: clientId,
            connected_at: Date.now(),
            ip: request.socket.remoteAddress,
            user_agent: request.headers['user-agent'] || 'Unknown',
            authenticated: false,
            device: null
        };

        // Store client connection
        this.clients.set(clientId, {
            ws,
            ...clientInfo,
            heartbeatInterval: null
        });

        this.stats.totalConnections++;
        this.stats.activeConnections++;

        logWebSocketConnection(clientId, 'connected', {
            ip: clientInfo.ip,
            userAgent: clientInfo.user_agent
        });

        // Send welcome message
        this.sendMessage(ws, MessageType.Status, {
            message: "Connected to Juno Cloud Server",
            client_id: clientId,
            server_capabilities: [
                "command_processing",
                "heartbeat",
                "authentication",
                "cloud_control"
            ],
            timestamp: Date.now()
        });

        // Setup event handlers for this connection
        this.setupConnectionHandlers(ws, clientId);

        // Start heartbeat
        this.startHeartbeat(clientId);
    }

    setupConnectionHandlers(ws, clientId) {
        // Handle incoming messages
        ws.on('message', async (data) => {
            try {
                await this.handleMessage(clientId, data);
            } catch (error) {
                logger.error(`Message handling error for client ${clientId}:`, error);
                this.sendError(clientId, 'MESSAGE_PROCESSING_ERROR', error.message);
                this.stats.errors++;
            }
        });

        // Handle connection close
        ws.on('close', (code, reason) => {
            this.handleDisconnection(clientId, code, reason);
        });

        // Handle connection error
        ws.on('error', (error) => {
            logger.error(`WebSocket error for client ${clientId}:`, error);
            this.handleDisconnection(clientId, 1006, 'Connection error');
        });
    }

    async handleMessage(clientId, rawData) {
        const client = this.clients.get(clientId);
        if (!client) {
            logger.warn(`Received message from unknown client: ${clientId}`);
            return;
        }

        // Rate limiting check
        const rateLimitResult = await this.rateLimiter.checkLimit(clientId, 'message');
        if (!rateLimitResult.allowed) {
            this.sendError(clientId, 'RATE_LIMIT_EXCEEDED', rateLimitResult.message);
            return;
        }

        // Track activity
        this.rateLimiter.trackActivity(clientId, 'message');

        let message;
        try {
            message = JSON.parse(rawData.toString());
        } catch (error) {
            this.sendError(clientId, 'INVALID_JSON', 'Invalid JSON format');
            return;
        }

        this.stats.messagesProcessed++;

        // Route message based on type
        switch (message.type) {
            case MessageType.Auth:
                await this.handleAuthentication(clientId, message.data);
                break;

            case MessageType.Command:
                await this.handleCommand(clientId, message.data);
                break;

            case MessageType.Heartbeat:
                await this.handleHeartbeat(clientId, message.data);
                break;

            case MessageType.Status:
                await this.handleStatusRequest(clientId, message.data);
                break;

            default:
                this.sendError(clientId, 'UNKNOWN_MESSAGE_TYPE', `Unknown message type: ${message.type}`);
        }
    }

    async handleAuthentication(clientId, authData) {
        const client = this.clients.get(clientId);
        if (!client) return;

        try {
            logWebSocketConnection(clientId, 'auth_attempt', { api_key: authData.api_key?.substring(0, 8) + '...' });

            // Authenticate with auth service
            const authResult = await this.authService.authenticateDevice(authData);

            if (authResult.success) {
                // Update client with device info
                client.authenticated = true;
                client.device = {
                    id: authResult.device_id,
                    name: authResult.device_name,
                    permissions: authResult.permissions
                };

                // Track device connection
                if (!this.deviceConnections.has(authResult.device_id)) {
                    this.deviceConnections.set(authResult.device_id, new Set());
                }
                this.deviceConnections.get(authResult.device_id).add(clientId);

                // Send authentication success
                this.sendMessage(client.ws, MessageType.Auth, {
                    success: true,
                    token: authResult.token,
                    device_id: authResult.device_id,
                    device_name: authResult.device_name,
                    permissions: authResult.permissions,
                    expires_at: authResult.expires_at,
                    session_id: authResult.session_id
                });

                logWebSocketConnection(clientId, 'authenticated', {
                    device_id: authResult.device_id,
                    device_name: authResult.device_name
                });

                // Log to audit table
                await this.database.logAudit({
                    device_id: authResult.device_id,
                    action: 'websocket_authenticated',
                    details: { client_id: clientId },
                    ip_address: client.ip,
                    user_agent: client.user_agent
                });
            } else {
                throw new Error('Authentication failed');
            }
        } catch (error) {
            logger.error(`Authentication failed for client ${clientId}:`, error);
            this.sendMessage(client.ws, MessageType.Auth, {
                success: false,
                error: error.message
            });

            logWebSocketConnection(clientId, 'auth_failed', { error: error.message });
        }
    }

    async handleCommand(clientId, commandData) {
        const client = this.clients.get(clientId);
        if (!client) return;

        if (!client.authenticated || !client.device) {
            this.sendError(clientId, 'NOT_AUTHENTICATED', 'Must authenticate before sending commands');
            return;
        }

        try {
            // Rate limit check for device commands
            const rateLimitResult = await this.rateLimiter.checkLimit(
                client.device.id,
                commandData.command_type || 'command'
            );

            if (!rateLimitResult.allowed) {
                this.sendError(clientId, 'DEVICE_RATE_LIMIT_EXCEEDED', rateLimitResult.message);
                return;
            }

            // Process command
            const commandId = commandData.id || uuidv4();

            logCommand(commandId, commandData.command_type, 'received', {
                device_id: client.device.id,
                client_id: clientId
            });

            // Store command in database
            await this.database.createCommand({
                id: commandId,
                device_id: client.device.id,
                command_type: commandData.command_type,
                payload: commandData.payload || {}
            });

            // Send immediate acknowledgment
            this.sendMessage(client.ws, MessageType.Response, this.createCommandResponse(
                commandId,
                ResponseStatus.InProgress,
                { text: "Command received and processing..." }
            ));

            // Process command asynchronously
            this.processCommandAsync(commandId, commandData, client);

            this.stats.commandsExecuted++;

        } catch (error) {
            logger.error(`Command handling error for client ${clientId}:`, error);
            this.sendError(clientId, 'COMMAND_PROCESSING_ERROR', error.message);
        }
    }

    async processCommandAsync(commandId, commandData, client) {
        try {
            // Use command processor to handle the actual command
            const response = await this.commandProcessor.processCommand(
                commandData,
                client.device
            );

            // Update command in database
            await this.database.updateCommand(commandId, {
                status: 'completed',
                response: response,
                executed_at: Math.floor(Date.now() / 1000)
            });

            // Send response to client
            this.sendMessage(client.ws, MessageType.Response, this.createCommandResponse(
                commandId,
                ResponseStatus.Success,
                response
            ));

            logCommand(commandId, commandData.command_type, 'completed', {
                device_id: client.device.id
            });

        } catch (error) {
            logger.error(`Command processing error for ${commandId}:`, error);

            // Update command with error
            await this.database.updateCommand(commandId, {
                status: 'error',
                error: error.message,
                executed_at: Math.floor(Date.now() / 1000)
            });

            // Send error response
            this.sendMessage(client.ws, MessageType.Response, this.createCommandResponse(
                commandId,
                ResponseStatus.Error,
                { text: "Command processing failed" },
                error.message
            ));

            logCommand(commandId, commandData.command_type, 'error', {
                device_id: client.device.id,
                error: error.message
            });
        }
    }

    async handleHeartbeat(clientId, heartbeatData) {
        const client = this.clients.get(clientId);
        if (!client) return;

        // Respond with heartbeat
        this.sendMessage(client.ws, MessageType.Heartbeat, {
            server_time: Date.now(),
            client_id: clientId,
            status: "alive",
            received_client_time: heartbeatData?.client_time
        });

        // Track activity
        this.rateLimiter.trackActivity(clientId, 'heartbeat');

        // Update device last seen if authenticated
        if (client.device) {
            await this.database.updateDeviceLastSeen(client.device.id);
        }
    }

    async handleStatusRequest(clientId, statusData) {
        const client = this.clients.get(clientId);
        if (!client) return;

        const status = {
            server_status: "healthy",
            client_id: clientId,
            authenticated: client.authenticated,
            uptime: process.uptime(),
            connected_clients: this.clients.size,
            device_info: client.device || null,
            timestamp: Date.now()
        };

        this.sendMessage(client.ws, MessageType.Status, status);
    }

    handleDisconnection(clientId, code, reason) {
        const client = this.clients.get(clientId);
        if (!client) return;

        // Stop heartbeat
        if (client.heartbeatInterval) {
            clearInterval(client.heartbeatInterval);
        }

        // Remove from device connections
        if (client.device) {
            const deviceConnections = this.deviceConnections.get(client.device.id);
            if (deviceConnections) {
                deviceConnections.delete(clientId);
                if (deviceConnections.size === 0) {
                    this.deviceConnections.delete(client.device.id);
                }
            }
        }

        // Remove from clients
        this.clients.delete(clientId);
        this.rateLimiter.removeClient(clientId);

        this.stats.activeConnections--;

        logWebSocketConnection(clientId, 'disconnected', {
            code,
            reason: reason?.toString(),
            device_id: client.device?.id
        });
    }

    startHeartbeat(clientId) {
        const client = this.clients.get(clientId);
        if (!client) return;

        const interval = parseInt(process.env.WS_HEARTBEAT_INTERVAL) || 30000;

        client.heartbeatInterval = setInterval(() => {
            if (client.ws.readyState === client.ws.OPEN) {
                this.sendMessage(client.ws, MessageType.Heartbeat, {
                    server_time: Date.now(),
                    client_id: clientId,
                    status: "ping"
                });
            } else {
                clearInterval(client.heartbeatInterval);
            }
        }, interval);
    }

    sendMessage(ws, type, data) {
        if (ws.readyState === ws.OPEN) {
            const message = {
                type,
                data,
                timestamp: Math.floor(Date.now() / 1000)
            };

            try {
                ws.send(JSON.stringify(message));
            } catch (error) {
                logger.error('Failed to send WebSocket message:', error);
            }
        }
    }

    sendError(clientId, errorCode, errorMessage) {
        const client = this.clients.get(clientId);
        if (!client) return;

        this.sendMessage(client.ws, MessageType.Error, {
            error_code: errorCode,
            error_message: errorMessage,
            timestamp: Date.now()
        });
    }

    createCommandResponse(commandId, status, data, error = null) {
        return {
            command_id: commandId,
            status,
            data,
            timestamp: Math.floor(Date.now() / 1000),
            error
        };
    }

    // Broadcast message to all connections of a device
    broadcastToDevice(deviceId, type, data) {
        const deviceConnections = this.deviceConnections.get(deviceId);
        if (!deviceConnections) return;

        for (const clientId of deviceConnections) {
            const client = this.clients.get(clientId);
            if (client && client.ws.readyState === client.ws.OPEN) {
                this.sendMessage(client.ws, type, data);
            }
        }
    }

    // Send command to specific device
    async sendCommandToDevice(deviceId, commandData) {
        const deviceConnections = this.deviceConnections.get(deviceId);
        if (!deviceConnections || deviceConnections.size === 0) {
            throw new Error(`Device ${deviceId} is not connected`);
        }

        // Send to first available connection
        for (const clientId of deviceConnections) {
            const client = this.clients.get(clientId);
            if (client && client.ws.readyState === client.ws.OPEN) {
                const commandId = uuidv4();

                // Store command
                await this.database.createCommand({
                    id: commandId,
                    device_id: deviceId,
                    command_type: commandData.command_type,
                    payload: commandData.payload || {}
                });

                // Send command
                this.sendMessage(client.ws, MessageType.Command, {
                    id: commandId,
                    ...commandData
                });

                return { success: true, command_id: commandId };
            }
        }

        throw new Error(`No active connections for device ${deviceId}`);
    }

    // Cleanup job for stale connections
    startCleanupJob() {
        setInterval(() => {
            const now = Date.now();
            const timeout = parseInt(process.env.WS_CONNECTION_TIMEOUT) || 300000; // 5 minutes

            for (const [clientId, client] of this.clients) {
                if (now - client.connected_at > timeout && !client.authenticated) {
                    logger.info(`Cleaning up unauthenticated connection: ${clientId}`);
                    client.ws.close(1000, 'Authentication timeout');
                }
            }

            // Clean up expired sessions
            this.authService.cleanupExpiredSessions();
        }, 60000); // Run every minute
    }

    // Get statistics
    getStats() {
        return {
            ...this.stats,
            activeConnections: this.clients.size,
            deviceConnections: this.deviceConnections.size,
            rateLimiter: this.rateLimiter.getStats()
        };
    }

    getTotalConnections() {
        return this.stats.totalConnections;
    }
}

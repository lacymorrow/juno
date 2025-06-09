import { logCommand, logger } from '../utils/logger.js';
import { validateCommandData } from '../utils/validation.js';

// Command types from your Tauri implementation
const CloudCommandType = {
    VoiceQuery: 'voice_query',
    TextQuery: 'text_query',
    SystemCommand: 'system_command',
    StatusRequest: 'status_request',
    Screenshot: 'screenshot',
    ConfigUpdate: 'config_update'
};

export class CommandProcessor {
    constructor(database, authService) {
        this.database = database;
        this.authService = authService;

        // Command execution statistics
        this.stats = {
            totalCommands: 0,
            successfulCommands: 0,
            failedCommands: 0,
            commandsByType: {},
            averageExecutionTime: 0
        };

        // Active commands (for cancellation)
        this.activeCommands = new Map();
    }

    async processCommand(commandData, device) {
        const startTime = Date.now();
        const commandId = commandData.id;

        try {
            // Validate command data
            validateCommandData(commandData);

            // Check device permissions
            if (!this.checkCommandPermissions(commandData.command_type, device)) {
                throw new Error(`Device does not have permission for command type: ${commandData.command_type}`);
            }

            // Track active command
            this.activeCommands.set(commandId, {
                startTime,
                commandType: commandData.command_type,
                deviceId: device.id
            });

            // Update statistics
            this.stats.totalCommands++;
            this.stats.commandsByType[commandData.command_type] =
                (this.stats.commandsByType[commandData.command_type] || 0) + 1;

            // Process command based on type
            let response;
            switch (commandData.command_type) {
                case CloudCommandType.StatusRequest:
                    response = await this.handleStatusRequest(commandData, device);
                    break;

                case CloudCommandType.TextQuery:
                    response = await this.handleTextQuery(commandData, device);
                    break;

                case CloudCommandType.VoiceQuery:
                    response = await this.handleVoiceQuery(commandData, device);
                    break;

                case CloudCommandType.Screenshot:
                    response = await this.handleScreenshot(commandData, device);
                    break;

                case CloudCommandType.SystemCommand:
                    response = await this.handleSystemCommand(commandData, device);
                    break;

                case CloudCommandType.ConfigUpdate:
                    response = await this.handleConfigUpdate(commandData, device);
                    break;

                default:
                    throw new Error(`Unknown command type: ${commandData.command_type}`);
            }

            // Update statistics
            const executionTime = Date.now() - startTime;
            this.updateExecutionStats(executionTime, true);

            // Remove from active commands
            this.activeCommands.delete(commandId);

            logCommand(commandId, commandData.command_type, 'success', {
                device_id: device.id,
                execution_time: executionTime
            });

            return response;

        } catch (error) {
            // Update statistics
            const executionTime = Date.now() - startTime;
            this.updateExecutionStats(executionTime, false);

            // Remove from active commands
            this.activeCommands.delete(commandId);

            logCommand(commandId, commandData.command_type, 'error', {
                device_id: device.id,
                error: error.message,
                execution_time: executionTime
            });

            throw error;
        }
    }

    async handleStatusRequest(commandData, device) {
        return {
            text: "Cloud server is running and healthy",
            audio_base64: null,
            screenshot_base64: null,
            agent_state: {
                status: "ready",
                last_command: commandData.id,
                device_name: device.name
            },
            progress: null,
            metadata: {
                server_status: "healthy",
                uptime: process.uptime(),
                device_permissions: device.permissions,
                timestamp: new Date().toISOString(),
                stats: this.getStats()
            }
        };
    }

    async handleTextQuery(commandData, device) {
        const query = commandData.payload?.query || "No query provided";

        // For now, return an echo response
        // In production, this would integrate with your AI service
        return {
            text: `Cloud AI Response: I received your query "${query}". This is a simulated response from the cloud server. In production, this would be processed by the AI agent.`,
            audio_base64: null,
            screenshot_base64: null,
            agent_state: {
                status: "completed",
                last_query: query,
                processing_time: Date.now()
            },
            progress: null,
            metadata: {
                original_query: query,
                processed_at: new Date().toISOString(),
                response_type: "text",
                device_id: device.id
            }
        };
    }

    async handleVoiceQuery(commandData, device) {
        const audioData = commandData.payload?.audio_base64;
        const transcript = commandData.payload?.transcript || "Voice transcription not provided";

        return {
            text: `Voice query processed: "${transcript}". This is a simulated cloud response to your voice input.`,
            audio_base64: null, // In production, could return synthesized speech
            screenshot_base64: null,
            agent_state: {
                status: "completed",
                last_transcript: transcript,
                has_audio: !!audioData
            },
            progress: null,
            metadata: {
                transcript,
                has_audio_data: !!audioData,
                processed_at: new Date().toISOString(),
                response_type: "voice",
                device_id: device.id
            }
        };
    }

    async handleScreenshot(commandData, device) {
        // In production, this might trigger the device to send a screenshot
        // or request specific screen capture operations

        return {
            text: "Screenshot command processed",
            audio_base64: null,
            screenshot_base64: null, // Would contain actual screenshot data
            agent_state: {
                status: "completed",
                screenshot_requested: true
            },
            progress: null,
            metadata: {
                command: "screenshot",
                processed_at: new Date().toISOString(),
                device_id: device.id,
                note: "In production, this would capture or request device screenshot"
            }
        };
    }

    async handleSystemCommand(commandData, device) {
        const action = commandData.payload?.action || commandData.payload?.parameters?.action;

        if (!action) {
            throw new Error("System command requires an action parameter");
        }

        // Validate system command permissions
        if (!this.authService.hasPermission(device, 'system_automation')) {
            throw new Error("Device does not have system automation permissions");
        }

        // In production, this would execute actual system commands
        return {
            text: `System command executed: ${action}`,
            audio_base64: null,
            screenshot_base64: null,
            agent_state: {
                status: "completed",
                last_action: action,
                execution_result: "success"
            },
            progress: null,
            metadata: {
                action,
                simulated: true,
                processed_at: new Date().toISOString(),
                device_id: device.id,
                note: "In production, this would execute real system commands"
            }
        };
    }

    async handleConfigUpdate(commandData, device) {
        const config = commandData.payload?.config || {};

        // In production, this would update device or server configuration
        return {
            text: "Configuration updated successfully",
            audio_base64: null,
            screenshot_base64: null,
            agent_state: {
                status: "completed",
                config_updated: true
            },
            progress: null,
            metadata: {
                config_keys: Object.keys(config),
                updated_at: new Date().toISOString(),
                device_id: device.id,
                note: "Configuration update processed"
            }
        };
    }

    checkCommandPermissions(commandType, device) {
        const requiredPermissions = {
            [CloudCommandType.VoiceQuery]: 'voice_transcription',
            [CloudCommandType.TextQuery]: 'text_processing',
            [CloudCommandType.SystemCommand]: 'system_automation',
            [CloudCommandType.Screenshot]: 'screenshot_capture',
            [CloudCommandType.StatusRequest]: null, // No special permission required
            [CloudCommandType.ConfigUpdate]: 'system_automation'
        };

        const requiredPermission = requiredPermissions[commandType];
        if (!requiredPermission) {
            return true; // No permission required
        }

        return device.permissions && device.permissions.includes(requiredPermission);
    }

    updateExecutionStats(executionTime, success) {
        if (success) {
            this.stats.successfulCommands++;
        } else {
            this.stats.failedCommands++;
        }

        // Update average execution time
        const totalCommands = this.stats.successfulCommands + this.stats.failedCommands;
        this.stats.averageExecutionTime =
            (this.stats.averageExecutionTime * (totalCommands - 1) + executionTime) / totalCommands;
    }

    // Cancel active command
    async cancelCommand(commandId) {
        const command = this.activeCommands.get(commandId);
        if (!command) {
            throw new Error(`Command ${commandId} not found or already completed`);
        }

        // Remove from active commands
        this.activeCommands.delete(commandId);

        // Update database
        await this.database.updateCommand(commandId, {
            status: 'cancelled',
            executed_at: Math.floor(Date.now() / 1000)
        });

        logCommand(commandId, command.commandType, 'cancelled', {
            device_id: command.deviceId,
            execution_time: Date.now() - command.startTime
        });

        return { success: true, message: 'Command cancelled' };
    }

    // Get command execution statistics
    getStats() {
        return {
            ...this.stats,
            activeCommands: this.activeCommands.size,
            successRate: this.stats.totalCommands > 0
                ? (this.stats.successfulCommands / this.stats.totalCommands * 100).toFixed(2) + '%'
                : '0%'
        };
    }

    // Get active commands
    getActiveCommands() {
        const commands = [];
        for (const [commandId, info] of this.activeCommands) {
            commands.push({
                id: commandId,
                type: info.commandType,
                deviceId: info.deviceId,
                startTime: info.startTime,
                duration: Date.now() - info.startTime
            });
        }
        return commands;
    }

    // Cleanup old commands (called periodically)
    async cleanup() {
        try {
            await this.database.cleanupOldCommands();
        } catch (error) {
            logger.error('Command cleanup failed:', error);
        }
    }
}

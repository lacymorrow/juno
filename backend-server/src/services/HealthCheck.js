import { logger } from '../utils/logger.js';

export class HealthCheck {
    constructor(database, webSocketServer) {
        this.database = database;
        this.webSocketServer = webSocketServer;
        this.status = 'unknown';
        this.lastCheck = null;
        this.healthData = {};
        this.interval = null;
        this.checkInterval = parseInt(process.env.HEALTH_CHECK_INTERVAL) || 60000; // 1 minute
    }

    start() {
        // Initial health check
        this.performHealthCheck();

        // Schedule periodic health checks
        this.interval = setInterval(() => {
            this.performHealthCheck();
        }, this.checkInterval);

        logger.info(`Health check service started (interval: ${this.checkInterval}ms)`);
    }

    stop() {
        if (this.interval) {
            clearInterval(this.interval);
            this.interval = null;
            logger.info('Health check service stopped');
        }
    }

    async performHealthCheck() {
        try {
            const startTime = Date.now();
            const checks = await Promise.allSettled([
                this.checkDatabase(),
                this.checkMemory(),
                this.checkWebSocket(),
                this.checkDiskSpace(),
                this.checkUptime()
            ]);

            const healthData = {
                timestamp: new Date().toISOString(),
                status: 'healthy',
                uptime: process.uptime(),
                checks: {},
                performance: {
                    healthCheckDuration: Date.now() - startTime
                }
            };

            // Process check results
            for (let i = 0; i < checks.length; i++) {
                const check = checks[i];
                const checkNames = ['database', 'memory', 'websocket', 'diskSpace', 'uptime'];
                const checkName = checkNames[i];

                if (check.status === 'fulfilled') {
                    healthData.checks[checkName] = {
                        status: 'healthy',
                        ...check.value
                    };
                } else {
                    healthData.checks[checkName] = {
                        status: 'unhealthy',
                        error: check.reason.message
                    };
                    healthData.status = 'degraded';
                }
            }

            // Determine overall status
            const unhealthyChecks = Object.values(healthData.checks)
                .filter(check => check.status === 'unhealthy');

            if (unhealthyChecks.length === 0) {
                healthData.status = 'healthy';
            } else if (unhealthyChecks.length <= 1) {
                healthData.status = 'degraded';
            } else {
                healthData.status = 'unhealthy';
            }

            this.status = healthData.status;
            this.healthData = healthData;
            this.lastCheck = Date.now();

            // Log health status changes
            if (this.status !== 'healthy') {
                logger.warn('Health check status:', {
                    status: this.status,
                    unhealthyChecks: unhealthyChecks.length,
                    checks: healthData.checks
                });
            }

        } catch (error) {
            logger.error('Health check failed:', error);
            this.status = 'unhealthy';
            this.healthData = {
                timestamp: new Date().toISOString(),
                status: 'unhealthy',
                error: error.message,
                uptime: process.uptime()
            };
            this.lastCheck = Date.now();
        }
    }

    async checkDatabase() {
        try {
            const startTime = Date.now();

            // Test database connection with a simple query
            await this.database.get('SELECT 1 as test');

            const dbStats = this.database.getStats();

            return {
                connected: true,
                responseTime: Date.now() - startTime,
                queries: dbStats.queries,
                errors: dbStats.errors,
                path: dbStats.path
            };
        } catch (error) {
            throw new Error(`Database check failed: ${error.message}`);
        }
    }

    async checkMemory() {
        try {
            const memUsage = process.memoryUsage();
            const totalMem = memUsage.heapTotal;
            const usedMem = memUsage.heapUsed;
            const memoryUsagePercent = (usedMem / totalMem) * 100;

            // Consider memory unhealthy if over 90% used
            if (memoryUsagePercent > 90) {
                throw new Error(`High memory usage: ${memoryUsagePercent.toFixed(2)}%`);
            }

            return {
                heapUsed: Math.round(usedMem / 1024 / 1024), // MB
                heapTotal: Math.round(totalMem / 1024 / 1024), // MB
                usage: `${memoryUsagePercent.toFixed(2)}%`,
                rss: Math.round(memUsage.rss / 1024 / 1024), // MB
                external: Math.round(memUsage.external / 1024 / 1024) // MB
            };
        } catch (error) {
            throw new Error(`Memory check failed: ${error.message}`);
        }
    }

    async checkWebSocket() {
        try {
            if (!this.webSocketServer) {
                throw new Error('WebSocket server not available');
            }

            const clients = this.webSocketServer.clients;
            const connectionCount = clients ? clients.size : 0;

            return {
                running: true,
                connections: connectionCount,
                port: process.env.WS_PORT || 8080
            };
        } catch (error) {
            throw new Error(`WebSocket check failed: ${error.message}`);
        }
    }

    async checkDiskSpace() {
        try {
            // Simple disk space check (this is basic - in production you might want more sophisticated checks)
            const fs = await import('fs');
            const stats = fs.statSync('.');

            return {
                available: true,
                note: 'Basic disk access verified'
            };
        } catch (error) {
            throw new Error(`Disk space check failed: ${error.message}`);
        }
    }

    async checkUptime() {
        try {
            const uptime = process.uptime();
            const uptimeHours = uptime / 3600;

            return {
                seconds: Math.round(uptime),
                hours: Math.round(uptimeHours * 100) / 100,
                formatted: this.formatUptime(uptime),
                startTime: new Date(Date.now() - uptime * 1000).toISOString()
            };
        } catch (error) {
            throw new Error(`Uptime check failed: ${error.message}`);
        }
    }

    formatUptime(seconds) {
        const days = Math.floor(seconds / 86400);
        const hours = Math.floor((seconds % 86400) / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = Math.floor(seconds % 60);

        const parts = [];
        if (days > 0) parts.push(`${days}d`);
        if (hours > 0) parts.push(`${hours}h`);
        if (minutes > 0) parts.push(`${minutes}m`);
        if (secs > 0 || parts.length === 0) parts.push(`${secs}s`);

        return parts.join(' ');
    }

    getStatus() {
        return {
            status: this.status,
            lastCheck: this.lastCheck,
            data: this.healthData,
            nextCheck: this.lastCheck ? this.lastCheck + this.checkInterval : null
        };
    }

    // Get detailed health report
    getDetailedReport() {
        return {
            ...this.getStatus(),
            configuration: {
                checkInterval: this.checkInterval,
                autoRestart: true,
                environment: process.env.NODE_ENV || 'development'
            },
            system: {
                nodeVersion: process.version,
                platform: process.platform,
                arch: process.arch,
                pid: process.pid
            }
        };
    }

    // Check if service is healthy
    isHealthy() {
        return this.status === 'healthy';
    }

    // Check if service is degraded but operational
    isDegraded() {
        return this.status === 'degraded';
    }

    // Check if service is unhealthy
    isUnhealthy() {
        return this.status === 'unhealthy';
    }

    // Force a health check
    async forceCheck() {
        await this.performHealthCheck();
        return this.getStatus();
    }
}

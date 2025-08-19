import cors from 'cors';
import dotenv from 'dotenv';
import express from 'express';
import helmet from 'helmet';
import { createServer } from 'http';
import { WebSocketServer } from 'ws';

// Import our modules
import { AuthService } from './auth/AuthService.js';
import jwt from 'jsonwebtoken';
import { v4 as uuidv4 } from 'uuid';
import { Database } from './database/Database.js';
import { RateLimiter } from './middleware/rateLimiter.js';
import { CommandProcessor } from './services/CommandProcessor.js';
import { HealthCheck } from './services/HealthCheck.js';
import { logger } from './utils/logger.js';
import { validateEnv } from './utils/validation.js';
import { WebSocketManager } from './websocket/WebSocketManager.js';
import { GitHubOAuthService } from './auth/GitHubOAuth.js';

// Load environment variables
dotenv.config();

class JunoCloudServer {
    constructor() {
        this.app = express();
        this.server = createServer(this.app);
        this.wss = null;
        this.database = null;
        this.authService = null;
        this.wsManager = null;
        this.commandProcessor = null;
        this.healthCheck = null;
        this.githubOAuth = null;

        this.isShuttingDown = false;
        this.activeConnections = new Set();
    }

    async initialize() {
        try {
            logger.info('🚀 Initializing Juno Cloud Server...');

            // Validate environment
            validateEnv();

            // Initialize database
            this.database = new Database();
            await this.database.initialize();
            logger.info('✅ Database initialized');

            // Initialize auth service
            this.authService = new AuthService(this.database);
            logger.info('✅ Auth service initialized');

            // Initialize GitHub OAuth service (optional)
            this.githubOAuth = new GitHubOAuthService(this.database);
            if (this.githubOAuth.isEnabled()) {
                logger.info('✅ GitHub OAuth enabled');
            } else {
                logger.warn('⚠️ GitHub OAuth not configured (set GITHUB_CLIENT_ID/SECRET/CALLBACK_URL)');
            }

            // Initialize command processor
            this.commandProcessor = new CommandProcessor(this.database, this.authService);
            logger.info('✅ Command processor initialized');

            // Setup Express app
            this.setupExpress();

            // Setup WebSocket server
            this.setupWebSocket();

            // Initialize health check
            this.healthCheck = new HealthCheck(this.database, this.wss);
            this.healthCheck.start();
            logger.info('✅ Health check service started');

            // Setup graceful shutdown
            this.setupGracefulShutdown();

            logger.info('🎉 Juno Cloud Server initialization complete!');
        } catch (error) {
            logger.error('❌ Failed to initialize server:', error);
            process.exit(1);
        }
    }

    setupExpress() {
        // Security middleware
        this.app.use(helmet({
            contentSecurityPolicy: false, // Allow WebSocket connections
        }));

        // CORS configuration
        this.app.use(cors({
            origin: process.env.CORS_ORIGIN || '*',
            credentials: true,
        }));

        // Parse JSON
        this.app.use(express.json({ limit: '10mb' }));

        // Rate limiting
        this.app.use(RateLimiter);

        // Health check endpoint
        this.app.get('/health', (req, res) => {
            const status = this.healthCheck?.getStatus() || { status: 'unknown' };
            res.json(status);
        });

        // Metrics endpoint (if enabled)
        if (process.env.METRICS_ENABLED === 'true') {
            this.app.get('/metrics', (req, res) => {
                const metrics = this.getMetrics();
                res.json(metrics);
            });
        }

        // Device registration endpoint
        this.app.post('/api/register', async (req, res) => {
            try {
                const result = await this.authService.registerDevice(req.body);
                res.json(result);
            } catch (error) {
                logger.error('Device registration failed:', error);
                res.status(400).json({ error: error.message });
            }
        });

        // Authentication endpoint
        this.app.post('/api/auth', async (req, res) => {
            try {
                const result = await this.authService.authenticateDevice(req.body);
                res.json(result);
            } catch (error) {
                logger.error('Authentication failed:', error);
                res.status(401).json({ error: error.message });
            }
        });

        // GitHub OAuth endpoints (optional)
        this.app.get('/auth/github/login', (req, res) => {
            if (!this.githubOAuth?.isEnabled()) {
                return res.status(503).json({ error: 'GitHub OAuth not configured' });
            }
            const { url } = this.githubOAuth.createAuthUrl();
            res.redirect(url);
        });

        this.app.get('/auth/github/callback', async (req, res) => {
            try {
                if (!this.githubOAuth?.isEnabled()) {
                    return res.status(503).json({ error: 'GitHub OAuth not configured' });
                }
                const { code, state } = req.query;
                if (!code || !state) {
                    return res.status(400).json({ error: 'Missing code or state' });
                }
                if (!this.githubOAuth.validateState(state)) {
                    return res.status(400).json({ error: 'Invalid OAuth state' });
                }

                // Exchange code for token and fetch user
                const accessToken = await this.githubOAuth.exchangeCodeForToken(code);
                const ghUser = await this.githubOAuth.fetchGitHubUser(accessToken);

                // Ensure user record exists
                const userId = await this.githubOAuth.ensureUser(ghUser.email, ghUser.name);

                // Issue app session token (JWT) tied to user (no device yet)
                const sessionId = uuidv4();
                const jwtPayload = { user_id: userId, provider: 'github', type: 'user_session' };
                const jwtToken = jwt.sign(jwtPayload, process.env.JWT_SECRET, {
                    expiresIn: process.env.JWT_EXPIRES_IN || '24h',
                    issuer: 'juno-cloud-server'
                });

                // Respond with minimal HTML to copy token or redirect URL if provided
                const redirect = process.env.GITHUB_POST_LOGIN_REDIRECT_URL;
                if (redirect) {
                    const url = new URL(redirect);
                    url.searchParams.set('token', jwtToken);
                    return res.redirect(url.toString());
                }
                res.send(`<html><body><h3>GitHub login successful</h3><p>Copy this token:</p><pre>${jwtToken}</pre></body></html>`);
            } catch (error) {
                logger.error('GitHub OAuth callback failed:', error);
                res.status(500).json({ error: 'GitHub OAuth failed' });
            }
        });

        // Premium features endpoint
        this.app.post('/api/premium/activate', async (req, res) => {
            try {
                // TODO: Implement Stripe integration
                res.json({ message: 'Premium features endpoint - coming soon!' });
            } catch (error) {
                logger.error('Premium activation failed:', error);
                res.status(400).json({ error: error.message });
            }
        });

        logger.info('✅ Express app configured');
    }

    setupWebSocket() {
        this.wss = new WebSocketServer({
            server: this.server,
            path: '/ws',
            perMessageDeflate: false,
        });

        this.wsManager = new WebSocketManager(
            this.wss,
            this.authService,
            this.commandProcessor,
            this.database
        );

        // Track connections for graceful shutdown
        this.wss.on('connection', (ws, request) => {
            this.activeConnections.add(ws);

            ws.on('close', () => {
                this.activeConnections.delete(ws);
            });
        });

        logger.info('✅ WebSocket server configured');
    }

    setupGracefulShutdown() {
        const shutdown = async (signal) => {
            if (this.isShuttingDown) return;

            logger.info(`📡 Received ${signal}, starting graceful shutdown...`);
            this.isShuttingDown = true;

            // Stop accepting new connections
            this.server.close(() => {
                logger.info('🚪 HTTP server closed');
            });

            // Close WebSocket connections
            if (this.activeConnections.size > 0) {
                logger.info(`🔌 Closing ${this.activeConnections.size} WebSocket connections...`);

                for (const ws of this.activeConnections) {
                    ws.close(1001, 'Server shutting down');
                }

                // Wait a bit for connections to close gracefully
                await new Promise(resolve => setTimeout(resolve, 2000));
            }

            // Stop health check
            if (this.healthCheck) {
                this.healthCheck.stop();
            }

            // Close database
            if (this.database) {
                await this.database.close();
                logger.info('🗄️ Database closed');
            }

            logger.info('👋 Graceful shutdown complete');
            process.exit(0);
        };

        process.on('SIGTERM', () => shutdown('SIGTERM'));
        process.on('SIGINT', () => shutdown('SIGINT'));
    }

    getMetrics() {
        return {
            timestamp: new Date().toISOString(),
            uptime: process.uptime(),
            memory: process.memoryUsage(),
            connections: {
                active: this.activeConnections.size,
                total: this.wsManager?.getTotalConnections() || 0,
            },
            database: this.database?.getStats() || {},
            health: this.healthCheck?.getStatus() || {},
        };
    }

    async start() {
        const port = process.env.PORT || 8080;
        const host = process.env.HOST || '0.0.0.0';

        this.server.listen(port, host, () => {
            logger.info(`🌐 Juno Cloud Server running on ${host}:${port}`);
            logger.info(`🔌 WebSocket endpoint: ws://${host}:${port}/ws`);
            logger.info(`🏥 Health check: http://${host}:${port}/health`);

            if (process.env.METRICS_ENABLED === 'true') {
                logger.info(`📊 Metrics: http://${host}:${port}/metrics`);
            }
        });
    }
}

// Start the server
async function main() {
    try {
        const server = new JunoCloudServer();
        await server.initialize();
        await server.start();
    } catch (error) {
        logger.error('💥 Failed to start server:', error);
        process.exit(1);
    }
}

// Handle unhandled rejections
process.on('unhandledRejection', (reason, promise) => {
    logger.error('Unhandled Rejection at:', promise, 'reason:', reason);
    process.exit(1);
});

process.on('uncaughtException', (error) => {
    logger.error('Uncaught Exception:', error);
    process.exit(1);
});

main();

import { RateLimiterMemory, RateLimiterRedis } from 'rate-limiter-flexible';
import redis from 'redis';
import { logger } from '../utils/logger.js';

let rateLimiter;
let redisClient = null;

// Initialize rate limiter based on configuration
function initializeRateLimiter() {
    const useRedis = process.env.USE_REDIS === 'true';
    const windowMs = parseInt(process.env.RATE_LIMIT_WINDOW_MS) || 900000; // 15 minutes
    const maxRequests = parseInt(process.env.RATE_LIMIT_MAX_REQUESTS) || 100;

    const options = {
        keyPrefix: 'juno_rl',
        points: maxRequests, // Number of requests
        duration: Math.floor(windowMs / 1000), // Duration in seconds
        blockDuration: Math.floor(windowMs / 1000), // Block for same duration
    };

    if (useRedis) {
        try {
            redisClient = redis.createClient({
                url: process.env.REDIS_URL || 'redis://localhost:6379',
                password: process.env.REDIS_PASSWORD || undefined,
                socket: {
                    reconnectStrategy: (retries) => Math.min(retries * 50, 500)
                }
            });

            redisClient.on('error', (err) => {
                logger.error('Redis client error:', err);
                // Fallback to memory-based rate limiting
                initializeMemoryRateLimiter(options);
            });

            redisClient.on('connect', () => {
                logger.info('Redis client connected for rate limiting');
            });

            redisClient.connect();

            rateLimiter = new RateLimiterRedis({
                ...options,
                storeClient: redisClient,
            });

            logger.info('Rate limiter initialized with Redis storage');
        } catch (error) {
            logger.error('Failed to initialize Redis rate limiter, falling back to memory:', error);
            initializeMemoryRateLimiter(options);
        }
    } else {
        initializeMemoryRateLimiter(options);
    }
}

function initializeMemoryRateLimiter(options) {
    rateLimiter = new RateLimiterMemory(options);
    logger.info('Rate limiter initialized with memory storage');
}

// Express middleware
export const RateLimiter = async (req, res, next) => {
    // Initialize rate limiter if not already done
    if (!rateLimiter) {
        initializeRateLimiter();
    }

    // Determine the key for rate limiting
    const key = getRateLimitKey(req);

    try {
        // Check rate limit
        const result = await rateLimiter.consume(key);

        // Add rate limit headers
        res.set({
            'X-RateLimit-Limit': rateLimiter.points,
            'X-RateLimit-Remaining': result.remainingPoints,
            'X-RateLimit-Reset': new Date(Date.now() + result.msBeforeNext)
        });

        next();
    } catch (rejRes) {
        // Rate limit exceeded
        const secs = Math.round(rejRes.msBeforeNext / 1000) || 1;

        res.set({
            'X-RateLimit-Limit': rateLimiter.points,
            'X-RateLimit-Remaining': 0,
            'X-RateLimit-Reset': new Date(Date.now() + rejRes.msBeforeNext),
            'Retry-After': secs
        });

        // Log rate limit hit
        logger.warn('Rate limit exceeded', {
            key,
            ip: req.ip,
            userAgent: req.get('User-Agent'),
            endpoint: `${req.method} ${req.path}`,
            retryAfter: secs
        });

        res.status(429).json({
            error: 'Too Many Requests',
            message: `Rate limit exceeded. Try again in ${secs} seconds.`,
            retryAfter: secs
        });
    }
};

// Get rate limiting key based on request
function getRateLimitKey(req) {
    // Priority: API key > IP address
    const apiKey = req.headers['x-api-key'] || req.body?.api_key;
    if (apiKey) {
        return `api_key:${apiKey}`;
    }

    // Fallback to IP address
    const ip = req.ip || req.connection.remoteAddress || req.socket.remoteAddress;
    return `ip:${ip}`;
}

// WebSocket rate limiter
export class WebSocketRateLimiter {
    constructor() {
        this.connections = new Map(); // clientId -> last activity info
        this.cleanup();
    }

    async checkLimit(clientId, action = 'message') {
        if (!rateLimiter) {
            initializeRateLimiter();
        }

        const key = `ws:${clientId}:${action}`;

        try {
            await rateLimiter.consume(key);
            return { allowed: true };
        } catch (rejRes) {
            const secs = Math.round(rejRes.msBeforeNext / 1000) || 1;
            logger.warn('WebSocket rate limit exceeded', {
                clientId,
                action,
                retryAfter: secs
            });

            return {
                allowed: false,
                retryAfter: secs,
                message: `Rate limit exceeded. Try again in ${secs} seconds.`
            };
        }
    }

    // Track connection activity
    trackActivity(clientId, action = 'activity') {
        this.connections.set(clientId, {
            lastActivity: Date.now(),
            action
        });
    }

    // Remove client tracking
    removeClient(clientId) {
        this.connections.delete(clientId);
    }

    // Cleanup old connection data
    cleanup() {
        setInterval(() => {
            const now = Date.now();
            const maxAge = 24 * 60 * 60 * 1000; // 24 hours

            for (const [clientId, data] of this.connections) {
                if (now - data.lastActivity > maxAge) {
                    this.connections.delete(clientId);
                }
            }
        }, 60 * 60 * 1000); // Run every hour
    }

    getStats() {
        return {
            activeConnections: this.connections.size,
            rateLimiterType: rateLimiter.constructor.name,
            redisConnected: redisClient?.isReady || false
        };
    }
}

// Device-specific rate limiter for high-value operations
export class DeviceRateLimiter {
    constructor() {
        // Different limits for different command types
        this.limits = {
            voice_query: { points: 50, duration: 3600 }, // 50 per hour
            text_query: { points: 100, duration: 3600 }, // 100 per hour
            screenshot: { points: 20, duration: 3600 }, // 20 per hour
            system_command: { points: 30, duration: 3600 }, // 30 per hour
            default: { points: 200, duration: 3600 } // 200 per hour default
        };

        this.limiters = {};
        this.initializeLimiters();
    }

    initializeLimiters() {
        for (const [commandType, config] of Object.entries(this.limits)) {
            const options = {
                keyPrefix: `device_${commandType}`,
                points: config.points,
                duration: config.duration,
                blockDuration: 300 // 5 minutes block
            };

            if (redisClient && redisClient.isReady) {
                this.limiters[commandType] = new RateLimiterRedis({
                    ...options,
                    storeClient: redisClient
                });
            } else {
                this.limiters[commandType] = new RateLimiterMemory(options);
            }
        }
    }

    async checkDeviceLimit(deviceId, commandType) {
        const limiter = this.limiters[commandType] || this.limiters.default;
        const key = deviceId;

        try {
            const result = await limiter.consume(key);
            return {
                allowed: true,
                remaining: result.remainingPoints,
                resetTime: new Date(Date.now() + result.msBeforeNext)
            };
        } catch (rejRes) {
            const secs = Math.round(rejRes.msBeforeNext / 1000) || 1;
            return {
                allowed: false,
                retryAfter: secs,
                message: `Command rate limit exceeded for ${commandType}. Try again in ${secs} seconds.`
            };
        }
    }
}

// Initialize on module load
initializeRateLimiter();

// Cleanup on process exit
process.on('SIGTERM', async () => {
    if (redisClient) {
        await redisClient.quit();
    }
});

process.on('SIGINT', async () => {
    if (redisClient) {
        await redisClient.quit();
    }
});

export default RateLimiter;

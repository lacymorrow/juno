import { logger } from './logger.js';

// Required environment variables
const REQUIRED_ENV_VARS = [
    'JWT_SECRET',
    'HMAC_SECRET'
];

// Optional environment variables with defaults
const OPTIONAL_ENV_VARS = {
    NODE_ENV: 'production',
    PORT: '8080',
    HOST: '0.0.0.0',
    WS_PORT: '8080',
    WS_HEARTBEAT_INTERVAL: '30000',
    WS_CONNECTION_TIMEOUT: '300000',
    JWT_EXPIRES_IN: '24h',
    JWT_REFRESH_EXPIRES_IN: '7d',
    DB_PATH: './data/juno.db',
    DB_BACKUP_INTERVAL: '3600000',
    USE_REDIS: 'false',
    REDIS_URL: 'redis://localhost:6379',
    RATE_LIMIT_WINDOW_MS: '900000',
    RATE_LIMIT_MAX_REQUESTS: '100',
    RATE_LIMIT_SKIP_SUCCESSFUL_REQUESTS: 'true',
    BCRYPT_ROUNDS: '12',
    CORS_ORIGIN: '*',
    LOG_LEVEL: 'info',
    LOG_FILE: './logs/server.log',
    LOG_MAX_SIZE: '10m',
    LOG_MAX_FILES: '5',
    PREMIUM_ENABLED: 'true',
    MAX_CONCURRENT_COMMANDS: '10',
    COMMAND_TIMEOUT_MS: '30000',
    ENABLE_COMMAND_HISTORY: 'true',
    MAX_COMMAND_HISTORY: '1000',
    HEALTH_CHECK_INTERVAL: '60000',
    METRICS_ENABLED: 'true'
};

export function validateEnv() {
    const errors = [];
    const warnings = [];

    // Check required variables
    for (const envVar of REQUIRED_ENV_VARS) {
        if (!process.env[envVar]) {
            errors.push(`Missing required environment variable: ${envVar}`);
        }
    }

    // Set defaults for optional variables
    for (const [envVar, defaultValue] of Object.entries(OPTIONAL_ENV_VARS)) {
        if (!process.env[envVar]) {
            process.env[envVar] = defaultValue;
            warnings.push(`Environment variable ${envVar} not set, using default: ${defaultValue}`);
        }
    }

    // Validate specific formats
    validateSpecificFormats(errors, warnings);

    // Log warnings
    if (warnings.length > 0) {
        logger.warn('Environment validation warnings:', warnings);
    }

    // Throw if there are errors
    if (errors.length > 0) {
        logger.error('Environment validation errors:', errors);
        throw new Error(`Environment validation failed:\n${errors.join('\n')}`);
    }

    logger.info('Environment validation passed');
}

function validateSpecificFormats(errors, warnings) {
    // Validate port numbers
    const port = parseInt(process.env.PORT);
    if (isNaN(port) || port < 1 || port > 65535) {
        errors.push('PORT must be a valid port number (1-65535)');
    }

    const wsPort = parseInt(process.env.WS_PORT);
    if (isNaN(wsPort) || wsPort < 1 || wsPort > 65535) {
        errors.push('WS_PORT must be a valid port number (1-65535)');
    }

    // Validate JWT secret length
    if (process.env.JWT_SECRET && process.env.JWT_SECRET.length < 32) {
        warnings.push('JWT_SECRET should be at least 32 characters long for security');
    }

    if (process.env.HMAC_SECRET && process.env.HMAC_SECRET.length < 32) {
        warnings.push('HMAC_SECRET should be at least 32 characters long for security');
    }

    // Validate numeric values
    const numericVars = [
        'WS_HEARTBEAT_INTERVAL',
        'WS_CONNECTION_TIMEOUT',
        'DB_BACKUP_INTERVAL',
        'RATE_LIMIT_WINDOW_MS',
        'RATE_LIMIT_MAX_REQUESTS',
        'BCRYPT_ROUNDS',
        'MAX_CONCURRENT_COMMANDS',
        'COMMAND_TIMEOUT_MS',
        'MAX_COMMAND_HISTORY',
        'HEALTH_CHECK_INTERVAL'
    ];

    for (const varName of numericVars) {
        const value = parseInt(process.env[varName]);
        if (isNaN(value)) {
            errors.push(`${varName} must be a valid number`);
        }
    }

    // Validate boolean values
    const booleanVars = [
        'USE_REDIS',
        'RATE_LIMIT_SKIP_SUCCESSFUL_REQUESTS',
        'PREMIUM_ENABLED',
        'ENABLE_COMMAND_HISTORY',
        'METRICS_ENABLED'
    ];

    for (const varName of booleanVars) {
        const value = process.env[varName];
        if (value && !['true', 'false'].includes(value.toLowerCase())) {
            errors.push(`${varName} must be 'true' or 'false'`);
        }
    }

    // Validate log level
    const validLogLevels = ['error', 'warn', 'info', 'debug'];
    if (!validLogLevels.includes(process.env.LOG_LEVEL)) {
        warnings.push(`LOG_LEVEL should be one of: ${validLogLevels.join(', ')}`);
    }

    // Validate CORS origin
    if (process.env.CORS_ORIGIN === '*') {
        warnings.push('CORS_ORIGIN is set to *, consider restricting for production');
    }
}

// Validate JWT token format
export function validateJwtToken(token) {
    if (!token) {
        throw new Error('Token is required');
    }

    if (typeof token !== 'string') {
        throw new Error('Token must be a string');
    }

    // Basic JWT format check (3 parts separated by dots)
    const parts = token.split('.');
    if (parts.length !== 3) {
        throw new Error('Invalid JWT token format');
    }

    return true;
}

// Validate device registration data
export function validateDeviceRegistration(data) {
    const errors = [];

    if (!data.device_name || typeof data.device_name !== 'string') {
        errors.push('device_name is required and must be a string');
    } else if (data.device_name.length > 100) {
        errors.push('device_name must be 100 characters or less');
    }

    if (data.device_type && !['desktop', 'mobile', 'server', 'embedded'].includes(data.device_type)) {
        errors.push('device_type must be one of: desktop, mobile, server, embedded');
    }

    if (data.user_email && !isValidEmail(data.user_email)) {
        errors.push('user_email must be a valid email address');
    }

    if (data.user_name && typeof data.user_name !== 'string') {
        errors.push('user_name must be a string');
    } else if (data.user_name && data.user_name.length > 100) {
        errors.push('user_name must be 100 characters or less');
    }

    if (errors.length > 0) {
        throw new Error(`Validation failed:\n${errors.join('\n')}`);
    }

    return true;
}

// Validate authentication data
export function validateAuthData(data) {
    const errors = [];

    if (!data.api_key || typeof data.api_key !== 'string') {
        errors.push('api_key is required and must be a string');
    }

    if (!data.timestamp || typeof data.timestamp !== 'number') {
        errors.push('timestamp is required and must be a number');
    }

    if (!data.signature || typeof data.signature !== 'string') {
        errors.push('signature is required and must be a string');
    }

    if (errors.length > 0) {
        throw new Error(`Validation failed:\n${errors.join('\n')}`);
    }

    return true;
}

// Validate command data
export function validateCommandData(data) {
    const errors = [];

    if (!data.command_type || typeof data.command_type !== 'string') {
        errors.push('command_type is required and must be a string');
    }

    const validCommandTypes = [
        'voice_query',
        'text_query',
        'system_command',
        'status_request',
        'screenshot',
        'config_update'
    ];

    if (data.command_type && !validCommandTypes.includes(data.command_type)) {
        errors.push(`command_type must be one of: ${validCommandTypes.join(', ')}`);
    }

    if (errors.length > 0) {
        throw new Error(`Validation failed:\n${errors.join('\n')}`);
    }

    return true;
}

// Helper function to validate email format
function isValidEmail(email) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return emailRegex.test(email);
}

// Sanitize input data
export function sanitizeInput(input) {
    if (typeof input === 'string') {
        // Remove potential XSS and injection patterns
        return input
            .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '')
            .replace(/javascript:/gi, '')
            .replace(/on\w+=/gi, '')
            .trim();
    }
    return input;
}

export default {
    validateEnv,
    validateJwtToken,
    validateDeviceRegistration,
    validateAuthData,
    validateCommandData,
    sanitizeInput
};

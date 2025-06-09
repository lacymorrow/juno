import path from 'path';
import { fileURLToPath } from 'url';
import winston from 'winston';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Create logs directory if it doesn't exist
import fs from 'fs';
const logsDir = path.join(__dirname, '../../logs');
if (!fs.existsSync(logsDir)) {
    fs.mkdirSync(logsDir, { recursive: true });
}

// Custom format for console output
const consoleFormat = winston.format.combine(
    winston.format.timestamp({ format: 'HH:mm:ss' }),
    winston.format.colorize(),
    winston.format.printf(({ timestamp, level, message, ...meta }) => {
        let metaStr = '';
        if (Object.keys(meta).length > 0) {
            metaStr = ' ' + JSON.stringify(meta, null, 2);
        }
        return `${timestamp} [${level}] ${message}${metaStr}`;
    })
);

// Custom format for file output
const fileFormat = winston.format.combine(
    winston.format.timestamp(),
    winston.format.errors({ stack: true }),
    winston.format.json()
);

// Create the logger
export const logger = winston.createLogger({
    level: process.env.LOG_LEVEL || 'info',
    defaultMeta: { service: 'juno-cloud-server' },
    transports: [
        // Console output
        new winston.transports.Console({
            format: consoleFormat,
            silent: process.env.NODE_ENV === 'test'
        }),

        // File output for all logs
        new winston.transports.File({
            filename: path.join(logsDir, 'server.log'),
            format: fileFormat,
            maxsize: parseInt(process.env.LOG_MAX_SIZE?.replace('m', '')) * 1024 * 1024 || 10 * 1024 * 1024, // 10MB default
            maxFiles: parseInt(process.env.LOG_MAX_FILES) || 5,
            tailable: true
        }),

        // Error-only file
        new winston.transports.File({
            filename: path.join(logsDir, 'error.log'),
            level: 'error',
            format: fileFormat,
            maxsize: 5 * 1024 * 1024, // 5MB
            maxFiles: 3
        })
    ]
});

// Add request logging helper
export const logRequest = (req, res, next) => {
    const start = Date.now();

    res.on('finish', () => {
        const duration = Date.now() - start;
        const logData = {
            method: req.method,
            url: req.url,
            status: res.statusCode,
            duration: `${duration}ms`,
            ip: req.ip || req.connection.remoteAddress,
            userAgent: req.get('User-Agent')
        };

        if (res.statusCode >= 400) {
            logger.error('HTTP Request', logData);
        } else {
            logger.info('HTTP Request', logData);
        }
    });

    next();
};

// WebSocket connection logger
export const logWebSocketConnection = (clientId, action, data = {}) => {
    logger.info(`WebSocket ${action}`, {
        clientId,
        action,
        ...data
    });
};

// Command execution logger
export const logCommand = (commandId, commandType, status, data = {}) => {
    const logData = {
        commandId,
        commandType,
        status,
        ...data
    };

    if (status === 'error') {
        logger.error('Command execution', logData);
    } else {
        logger.info('Command execution', logData);
    }
};

// Authentication logger
export const logAuth = (deviceId, action, success, data = {}) => {
    const logData = {
        deviceId,
        action,
        success,
        ...data
    };

    if (!success) {
        logger.warn('Authentication failed', logData);
    } else {
        logger.info('Authentication success', logData);
    }
};

export default logger;

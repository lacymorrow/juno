import fs from 'fs';
import path from 'path';
import sqlite3 from 'sqlite3';
import { fileURLToPath } from 'url';
import { logger } from '../utils/logger.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export class Database {
    constructor() {
        this.db = null;
        this.dbPath = process.env.DB_PATH || path.join(__dirname, '../../data/juno.db');
        this.stats = {
            connections: 0,
            queries: 0,
            errors: 0
        };
    }

    async initialize() {
        try {
            // Ensure data directory exists
            const dataDir = path.dirname(this.dbPath);
            if (!fs.existsSync(dataDir)) {
                fs.mkdirSync(dataDir, { recursive: true });
            }

            // Open database connection
            this.db = new sqlite3.Database(this.dbPath, (err) => {
                if (err) {
                    throw new Error(`Failed to open database: ${err.message}`);
                }
            });

            // Enable foreign keys
            await this.run('PRAGMA foreign_keys = ON');

            // Create tables
            await this.createTables();

            // Setup periodic stats reset
            setInterval(() => {
                this.stats.queries = 0;
                this.stats.errors = 0;
            }, 3600000); // Reset hourly

            logger.info(`Database initialized at ${this.dbPath}`);
        } catch (error) {
            logger.error('Database initialization failed:', error);
            throw error;
        }
    }

    async createTables() {
        const tables = [
            // Users table
            `CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE,
                name TEXT,
                premium_until INTEGER,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER DEFAULT (strftime('%s', 'now'))
            )`,

            // Devices table
            `CREATE TABLE IF NOT EXISTS devices (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                device_name TEXT NOT NULL,
                device_type TEXT,
                api_key TEXT UNIQUE NOT NULL,
                hmac_secret TEXT NOT NULL,
                last_seen INTEGER,
                is_active BOOLEAN DEFAULT 1,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
            )`,

            // Sessions table
            `CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                jwt_token TEXT NOT NULL,
                expires_at INTEGER NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (device_id) REFERENCES devices (id) ON DELETE CASCADE
            )`,

            // Commands table
            `CREATE TABLE IF NOT EXISTS commands (
                id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                command_type TEXT NOT NULL,
                payload TEXT,
                status TEXT DEFAULT 'pending',
                response TEXT,
                error TEXT,
                executed_at INTEGER,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (device_id) REFERENCES devices (id) ON DELETE CASCADE
            )`,

            // Audit log table
            `CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id TEXT,
                action TEXT NOT NULL,
                details TEXT,
                ip_address TEXT,
                user_agent TEXT,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (device_id) REFERENCES devices (id) ON DELETE SET NULL
            )`,

            // Premium subscriptions table
            `CREATE TABLE IF NOT EXISTS subscriptions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                stripe_subscription_id TEXT UNIQUE,
                status TEXT NOT NULL,
                current_period_start INTEGER,
                current_period_end INTEGER,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
            )`,

            // Rate limiting table
            `CREATE TABLE IF NOT EXISTS rate_limits (
                key TEXT PRIMARY KEY,
                count INTEGER DEFAULT 0,
                reset_time INTEGER,
                updated_at INTEGER DEFAULT (strftime('%s', 'now'))
            )`
        ];

        const indexes = [
            'CREATE INDEX IF NOT EXISTS idx_devices_user_id ON devices (user_id)',
            'CREATE INDEX IF NOT EXISTS idx_devices_api_key ON devices (api_key)',
            'CREATE INDEX IF NOT EXISTS idx_sessions_device_id ON sessions (device_id)',
            'CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at)',
            'CREATE INDEX IF NOT EXISTS idx_commands_device_id ON commands (device_id)',
            'CREATE INDEX IF NOT EXISTS idx_commands_status ON commands (status)',
            'CREATE INDEX IF NOT EXISTS idx_commands_created_at ON commands (created_at)',
            'CREATE INDEX IF NOT EXISTS idx_audit_log_device_id ON audit_log (device_id)',
            'CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log (created_at)',
            'CREATE INDEX IF NOT EXISTS idx_subscriptions_user_id ON subscriptions (user_id)',
            'CREATE INDEX IF NOT EXISTS idx_rate_limits_reset_time ON rate_limits (reset_time)'
        ];

        // Create tables
        for (const tableSQL of tables) {
            await this.run(tableSQL);
        }

        // Create indexes
        for (const indexSQL of indexes) {
            await this.run(indexSQL);
        }

        logger.info('Database tables and indexes created/verified');
    }

    // Promisified database operations
    run(sql, params = []) {
        return new Promise((resolve, reject) => {
            this.stats.queries++;
            this.db.run(sql, params, function(err) {
                if (err) {
                    this.stats.errors++;
                    logger.error('Database run error:', { sql, params, error: err.message });
                    reject(err);
                } else {
                    resolve({ id: this.lastID, changes: this.changes });
                }
            }.bind(this));
        });
    }

    get(sql, params = []) {
        return new Promise((resolve, reject) => {
            this.stats.queries++;
            this.db.get(sql, params, (err, row) => {
                if (err) {
                    this.stats.errors++;
                    logger.error('Database get error:', { sql, params, error: err.message });
                    reject(err);
                } else {
                    resolve(row);
                }
            });
        });
    }

    all(sql, params = []) {
        return new Promise((resolve, reject) => {
            this.stats.queries++;
            this.db.all(sql, params, (err, rows) => {
                if (err) {
                    this.stats.errors++;
                    logger.error('Database all error:', { sql, params, error: err.message });
                    reject(err);
                } else {
                    resolve(rows);
                }
            });
        });
    }

    // Utility methods
    async createUser(userData) {
        const { id, email, name } = userData;
        const result = await this.run(
            'INSERT INTO users (id, email, name) VALUES (?, ?, ?)',
            [id, email, name]
        );
        return result;
    }

    async createDevice(deviceData) {
        const { id, user_id, device_name, device_type, api_key, hmac_secret } = deviceData;
        const result = await this.run(
            'INSERT INTO devices (id, user_id, device_name, device_type, api_key, hmac_secret) VALUES (?, ?, ?, ?, ?, ?)',
            [id, user_id, device_name, device_type, api_key, hmac_secret]
        );
        return result;
    }

    async getDeviceByApiKey(apiKey) {
        return await this.get(
            'SELECT * FROM devices WHERE api_key = ? AND is_active = 1',
            [apiKey]
        );
    }

    async updateDeviceLastSeen(deviceId) {
        const now = Math.floor(Date.now() / 1000);
        return await this.run(
            'UPDATE devices SET last_seen = ? WHERE id = ?',
            [now, deviceId]
        );
    }

    async createSession(sessionData) {
        const { id, device_id, jwt_token, expires_at } = sessionData;
        return await this.run(
            'INSERT INTO sessions (id, device_id, jwt_token, expires_at) VALUES (?, ?, ?, ?)',
            [id, device_id, jwt_token, expires_at]
        );
    }

    async getValidSession(token) {
        const now = Math.floor(Date.now() / 1000);
        return await this.get(
            'SELECT s.*, d.* FROM sessions s JOIN devices d ON s.device_id = d.id WHERE s.jwt_token = ? AND s.expires_at > ? AND d.is_active = 1',
            [token, now]
        );
    }

    async createCommand(commandData) {
        const { id, device_id, command_type, payload } = commandData;
        return await this.run(
            'INSERT INTO commands (id, device_id, command_type, payload) VALUES (?, ?, ?, ?)',
            [id, device_id, command_type, JSON.stringify(payload)]
        );
    }

    async updateCommand(commandId, updates) {
        const fields = [];
        const values = [];

        if (updates.status) {
            fields.push('status = ?');
            values.push(updates.status);
        }
        if (updates.response) {
            fields.push('response = ?');
            values.push(JSON.stringify(updates.response));
        }
        if (updates.error) {
            fields.push('error = ?');
            values.push(updates.error);
        }
        if (updates.executed_at) {
            fields.push('executed_at = ?');
            values.push(updates.executed_at);
        }

        if (fields.length === 0) return;

        values.push(commandId);
        const sql = `UPDATE commands SET ${fields.join(', ')} WHERE id = ?`;

        return await this.run(sql, values);
    }

    async logAudit(auditData) {
        const { device_id, action, details, ip_address, user_agent } = auditData;
        return await this.run(
            'INSERT INTO audit_log (device_id, action, details, ip_address, user_agent) VALUES (?, ?, ?, ?, ?)',
            [device_id, action, JSON.stringify(details), ip_address, user_agent]
        );
    }

    async cleanupExpiredSessions() {
        const now = Math.floor(Date.now() / 1000);
        const result = await this.run('DELETE FROM sessions WHERE expires_at < ?', [now]);
        logger.info(`Cleaned up ${result.changes} expired sessions`);
        return result;
    }

    async cleanupOldCommands(daysToKeep = 30) {
        const cutoff = Math.floor(Date.now() / 1000) - (daysToKeep * 24 * 60 * 60);
        const result = await this.run('DELETE FROM commands WHERE created_at < ?', [cutoff]);
        logger.info(`Cleaned up ${result.changes} old commands`);
        return result;
    }

    async cleanupOldAuditLogs(daysToKeep = 90) {
        const cutoff = Math.floor(Date.now() / 1000) - (daysToKeep * 24 * 60 * 60);
        const result = await this.run('DELETE FROM audit_log WHERE created_at < ?', [cutoff]);
        logger.info(`Cleaned up ${result.changes} old audit logs`);
        return result;
    }

    getStats() {
        return {
            ...this.stats,
            path: this.dbPath,
            connected: !!this.db
        };
    }

    async close() {
        if (this.db) {
            return new Promise((resolve, reject) => {
                this.db.close((err) => {
                    if (err) {
                        logger.error('Error closing database:', err);
                        reject(err);
                    } else {
                        logger.info('Database connection closed');
                        resolve();
                    }
                });
            });
        }
    }
}

import crypto from 'crypto';
import jwt from 'jsonwebtoken';
import { v4 as uuidv4 } from 'uuid';
import { logAuth, logger } from '../utils/logger.js';

export class AuthService {
    constructor(database) {
        this.db = database;
        this.jwtSecret = process.env.JWT_SECRET;
        this.hmacSecret = process.env.HMAC_SECRET;

        if (!this.jwtSecret || !this.hmacSecret) {
            throw new Error('JWT_SECRET and HMAC_SECRET must be set in environment variables');
        }
    }

    // Generate API key and HMAC secret for new device
    generateDeviceCredentials() {
        const apiKey = crypto.randomBytes(32).toString('hex');
        const hmacSecret = crypto.randomBytes(32).toString('hex');
        return { apiKey, hmacSecret };
    }

    // Generate HMAC signature for request validation
    generateHmacSignature(method, path, body, timestamp, hmacSecret) {
        const payload = `${method}:${path}:${body || ''}:${timestamp}`;
        return crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
    }

    // Validate HMAC signature
    validateHmacSignature(method, path, body, timestamp, signature, hmacSecret) {
        const expectedSignature = this.generateHmacSignature(method, path, body, timestamp, hmacSecret);

        // Check timestamp (5 minutes tolerance)
        const now = Math.floor(Date.now() / 1000);
        const timestampDiff = Math.abs(now - timestamp);
        if (timestampDiff > 300) {
            throw new Error('Request timestamp too old');
        }

        // Compare signatures
        if (!crypto.timingSafeEqual(Buffer.from(signature, 'hex'), Buffer.from(expectedSignature, 'hex'))) {
            throw new Error('Invalid HMAC signature');
        }

        return true;
    }

    // Register a new device
    async registerDevice(registrationData) {
        try {
            const {
                device_name,
                device_type = 'desktop',
                user_email = null,
                user_name = null
            } = registrationData;

            if (!device_name) {
                throw new Error('Device name is required');
            }

            // Generate device credentials
            const { apiKey, hmacSecret } = this.generateDeviceCredentials();
            const deviceId = uuidv4();

            // Create or get user if email provided
            let userId = null;
            if (user_email) {
                userId = uuidv4();
                try {
                    await this.db.createUser({
                        id: userId,
                        email: user_email,
                        name: user_name
                    });
                } catch (error) {
                    // User might already exist
                    if (error.message.includes('UNIQUE constraint failed')) {
                        const existingUser = await this.db.get('SELECT id FROM users WHERE email = ?', [user_email]);
                        if (existingUser) {
                            userId = existingUser.id;
                        }
                    } else {
                        throw error;
                    }
                }
            }

            // Create device record
            await this.db.createDevice({
                id: deviceId,
                user_id: userId,
                device_name,
                device_type,
                api_key: apiKey,
                hmac_secret: hmacSecret
            });

            // Log successful registration
            await this.db.logAudit({
                device_id: deviceId,
                action: 'device_registered',
                details: { device_name, device_type },
                ip_address: null,
                user_agent: null
            });

            logAuth(deviceId, 'register', true, { device_name });

            return {
                success: true,
                device_id: deviceId,
                api_key: apiKey,
                hmac_secret: hmacSecret,
                message: 'Device registered successfully'
            };

        } catch (error) {
            logger.error('Device registration failed:', error);
            logAuth(null, 'register', false, { error: error.message });
            throw error;
        }
    }

    // Authenticate device and create session
    async authenticateDevice(authData) {
        try {
            const {
                api_key,
                timestamp,
                signature,
                method = 'POST',
                path = '/api/auth',
                body = null
            } = authData;

            if (!api_key || !timestamp || !signature) {
                throw new Error('Missing required authentication fields');
            }

            // Get device by API key
            const device = await this.db.getDeviceByApiKey(api_key);
            if (!device) {
                throw new Error('Invalid API key');
            }

            // Validate HMAC signature
            this.validateHmacSignature(
                method,
                path,
                body ? JSON.stringify(body) : '',
                timestamp,
                signature,
                device.hmac_secret
            );

            // Create JWT token
            const tokenPayload = {
                device_id: device.id,
                api_key: device.api_key,
                type: 'device_session'
            };

            const jwtToken = jwt.sign(tokenPayload, this.jwtSecret, {
                expiresIn: process.env.JWT_EXPIRES_IN || '24h',
                issuer: 'juno-cloud-server'
            });

            // Calculate expiration time
            const expiresAt = Math.floor(Date.now() / 1000) + (24 * 60 * 60); // 24 hours

            // Create session record
            const sessionId = uuidv4();
            await this.db.createSession({
                id: sessionId,
                device_id: device.id,
                jwt_token: jwtToken,
                expires_at: expiresAt
            });

            // Update device last seen
            await this.db.updateDeviceLastSeen(device.id);

            // Log successful authentication
            await this.db.logAudit({
                device_id: device.id,
                action: 'device_authenticated',
                details: { session_id: sessionId },
                ip_address: null,
                user_agent: null
            });

            logAuth(device.id, 'authenticate', true, { device_name: device.device_name });

            // Get device permissions (basic set for now)
            const permissions = this.getDevicePermissions(device);

            return {
                success: true,
                token: jwtToken,
                device_id: device.id,
                device_name: device.device_name,
                permissions,
                expires_at: expiresAt,
                session_id: sessionId
            };

        } catch (error) {
            logger.error('Device authentication failed:', error);
            logAuth(null, 'authenticate', false, { error: error.message });
            throw error;
        }
    }

    // Validate JWT token and return device info
    async validateToken(token) {
        try {
            // Verify JWT
            const decoded = jwt.verify(token, this.jwtSecret);

            if (decoded.type !== 'device_session') {
                throw new Error('Invalid token type');
            }

            // Get session from database
            const session = await this.db.getValidSession(token);
            if (!session) {
                throw new Error('Invalid or expired session');
            }

            // Update device last seen
            await this.db.updateDeviceLastSeen(session.device_id);

            return {
                valid: true,
                device_id: session.device_id,
                device_name: session.device_name,
                api_key: session.api_key,
                permissions: this.getDevicePermissions(session)
            };

        } catch (error) {
            if (error.name === 'JsonWebTokenError' || error.name === 'TokenExpiredError') {
                return { valid: false, error: 'Invalid or expired token' };
            }
            throw error;
        }
    }

    // Get device permissions based on device and user status
    getDevicePermissions(device) {
        const basePermissions = [
            'text_processing',
            'voice_transcription',
            'screenshot_capture',
            'system_automation',
            'file_operations'
        ];

        // Add premium permissions if user has active subscription
        // TODO: Check user premium status from database
        const premiumPermissions = [
            'advanced_automation',
            'cloud_storage',
            'priority_processing',
            'extended_history'
        ];

        // For now, return base permissions
        // In the future, check user.premium_until > current_time
        return basePermissions;
    }

    // Revoke device session
    async revokeSession(token) {
        try {
            const decoded = jwt.verify(token, this.jwtSecret);

            // Delete session from database
            const result = await this.db.run(
                'DELETE FROM sessions WHERE jwt_token = ?',
                [token]
            );

            if (result.changes > 0) {
                logAuth(decoded.device_id, 'logout', true);
                return { success: true, message: 'Session revoked' };
            } else {
                return { success: false, message: 'Session not found' };
            }

        } catch (error) {
            logger.error('Session revocation failed:', error);
            throw error;
        }
    }

    // Revoke all sessions for a device
    async revokeAllDeviceSessions(deviceId) {
        try {
            const result = await this.db.run(
                'DELETE FROM sessions WHERE device_id = ?',
                [deviceId]
            );

            logAuth(deviceId, 'logout_all', true, { sessions_revoked: result.changes });
            return { success: true, sessions_revoked: result.changes };

        } catch (error) {
            logger.error('Device session revocation failed:', error);
            throw error;
        }
    }

    // Check if device has specific permission
    hasPermission(device, permission) {
        const permissions = this.getDevicePermissions(device);
        return permissions.includes(permission);
    }

    // Cleanup expired sessions (called periodically)
    async cleanupExpiredSessions() {
        try {
            await this.db.cleanupExpiredSessions();
        } catch (error) {
            logger.error('Session cleanup failed:', error);
        }
    }
}

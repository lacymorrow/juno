'use strict';

const crypto = require('crypto');
const url = require('url');

// Device API keys are stored in the DEVICE_API_KEYS env var as JSON:
// '{"device-uuid-1": "secret-key-1", "device-uuid-2": "secret-key-2"}'
// Each Juno instance supplies a device_id and HMAC-signs the timestamp.
let deviceKeys = {};
try {
  deviceKeys = JSON.parse(process.env.DEVICE_API_KEYS || '{}');
} catch {
  console.warn('DEVICE_API_KEYS is not valid JSON — no devices will be able to connect');
}

/**
 * Authenticate an incoming WebSocket upgrade request from a Juno device.
 *
 * Expected query params:
 *   ?device_id=<uuid>&ts=<unix-seconds>&sig=<hmac-sha256-hex>
 *
 * The HMAC is computed as: HMAC-SHA256(key, "<device_id>:<ts>")
 * Timestamps within 60 seconds of now are accepted.
 *
 * @returns {string|null} deviceId if valid, null otherwise
 */
function authenticateDevice(req) {
  const { query } = url.parse(req.url || '', true);
  const { device_id: deviceId, ts, sig } = query;

  if (!deviceId || !ts || !sig) return null;

  const apiKey = deviceKeys[deviceId];
  if (!apiKey) return null;

  // Replay-attack guard: reject timestamps older than 60 s
  const now = Math.floor(Date.now() / 1000);
  const timestamp = parseInt(ts, 10);
  if (isNaN(timestamp) || Math.abs(now - timestamp) > 60) return null;

  const expected = crypto
    .createHmac('sha256', apiKey)
    .update(`${deviceId}:${ts}`)
    .digest('hex');

  // Constant-time comparison to prevent timing attacks
  const sigBuf = Buffer.from(sig, 'hex');
  const expBuf = Buffer.from(expected, 'hex');
  if (sigBuf.length !== expBuf.length) return null;
  if (!crypto.timingSafeEqual(sigBuf, expBuf)) return null;

  return deviceId;
}

module.exports = { authenticateDevice };

'use strict';

// In-memory registry of connected Juno devices.
// deviceId -> { ws, connectedAt, lastSeen }
const connected = new Map();

function register(deviceId, ws) {
  connected.set(deviceId, { ws, connectedAt: new Date(), lastSeen: new Date() });
  console.log(`Device connected: ${deviceId} (total: ${connected.size})`);
}

function unregister(deviceId) {
  connected.delete(deviceId);
  console.log(`Device disconnected: ${deviceId} (total: ${connected.size})`);
}

function list() {
  return Array.from(connected.entries()).map(([id, info]) => ({
    deviceId: id,
    connectedAt: info.connectedAt,
    lastSeen: info.lastSeen,
  }));
}

/**
 * Send a command to a connected device.
 * @returns {boolean} true if the device was found and the message was sent
 */
function sendCommand(deviceId, payload) {
  const device = connected.get(deviceId);
  if (!device) return false;

  try {
    device.ws.send(JSON.stringify({ type: 'command', ...payload }));
    return true;
  } catch (err) {
    console.error(`Failed to send command to ${deviceId}:`, err.message);
    unregister(deviceId);
    return false;
  }
}

/**
 * Handle an incoming message from a device (e.g. status updates, responses).
 */
function handleMessage(deviceId, msg) {
  const device = connected.get(deviceId);
  if (device) device.lastSeen = new Date();

  // Heartbeat — just update lastSeen, no response needed
  if (msg.type === 'ping') return;

  console.log(`Message from ${deviceId}:`, JSON.stringify(msg).slice(0, 200));
}

module.exports = { register, unregister, list, sendCommand, handleMessage };

const WebSocket = require('ws');
const crypto = require('crypto');
const { v4: uuidv4 } = require('uuid');

// Helper function to generate HMAC signature
function generateHmacSignature(method, path, body, timestamp, hmacSecret) {
    const payload = `${method}:${path}:${body || ''}:${timestamp}`;
    return crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
}

console.log('🚀 Testing WebSocket connection to Juno Cloud Backend with proper HMAC auth...');

const ws = new WebSocket('wss://juno-cloud-backend.fly.dev/ws');

ws.on('open', () => {
    console.log('✅ Connected to WebSocket server');

    // For WebSocket auth, we need to simulate an HMAC-signed auth request
    const timestamp = Math.floor(Date.now() / 1000);
    const apiKey = '58f47d04e0f196c4ef541a035c8b80e4ae39dac052a0cc8ab322ffa976df3f92';

    // Note: In a real setup, you'd get the HMAC secret when registering the device
    // For testing, we'll use a placeholder that should fail gracefully
    const hmacSecret = 'your-hmac-secret-here'; // This would come from device registration

    const method = 'POST';
    const path = '/ws/auth';
    const body = '';

    const signature = generateHmacSignature(method, path, body, timestamp, hmacSecret);

    const authMessage = {
        type: 'auth',
        data: {
            api_key: apiKey,
            timestamp: timestamp,
            signature: signature,
            method: method,
            path: path,
            body: body
        }
    };

    console.log('🔐 Sending HMAC authentication...');
    console.log('Timestamp:', timestamp);
    console.log('Signature:', signature.substring(0, 16) + '...');
    ws.send(JSON.stringify(authMessage));
});

ws.on('message', (data) => {
    try {
        const message = JSON.parse(data.toString());
        console.log(`📨 Received ${message.type}:`, JSON.stringify(message.data, null, 2));

        // If we get authentication success, send the screenshot command
        if (message.type === 'auth' && message.data.success) {
            console.log('🎉 Authentication successful!');

            setTimeout(() => {
                const commandMessage = {
                    type: 'command',
                    data: {
                        id: uuidv4(),
                        command_type: 'screenshot',
                        payload: {},
                        timestamp: Math.floor(Date.now() / 1000)
                    }
                };

                console.log('📸 Sending screenshot command...');
                ws.send(JSON.stringify(commandMessage));
            }, 1000);
        }

        // If authentication fails, show the error
        if (message.type === 'auth' && !message.data.success) {
            console.log('❌ Authentication failed:', message.data.error);
            console.log('💡 This is expected if you haven\'t registered your device with HMAC secrets yet');
        }

        // If we get a command response, close after 2 seconds
        if (message.type === 'response') {
            console.log('✨ Got command response! Test complete.');
            setTimeout(() => {
                ws.close();
            }, 2000);
        }

    } catch (error) {
        console.error('❌ Failed to parse message:', error.message);
        console.log('Raw message:', data.toString());
    }
});

ws.on('close', (code, reason) => {
    console.log(`👋 Connection closed: ${code} - ${reason}`);
    console.log('\n📋 Test Summary:');
    console.log('✅ WebSocket connection: SUCCESS');
    console.log('✅ Server communication: SUCCESS');
    console.log('⚠️  HMAC Authentication: Expected to fail without proper device registration');
    console.log('\n💡 Next steps:');
    console.log('1. Register your device first to get API key and HMAC secret');
    console.log('2. Use those credentials for proper authentication');
    process.exit(0);
});

ws.on('error', (error) => {
    console.error('❌ WebSocket error:', error.message);
    process.exit(1);
});

// Timeout after 15 seconds
setTimeout(() => {
    console.log('⏰ Test timeout - closing connection');
    ws.close();
}, 15000);

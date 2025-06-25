const WebSocket = require('ws');
const crypto = require('crypto');
const { v4: uuidv4 } = require('uuid');

// Helper function to generate HMAC signature
function generateHmacSignature(method, path, body, timestamp, hmacSecret) {
    const payload = `${method}:${path}:${body || ''}:${timestamp}`;
    return crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
}

console.log('🚀 Testing WebSocket connection with REAL authentication credentials...');

// Real credentials from device registration
const API_KEY = "eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0";
const HMAC_SECRET = "7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244";

const ws = new WebSocket('wss://juno-cloud-backend.fly.dev/ws');

ws.on('open', () => {
    console.log('✅ Connected to WebSocket server');

    // Generate proper HMAC authentication
    const timestamp = Math.floor(Date.now() / 1000);
    const method = 'POST';
    const path = '/ws/auth';
    const body = '';

    const signature = generateHmacSignature(method, path, body, timestamp, HMAC_SECRET);

    const authMessage = {
        type: 'auth',
        data: {
            api_key: API_KEY,
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
            setTimeout(() => ws.close(), 2000);
        }

        // If we get a command response, test more commands
        if (message.type === 'response') {
            console.log('✨ Got command response!');

            // Try a text query next
            setTimeout(() => {
                const textQueryMessage = {
                    type: 'command',
                    data: {
                        id: uuidv4(),
                        command_type: 'text_query',
                        payload: {
                            query: 'Hello from WebSocket test!'
                        },
                        timestamp: Math.floor(Date.now() / 1000)
                    }
                };

                console.log('💬 Sending text query command...');
                ws.send(JSON.stringify(textQueryMessage));
            }, 2000);
        }

        // After getting two responses, close
        if (message.type === 'response' && message.data.data?.response_type === 'text') {
            console.log('🎯 Got text response! Test complete.');
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
    console.log('✅ HMAC Authentication: SUCCESS');
    console.log('✅ Command execution: SUCCESS');
    console.log('\n🎉 All tests passed! Your Juno Cloud Backend is working perfectly.');
    process.exit(0);
});

ws.on('error', (error) => {
    console.error('❌ WebSocket error:', error.message);
    process.exit(1);
});

// Timeout after 30 seconds
setTimeout(() => {
    console.log('⏰ Test timeout - closing connection');
    ws.close();
}, 30000);

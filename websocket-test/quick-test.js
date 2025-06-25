const WebSocket = require('ws');
const { v4: uuidv4 } = require('uuid');

console.log('🚀 Testing WebSocket connection to Juno Cloud Backend...');

const ws = new WebSocket('wss://juno-cloud-backend.fly.dev/ws');

ws.on('open', () => {
    console.log('✅ Connected to WebSocket server');

    // First, send authentication
    const authMessage = {
        type: 'auth',
        data: {
            api_key: '58f47d04e0f196c4ef541a035c8b80e4ae39dac052a0cc8ab322ffa976df3f92',
            device_id: 'test-client-' + Date.now(),
            device_name: 'Test Client'
        },
        timestamp: Math.floor(Date.now() / 1000)
    };

    console.log('🔐 Sending authentication...');
    ws.send(JSON.stringify(authMessage));
});

ws.on('message', (data) => {
    try {
        const message = JSON.parse(data.toString());
        console.log(`📨 Received ${message.type}:`, JSON.stringify(message.data, null, 2));

        // If we get a successful auth or status message, send the screenshot command
        if ((message.type === 'auth' && message.data.success) ||
            (message.type === 'status' && message.data.message)) {

            setTimeout(() => {
                const commandMessage = {
                    type: 'command',
                    data: {
                        id: uuidv4(),
                        command_type: 'screenshot',  // This is the correct command type
                        payload: {},
                        timestamp: Math.floor(Date.now() / 1000)
                    }
                };

                console.log('📸 Sending screenshot command...');
                ws.send(JSON.stringify(commandMessage));
            }, 1000);
        }

        // If we get a command response, close after 2 seconds
        if (message.type === 'response') {
            console.log('✨ Got response! Closing in 2 seconds...');
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

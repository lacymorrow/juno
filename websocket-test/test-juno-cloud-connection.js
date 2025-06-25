const WebSocket = require('ws');
const crypto = require('crypto');

// Test configuration
const SERVER_URL = 'wss://juno-cloud-backend.fly.dev/ws';
const API_KEY = 'eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0';
const HMAC_SECRET = '7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244';

console.log('🧪 Testing Juno Cloud Connection After Native WebSocket Fix');
console.log('=' .repeat(70));

// Create HMAC signature (using the correct format)
function createHmacSignature(method, path, body, timestamp, secret) {
    const payload = `${method}:${path}:${body || ''}:${timestamp}`;
    return crypto.createHmac('sha256', secret)
        .update(payload)
        .digest('hex');
}

// Create WebSocket message
function createMessage(type, data) {
    return JSON.stringify({
        type: type,
        data: data,
        timestamp: Math.floor(Date.now() / 1000)
    });
}

async function testCloudConnection() {
    return new Promise((resolve, reject) => {
        console.log('🔌 Connecting to cloud server...');

        const ws = new WebSocket(SERVER_URL);
        let authCompleted = false;
        let testCompleted = false;

        const timeout = setTimeout(() => {
            if (!testCompleted) {
                console.log('⏰ Test timeout after 30 seconds');
                ws.close();
                reject(new Error('Test timeout'));
            }
        }, 30000);

        ws.on('open', () => {
            console.log('✅ WebSocket connected successfully');

            // Send authentication using correct HMAC format
            const timestamp = Math.floor(Date.now() / 1000);
            const method = 'POST';
            const path = '/ws/auth';
            const body = '';

            const signature = createHmacSignature(method, path, body, timestamp, HMAC_SECRET);

            const authData = {
                api_key: API_KEY,
                timestamp: timestamp,
                signature: signature,
                method: method,
                path: path
            };

            const authMessage = createMessage('auth', authData);
            console.log('🔐 Sending authentication...');
            ws.send(authMessage);
        });

        ws.on('message', (data) => {
            try {
                const message = JSON.parse(data.toString());
                console.log('📨 Received message:', message.type);

                if (message.type === 'auth' && !authCompleted) {
                    if (message.data.success === true) {
                        console.log('✅ Authentication successful!');
                        authCompleted = true;

                        // Send a test command
                        const testCommand = {
                            id: 'test-' + Date.now(),
                            command_type: 'screenshot',
                            payload: {},
                            timestamp: Math.floor(Date.now() / 1000)
                        };

                        const commandMessage = createMessage('command', testCommand);
                        console.log('🚀 Sending test command...');
                        ws.send(commandMessage);

                    } else {
                        const error = message.data.error || 'Unknown authentication error';
                        console.log('❌ Authentication failed:', error);
                        clearTimeout(timeout);
                        ws.close();
                        reject(new Error(`Authentication failed: ${error}`));
                    }
                } else if (message.type === 'response' && authCompleted && !testCompleted) {
                    console.log('✅ Received command response!');
                    console.log('📊 Response data:', JSON.stringify(message.data, null, 2));
                    testCompleted = true;
                    clearTimeout(timeout);
                    ws.close();
                    resolve({
                        success: true,
                        authenticated: true,
                        commandResponseReceived: true,
                        responseData: message.data
                    });
                }
            } catch (error) {
                console.error('❌ Error parsing message:', error);
            }
        });

        ws.on('error', (error) => {
            console.error('❌ WebSocket error:', error);
            clearTimeout(timeout);
            reject(error);
        });

        ws.on('close', (code, reason) => {
            console.log(`🔌 WebSocket closed: ${code} ${reason}`);
            clearTimeout(timeout);
            if (!testCompleted && !authCompleted) {
                reject(new Error('Connection closed before test completion'));
            }
        });
    });
}

async function main() {
    try {
        console.log('📋 Test Summary:');
        console.log('   1. Connect to cloud WebSocket server');
        console.log('   2. Authenticate using HMAC signature');
        console.log('   3. Send test command');
        console.log('   4. Verify response received');
        console.log('');

        const result = await testCloudConnection();

        console.log('');
        console.log('🎉 ALL TESTS PASSED!');
        console.log('✅ Cloud backend is working correctly');
        console.log('✅ Authentication is working');
        console.log('✅ Command processing is working');
        console.log('');
        console.log('🔧 Next Steps:');
        console.log('   1. Open Juno app');
        console.log('   2. Go to Settings → Network');
        console.log('   3. Set API Key: eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0');
        console.log('   4. Click "Start Connector"');
        console.log('   5. Status should show "Ready" instead of "Reconnecting"');

    } catch (error) {
        console.log('');
        console.log('❌ TEST FAILED');
        console.error('Error:', error.message);
        console.log('');
        console.log('🔧 Possible Issues:');
        console.log('   - Cloud backend may be down');
        console.log('   - Network connectivity issues');
        console.log('   - Authentication credentials may have changed');
        process.exit(1);
    }
}

main();

const WebSocket = require('ws');
const crypto = require('crypto');
const { v4: uuidv4 } = require('uuid');

// Test configuration
const SERVER_URL = 'wss://juno-cloud-backend.fly.dev/ws';
const API_KEY = 'eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0';
const HMAC_SECRET = '7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244';

console.log('🧪 Testing Cloud Connection After WebSocket Fixes');
console.log('=' .repeat(60));

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
        console.log(`🔌 Connecting to: ${SERVER_URL}`);

        const ws = new WebSocket(SERVER_URL);
        let authCompleted = false;
        let testResults = {
            connected: false,
            authenticated: false,
            commandSent: false,
            responseReceived: false,
            error: null
        };

        // Connection timeout
        const timeout = setTimeout(() => {
            console.log('⏰ Connection timeout after 10 seconds');
            testResults.error = 'Connection timeout';
            ws.close();
            resolve(testResults);
        }, 10000);

        ws.on('open', () => {
            console.log('✅ Connected to WebSocket server');
            testResults.connected = true;

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
                path: path,
                body: body
            };

            const authMessage = createMessage('auth', authData);
            console.log('🔐 Sending authentication...');
            ws.send(authMessage);
        });

        ws.on('message', (data) => {
            try {
                const message = JSON.parse(data.toString());
                console.log('📨 Received message:', message.type);

                // Debug: Log the actual message content
                if (message.type === 'auth') {
                    console.log('🔍 Auth message data:', JSON.stringify(message.data, null, 2));
                }

                if (message.type === 'auth' && message.data.success === true) {
                    console.log('✅ Authentication successful');
                    testResults.authenticated = true;
                    authCompleted = true;

                    // Send a test command
                    const testCommand = {
                        id: uuidv4(),
                        command_type: 'test',
                        data: {
                            message: 'Hello from connection test!'
                        },
                        timeout: 30
                    };

                    const commandMessage = createMessage('command', testCommand);
                    console.log('📤 Sending test command...');
                    ws.send(commandMessage);
                    testResults.commandSent = true;

                } else if (message.type === 'response') {
                    console.log('✅ Received command response');
                    testResults.responseReceived = true;

                    // Test completed successfully
                    clearTimeout(timeout);
                    ws.close();
                    resolve(testResults);

                } else if (message.type === 'error') {
                    console.log('❌ Received error:', message.data);
                    testResults.error = message.data.message || 'Unknown error';
                    clearTimeout(timeout);
                    ws.close();
                    resolve(testResults);
                }

            } catch (error) {
                console.error('❌ Error parsing message:', error);
                testResults.error = `Message parsing error: ${error.message}`;
            }
        });

        ws.on('close', () => {
            console.log('🔌 WebSocket connection closed');
            if (!testResults.error && authCompleted) {
                // Normal completion
                resolve(testResults);
            }
        });

        ws.on('error', (error) => {
            console.error('❌ WebSocket error:', error.message);
            testResults.error = error.message;
            clearTimeout(timeout);
            resolve(testResults);
        });
    });
}

async function main() {
    try {
        const results = await testCloudConnection();

        console.log('\n📊 Test Results:');
        console.log('=' .repeat(40));
        console.log(`Connected: ${results.connected ? '✅' : '❌'}`);
        console.log(`Authenticated: ${results.authenticated ? '✅' : '❌'}`);
        console.log(`Command Sent: ${results.commandSent ? '✅' : '❌'}`);
        console.log(`Response Received: ${results.responseReceived ? '✅' : '❌'}`);

        if (results.error) {
            console.log(`Error: ❌ ${results.error}`);
        }

        // Overall status
        const allPassed = results.connected && results.authenticated &&
                         results.commandSent && results.responseReceived && !results.error;

        console.log('\n🎯 Overall Status:');
        if (allPassed) {
            console.log('✅ ALL TESTS PASSED - Cloud connection is working properly!');
            console.log('🚀 The Juno Rust backend should now be able to connect successfully');
        } else {
            console.log('❌ Some tests failed - connection issues remain');
            if (!results.connected) {
                console.log('   • WebSocket connection failed');
            }
            if (!results.authenticated) {
                console.log('   • Authentication failed');
            }
            if (!results.commandSent) {
                console.log('   • Command sending failed');
            }
            if (!results.responseReceived) {
                console.log('   • Response receiving failed');
            }
        }

        console.log('\n💡 Next Steps:');
        if (allPassed) {
            console.log('1. Open Juno AI app');
            console.log('2. Go to Network Settings');
            console.log('3. Click "Start Connector"');
            console.log('4. Check status should show "Ready" instead of "Reconnecting"');
        } else {
            console.log('1. Check cloud backend server status');
            console.log('2. Verify API key and HMAC secret');
            console.log('3. Check network connectivity');
        }

    } catch (error) {
        console.error('❌ Test failed with error:', error);
    }
}

main();

const WebSocket = require('ws');
const { v4: uuidv4 } = require('uuid');

// Test client configuration
const SERVER_URL = 'ws://localhost:8080/ws';
const TEST_COMMANDS = [
    {
        name: 'Status Request',
        command: {
            id: uuidv4(),
            command_type: 'status_request',
            payload: {},
            timestamp: Math.floor(Date.now() / 1000)
        }
    },
    {
        name: 'Text Query',
        command: {
            id: uuidv4(),
            command_type: 'text_query',
            payload: {
                query: 'Hello from test client'
            },
            timestamp: Math.floor(Date.now() / 1000)
        }
    },
    {
        name: 'System Command - Screenshot',
        command: {
            id: uuidv4(),
            command_type: 'system_command',
            payload: {
                action: 'screenshot'
            },
            timestamp: Math.floor(Date.now() / 1000)
        }
    }
];

function createWebSocketMessage(type, data) {
    return {
        type: type,
        data: data,
        timestamp: Math.floor(Date.now() / 1000)
    };
}

function runTests() {
    console.log('🧪 Starting WebSocket Test Client');
    console.log(`📡 Connecting to: ${SERVER_URL}\n`);

    const ws = new WebSocket(SERVER_URL);
    let testIndex = 0;
    let responses = [];

    ws.on('open', () => {
        console.log('✅ Connected to WebSocket server');
        console.log('📬 Waiting for welcome message...\n');

        // Send authentication first
        const authMessage = createWebSocketMessage('auth', {
            device_id: 'test-client-' + Date.now(),
            device_name: 'Test Client',
            api_key: 'test-key-123'
        });

        ws.send(JSON.stringify(authMessage));
    });

    ws.on('message', (data) => {
        try {
            const message = JSON.parse(data.toString());
            console.log(`📨 Received: ${message.type}`);

            switch (message.type) {
                case 'status':
                    console.log(`   Welcome: ${message.data.message || 'Connected'}`);
                    if (message.data.client_id) {
                        console.log(`   Client ID: ${message.data.client_id}`);
                    }
                    // Start sending test commands after welcome
                    setTimeout(() => sendNextTest(), 1000);
                    break;

                case 'auth':
                    console.log(`   Auth Success: ${message.data.success}`);
                    if (message.data.token) {
                        console.log(`   Token: ${message.data.token.substring(0, 20)}...`);
                    }
                    break;

                case 'response':
                    const response = message.data;
                    responses.push(response);
                    console.log(`   Response to: ${response.command_id}`);
                    console.log(`   Status: ${response.status}`);
                    if (response.data?.text) {
                        console.log(`   Text: ${response.data.text}`);
                    }
                    if (response.data?.metadata) {
                        console.log(`   Metadata: ${JSON.stringify(response.data.metadata, null, 2)}`);
                    }

                    // Send next test after a delay
                    setTimeout(() => sendNextTest(), 2000);
                    break;

                case 'heartbeat':
                    console.log(`   Heartbeat: Server time ${message.data.server_time}`);
                    break;

                case 'error':
                    console.error(`   Error: ${message.data.message}`);
                    break;

                default:
                    console.log(`   Data: ${JSON.stringify(message.data, null, 2)}`);
            }

            console.log(''); // Empty line for readability

        } catch (error) {
            console.error('❌ Failed to parse message:', error.message);
        }
    });

    function sendNextTest() {
        if (testIndex >= TEST_COMMANDS.length) {
            console.log('🎉 All tests completed!');
            console.log(`📊 Total responses received: ${responses.length}`);

            // Summary
            console.log('\n📋 Test Summary:');
            responses.forEach((response, index) => {
                const testName = TEST_COMMANDS[index]?.name || 'Unknown';
                console.log(`   ${index + 1}. ${testName}: ${response.status}`);
            });

            console.log('\n👋 Closing connection...');
            ws.close();
            return;
        }

        const test = TEST_COMMANDS[testIndex];
        console.log(`🧪 Test ${testIndex + 1}: ${test.name}`);
        console.log(`   Command ID: ${test.command.id}`);
        console.log(`   Type: ${test.command.command_type}`);

        const message = createWebSocketMessage('command', test.command);
        ws.send(JSON.stringify(message));

        testIndex++;
    }

    ws.on('close', () => {
        console.log('👋 Disconnected from WebSocket server');
        process.exit(0);
    });

    ws.on('error', (error) => {
        console.error('❌ WebSocket error:', error.message);
        process.exit(1);
    });

    // Handle process termination
    process.on('SIGINT', () => {
        console.log('\n⚠️  Test interrupted by user');
        ws.close();
        process.exit(0);
    });
}

// Start tests
runTests();

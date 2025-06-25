#!/usr/bin/env node

/**
 * Cloud Control Testing Script
 *
 * This script demonstrates how to control your Juno AI agent from a remote command-line interface
 * using curl commands and WebSocket connections.
 */

const WebSocket = require('ws');

console.log('🚀 Juno AI Cloud Control Test');
console.log('================================\n');

// Configuration
const BACKEND_URL = 'https://juno-cloud-backend.fly.dev';
const WS_URL = 'wss://juno-cloud-backend.fly.dev/ws';

async function registerDevice() {
    console.log('📱 Step 1: Registering test device...');

    const response = await fetch(`${BACKEND_URL}/api/register`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify({
            device_name: 'CLI-Test-Agent',
            device_type: 'desktop',
            platform: 'macos'
        })
    });

    if (!response.ok) {
        throw new Error(`Registration failed: ${response.statusText}`);
    }

    const data = await response.json();
    console.log('✅ Device registered successfully!');
    console.log(`   Device ID: ${data.device_id}`);
    console.log(`   API Key: ${data.api_key}\n`);

    return data;
}

function testWebSocketControl(apiKey, deviceId) {
    return new Promise((resolve, reject) => {
        console.log('🔌 Step 2: Testing WebSocket control...');

        const ws = new WebSocket(WS_URL);
        let authenticated = false;

        ws.on('open', () => {
            console.log('✅ WebSocket connected!');

            // Send authentication
            const authMessage = {
                type: 'auth',
                device_id: deviceId,
                api_key: apiKey
            };

            ws.send(JSON.stringify(authMessage));
            console.log('📤 Sent authentication message');
        });

        ws.on('message', (data) => {
            const message = JSON.parse(data.toString());
            console.log('📥 Received:', message);

            if (message.type === 'auth_success') {
                authenticated = true;
                console.log('🔐 Authentication successful!');

                // Send test commands
                setTimeout(() => {
                    console.log('📤 Sending test commands...\n');

                    // Test command 1: Take screenshot
                    ws.send(JSON.stringify({
                        type: 'command',
                        command: 'take_screenshot',
                        content: 'Take a screenshot of the current screen'
                    }));

                    // Test command 2: Get system status
                    setTimeout(() => {
                        ws.send(JSON.stringify({
                            type: 'command',
                            command: 'system_status',
                            content: 'Get current system status'
                        }));
                    }, 2000);

                    // Close connection after tests
                    setTimeout(() => {
                        ws.close();
                        resolve(true);
                    }, 5000);

                }, 1000);
            }
        });

        ws.on('error', (error) => {
            console.error('❌ WebSocket error:', error.message);
            reject(error);
        });

        ws.on('close', () => {
            console.log('🔌 Connection closed\n');
            if (authenticated) {
                resolve(true);
            } else {
                reject(new Error('Connection closed before authentication'));
            }
        });
    });
}

function generateCurlExamples(apiKey, deviceId) {
    console.log('📋 Step 3: Generated curl examples for manual testing:\n');

    console.log('🔹 Register a new device:');
    console.log(`curl -X POST -H "Content-Type: application/json" \\
  -d '{"device_name":"my-agent","device_type":"desktop","platform":"macos"}' \\
  ${BACKEND_URL}/api/register\n`);

    console.log('🔹 Test WebSocket with Node.js:');
    console.log(`node -e "
const WebSocket = require('ws');
const ws = new WebSocket('${WS_URL}');
ws.on('open', () => {
  // Authenticate
  ws.send(JSON.stringify({
    type: 'auth',
    device_id: '${deviceId}',
    api_key: '${apiKey}'
  }));

  // Send command after 1 second
  setTimeout(() => {
    ws.send(JSON.stringify({
      type: 'command',
      command: 'take_screenshot',
      content: 'Take a screenshot'
    }));
  }, 1000);

  // Close after 5 seconds
  setTimeout(() => ws.close(), 5000);
});
ws.on('message', (data) => console.log('Received:', data.toString()));
"\n`);

    console.log('🔹 Available commands to test:');
    console.log('   • take_screenshot - Capture current screen');
    console.log('   • system_status - Get system information');
    console.log('   • type_text - Type text (add "text" field)');
    console.log('   • click_at - Click at coordinates (add "x", "y" fields)');
    console.log('   • open_app - Open application (add "app_name" field)\n');
}

async function main() {
    try {
        // Step 1: Register device
        const registration = await registerDevice();

        // Step 2: Test WebSocket control
        await testWebSocketControl(registration.api_key, registration.device_id);

        // Step 3: Show curl examples
        generateCurlExamples(registration.api_key, registration.device_id);

        console.log('🎉 Cloud control test completed successfully!');
        console.log('🔧 Now set the API key in your Juno settings to connect your agent.');

    } catch (error) {
        console.error('❌ Test failed:', error.message);
        process.exit(1);
    }
}

// Run the test
if (require.main === module) {
    main();
}

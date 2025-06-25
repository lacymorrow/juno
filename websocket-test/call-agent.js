#!/usr/bin/env node

const WebSocket = require('ws');
const crypto = require('crypto');
const { v4: uuidv4 } = require('uuid');

// Configuration
const API_KEY = "eea05e0e52e3b07be0647b862ff71680168858d68cbb0c3e83dcb9c77dc87af0";
const HMAC_SECRET = "7fd8a36b1fec73e50ca6be13e47992beb5c48e2a9a0af41328626378b8418244";
const SERVER_URL = 'wss://juno-cloud-backend.fly.dev/ws';

// Get query from command line arguments
const query = process.argv.slice(2).join(' ') || 'Hello, AI agent! Please introduce yourself and tell me what you can do.';

console.log('🤖 Calling Juno AI Agent...');
console.log(`📝 Query: "${query}"`);
console.log('');

// Helper function to generate HMAC signature
function generateHmacSignature(method, path, body, timestamp, hmacSecret) {
    const payload = `${method}:${path}:${body || ''}:${timestamp}`;
    return crypto.createHmac('sha256', hmacSecret).update(payload).digest('hex');
}

// Create WebSocket connection
const ws = new WebSocket(SERVER_URL);

// State tracking
let isAuthenticated = false;
let queryId = null;

ws.on('open', () => {
    console.log('✅ Connected to Juno Cloud Server');

    // Generate authentication
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

    console.log('🔐 Authenticating...');
    ws.send(JSON.stringify(authMessage));
});

ws.on('message', (data) => {
    try {
        const message = JSON.parse(data.toString());

        switch (message.type) {
            case 'status':
                console.log(`📡 Server: ${message.data.message}`);
                break;

            case 'auth':
                if (message.data.success && !isAuthenticated) {
                    console.log('🎉 Authentication successful!');
                    console.log(`🎫 Session token: ${message.data.token?.substring(0, 20)}...`);
                    isAuthenticated = true;

                    // Send the AI agent query
                    setTimeout(() => {
                        queryId = uuidv4();
                        const agentQuery = {
                            type: 'command',
                            data: {
                                id: queryId,
                                command_type: 'text_query',
                                payload: {
                                    query: query
                                },
                                timestamp: Math.floor(Date.now() / 1000)
                            }
                        };

                        console.log('🚀 Sending query to AI agent...');
                        ws.send(JSON.stringify(agentQuery));
                    }, 500);
                } else if (!message.data.success) {
                    console.error('❌ Authentication failed:', message.data.error);
                    process.exit(1);
                }
                break;

            case 'response':
                const response = message.data;

                if (response.command_id === queryId) {
                    if (response.status === 'in_progress') {
                        console.log('⏳ AI agent is processing your query...');
                    } else if (response.status === 'success') {
                        console.log('\n🤖 AI Agent Response:');
                        console.log('─'.repeat(50));
                        console.log(response.data.text);
                        console.log('─'.repeat(50));

                        if (response.data.agent_state) {
                            console.log(`\n📊 Agent Status: ${response.data.agent_state.status}`);
                            console.log(`⏱️  Processing Time: ${Date.now() - (response.data.agent_state.processing_time || Date.now())}ms`);
                        }

                        console.log('\n✨ Query completed successfully!');
                        setTimeout(() => ws.close(), 1000);
                    } else if (response.status === 'error') {
                        console.error('\n❌ AI Agent Error:');
                        console.error(response.error || 'Unknown error occurred');
                        setTimeout(() => ws.close(), 1000);
                    }
                }
                break;

            case 'error':
                console.error('❌ Server Error:', message.data.error_message);
                setTimeout(() => ws.close(), 1000);
                break;

            case 'heartbeat':
                // Silently handle heartbeats
                break;

            default:
                console.log(`📨 Received ${message.type}:`, message.data);
        }

    } catch (error) {
        console.error('❌ Failed to parse server message:', error.message);
    }
});

ws.on('close', (code, reason) => {
    console.log(`\n👋 Disconnected from server (${code})`);
    process.exit(0);
});

ws.on('error', (error) => {
    console.error('❌ Connection error:', error.message);
    process.exit(1);
});

// Timeout after 30 seconds
setTimeout(() => {
    console.log('\n⏰ Query timeout - closing connection');
    ws.close();
    process.exit(1);
}, 30000);

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
    console.log('\n🛑 Interrupted by user');
    ws.close();
    process.exit(0);
});

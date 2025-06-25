const https = require('https');

console.log('🔐 Registering device with Juno Cloud Backend...');

const registrationData = {
    device_name: 'Test Device',
    device_type: 'desktop',
    user_email: 'test@example.com',
    user_name: 'Test User'
};

const postData = JSON.stringify(registrationData);

const options = {
    hostname: 'juno-cloud-backend.fly.dev',
    port: 443,
    path: '/api/register',
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(postData)
    },
    // For testing purposes - in production, use proper SSL verification
    rejectUnauthorized: false
};

const req = https.request(options, (res) => {
    console.log(`📡 Status: ${res.statusCode}`);
    console.log(`📋 Headers:`, res.headers);

    let data = '';
    res.on('data', (chunk) => {
        data += chunk;
    });

    res.on('end', () => {
        try {
            const response = JSON.parse(data);
            console.log('\n✨ Registration Response:');
            console.log(JSON.stringify(response, null, 2));

            if (response.success) {
                console.log('\n🎉 Device registered successfully!');
                console.log(`🔑 API Key: ${response.api_key}`);
                console.log(`🔐 HMAC Secret: ${response.hmac_secret}`);
                console.log(`📱 Device ID: ${response.device_id}`);

                console.log('\n💡 Save these credentials to use in WebSocket authentication:');
                console.log(`API_KEY="${response.api_key}"`);
                console.log(`HMAC_SECRET="${response.hmac_secret}"`);
            } else {
                console.log('❌ Registration failed:', response.error || 'Unknown error');
            }
        } catch (error) {
            console.error('❌ Failed to parse response:', error.message);
            console.log('Raw response:', data);
        }
    });
});

req.on('error', (error) => {
    console.error('❌ Registration request failed:', error.message);
});

req.write(postData);
req.end();

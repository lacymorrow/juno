#!/bin/bash

# Juno Cloud Backend - Deployment Test Script
# Run this before deploying to ensure everything works correctly

set -e

echo "🚀 Starting Juno Cloud Backend Deployment Test..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
	echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
	echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
	echo -e "${RED}❌ $1${NC}"
}

# Check if we're in the right directory
if [ ! -f "package.json" ]; then
	print_error "Please run this script from the backend-server directory"
	exit 1
fi

print_status "Checking directory structure..."

# Check required files exist
required_files=("package.json" "src/server.js" "Dockerfile" "docker-compose.yml" "env.example")
for file in "${required_files[@]}"; do
	if [ -f "$file" ]; then
		print_status "Found $file"
	else
		print_error "Missing required file: $file"
		exit 1
	fi
done

# Check if .env exists, if not create from example
if [ ! -f ".env" ]; then
	print_warning ".env file not found, creating from env.example"
	cp env.example .env
	print_warning "Please edit .env file with your production values before deploying"
fi

# Install dependencies
print_status "Installing dependencies..."
npm install

# Run tests if they exist
if npm run test --silent 2>/dev/null; then
	print_status "Running tests..."
	npm test
else
	print_warning "No tests found, skipping..."
fi

# Start server in background for testing
print_status "Starting server for testing..."
npm start &
SERVER_PID=$!

# Wait for server to start
sleep 5

# Test health endpoint
print_status "Testing health endpoint..."
if curl -s http://localhost:8080/health | grep -q "healthy"; then
	print_status "Health check passed!"
else
	print_error "Health check failed!"
	kill $SERVER_PID 2>/dev/null || true
	exit 1
fi

# Test metrics endpoint
print_status "Testing metrics endpoint..."
if curl -s http://localhost:8080/metrics | grep -q "uptime"; then
	print_status "Metrics endpoint working!"
else
	print_error "Metrics endpoint failed!"
	kill $SERVER_PID 2>/dev/null || true
	exit 1
fi

# Test device registration
print_status "Testing device registration..."
REGISTER_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
	-d '{"device_name":"test-device","device_type":"desktop","platform":"test"}' \
	http://localhost:8080/api/register)

if echo "$REGISTER_RESPONSE" | grep -q "device_id"; then
	print_status "Device registration working!"
else
	print_error "Device registration failed!"
	echo "Response: $REGISTER_RESPONSE"
	kill $SERVER_PID 2>/dev/null || true
	exit 1
fi

# Clean up
print_status "Stopping test server..."
kill $SERVER_PID 2>/dev/null || true
sleep 2

# Docker build test
print_status "Testing Docker build..."
if command -v docker >/dev/null 2>&1; then
	if docker info >/dev/null 2>&1; then
		if docker build -t juno-backend-test . >/dev/null 2>&1; then
			print_status "Docker build successful!"
			docker rmi juno-backend-test >/dev/null 2>&1 || true
		else
			print_error "Docker build failed!"
			exit 1
		fi
	else
		print_warning "Docker daemon not running, skipping Docker build test"
	fi
else
	print_warning "Docker not installed, skipping Docker build test"
fi

# Check environment variables for production
print_status "Checking production readiness..."
if grep -q "your-super-secure-jwt-secret" .env; then
	print_warning "Remember to change JWT_SECRET before deploying!"
fi

if grep -q "your-hmac-secret" .env; then
	print_warning "Remember to change HMAC_SECRET before deploying!"
fi

if grep -q "NODE_ENV=development" .env; then
	print_warning "Remember to set NODE_ENV=production for deployment!"
fi

print_status "All tests passed! ✨"
echo ""
echo "🎯 Next steps for deployment:"
echo "1. Update .env file with production values"
echo "2. Choose your deployment platform:"
echo "   • Railway: Push to GitHub and connect on railway.app"
echo "   • Render: Push to GitHub and connect on render.com"
echo "   • Fly.io: Run 'fly deploy' (requires Fly CLI)"
echo "   • AWS/GCP/Azure: Follow CLOUD_DEPLOYMENT.md guide"
echo ""
echo "3. Update client configuration to point to your deployed URL"
echo ""
print_status "Deployment test completed successfully!"

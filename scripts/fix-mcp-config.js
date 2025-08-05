#!/usr/bin/env node

/**
 * Fix MCP Configuration Script
 * 
 * This script helps fix the MCP configuration issue in Juno by
 * creating a proper test configuration that can be added via the UI.
 */

console.log('🔧 MCP Configuration Fix for Juno\n');

console.log('📋 Step 1: Open Juno and go to Settings → Network\n');

console.log('📝 Step 2: Copy and paste this configuration into the MCP Server JSON field:\n');

const testConfig = {
  "everything-test": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-everything"],
    "description": "Test MCP server with everything capabilities"
  }
};

console.log(JSON.stringify(testConfig, null, 2));

console.log('\n📝 Alternative configuration with file system access:\n');

const fileSystemConfig = {
  "filesystem": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-filesystem", process.env.HOME || "/Users"],
    "description": "MCP server for file system operations"
  }
};

console.log(JSON.stringify(fileSystemConfig, null, 2));

console.log('\n📝 Complete configuration with multiple servers:\n');

const multiServerConfig = {
  "everything": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-everything"],
    "description": "General purpose MCP server"
  },
  "filesystem": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-filesystem", process.env.HOME || "/Users"],
    "description": "File system operations"
  },
  "memory": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-memory"],
    "description": "Knowledge graph memory"
  },
  "sequential-thinking": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-sequential-thinking"],
    "description": "Problem solving and planning"
  }
};

console.log(JSON.stringify(multiServerConfig, null, 2));

console.log('\n✅ Step 3: Click "Add Server"\n');
console.log('✅ Step 4: Make sure the server toggle is enabled\n');
console.log('✅ Step 5: You should see the server status change to "Connected"\n');

console.log('\n🚀 If the server doesn\'t connect:\n');
console.log('   1. Check the Juno logs for errors');
console.log('   2. Make sure Node.js and npm are installed');
console.log('   3. Try restarting Juno');
console.log('   4. Run "npx @modelcontextprotocol/server-everything --help" to test the package\n');

console.log('💡 Note: The first time you use an MCP server, npm will download it automatically.\n');
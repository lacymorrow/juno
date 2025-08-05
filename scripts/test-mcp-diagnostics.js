#!/usr/bin/env node

/**
 * MCP Diagnostics Test Script
 * 
 * This script helps diagnose MCP (Model Context Protocol) issues in Juno
 * by testing the MCP server configuration and connectivity.
 */

import { spawn, execSync } from 'child_process';
import path from 'path';
import fs from 'fs';
import os from 'os';

console.log('🔍 MCP Diagnostics Test for Juno\n');

// Check Node.js environment
console.log('1️⃣ Checking Node.js environment...');
console.log(`   Node version: ${process.version}`);
console.log(`   NPM version: ${getNpmVersion()}`);
console.log(`   Platform: ${os.platform()}`);
console.log(`   Architecture: ${os.arch()}\n`);

// Check if required MCP packages are available
console.log('2️⃣ Checking MCP server packages...');
const mcpPackages = [
  '@modelcontextprotocol/server-everything',
  '@modelcontextprotocol/server-filesystem',
  '@modelcontextprotocol/server-memory',
  '@modelcontextprotocol/server-sequential-thinking'
];

async function checkPackages() {
  for (const pkg of mcpPackages) {
    await checkPackageAvailability(pkg);
  }
}

function getNpmVersion() {
  try {
    return execSync('npm --version').toString().trim();
  } catch (e) {
    return 'Not found';
  }
}

async function checkPackageAvailability(packageName) {
  return new Promise((resolve) => {
    const npmView = spawn('npm', ['view', packageName, 'version'], {
      stdio: ['ignore', 'pipe', 'pipe']
    });

    let output = '';
    let error = '';

    npmView.stdout.on('data', (data) => {
      output += data.toString();
    });

    npmView.stderr.on('data', (data) => {
      error += data.toString();
    });

    npmView.on('close', (code) => {
      if (code === 0 && output.trim()) {
        console.log(`   ✅ ${packageName} - v${output.trim()}`);
      } else {
        console.log(`   ❌ ${packageName} - Not found on npm`);
        if (error) {
          console.log(`      Error: ${error.trim()}`);
        }
      }
      resolve();
    });
  });
}

// Test MCP server startup
console.log('\n3️⃣ Testing MCP server startup...');
async function testMcpServer() {
  const testServer = '@modelcontextprotocol/server-everything';
  console.log(`   Testing ${testServer}...`);

  return new Promise((resolve) => {
    const server = spawn('npx', [testServer, '--help'], {
      stdio: ['ignore', 'pipe', 'pipe']
    });

    let output = '';
    let error = '';

    server.stdout.on('data', (data) => {
      output += data.toString();
    });

    server.stderr.on('data', (data) => {
      error += data.toString();
    });

    server.on('close', (code) => {
      if (code === 0 || output.includes('usage') || output.includes('help')) {
        console.log('   ✅ MCP server can be started successfully');
      } else {
        console.log('   ❌ Failed to start MCP server');
        if (error) {
          console.log(`      Error: ${error.trim()}`);
        }
      }
      resolve();
    });

    // Kill after 5 seconds if still running
    setTimeout(() => {
      server.kill();
    }, 5000);
  });
}

// Check Juno configuration
console.log('\n4️⃣ Checking Juno MCP configuration...');
function checkJunoConfig() {
  const configPaths = [
    path.join(os.homedir(), 'Library', 'Application Support', 'dev.juno.ai', 'settings.json'),
    path.join(os.homedir(), '.juno', 'settings.json'),
    './settings.json'
  ];

  let configFound = false;
  for (const configPath of configPaths) {
    if (fs.existsSync(configPath)) {
      console.log(`   ✅ Found configuration at: ${configPath}`);
      configFound = true;
      
      try {
        const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
        if (config.tools && config.tools.mcp_servers) {
          console.log(`   📋 MCP servers configured: ${config.tools.mcp_servers.length}`);
          config.tools.mcp_servers.forEach(server => {
            console.log(`      - ${server.name} (${server.enabled ? 'enabled' : 'disabled'})`);
          });
        } else {
          console.log('   ⚠️  No MCP servers configured in settings');
        }
      } catch (e) {
        console.log(`   ❌ Error reading configuration: ${e.message}`);
      }
      break;
    }
  }

  if (!configFound) {
    console.log('   ⚠️  No Juno configuration file found');
  }
}

// Provide recommendations
console.log('\n5️⃣ Recommendations:');
async function provideRecommendations() {
  console.log('\n   To fix MCP in Juno:');
  console.log('   1. Open Juno Settings → Network');
  console.log('   2. Add this configuration in the MCP Server JSON field:\n');
  
  const exampleConfig = {
    "everything": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-everything"]
    }
  };
  
  console.log('   ' + JSON.stringify(exampleConfig, null, 2).split('\n').join('\n   '));
  console.log('\n   3. Click "Add Server"');
  console.log('   4. Enable the server using the toggle');
  console.log('   5. Restart Juno if needed\n');
  
  console.log('   💡 Alternative: Use the File System server for file operations:');
  const fsConfig = {
    "filesystem": {
      "command": "npx",
      "args": ["@modelcontextprotocol/server-filesystem", process.env.HOME || "/Users"]
    }
  };
  console.log('   ' + JSON.stringify(fsConfig, null, 2).split('\n').join('\n   '));
}

// Run all tests
async function runDiagnostics() {
  await checkPackages();
  await testMcpServer();
  checkJunoConfig();
  await provideRecommendations();
  
  console.log('\n✅ Diagnostics complete!\n');
}

runDiagnostics().catch(console.error);
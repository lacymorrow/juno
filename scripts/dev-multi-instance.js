#!/usr/bin/env node

import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// Get current directory in ES modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Parse command line arguments
const args = process.argv.slice(2);
const portArg = args.find(arg => arg.startsWith('--port='));
const port = portArg ? parseInt(portArg.split('=')[1]) : 1422; // Default to 1422 for second instance
const hmrPort = port + 1;

console.log(`🚀 Starting Tauri dev instance on port ${port} (HMR: ${hmrPort})`);

// Create a temporary tauri config for this instance
const originalConfigPath = path.join(__dirname, '../src-tauri/tauri.conf.json');
const tempConfigPath = path.join(__dirname, `../src-tauri/tauri.conf.${port}.json`);

// Read the original config
const originalConfig = JSON.parse(fs.readFileSync(originalConfigPath, 'utf8'));

// Create a modified config with the new port
const modifiedConfig = {
  ...originalConfig,
  build: {
    ...originalConfig.build,
    devUrl: `http://localhost:${port}`,
    beforeDevCommand: `bun run vite --port ${port}`
  }
};

// Write the temporary config
fs.writeFileSync(tempConfigPath, JSON.stringify(modifiedConfig, null, 2));

console.log(`📝 Created temporary config: ${tempConfigPath}`);
console.log(`🌐 Vite will run on: http://localhost:${port}`);

// Start Tauri with the custom config (which will start Vite automatically)
const tauriProcess = spawn('bunx', ['tauri', 'dev', '--config', tempConfigPath], {
  stdio: 'inherit'
});

// Handle process cleanup
const cleanup = () => {
  console.log('\n🧹 Cleaning up...');
  tauriProcess.kill();

  // Remove temporary config file
  try {
    fs.unlinkSync(tempConfigPath);
    console.log('✅ Removed temporary config');
  } catch (err) {
    console.log('⚠️  Could not remove temporary config (might not exist)');
  }

  process.exit(0);
};

process.on('SIGINT', cleanup);
process.on('SIGTERM', cleanup);

tauriProcess.on('exit', (code) => {
  console.log(`Tauri process exited with code ${code}`);
  cleanup();
});

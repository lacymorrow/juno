// mcp-bridge.ts
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { spawn } from "child_process";
import { z } from "zod";

// Path to your rust binary
const RUST_BINARY = "/Users/matthewdi/Desktop/screenpipe/computer-use-ai-sdk/mcp-server-os-level/target/debug/server";

// Create server
const server = new Server(
  {
    name: "ui-automation-bridge",
    version: "1.0.0",
  },
  {
    capabilities: {
      tools: {
        // Define the same tools as your Rust server
      },
    },
  }
);

// Start your Rust server in HTTP mode (not STDIO)
const rustProcess = spawn(RUST_BINARY, [], {
  stdio: 'ignore' // Run in background
});

// Set up clean exit
process.on('exit', () => {
  rustProcess.kill();
});

// Define handlers that forward requests to your Rust HTTP endpoint
server.setRequestHandler(/* ... */, async (request) => {
  // Forward the request to your Rust server running on HTTP
  const response = await fetch("http://127.0.0.1:8080/api/click-by-index", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request.params),
  });
  
  const data = await response.json();
  return data;
});

// Start bridge server
async function runServer() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("UI Automation Bridge running on stdio");
}

runServer().catch((error) => {
  console.error("Fatal error running server:", error);
  rustProcess.kill();
  process.exit(1);
});
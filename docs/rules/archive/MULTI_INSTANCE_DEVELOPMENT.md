# Multi-Instance Development Setup

This setup allows you to run multiple Tauri development instances simultaneously with different ports, enabling testing of multiple app instances or different configurations.

## Quick Start

### Run the primary instance (default port 1420):
```bash
bun run tauri:dev
```

### Run a secondary instance (port 1422):
```bash
bun run tauri:dev:instance2
```

### Run a third instance (port 1424):
```bash
bun run tauri:dev:instance3
```

### Run a custom instance on any port:
```bash
bun run tauri:dev:multi --port=1430
```

## How It Works

The multi-instance script (`scripts/dev-multi-instance.js`) does the following:

1. **Port Configuration**: Takes a `--port` parameter to specify the Vite dev server port
2. **Temporary Config**: Creates a temporary Tauri configuration file with the custom port
3. **Automatic Cleanup**: Removes temporary files when the process exits
4. **Process Management**: Handles both Vite and Tauri processes together

## Port Mapping

- **Instance 1** (default): Vite on `1420`, HMR on `1421`
- **Instance 2**: Vite on `1422`, HMR on `1423`  
- **Instance 3**: Vite on `1424`, HMR on `1425`
- **Custom**: Vite on `<port>`, HMR on `<port+1>`

## Available Scripts

| Script | Description | Port |
|--------|-------------|------|
| `bun run tauri:dev` | Primary development instance | 1420 |
| `bun run tauri:dev:instance2` | Second instance | 1422 |
| `bun run tauri:dev:instance3` | Third instance | 1424 |
| `bun run tauri:dev:multi --port=XXXX` | Custom port instance | XXXX |

## Development Workflow

1. **Primary Development**: Use `bun run tauri:dev` for main development work
2. **Testing Multiple Instances**: Start additional instances with different ports to test:
   - Multi-user scenarios
   - Different configurations
   - Cross-instance communication
   - Performance under multiple instances

## Troubleshooting

### Port Already in Use
If you get a port conflict, either:
- Stop the conflicting process
- Use a different port: `bun run tauri:dev:multi --port=1426`

### Cleanup Issues
If temporary config files aren't cleaned up automatically:
```bash
rm src-tauri/tauri.conf.*.json
```

### Process Not Stopping
Use `Ctrl+C` to gracefully stop instances. If processes hang:
```bash
pkill -f "tauri dev"
pkill -f "vite.*--port"
```

## Notes

- Each instance creates a separate Tauri application window
- Database and file storage should be configured to avoid conflicts between instances
- Memory usage scales with the number of instances
- Each instance will have its own dev server with hot reloading
# Configuration

## Environment Variables

### Required API Keys

#### Primary AI Providers
```env
# Anthropic (Primary)
ANTHROPIC_API_KEY=sk-ant-api03-...
# Format: sk-ant-api03-[key]
# Purpose: Main AI agent provider (Claude models)

# OpenAI (Alternative)  
OPENAI_API_KEY=sk-...
# Format: sk-[key]
# Purpose: Alternative AI provider (GPT models)
```

#### Optional AI Providers
```env
# Google Gemini
GOOGLE_GEMINI_API_KEY=AI...
# Purpose: Multimodal AI capabilities

# Perplexity
PERPLEXITY_API_KEY=pplx-...
# Purpose: Research and web-enhanced responses

# HuggingFace
HUGGINGFACE_API_KEY=hf_...
# Purpose: Open source model access

# Replicate
REPLICATE_API_TOKEN=r8_...
# Purpose: Cloud model inference

# FAL.ai
FAL_KEY=...
# Purpose: Fast AI model inference
```

#### Voice & Audio
```env
# ElevenLabs (TTS)
ELEVENLABS_API_KEY=...
# Purpose: High-quality text-to-speech
```

### Development Settings
```env
# Logging Level
RUST_LOG=info
# Options: trace, debug, info, warn, error

# Development Mode
TAURI_DEV=true
# Purpose: Enable development features

# Debug Mode
DEBUG=true
# Purpose: Additional debugging output
```

## Provider Configuration

### AI Provider Priority
1. **Anthropic Claude** - Primary agent provider
2. **OpenAI GPT** - Fallback and alternative tasks  
3. **Google Gemini** - Multimodal and vision tasks
4. **Local/Ollama** - Offline operation

### Provider Settings
```rust
// Runtime provider switching
set_active_provider("anthropic") // Switch active provider
get_active_provider()            // Get current provider
get_providers()                  // List available providers

// Provider-specific settings
update_provider_model("anthropic", "claude-3-5-sonnet-20241022")
update_provider_max_tokens("openai", 4096)
update_provider_temperature("anthropic", 0.7)
```

### Supported Models

#### Anthropic
- `claude-3-5-sonnet-20241022` (Default)
- `claude-3-opus-20240229`
- `claude-3-haiku-20240307`

#### OpenAI
- `gpt-4o` (Default)
- `gpt-4o-mini`
- `gpt-4-turbo`
- `gpt-3.5-turbo`

#### Google
- `gemini-1.5-pro`
- `gemini-1.5-flash`

## Application Settings

### Global Shortcuts
```rust
// Default shortcuts (configurable)
"Escape"    // Cancel agent execution (dynamic)
"Alt+D"     // Toggle dictation (macOS: Option+D)
```

### Agent Configuration
```rust
// Agent Architecture Mode
AGENT_MODE: Multi         // Single or Multi-agent mode
// Single: Direct execution with all tools (faster)
// Multi: Orchestrated delegation (robust)

// Execution limits
MAX_ITERATIONS: 15        // Maximum agent steps
TOOL_TIMEOUT: 30_000     // Tool timeout (ms)
MEMORY_LIMIT: 100        // Conversation history limit

// Cancellation
ESCAPE_KEY_DYNAMIC: true  // Only register during execution

// Mode switching commands
get_agent_mode()          // Get current mode
set_agent_mode("single")  // Switch to single agent
set_agent_mode("multi")   // Switch to multi-agent
```

### UI Configuration
```rust
// Floating bar behavior
FLOATING_BAR_LEVEL: 5     // Window level (macOS)
ALWAYS_ON_TOP: true       // Keep above other windows
TRANSPARENCY: true        // Allow transparent areas
```

## File Locations

### Configuration Files
```
├── .env                           # Environment variables
├── .env.example                   # Template file
├── src-tauri/tauri.conf.json     # Tauri configuration
├── src-tauri/Cargo.toml          # Rust dependencies
├── package.json                   # Node.js dependencies
└── docs/                         # Documentation
```

### Runtime Configuration
```
├── ~/.config/juno/               # User configuration (macOS)
│   └── ai_providers.json         # AI provider and agent mode settings
├── ~/Library/Logs/juno/          # Application logs
└── ~/Library/Caches/juno/        # Cache files
```

### Agent Mode Configuration File
```json
// ~/.config/juno/ai_providers.json
{
  "active_provider": "anthropic",
  "agent_mode": "Multi",           // "Single" or "Multi"
  "providers": [
    {
      "id": "anthropic",
      "api_key": "sk-ant-api03-...",
      "model": "claude-3-5-sonnet-20241022",
      "max_tokens": 4096,
      "temperature": 0.7,
      "system_prompt": null
    }
  ]
}
```

## Security Settings

### API Key Protection
- **Never commit** `.env` files
- **Use .env.example** for templates
- **Rotate keys** regularly
- **Limit permissions** when possible

### Tauri Capabilities
```json
// src-tauri/capabilities/default.json
{
  "permissions": [
    "core:default",
    "shell:allow-open",
    "global-shortcut:allow-register",
    "notification:default"
  ]
}
```

### macOS Permissions
Required system permissions:
- **Accessibility** - Screen reading and automation
- **Screen Recording** - Screenshot capture
- **Input Monitoring** - Keyboard and mouse control

## Performance Settings

### Memory Management
```rust
// Context window limits
CONTEXT_WINDOW_ANTHROPIC: 200_000    // Claude context tokens
CONTEXT_WINDOW_OPENAI: 128_000       // GPT context tokens
MEMORY_CLEANUP_THRESHOLD: 150        // Messages before cleanup
```

### Resource Limits
```rust
// Browser settings
BROWSER_TIMEOUT: 30_000              // Page load timeout
BROWSER_MEMORY_LIMIT: 512_MB         // Browser memory limit
SCREENSHOT_QUALITY: 80               // JPEG quality (1-100)

// Tool execution
CONCURRENT_TOOLS: 3                  // Max parallel tools
TOOL_RETRY_COUNT: 2                  // Failed tool retries
```

## Logging Configuration

### Log Levels
```env
# Development
RUST_LOG=debug

# Production  
RUST_LOG=info

# Specific modules
RUST_LOG=juno::agent=debug,juno::tools=info
```

### Log Outputs
- **Console** - Development output
- **File** - `~/Library/Logs/juno/app.log`
- **Tauri DevTools** - Browser console integration

## Testing Configuration

### Test Environment
```env
# Test mode settings
TEST_MODE=true
MOCK_API_CALLS=true
DISABLE_SCREENSHOTS=true
FAST_MODE=true
```

### QA Test Settings
```rust
// Test execution
QA_TEST_DELAY: 100           // ms between actions
QA_TEST_TIMEOUT: 5000        // Test timeout
QA_SCREENSHOT_ON_FAIL: true  // Capture on failure
```

## Platform-Specific Settings

### macOS Configuration
```rust
// Window behavior
NS_WINDOW_LEVEL: 5                   // Floating window level
COLLECTION_BEHAVIOR: [
    "CanJoinAllSpaces",              // Visible on all spaces
    "Stationary",                    // Stay during space switch
    "IgnoresCycle"                   // Exclude from Cmd+` cycle
]

// Accessibility
ACCESSIBILITY_REQUIRED: true         // Require accessibility access
SCREEN_RECORDING_REQUIRED: true      // Require screen recording
```

### Cross-Platform Considerations
```rust
// Feature availability
DESKTOP_AUTOMATION: macOS            // Primary platform
BROWSER_CONTROL: All                 // Cross-platform
VOICE_TRANSCRIPTION: macOS           // Platform-specific
GLOBAL_SHORTCUTS: All                // Cross-platform
```

## Troubleshooting Configuration

### Common Issues
1. **Missing API Keys** - Check `.env` file exists and is populated
2. **Permission Denied** - Verify macOS accessibility permissions
3. **Build Failures** - Ensure all required tools are installed
4. **Runtime Errors** - Check log files for detailed errors

### Configuration Validation
```bash
# Check environment
echo $ANTHROPIC_API_KEY | cut -c1-10  # Should show sk-ant-api

# Validate setup
bun run tauri dev --verbose          # Detailed startup logs

# Test configuration
./test-rust-units.sh                 # Run configuration tests
``` 

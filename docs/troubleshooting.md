# Troubleshooting

## Quick Diagnostics

### Check System Status
```bash
# Verify installation
which bun && which cargo && which node
bun --version && cargo --version && node --version

# Check API keys
echo $ANTHROPIC_API_KEY | cut -c1-15  # Should show sk-ant-api03-

# Test build
cargo check --manifest-path src-tauri/Cargo.toml
```

## Common Issues

### 1. Build & Installation Errors

#### "Command not found: bun"
```bash
# Install Bun
curl -fsSL https://bun.sh/install | bash
source ~/.bashrc  # or ~/.zshrc
```

#### "Failed to resolve dependencies"
```bash
# Clear and reinstall
rm -rf node_modules bun.lockb
bun install

# Clear Rust cache
cargo clean --manifest-path src-tauri/Cargo.toml
```

#### "Tauri CLI not found"
```bash
# Install Tauri CLI v2
cargo install tauri-cli --version "^2.0"
# or
bun add -g @tauri-apps/cli@next
```

#### "Rust compiler errors"
```bash
# Update Rust toolchain
rustup update stable
rustup default stable

# Check minimum version (1.70+)
rustc --version
```

### 2. Runtime Errors

#### "Failed to initialize Desktop Automation Engine"
**Cause**: Missing macOS accessibility permissions  
**Solution**:
1. Open System Preferences → Security & Privacy → Privacy
2. Select "Accessibility" 
3. Add Juno app or Terminal (for development)
4. Restart application

#### "API key not found" or "Authentication failed"
```bash
# Check environment file exists
ls -la .env

# Verify API key format
echo $ANTHROPIC_API_KEY | grep -E "^sk-ant-api03-"
echo $OPENAI_API_KEY | grep -E "^sk-"

# Copy from example if missing
cp .env.example .env
# Edit .env with your actual keys
```

#### "Main window not found"
**Cause**: Frontend build issues or Tauri configuration  
**Solution**:
```bash
# Rebuild frontend
bun run build

# Check Tauri config
cat src-tauri/tauri.conf.json | grep -A5 "windows"

# Clear cache and restart
rm -rf dist/ .vite/
bun run tauri dev
```

### 3. Agent Execution Issues

#### "Agent execution cancelled immediately"
**Cause**: Escape key capture or signal issues  
**Solution**:
```bash
# Check for running instances
ps aux | grep juno
killall juno  # Kill existing instances

# Restart with debug logging
RUST_LOG=debug bun run tauri dev
```

#### "Tool execution failed"
**Check**:
1. macOS permissions (Accessibility, Screen Recording)
2. Tool parameter validation
3. System resource availability

#### "Browser tools not working"
```bash
# Check browser controller initialization
# Look for browser-related errors in logs
tail -f ~/Library/Logs/juno/app.log | grep -i browser

# Clear browser cache
rm -rf ~/Library/Caches/juno/browser/
```

### 4. Performance Issues

#### "High memory usage"
**Causes & Solutions**:
- **Large screenshots**: Reduce screenshot quality in config
- **Memory leaks**: Restart application periodically  
- **Too many tools**: Limit concurrent tool execution

#### "Slow response times"
**Optimization**:
```bash
# Check API response times
curl -w "%{time_total}" -s https://api.anthropic.com/v1/health

# Monitor resource usage
top -pid $(pgrep juno)

# Reduce context window if needed
# Edit provider settings to lower max_tokens
```

#### "Application freezing"
**Debug steps**:
1. Check Console.app for crash logs
2. Look for deadlocks in Rust code
3. Verify async operations are properly handled

### 5. Voice & Audio Issues

#### "Dictation not starting"
**Check**:
1. Microphone permissions in System Preferences
2. Voice transcription plugin loaded correctly
3. Audio input device availability

#### "TTS not working"
```bash
# Verify ElevenLabs API key
echo $ELEVENLABS_API_KEY

# Check TTS provider status
# Use app command: get_tts_provider_command()

# Test audio output
say "test"  # macOS built-in TTS
```

### 6. UI Issues

#### "Floating bar not appearing"
**Check**:
```bash
# Verify window creation
# Look for window-related errors in logs
grep -i "floating" ~/Library/Logs/juno/app.log

# Reset window state
rm -rf ~/Library/Preferences/com.juno.*
```

#### "Mouse/keyboard events not working"
**Solutions**:
1. Grant Input Monitoring permissions
2. Restart application after permission changes
3. Check for conflicting applications

#### "Global shortcuts not responding"
**Debug**:
```bash
# Check shortcut registration
grep -i "shortcut" ~/Library/Logs/juno/app.log

# Verify no conflicts with other apps
# Try different key combinations
```

### 7. Development Issues

#### "Hot reload not working"
```bash
# Restart development server
bun run tauri dev

# Check file watchers
ls -la src/ | wc -l  # Count files
ulimit -n  # Check file descriptor limit

# Increase if needed
ulimit -n 4096
```

#### "Tests failing"
```bash
# Run specific test suites
bun run test -- --reporter=verbose
./test-rust-units.sh --nocapture
./test-qa.sh --debug

# Check test environment
echo $TEST_MODE
```

## Error Code Reference

### Agent Errors
- **E001**: `Terminated` - User cancelled execution
- **E002**: `MaxStepsReached` - Hit iteration limit (15)
- **E003**: `ToolNotFound` - Invalid tool requested
- **E004**: `ProviderError` - AI API failure
- **E005**: `ToolExecutionError` - Tool failed to execute

### System Errors
- **S001**: Desktop automation initialization failed
- **S002**: Browser controller initialization failed  
- **S003**: Audio system initialization failed
- **S004**: Permission denied (accessibility/recording)
- **S005**: Resource exhaustion (memory/CPU)

### API Errors
- **A001**: Authentication failed (invalid API key)
- **A002**: Rate limit exceeded
- **A003**: Network timeout
- **A004**: API service unavailable
- **A005**: Invalid request format

## Debugging Tools

### Logging Commands
```bash
# Enable detailed logging
export RUST_LOG=trace
bun run tauri dev

# Monitor specific modules
export RUST_LOG=juno::agent=debug,juno::tools=trace

# Real-time log monitoring
tail -f ~/Library/Logs/juno/app.log
```

### System Monitoring
```bash
# Monitor application resources
top -pid $(pgrep juno)

# Check file descriptors
lsof -p $(pgrep juno) | wc -l

# Network connections
netstat -an | grep $(pgrep juno)
```

### Test Commands
```bash
# Comprehensive system test
./run-all-tests.sh

# Quick smoke test
bun run test -- tests/smoke.test.ts

# Agent functionality test
cargo test --manifest-path src-tauri/Cargo.toml agent_tests
```

## Getting Help

### Log Collection
Before reporting issues, collect:
```bash
# System info
system_profiler SPSoftwareDataType
uname -a

# Application logs
cp ~/Library/Logs/juno/app.log ./juno-debug.log

# Environment (sanitized)
env | grep -E "(RUST_LOG|TAURI|DEBUG)" > env-vars.txt

# Build info
bun --version > build-info.txt
cargo --version >> build-info.txt
rustc --version >> build-info.txt
```

### Support Channels
1. **Documentation**: Check docs/ directory first
2. **GitHub Issues**: Report bugs with logs attached
3. **Discussions**: Ask questions in project discussions
4. **Debug Mode**: Run with `RUST_LOG=debug` for detailed output

### Emergency Recovery
```bash
# Complete reset (WARNING: loses all data)
rm -rf ~/.config/juno/
rm -rf ~/Library/Caches/juno/
rm -rf ~/Library/Logs/juno/
rm -rf node_modules/
rm -rf src-tauri/target/

# Reinstall
bun install
bun run tauri dev
``` 

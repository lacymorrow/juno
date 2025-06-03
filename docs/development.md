# Development Guide

## Quick Start

### Prerequisites
```bash
# Required tools
- Node.js (18+)
- Bun package manager
- Rust (1.70+)
- Cargo
- Tauri CLI v2
```

### Setup
```bash
# 1. Clone and install
git clone <repository>
cd dotdot
bun install

# 2. Environment setup
cp .env.example .env
# Edit .env with your API keys

# 3. Development server
bun run tauri dev

# 4. Production build
bun run tauri build
```

## Project Structure

```
dotdot/
├── src/                    # React frontend
│   ├── components/ui/      # UI components
│   ├── lib/               # Frontend utilities
│   └── styles/            # CSS and styling
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── agent/         # AI agent system
│   │   ├── commands/      # Tauri commands
│   │   ├── tools/         # Desktop automation
│   │   └── lib.rs         # Main application
│   └── Cargo.toml         # Rust dependencies
├── docs/                  # Documentation
├── tasks/                 # Task management
└── tauri-plugin-*         # Custom Tauri plugins
```

## Development Workflow

### Code Changes
1. **Frontend Changes**: React hot reload automatic
2. **Backend Changes**: Restart `bun run tauri dev`
3. **Rust Compilation**: Always run `cargo check` after changes

### Required Check
After every Rust modification:
```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

## Testing

### Comprehensive Test Suite
```bash
# Run all tests
./run-all-tests.sh

# Individual test suites
bun run test              # Frontend tests
./test-rust-units.sh      # Rust unit tests
./test-qa.sh             # QA test suite
```

### Test Structure
- **Unit Tests**: Component and function testing
- **Integration Tests**: Cross-module testing
- **QA Tests**: User interaction testing
- **Rust Tests**: Backend logic testing

### Test Files
- `test-results/` - Test output and reports
- `vitest.config.ts` - Frontend test configuration
- `src-tauri/src/*/tests/` - Rust test modules

## Code Standards

### Rust Guidelines
- **Error Handling**: Use `Result<T, String>` consistently
- **Async**: All tools and commands should be async
- **Documentation**: Document public APIs
- **Safety**: Avoid `unwrap()`, use proper error handling
- **Naming**: Use descriptive variable names

### TypeScript Guidelines
- **Types**: Strict typing enabled
- **Components**: Functional components with hooks
- **Imports**: Absolute imports preferred
- **Error Handling**: Proper try/catch blocks

### File Organization
- **Max Length**: Keep files under 700 lines
- **Modularity**: Separate concerns into modules
- **Exports**: Use clear export patterns
- **Dependencies**: Minimize circular dependencies

## Architecture Guidelines

### Adding New Commands
1. **Define Command**: In appropriate `commands/*.rs` file
2. **Add to Handler**: Register in `lib.rs` invoke_handler
3. **Document**: Add to API reference
4. **Test**: Create unit and integration tests

### Adding New Tools
1. **Tool Definition**: Create `ToolDefinition` struct
2. **Executor**: Implement async executor function
3. **Registration**: Add to tool provider
4. **Documentation**: Update agent system docs

### Adding New Providers
1. **Provider Trait**: Implement `AgentBrain` trait
2. **Factory**: Add to `BrainFactory`
3. **Configuration**: Add to provider settings
4. **Testing**: Comprehensive provider testing

## Debugging

### Logging
```rust
// Rust logging levels
tracing::info!("Information message");
tracing::warn!("Warning message");
tracing::error!("Error message");

// Enable debug logging
RUST_LOG=debug bun run tauri dev
```

### Browser DevTools
- **Frontend**: Standard React DevTools
- **Backend**: Tauri DevTools for IPC debugging
- **Network**: Monitor API calls in Network tab

### Common Issues

#### Compilation Errors
```bash
# Clear cache and rebuild
cargo clean --manifest-path src-tauri/Cargo.toml
bun run tauri dev
```

#### Permission Errors
- macOS: Check Accessibility permissions
- Tauri: Verify capability configurations

#### Missing Dependencies
```bash
# Reinstall dependencies
rm -rf node_modules
bun install
```

## Contributing

### Pull Request Process
1. **Feature Branch**: Create from main
2. **Implement**: Follow coding standards
3. **Test**: Run full test suite
4. **Document**: Update relevant docs
5. **Review**: Submit PR for review

### Commit Guidelines
```bash
# Format: type(scope): description
feat(agent): add new desktop tool
fix(ui): resolve floating bar positioning
docs(api): update command reference
test(qa): add mouse interaction tests
```

### Code Review Checklist
- [ ] All tests pass
- [ ] Documentation updated
- [ ] No security vulnerabilities
- [ ] Performance implications considered
- [ ] Error handling implemented
- [ ] Logging added where appropriate

## Release Process

### Version Management
- **Semantic Versioning**: Major.Minor.Patch
- **Tauri Config**: Update version in `tauri.conf.json`
- **Package.json**: Update frontend version
- **Cargo.toml**: Update Rust crate version

### Build Process
```bash
# Production build
bun run tauri build

# Code signing (macOS)
# Handled automatically in CI/CD

# Distribution
# App bundle in target/release/bundle/
```

## Environment Configuration

### Required Variables
```env
# AI Providers
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GOOGLE_GEMINI_API_KEY=AI...

# Optional Providers
ELEVENLABS_API_KEY=...
PERPLEXITY_API_KEY=...
HUGGINGFACE_API_KEY=...
REPLICATE_API_TOKEN=...
FAL_KEY=...

# Development
RUST_LOG=info
TAURI_DEV=true
```

### Configuration Files
- `.env` - Development environment
- `.env.example` - Template file
- `src-tauri/tauri.conf.json` - Tauri configuration
- `src-tauri/Cargo.toml` - Rust dependencies

## Troubleshooting Development

### Build Issues
- **Rust Compiler**: Update to latest stable
- **Node Dependencies**: Clear cache and reinstall
- **Tauri CLI**: Ensure v2 compatibility

### Runtime Issues
- **API Keys**: Verify all required keys are set
- **Permissions**: Check macOS accessibility settings
- **Resources**: Monitor memory and CPU usage

### Performance Optimization
- **Bundle Size**: Analyze and optimize frontend bundle
- **Memory Usage**: Profile Rust memory allocation
- **Tool Execution**: Optimize heavy automation operations 

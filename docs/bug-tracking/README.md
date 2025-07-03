# Juno Bug Tracking & Regression Prevention

This directory contains comprehensive documentation of bugs, regressions, and their fixes to prevent future issues in the Juno AI Computer Use Agent.

## Directory Structure

```
docs/bug-tracking/
├── README.md                    # This file - overview and guidelines
├── regressions/                 # Documented regressions and fixes
│   ├── 2024-12-streaming-jsx-conflict.md
│   ├── 2024-12-chat-interface-disabled.md
│   └── template.md             # Template for new regression reports
├── known-issues/               # Current known issues
│   └── template.md
├── test-scenarios/             # Critical test scenarios to prevent regressions
│   ├── streaming-responses.md
│   └── chat-interface-state.md
└── prevention-checklist.md     # Pre-release checklist
```

## Bug Classification

### Severity Levels

- **Critical**: System unusable, core functionality broken
- **High**: Major feature broken, significant user impact
- **Medium**: Feature partially broken, workaround available
- **Low**: Minor issues, cosmetic problems

### Categories

- **Regression**: Previously working functionality broken by new changes
- **Integration**: Issues between different system components
- **Performance**: Degraded system performance
- **UI/UX**: User interface and experience issues
- **Security**: Security vulnerabilities or concerns
- **Compatibility**: Platform or dependency compatibility issues

## Regression Prevention Process

### 1. Before Making Changes

- [ ] Review related regression reports
- [ ] Identify potential interaction points
- [ ] Run relevant test scenarios
- [ ] Consider impact on streaming, rendering, and core features

### 2. After Making Changes

- [ ] Run compilation check: `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] Test streaming responses
- [ ] Test JSX/React component rendering
- [ ] Test voice integration
- [ ] Test modal functionality
- [ ] Verify no new console errors

### 3. When Finding Bugs

- [ ] Create detailed regression report using template
- [ ] Document root cause analysis
- [ ] Record fix implementation
- [ ] Add test scenario to prevent recurrence
- [ ] Update prevention checklist if needed

## Quick Reference

### Recent Critical Regressions

1. **Chat Interface Disabled (2024-12)**: Chat interface permanently disabled after agent execution
   - **File**: `docs/bug-tracking/regressions/2024-12-chat-interface-disabled.md`
   - **Fix**: Fixed streaming event names to match frontend listeners (`agent-stream-end` vs `agent-event`)

2. **Streaming/JSX Conflict (2024-12)**: JSX detection interfering with streaming responses
   - **File**: `docs/bug-tracking/regressions/2024-12-streaming-jsx-conflict.md`
   - **Fix**: Added `!msg.isStreaming &&` condition to prevent JSX detection during streaming

### Test Scenarios to Always Check

- Chat interface state management during agent execution (enable/disable cycle)
- Streaming agent responses with partial content
- JSX/React component rendering in completed messages
- Voice transcription and dictation modes
- Modal functionality (help, feedback, import/export)
- Command overlay display
- MCP tool integration
- Memory management operations

### Files Most Prone to Regressions

- `src/App.tsx` (lines 1459-1555: streaming logic, line 2620: message rendering)
- `src-tauri/src/agent/tool_logger.rs` (streaming event emission)
- `src/components/jsx-message-renderer.tsx` (JSX detection)
- `src-tauri/src/anthropic.rs` (main agent orchestration)
- `src-tauri/src/agent/tools/` (tool implementations)

## Contributing to Bug Tracking

1. **Report New Issues**: Use the appropriate template in `regressions/` or `known-issues/`
2. **Update Test Scenarios**: Add critical test cases to `test-scenarios/`
3. **Improve Prevention**: Update `prevention-checklist.md` with new checks
4. **Document Fixes**: Always document both the problem and solution thoroughly

## Integration with Development

This bug tracking system integrates with:

- **Code Reviews**: Reference relevant regression reports
- **Testing**: Use documented test scenarios
- **Release Process**: Follow prevention checklist
- **Issue Tracking**: Link to GitHub issues when appropriate

---

*Last Updated: December 2024*
*Maintainer: Development Team*

# Juno Regression Prevention Checklist

This checklist should be completed before any release or major feature deployment to prevent regressions.

## Pre-Development Checks

### Planning Phase

- [ ] Review related regression reports in `docs/bug-tracking/regressions/`
- [ ] Identify components that might be affected by changes
- [ ] Check interaction points with existing features
- [ ] Review files most prone to regressions (see main README)

### Code Review Preparation

- [ ] Document expected interaction points
- [ ] Prepare test scenarios for new features
- [ ] Identify integration testing requirements

## Development Phase Checks

### Code Quality

- [ ] **Mandatory**: Run `cargo check --manifest-path src-tauri/Cargo.toml` (must exit with code 0)
- [ ] No TypeScript compilation errors
- [ ] No linter warnings in critical files
- [ ] Proper error handling implemented

### Core Functionality Testing

- [ ] **Streaming Responses**: Test agent streaming with various content types
- [ ] **JSX Rendering**: Verify React components render in completed messages
- [ ] **Voice Integration**: Test both dictation mode (spacebar) and agent mode (Option+D)
- [ ] **Modal Functionality**: Test help modal, feedback modal, import/export
- [ ] **Command Overlay**: Verify command execution display works
- [ ] **Memory Management**: Test conversation persistence and cleanup

## Feature-Specific Checks

### Message Rendering & Streaming

- [ ] Text streams progressively during agent responses
- [ ] JSX detection only runs on completed messages (`!msg.isStreaming &&` condition)
- [ ] Markdown rendering works with streaming
- [ ] No console errors during streaming
- [ ] Streaming indicators display properly
- [ ] Final message state transition works correctly

### Voice & Input Systems

- [ ] Dictation mode outputs to current focus (spacebar hold)
- [ ] Agent mode triggers AI agent (Option+D)
- [ ] Voice transcription accuracy maintained
- [ ] Wake word detection works in always-listening mode
- [ ] No microphone permission issues

### Tool Integration

- [ ] MCP tool integration functions correctly
- [ ] Basic tools (file operations, commands) work securely
- [ ] Desktop automation tools function properly
- [ ] Browser automation integration works
- [ ] Timer system with context resumption functions

### Security & Performance

- [ ] File operations respect security constraints
- [ ] Command execution follows whitelist/security rules
- [ ] No memory leaks during extended use
- [ ] Hardware monitoring collects data properly
- [ ] Performance remains acceptable under load

## Integration Testing

### Component Interactions

- [ ] Streaming + JSX rendering interaction works correctly
- [ ] Voice + streaming response interaction
- [ ] Command overlay + streaming response display
- [ ] Modal + background agent operations
- [ ] Memory management + tool execution

### Cross-Platform Considerations

- [ ] macOS accessibility permissions work in built apps
- [ ] Tauri entitlements and Info.plist are correct
- [ ] Platform-specific features function properly

## Regression-Specific Checks

### Known Problem Areas

Based on documented regressions, specifically test:

- [ ] **Streaming/JSX Conflict**: Verify `isJsxContent()` not called during streaming
- [ ] **Permission Detection**: Test both `computer_use_ai_sdk` and fallback mechanisms
- [ ] **Event Handler Cleanup**: Ensure proper listener cleanup on component unmount
- [ ] **State Management**: Verify streaming state flags are properly managed

### Common Failure Patterns

- [ ] Feature A works in isolation but breaks when Feature B is active
- [ ] New content processing interferes with existing rendering
- [ ] Event listeners accumulate without proper cleanup
- [ ] State flags become inconsistent between components

## Performance & Stability

### Resource Management

- [ ] Memory usage remains stable during extended sessions
- [ ] No file handle leaks
- [ ] Event listeners are properly cleaned up
- [ ] Background processes terminate correctly

### Error Handling

- [ ] Graceful degradation when components fail
- [ ] Proper error messages shown to users
- [ ] No silent failures that appear to work
- [ ] Recovery mechanisms function properly

## User Experience Validation

### Core User Flows

- [ ] **New User Onboarding**: First-time setup works smoothly
- [ ] **Daily Usage**: Common operations work reliably
- [ ] **Voice Workflow**: Voice-to-agent pipeline functions end-to-end
- [ ] **File Operations**: Import/export functionality works
- [ ] **Settings Management**: Configuration changes persist and take effect

### Accessibility & UI

- [ ] Keyboard navigation works properly
- [ ] Screen reader compatibility maintained
- [ ] Modal focus management works correctly
- [ ] Visual feedback for all user actions

## Documentation & Tracking

### Documentation Updates

- [ ] Update relevant documentation for changes made
- [ ] Add new test scenarios if needed
- [ ] Document any new known issues or limitations
- [ ] Update API documentation if applicable

### Bug Tracking

- [ ] Create regression report if any issues found
- [ ] Update prevention checklist if new patterns discovered
- [ ] Document lessons learned for future reference

## Final Release Checks

### Build Verification

- [ ] Development build works correctly
- [ ] Production build compiles successfully
- [ ] Built application passes all core functionality tests
- [ ] macOS app bundle includes correct entitlements

### Deployment Readiness

- [ ] All critical issues resolved
- [ ] Performance metrics within acceptable ranges
- [ ] Security checks completed
- [ ] Backup/rollback plan prepared if needed

---

## Emergency Rollback Criteria

If any of these conditions are met, consider rolling back the release:

- **Critical Functionality Lost**: Core features completely broken
- **Data Loss Risk**: User data could be corrupted or lost
- **Security Vulnerability**: New security risks introduced
- **Widespread Crashes**: Application becomes unstable for most users
- **Performance Degradation**: >50% performance decrease in core operations

---

**Checklist Completed By**: ___________  
**Date**: ___________  
**Release Version**: ___________  
**Review Sign-off**: ___________

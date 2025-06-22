# Juno AI Computer Use Agent - Rules & Documentation

**📍 Documentation has been reorganized and moved to `docs/rules/`**

## 🚨 CRITICAL DEVELOPMENT RULES 🚨

### ⚠️ BEFORE EVERY COMMIT - MANDATORY CHECKS

```bash
# 1. Compilation Check (MUST pass)
cargo check --manifest-path src-tauri/Cargo.toml

# 2. Duplicate Event Listener Check (ALL counts must be 1)
grep -n 'app\.listen("' src-tauri/src/lib.rs | cut -d'"' -f2 | sort | uniq -c | sort -nr
```

### 🚫 NO DUPLICATE EVENT LISTENERS

**CRITICAL**: Each event type MUST have exactly ONE listener. Duplicates cause race conditions and crashes.
📖 **Required Reading**: `event-listener-rules.md` - Contains mandatory rules for preventing application crashes.

## 🎯 Quick Navigation

### **Main Documentation Index**

➡️ **[docs/rules/INDEX.md](../../docs/rules/INDEX.md)** - Complete navigation for all documentation

### **Essential Documentation**

- **[Consolidated Documentation](../../docs/rules/CONSOLIDATED_DOCUMENTATION.md)** - Complete project overview
- **[Core Architecture Patterns](../../docs/rules/core-architecture-patterns.mdc)** - System design patterns
- **[Security Framework](../../docs/rules/security-stability-fixes.mdc)** - Security requirements and patterns
- **[Event Listener Rules](event-listener-rules.md)** - **CRITICAL** - Duplicate prevention rules

### **Organized Categories**

- **[Implementation](../../docs/rules/implementation/)** - Feature implementations and milestones
- **[Security](../../docs/rules/security/)** - Security framework and permissions  
- **[Testing](../../docs/rules/testing/)** - Testing strategies and validation
- **[Voice](../../docs/rules/voice/)** - Voice system implementation
- **[Cloud](../../docs/rules/cloud/)** - Cloud connector and remote control
- **[Tools](../../docs/rules/tools/)** - Tool system implementations
- **[UI](../../docs/rules/ui/)** - User interface and frontend

## 🔄 Migration Complete

All documentation has been successfully organized into logical categories under `docs/rules/` for better maintainability and navigation.

**Status**: ✅ **Organized and Current**

# Cursor Rules for Juno AI Computer Use Agent

This directory contains Cursor-specific rules and documentation to help AI assistants understand and work with the Juno codebase effectively.

## Rule Categories

### Core Architecture

- **[Tauri Architecture Patterns](tauri-architecture-patterns.mdc)** - Tauri v2 patterns, command structure, and state management
- **[Event System Patterns](event-system-patterns.mdc)** - Event-driven architecture and communication patterns
- **[Settings and Persistence](settings-and-persistence.mdc)** - Configuration management and data persistence

### Agent System

- **[Agent Iteration Limits](agent-iteration-limits.mdc)** - Managing agent execution limits and continuation system
- **[Agent Trigger Modes](agent-trigger-modes.mdc)** - Different modes of agent activation and execution
- **[Constants Management](constants-management.mdc)** - Rust-to-TypeScript constants generation and management

### Specialized Systems  

- **[Voice System Architecture](voice-system-architecture.mdc)** - Voice transcription and always-listening functionality
- **[Event Listener Rules](event-listener-rules.md)** - Frontend event handling patterns and safety
- **[Event Listener Safety](event-listener-safety.mdc)** - Safety patterns for event listeners

## Usage Guidelines

### For AI Assistants

1. **Read relevant rules** before making changes to understand patterns
2. **Follow established patterns** rather than creating new ones
3. **Reference rule files** when explaining architectural decisions
4. **Update rules** when introducing new patterns or fixing issues

### For Developers

1. **Create new rules** when establishing new architectural patterns
2. **Update existing rules** when patterns evolve or issues are discovered
3. **Use `.mdc` extension** for Cursor-specific markdown files
4. **Reference code files** using `[filename](mdc:path/to/file)` syntax

## Recent Updates

### 2024-01 - Agent Control Systems

- **NEW**: [Agent Iteration Limits](agent-iteration-limits.mdc) - Comprehensive guide to iteration limits and continuation UX
- **NEW**: [Constants Management](constants-management.mdc) - Rust-to-TypeScript constants generation system
- **UPDATED**: Improved agent execution control and user experience patterns

### Key Improvements

- **Fixed infinite agent execution** by implementing proper iteration limits (15 steps)
- **Improved continuation UX** with dual toast notifications prioritizing stop over continue
- **Consolidated constants system** eliminating duplication between generated and manual files
- **Enhanced debugging guides** for common agent execution issues

## File Naming Convention

- Use `.mdc` extension for Cursor-specific markdown files
- Use descriptive, hyphenated names (e.g., `agent-iteration-limits.mdc`)
- Group related rules by system/domain
- Include metadata headers with title, description, and tags

## Cross-References

Rules frequently reference each other and core codebase files:

- Agent rules reference event system patterns
- Event patterns reference Tauri architecture  
- Voice system integrates with agent trigger modes
- Constants management affects all system communication

This interconnected documentation helps maintain consistency across the complex Juno architecture.

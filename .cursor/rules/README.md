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

This directory contains comprehensive Cursor Rules that guide AI development and maintenance of the Juno AI Computer Use Agent. These rules establish patterns, best practices, and architectural guidelines to ensure consistent, high-quality development.

## Core Architecture Rules

### [no-backward-compatibility.mdc](mdc:no-backward-compatibility.mdc)

**NEW APPLICATION BUILD** - No backward compatibility, legacy code, or deprecated patterns. Clean, modern implementation only.

### [no-micromanaging-over-engineering.mdc](mdc:no-micromanaging-over-engineering.mdc)

Trust the AI agent's decisions. No hardcoded pattern detection or micromanaging. Execute what the agent provides.

### [tauri-architecture-patterns.mdc](mdc:tauri-architecture-patterns.mdc)

Comprehensive Tauri v2 patterns including commands, state management, async operations, and frontend integration.

### [event-system-patterns.mdc](mdc:event-system-patterns.mdc)

Event system architecture, listener management, and frontend-backend communication patterns.

## Token Management & Performance

### [token-optimization-visual-compression.mdc](mdc:token-optimization-visual-compression.mdc) **NEW**

**CRITICAL**: Visual Context Compression system for handling token overflow issues. Converts 50,000+ token screenshots to ~50 token summaries, achieving 70%+ token reduction for extended autonomous workflows.

### [memory-management-optimization.mdc](mdc:memory-management-optimization.mdc) **NEW**

Advanced memory management patterns, cross-agent memory sharing, conversation integrity, and performance monitoring for enterprise-grade AI agent operations.

## Enterprise AI Architecture

### [enterprise-ai-agent-architecture.mdc](mdc:enterprise-ai-agent-architecture.mdc) **NEW**

Next-generation enterprise AI agent architecture positioning Juno beyond traditional GenAI through autonomous decision-making, real-time data integration, and OS-level interaction capabilities.

## Agent Development

### [agent-prompt-guidelines.mdc](mdc:agent-prompt-guidelines.mdc)

Prompt engineering patterns and guidelines for effective AI agent communication.

### [agent-iteration-limits.mdc](mdc:agent-iteration-limits.mdc)

Agent execution patterns, iteration limits, and continuation system architecture.

### [agent-trigger-modes.mdc](mdc:agent-trigger-modes.mdc)

Voice activation patterns and agent trigger mode configuration.

## Tool Management

### [anthropic-computer-use-api-compliance.mdc](mdc:anthropic-computer-use-api-compliance.mdc)

Official Anthropic Computer Use API compliance and implementation patterns.

### [tool-consolidation-patterns.mdc](mdc:tool-consolidation-patterns.mdc)

Tool consolidation strategies, batching optimization, and performance improvements.

## System Integration

### [clean-architecture-maintenance.mdc](mdc:clean-architecture-maintenance.mdc)

Clean architecture principles, modular design patterns, and maintenance guidelines.

### [constants-management.mdc](mdc:constants-management.mdc)

Constants generation, synchronization, and management across frontend/backend.

### [settings-and-persistence.mdc](mdc:settings-and-persistence.mdc)

Settings management, persistence patterns, and configuration synchronization.

## Voice & Audio

### [voice-system-architecture.mdc](mdc:voice-system-architecture.mdc)

Voice transcription, audio processing, and voice command integration patterns.

## Cloud & Networking

### [cloud-connection-fix.mdc](mdc:cloud-connection-fix.mdc)

Cloud connectivity, authentication, and connection management patterns.

### [cloud-testing-patterns.mdc](mdc:cloud-testing-patterns.mdc)

Cloud service testing, mocking, and integration validation patterns.

### [websocket-troubleshooting.mdc](mdc:websocket-troubleshooting.mdc)

WebSocket connection management, error handling, and troubleshooting patterns.

## Development & Testing

### [keyboard-shortcut-validation.mdc](mdc:keyboard-shortcut-validation.mdc)

Keyboard shortcut implementation, validation, and cross-platform compatibility.

### [event-listener-rules.md](mdc:event-listener-rules.md)

Event listener safety, lifecycle management, and memory leak prevention.

### [event-listener-safety.mdc](mdc:event-listener-safety.mdc)

Enhanced event listener safety patterns and best practices.

### [appstate-helper-methods-fix.mdc](mdc:appstate-helper-methods-fix.mdc)

AppState helper methods and state management utilities.

## Critical Performance Optimizations

### Token Management Crisis Resolution

The Visual Context Compression system addresses the critical token overflow issue where Anthropic's API was rejecting requests due to exceeding 200,000 token limits (observed: 240,048 tokens). This system:

- **Reduces Token Usage by 70%+**: Screenshots consuming 50,000+ tokens are compressed to ~50 tokens
- **Enables Extended Workflows**: Agents can operate for hours without hitting token limits
- **Maintains Context**: Visual summaries preserve essential UI context and interaction history
- **Production Ready**: Automatic compression, fallback strategies, and error recovery

### Enterprise AI Positioning

The enterprise architecture rules position Juno as:

- **Autonomous Agent**: Self-optimizing workflows with minimal human intervention
- **Cross-System Integration**: Seamless operation across all enterprise systems
- **Cost Efficient**: 70%+ reduction in operational API costs
- **Scalable**: Extended autonomous workflows without technical limitations
- **Compliant**: Enterprise-grade security and audit capabilities

## Rule Development Guidelines

### Creating New Rules

1. Use `.mdc` extension for Cursor-specific markdown
2. Reference files using `[filename.ext](mdc:filename.ext)` format
3. Include practical code examples and implementation patterns
4. Focus on actionable guidance for AI development
5. Document both the problem and the solution

### Rule Categories

- **Architecture**: Core system design and patterns
- **Performance**: Optimization strategies and bottleneck resolution
- **Integration**: External system connectivity and data flow
- **Security**: Access control, validation, and compliance
- **Development**: Coding standards, testing, and maintenance

### Best Practices

- Keep rules focused and actionable
- Include code examples and implementation details
- Reference specific files and functions where applicable
- Document performance impact and metrics
- Provide troubleshooting guidance

## Enterprise Value Proposition

These rules establish Juno as a next-generation enterprise AI agent capable of:

1. **Autonomous Operation**: Extended workflows without human intervention
2. **Token Efficiency**: 70%+ cost reduction through intelligent optimization
3. **Cross-Agent Orchestration**: Specialist agents with shared context
4. **Production Scalability**: Enterprise-grade reliability and performance
5. **Competitive Differentiation**: Beyond traditional GenAI limitations

The comprehensive rule system ensures consistent development practices that maintain Juno's position as a leading enterprise AI agent solution.

## Quick Reference

### Most Critical Rules

1. [token-optimization-visual-compression.mdc](mdc:token-optimization-visual-compression.mdc) - Prevents token overflow
2. [memory-management-optimization.mdc](mdc:memory-management-optimization.mdc) - Ensures conversation integrity
3. [enterprise-ai-agent-architecture.mdc](mdc:enterprise-ai-agent-architecture.mdc) - Strategic positioning
4. [no-backward-compatibility.mdc](mdc:no-backward-compatibility.mdc) - Clean development approach
5. [tauri-architecture-patterns.mdc](mdc:tauri-architecture-patterns.mdc) - Technical foundation

### Emergency References

- Token overflow issues → Visual Context Compression
- Memory corruption → Memory Manager recovery patterns
- Agent performance → Enterprise optimization strategies
- System integration → MCP and cross-agent patterns
- Production deployment → Enterprise architecture guidelines

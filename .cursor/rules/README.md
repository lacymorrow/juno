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

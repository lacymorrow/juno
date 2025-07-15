# Tool Alias Routing Bug Fix Summary

## Problem Description

The SystemAgent was updated to remove what appeared to be redundant aliases for file operations (`list_files`, `get_file_content`, `set_file_content`), but this created a critical routing bug:

### Issues Created

1. **`system_*` tools**: The `ToolMappingService` fallback logic still routes `system_list_files`, `system_read_file`, and `system_write_file` to `SystemAgent`, but the agent no longer handles them → `ToolNotFound` errors
2. **`dev_*` tools**: These were incorrectly routed to `DesktopExpert` due to prefix matching instead of `SystemAgent` → Unusable tools
3. **Inconsistency**: `bash_command` still supported aliases while file operations didn't

## Root Cause Analysis

The issue was in the **ToolMappingService routing logic** in `src-tauri/src/agent/tools/tool_mapping.rs`:

```rust
// Fallback to prefix matching for dynamically named tools
if tool_name.starts_with("dev_") || tool_name.starts_with("desktop_") {
    return Some(ToolCategory::Desktop);  // ❌ Routes dev_list_files to Desktop
}
if tool_name.starts_with("system_") {
    return Some(ToolCategory::Basic);    // ❌ Routes system_list_files to Basic (SystemAgent)
}
```

The routing system **expected** these aliases to exist, but the SystemAgent pattern matching was removed.

## Solution Implemented

### 1. **Restored SystemAgent Pattern Matching**

Updated `src-tauri/src/agents/system_agent.rs` to handle all aliases:

```rust
// Before (broken)
"list_files" => { ... }
"get_file_content" => { ... }
"set_file_content" => { ... }

// After (fixed)
"list_files" | "dev_list_files" | "system_list_files" => { ... }
"get_file_content" | "dev_get_file_content" | "system_read_file" => { ... }
"set_file_content" | "dev_set_file_content" | "system_write_file" => { ... }
```

### 2. **Restored Tool Mapping Entries**

Updated `src-tauri/src/agent/tools/tool_mapping.rs` to include all aliases:

```rust
map.insert("system_list_files", ToolCategory::Basic);
map.insert("system_read_file", ToolCategory::Basic);
map.insert("system_write_file", ToolCategory::Basic);
map.insert("dev_list_files", ToolCategory::Basic);
map.insert("dev_get_file_content", ToolCategory::Basic);
map.insert("dev_set_file_content", ToolCategory::Basic);
```

## Key Learning Points

1. **Tool Routing is System-Wide**: Removing aliases from one component breaks the entire routing system
2. **Consistency Matters**: If `bash_command` has aliases, file operations should too
3. **Fallback Logic Dependencies**: The `ToolMappingService` prefix matching depends on agents actually handling those prefixes
4. **Backward Compatibility**: Tool aliases provide important backward compatibility for existing workflows

## Verification

- ✅ **Compilation**: `cargo check` passes with exit code 0
- ✅ **Routing**: All tool aliases now properly route to SystemAgent
- ✅ **Consistency**: File operations now match `bash_command` alias pattern
- ✅ **Backward Compatibility**: Existing code using any alias will continue working

## Files Modified

1. `src-tauri/src/agents/system_agent.rs` - Restored alias pattern matching
2. `src-tauri/src/agent/tools/tool_mapping.rs` - Added alias tool mappings

The fix maintains the single implementation approach while providing the expected alias routing behavior throughout the system.

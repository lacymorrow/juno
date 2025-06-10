# Permission Validation Implementation

## Overview

This document outlines the comprehensive permission validation system implemented to gracefully handle permission errors before tool execution, preventing cryptic `AgentError::ToolError` messages and providing helpful user guidance.

## Problem Solved

**Before**: Tools would execute and fail with unhelpful errors like:
- "Tool execution error: Desktop automation is not available"
- "Tool execution error: Failed to capture screenshot"
- Generic error messages without guidance

**After**: Tools check permissions first and provide specific, actionable error messages:
- "Tool 'capture_screenshot' requires screen recording permissions but they are not granted. Please grant screen recording permissions in System Settings > Privacy & Security > Screen Recording"
- "Tool 'desktop_click' requires accessibility permissions for desktop automation but they are not granted. Please grant accessibility permissions in System Settings > Privacy & Security > Accessibility and restart the app"

## Implementation Details

### 1. New AgentError Variant

Added `PermissionDenied(String)` to both AgentError enums:
- `src-tauri/src/agent/structs.rs`
- `src-tauri/src/agent/core.rs`

This provides a specific error type for permission-related failures, distinct from generic tool errors.

### 2. Permission Validation Utility

Created comprehensive permission validator in `src-tauri/src/utils/mod.rs`:

```rust
pub mod permission_validator {
    pub enum RequiredPermission {
        Accessibility,
        ScreenRecording,
        Microphone,
        InputMonitoring,
        AccessibilityAndScreenRecording,
    }

    pub async fn validate_permission(
        app_handle: &AppHandle,
        required: RequiredPermission,
        tool_name: &str,
    ) -> Result<(), AgentError>
}
```

**Features:**
- Checks specific permission types before tool execution
- Provides user-friendly error messages with instructions
- Handles optional permissions (microphone, input monitoring) gracefully
- Maps tools to required permissions automatically

### 3. Enhanced Error Recovery

Updated `src-tauri/src/agent/error_recovery.rs`:
- Added `PermissionDenied` to error pattern recognition
- Maps permission errors to appropriate recovery strategies
- Recognizes permission-related error messages in text

### 4. Tool-Level Permission Checks

Implemented permission validation in critical tools:

**Anthropic Computer Use Tools** (`src-tauri/src/agent/tools/anthropic_computer_use.rs`):
- Screenshot actions: Check screen recording permissions
- Mouse/keyboard actions: Check accessibility permissions
- Wait actions: No permission requirements

**Desktop Tools** (`src-tauri/src/agent/tools/desktop_tools.rs`):
- `capture_screenshot`: Screen recording permissions
- `get_focused_element_info`: Accessibility permissions
- `capture_element_screenshot`: Accessibility permissions
- `type_text`: Accessibility permissions
- `desktop_click`, `left_click`, `right_click`, `middle_click`, `double_click`: Accessibility permissions
- `mouse_move`, `left_click_drag`, `cursor_position`: Accessibility permissions

## Permission Categories

### Required Permissions by Tool Category

**Accessibility Permissions** (Critical):
- Mouse control: `desktop_click`, `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`
- Mouse movement: `mouse_move`, `left_click_drag`, `cursor_position`
- Keyboard input: `type_text`, `press_key`, `key`, `hold_key`
- Element interaction: `get_focused_element_info`, `element_interaction`
- Application control: `open_application`, `focus_application`, `list_windows`

**Screen Recording Permissions** (Critical):
- Screenshots: `capture_screenshot`, `screenshot`, `capture_element_screenshot`
- Visual analysis: All computer vision and AI analysis tools

**Microphone Permissions** (Optional):
- Voice features: `voice_transcription`, `always_listening`

**Input Monitoring Permissions** (Optional):
- Global shortcuts: `hotkey_registration`, `global_shortcuts`

## Error Message Examples

### Before Permission Check
```
AgentError::ToolError("Desktop automation is not available. Please grant accessibility permissions and restart the app.")
```

### After Permission Check
```
AgentError::PermissionDenied("Tool 'desktop_click' requires accessibility permissions for desktop automation but they are not granted. Please grant accessibility permissions in System Settings > Privacy & Security > Accessibility and restart the app")
```

## Usage

Tools now automatically validate permissions before execution:

```rust
// Before (in tool execution)
if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "desktop_click").await {
    return Err(e.to_string());
}

// Tool proceeds only if permissions are granted
let result = perform_desktop_click(x, y).await?;
```

## Benefits

1. **Proactive Permission Checking**: Validates permissions before attempting operations
2. **Clear Error Messages**: Specific guidance on what permissions are needed and how to grant them
3. **Graceful Degradation**: Optional permissions don't block execution
4. **User Guidance**: Direct instructions for fixing permission issues
5. **Better UX**: Users understand exactly what to do when permissions are missing
6. **Reduced Support**: Self-explanatory error messages reduce user confusion

## Integration with Existing Systems

- **Error Recovery**: Permission errors trigger appropriate recovery strategies
- **Agent Flow**: Permission failures are handled gracefully in agent execution
- **UI Integration**: Permission errors can be displayed to users with actionable buttons
- **Monitoring**: Permission status is logged and can be monitored

## Future Enhancements

1. **Automatic Permission Requests**: Could trigger permission dialogs automatically
2. **Permission Caching**: Cache permission status to avoid repeated checks
3. **Partial Functionality**: Allow tools to work with reduced capabilities when permissions are limited
4. **Permission Monitoring**: Watch for permission changes and update tool availability

This implementation provides a robust foundation for graceful permission handling throughout the agent system, ensuring users receive clear guidance when permissions are required.
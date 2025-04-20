# Implementation Plan for Anthropic Computer Use SDK

## Target Tool Specification (Based on Anthropic Docs)

Based on the [Anthropic Computer Use documentation](https://docs.anthropic.com/en/docs/agents-and-tools/computer-use) (computer_20250124 schema), the tools should ideally support:

**1. `computer` Tool Actions:**

*   **Keyboard:**
    *   `key`: Press a key/combination.
    *   `hold_key`: Hold down key/combination.
    *   `type`: Type text.
*   **Mouse:**
    *   `cursor_position`: Get cursor position.
    *   `mouse_move`: Move cursor.
    *   `left_mouse_down`/`up`: Press/release left button.
    *   `left_click`: Coordinate-based left click.
    *   `left_click_drag`: Drag with left button.
    *   `right_click`: Coordinate-based right click.
    *   `middle_click`: Coordinate-based middle click.
    *   `double_click`: Coordinate-based double click.
    *   `triple_click`: Coordinate-based triple click.
    *   `scroll`: Scroll at coordinates.
*   **Other:**
    *   `wait`: Wait for duration.
    *   `screenshot`: Take screenshot.

**2. `text_editor` Tool Actions:**

*   `view`: Read file content.
*   `create`: Create file with content.
*   `str_replace`: Find/replace string in file.
*   `insert`: Insert text at line number.
*   `undo_edit`: Undo last text edit.

**3. `bash` Tool Actions:**

*   `command`: Execute shell command.
*   `restart`: Restart shell process (potentially).

## Completed Steps

*   **[Done] Initial Setup & Analysis:**
    *   Analyzed existing macOS SDK code (`mcp-server-os-level`).
    *   Reviewed Anthropic documentation for target specification.
    *   Identified initial implementation gaps.
*   **[Done] Implement `wait`:**
    *   Added `wait` method to SDK (`AccessibilityEngine`, `MacOSEngine`).
    *   Exposed via `Desktop` struct.
    *   Added `wait` tool definition and handler in `src-tauri/src/lib.rs`.
*   **[Done] Implement Mouse Actions:**
    *   Implemented `cursor_position`, `mouse_move`, `left_mouse_down`, `left_mouse_up`, `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `left_click_drag`, `scroll_at_position` in SDK (`MacOSEngine`, `AccessibilityEngine` trait).
    *   Exposed methods via `Desktop` struct in SDK `lib.rs`.
*   **[Done] Implement Keyboard Actions:**
    *   Implemented `press_key`, `type_text`, `hold_key`, `release_key` in SDK.
    *   Exposed methods via `Desktop` struct.
*   **[Done] Implement Screenshot Actions:**
    *   Implemented `capture_screenshot` and `capture_element_screenshot` in SDK and Tauri backend (`tools.rs`, `commands.rs`).
    *   Exposed via Tauri commands and integrated into tool handling logic in `tools.rs`.
*   **[Done] Implement Clipboard Actions:**
    *   Implemented `get_clipboard_content`, `set_clipboard_content` in SDK.
    *   Exposed methods via `Desktop` struct.
*   **[Done] Implement Text Editor Actions:**
    *   Added `text_editor_view`, `text_editor_create`, `text_editor_str_replace`, `text_editor_insert` tool definitions and handlers using `std::fs` in `src-tauri/src/lib.rs`.
    *   Skipped `text_editor_undo_edit` due to state management complexity.
*   **[Done] Implement Bash Action:**
    *   Added `bash` tool definition and handler using `std::process::Command` in `src-tauri/src/lib.rs`.
    *   Timeout parameter defined in schema but not yet implemented in handler.
*   **[Done] Integrate Tool Definitions & Handlers:**
    *   Added comprehensive `ToolDefinition` structs for all implemented SDK actions to `src-tauri/src/lib.rs`.
    *   Updated `call_tool` function to handle these tools, parse parameters, and call corresponding `Desktop` methods or file system/process functions.
    *   Updated `submit_query` to use the local `list_tools` function.
    *   Ensured `cargo check` passes after integration.
*   **[Done] Implement `bash` Timeout:**
    *   Added `wait-timeout` crate dependency.
    *   Modified `call_tool` function's `bash` handler to spawn the command, wait with timeout using `child.wait_timeout`, and handle timeout/completion results.

## Current Status & Remaining Gaps

*   **Core Functionality:** Most core `computer`, `text_editor`, and `bash` actions specified by Anthropic are implemented and integrated into the Tauri backend (`src-tauri/src/lib.rs`), including `bash` timeout.
*   **Remaining Gaps:**
    *   `bash.restart`: The ability to restart the bash process is not implemented.
    *   `text_editor_undo_edit`: Not implemented due to state complexity. Requires tracking file changes or using a temporary file strategy.
    *   `cargo check` warnings: Several warnings related to unused code (helper functions, test functions, SDK internal functions) exist. These can be addressed later or ignored if the code is intended for future use.
    *   Error Handling: While basic error handling is present, further refinement might be needed for specific edge cases or providing more user-friendly error messages.
    *   Testing: No automated tests for the tool implementations currently exist.

## Next Steps

1.  **Implement `text_editor_undo_edit`:**
    *   Add `last_edited_file` and `previous_content` fields (with `Mutex`) to `AppState`.
    *   Update `create`, `insert`, and `str_replace` handlers in `call_tool` to store the previous state in `AppState`.
    *   Implement the `text_editor_undo_edit` handler in `call_tool` to restore the previous content or delete the file based on the stored state. Add its definition to `list_tools`.
2.  **Address `cargo check` Warnings:**
    *   Add `#[allow(dead_code)]` and `#[allow(unused_variables)]` annotations to silence warnings in `src-tauri/src/lib.rs` and `src/tts/` files for potentially unused but necessary helper functions, state fields, and test functions. Remove genuinely unused variables. Ensure `cargo check` passes.
3.  **Implement Anthropic API Streaming:**
    *   Modify backend (`src-tauri/src/lib.rs`) to use `stream: true` with the Anthropic API.
    *   Parse Server-Sent Events (SSE) from the response stream.
    *   Emit Tauri events to the frontend with text deltas, tool call information, and stream status.
    *   Adapt the tool execution loop for streaming.
    *   Modify frontend UI to listen for Tauri events and display the streaming response incrementally.
4.  **Refine Error Handling:**
    *   Review potential error scenarios in tool handlers and provide more specific/helpful error messages in the JSON results.
5.  **Add Tests:**
    *   Implement unit or integration tests for the tool handlers in `src-tauri/src/lib.rs` to verify correct parameter parsing and execution logic.

---
*This plan will be updated as steps are completed.*


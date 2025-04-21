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
    *   Implemented `text_editor_undo_edit` using internal state tracking (`AppState`) to revert file changes.
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
*   **[Done] Implement Window Management Functions:**
    *   Implemented `get_window_title`, `list_windows`, `close_window`, `maximize_window`, `minimize_window`, `resize_window`, `move_window` in `MacOSEngine`.
*   **[Done] Enhance Element Attributes:**
    *   Implemented `is_enabled`, `is_focused` in `MacOSUIElement`.
    *   Enhanced `get_all_attributes` in `MacOSUIElement` to fetch more standard attributes explicitly.
*   **[Done] Implement Advanced Selectors (Partial):**
    *   Implemented `Selector::Chain` for `find_element` in `MacOSEngine`.
    *   Implemented basic `Selector::Path` (simple chains) for `find_element` in `MacOSEngine`.
    *   Marked `Path`, `Filter`, `Chain` as unsupported for `find_elements`.
*   **[Done] Verify `get_element_tree` Implementation:**
    *   Confirmed `get_tree` implementation exists in `MacOSUIElement`.
    *   Confirmed `get_ui_tree` implementation exists in `MacOSEngine`.
    *   Confirmed `getUiTree` tool handler exists in `lib.rs` and correctly calls the engine function.

## Current Status & Remaining Gaps

*   **Core Functionality:** Many core `computer` actions specified by Anthropic are implemented and exposed as Tauri commands (`dev_...` functions).
*   **Remaining Gaps:**
    *   **Missing Tools (Tauri Layer):**
        *   `text_editor` Tool: None of the actions (`view`, `create`, `str_replace`, `insert`, `undo_edit`) are exposed as Tauri commands.
        *   `bash` Tool: Neither `command` nor `restart` actions are exposed as Tauri commands.
    *   **Computer Tool Gaps (Tauri Layer):**
        *   `hold_key` (`dev_hold_key`): Lacks the required `duration` parameter from the Anthropic spec. Current implementation holds indefinitely.
        *   Modifier Keys: Click actions (`dev_left_click`, etc.) and `dev_scroll_window` do not support holding modifier keys via the `text` parameter as described in the `computer_20250124` spec.
        *   `key` vs `press_key`: Anthropic's `key` implies a global key press, while `dev_press_key` acts on the focused element. `dev_global_type_text` handles global text input, but not single key presses/combinations globally.
    *   **Other:**
        *   `bash.restart`: The underlying ability to restart the bash process is not implemented in the SDK or handlers.
        *   `cargo check` warnings: Several warnings related to unused code exist.
        *   Error Handling: Could be refined for more specific user feedback.
        *   Testing: Lack of automated tests for Tauri commands/tool implementations.

## Next Steps

1.  **[Done] Implement `text_editor_undo_edit`:**
    *   Added `last_edited_file` and `previous_content` fields (with `Mutex`) to `AppState`.
    *   Updated `create`, `insert`, and `str_replace` handlers in `call_tool` to store the previous state in `AppState`.
    *   Implemented the `text_editor_undo_edit` handler in `call_tool` to restore the previous content or delete the file based on the stored state. Added its definition to `list_tools`.
    *   **Summary:** Verified the existing implementation in `dispatch.rs` which uses `AppState` to track the last edited file and its content (or lack thereof for creation) to perform undo by restoring content or deleting the file. Updated `plan.md` to reflect completion.
2.  **[Done] Expose `text_editor` Tool Commands:**
    *   Create Tauri commands (`#[tauri::command]`) in `src-tauri/src/commands.rs` for `view`, `create`, `str_replace`, `insert`, and `undo_edit`.
    *   These commands should use `std::fs` for basic operations and interact with `AppState` for `undo_edit`.
    *   Add these new commands to the `tauri::generate_handler!` macro in `src-tauri/src/lib.rs`.
    *   **Summary:** Added `dev_text_editor_view`, `dev_text_editor_create`, `dev_text_editor_str_replace`, `dev_text_editor_insert`, and `dev_text_editor_undo_edit` commands to `commands.rs`. Registered them in the handler in `lib.rs`. Fixed type errors related to `PathBuf` and `Option<Option<String>>` discovered during `cargo check`.
3.  **[Done] Expose `bash` Tool Commands:**
    *   Create a Tauri command (`#[tauri::command]`) in `src-tauri/src/commands.rs` for the `bash` `command` action, reusing the logic involving `std::process::Command` and `wait-timeout`.
    *   Add this command to the `tauri::generate_handler!` macro in `src-tauri/src/lib.rs`.
    *   (Optional: Implement `bash.restart` functionality if needed).
    *   **Summary:** Added `dev_bash_command` to `commands.rs` which takes command and optional timeout/restart parameters. Registered it in `lib.rs` handler.
4.  **Address `cargo check` Warnings:**
    *   Add `#[allow(dead_code)]` and `#[allow(unused_variables)]` annotations or remove unused code as appropriate. Ensure `cargo check` passes.
5.  **Implement Anthropic API Streaming:**
    *   Modify backend (`src-tauri/src/lib.rs`) to use `stream: true`.
    *   Parse SSE and emit events to frontend.
    *   Adapt frontend UI.
6.  **Refine Error Handling:**
    *   Review error scenarios and improve messages.
7.  **Add Tests:**
    *   Implement tests for Tauri commands and tool logic.
8.  **(Optional) Address Computer Tool Gaps:**
    *   Modify SDK and commands to support `hold_key` duration.
    *   Modify SDK and commands to support modifier keys for clicks/scrolls.
    *   Add a global `dev_press_key_global` command distinct from the element-focused one.

---
*This plan will be updated as steps are completed.*


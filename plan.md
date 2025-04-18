*   **[Done] Implement `wait`:**
    *   Added `wait` method to `AccessibilityEngine` trait.
    *   Implemented `wait` in `MacOSEngine` using `std::thread::sleep`.
    *   Exposed `wait` via `Desktop` struct in `lib.rs`.
    *   Added `wait` tool definition and handler to `lib.rs`.
    *   Ensured `cargo check` passes.
    *   *Summary:* Added a tool to pause execution for a specified duration in milliseconds.

*   **Next Steps:**
    *   **Implement Mouse Actions (macOS `interaction.rs`, `engine.rs`, `lib.rs`):**
        *   [Done] `cursor_position`: Get current mouse coordinates.
        *   [Done] `mouse_move`: Move mouse to specific coordinates.
        *   [Done] `left_mouse_down`/`up`: Simulate pressing/releasing the left mouse button.
        *   [Done] `left_click`: Coordinate-based left click.
        *   [Done] `right_click`: Coordinate-based right click.
        *   [Done] `middle_click`: Coordinate-based middle click.
        *   [Done] `double_click`: Coordinate-based double click.
        *   [Done] `triple_click`: Coordinate-based triple click.
        *   [Done] `left_click_drag`: Simulate dragging with the left button.
        *   [Done] Refine `scroll` to potentially accept coordinates as per Anthropic spec, or add a coordinate-based scroll action. (Covered by `scroll_at_position`)
        *   [Done] Add corresponding methods to `AccessibilityEngine` trait. (Methods exist)
        *   [Done] Implement methods in `MacOSEngine`. (Methods exist)
        *   [Done] Expose methods via `Desktop` struct. (Methods exist)
        *   [Done] Add `ToolDefinition` and handlers in `list_tools`/`call_tool`. (Requires manual merge/fix of provided code edit)
            *   *Summary:* Mouse actions are implemented in the SDK (`mcp-server-os-level`). Definitions/handlers need manual integration into `src-tauri/mcp-server-os-level/src/lib.rs`.
    *   [Done] Implement `text_editor` tool functionality.
        *   Added `text_editor_view`, `text_editor_create`, `text_editor_str_replace`, `text_editor_insert` tools.
        *   Skipped `text_editor_undo_edit` due to state management complexity.
        *   *Summary:* Basic file viewing, creation, replacement, and insertion tools added using `std::fs`.
    *   [Done] Implement `bash` tool functionality.
        *   Added `bash` tool definition and handler using `std::process::Command`.
        *   Timeout parameter is defined but not yet implemented.
        *   *Summary:* Basic shell command execution implemented, returning stdout/stderr/exit code.
    *   Address `cargo check` warnings.
    *   Refine error handling and test implementations.

---
*This plan will be updated after each significant step.*


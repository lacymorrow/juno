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
        *   `left_click_drag`: Simulate dragging with the left button.
        *   Refine `scroll` to potentially accept coordinates as per Anthropic spec, or add a coordinate-based scroll action.
        *   Add corresponding methods to `AccessibilityEngine` trait.
        *   Implement methods in `MacOSEngine`.
        *   Expose methods via `Desktop` struct.
        *   Add `ToolDefinition` and handlers in `list_tools`/`call_tool`.
    *   Implement `text_editor` tool functionality.
    *   Implement `bash` tool functionality.
    *   Address `cargo check` warnings.
    *   Refine error handling and test implementations.

---
*This plan will be updated after each significant step.*


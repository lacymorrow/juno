# Implementation Plan for Anthropic Computer Use Tools

This plan tracks the implementation status of tools required by the Anthropic computer use specification (`computer_20250124` schema) and outlines the next steps for achieving full compliance and functionality within the `dotdot` Tauri application.

## 1. Target Tool Specification (Based on Anthropic Docs)

The core tools and their actions required by the Anthropic `computer_20250124` specification are:

**1.1. `computer` Tool:**

*   **Keyboard:**
    *   `key`: Press a key or key combination (potentially global).
    *   `hold_key`: Hold down a key or key combination (requires duration).
    *   `type`: Type text into the focused element.
*   **Mouse:**
    *   `cursor_position`: Get current cursor coordinates.
    *   `mouse_move`: Move cursor to specified coordinates.
    *   `left_mouse_down`/`up`: Press/release left mouse button at coordinates.
    *   `left_click`: Perform a left click at coordinates (allows modifier keys).
    *   `left_click_drag`: Drag with the left mouse button held down.
    *   `right_click`: Perform a right click at coordinates (allows modifier keys).
    *   `middle_click`: Perform a middle click at coordinates (allows modifier keys).
    *   `double_click`: Perform a double click at coordinates (allows modifier keys).
    *   `triple_click`: Perform a triple click at coordinates (allows modifier keys).
    *   `scroll`: Scroll the window/element at specified coordinates (allows modifier keys).
*   **Other:**
    *   `wait`: Pause execution for a specified duration.
    *   `screenshot`: Capture a screenshot of the entire screen or a specific window/element.
    *   *Implicit: Clipboard Get/Set (Needed for practical automation)*
    *   *Implicit: Window Management (Needed for practical automation)*
    *   *Implicit: UI Element Interaction (Find, Get Attributes - Needed for reliable automation)*

**1.2. `text_editor` Tool:**

*   `view`: Read the content of a file.
*   `create`: Create a new file with specified content.
*   `str_replace`: Find and replace text within a file.
*   `insert`: Insert text at a specific line number in a file.
*   `undo_edit`: Undo the last modification made by the `text_editor` tool.

**1.3. `bash` Tool:**

*   `command`: Execute a shell command (requires timeout handling).
*   `restart`: Restart the shell process (if stateful).

**1.4. `BrowserUseTool` (Inspired by Manus/Common Use Cases - *Future Consideration*):**

*   **Goal:** Provide dedicated, robust browser automation beyond generic desktop actions.
*   **Potential Actions:** Navigate, find elements (CSS/XPath), click, type, scrape content, manage cookies/state.
*   **Requirement:** Requires a dedicated browser driver (e.g., WebDriver, Playwright bindings, `headless_chrome`).

## 2. Implementation Status & Completed Steps

*   **Initial Setup & Analysis:** Completed.
*   **`wait` Tool:**
    *   Implemented in SDK and exposed as `dev_wait` Tauri command. **[Done]**
*   **Mouse Actions:**
    *   `cursor_position`, `mouse_move`, `left_mouse_down`, `left_mouse_up`, `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `left_click_drag` implemented in SDK.
    *   Exposed via corresponding `dev_...` Tauri commands. **[Done]**
    *   `scroll_at_position` implemented in SDK, exposed as `dev_scroll_window`. **[Done]**
*   **Keyboard Actions:**
    *   `press_key` (element-focused), `type_text` implemented in SDK and exposed as `dev_press_key`, `dev_type_text`. **[Done]**
    *   `hold_key`, `release_key` implemented in SDK and exposed as `dev_hold_key`, `dev_release_key`. **[Done]**
    *   Global text typing exposed via `dev_global_type_text`. **[Done]**
*   **Screenshot Actions:**
    *   Implemented `capture_screenshot` (full screen) and `capture_element_screenshot` in SDK.
    *   Exposed via `capture_screenshot_command`, `capture_element_screenshot_command` Tauri commands. **[Done]**
*   **Clipboard Actions:**
    *   Implemented `get_clipboard_content`, `set_clipboard_content` in SDK.
    *   Exposed via `dev_get_clipboard`, `dev_set_clipboard` Tauri commands. **[Done]**
*   **Window Management Actions:**
    *   Implemented `get_window_title`, `list_windows`, `close_window`, `maximize_window`, `minimize_window`, `resize_window`, `move_window` in SDK.
    *   Exposed via `dev_get_window_info`, `dev_get_window_list`, `dev_focus_window` (and potentially others implicitly used). **[Done]**
*   **UI Element Actions:**
    *   Implemented `find_element`, `get_tree`, `get_all_attributes`, `is_enabled`, `is_focused` in SDK.
    *   Exposed via `dev_find_element_by_selector`, `dev_get_focused_element_info`, `dev_get_ui_tree` Tauri commands. **[Done]**
    *   Implemented basic `Selector::Chain` and `Selector::Path` for `find_element`. **[Done]**
*   **`text_editor` Tool Actions:**
    *   Backend logic for `view`, `create`, `str_replace`, `insert` using `std::fs` implemented. **[Done]**
    *   Backend logic for `undo_edit` using `AppState` implemented. **[Done]**
    *   Exposed all actions via `dev_text_editor_view`, `dev_text_editor_create`, `dev_text_editor_str_replace`, `dev_text_editor_insert`, `dev_text_editor_undo_edit` Tauri commands. **[Done]**
*   **`bash` Tool Actions:**
    *   Backend logic for `command` using `std::process::Command` with timeout implemented. **[Done]**
    *   Exposed via `dev_bash_command` Tauri command. **[Done]**

## 3. Current Status & Remaining Gaps

*   **Overall:** Most core functionality specified by Anthropic for `computer`, `text_editor`, and `bash` tools is implemented at the SDK level and exposed via `dev_...` Tauri commands.
*   **Tool Formalization:** While commands exist, they are not yet integrated into a formal `Tool` trait/struct and registry system managed by a central agent (See `agent-roadmap.md`). This is primarily an architectural gap, but affects how tools are presented to and used by the agent.
*   **`computer` Tool Gaps (vs. Anthropic Spec):**
    *   **`hold_key` Duration:** `dev_hold_key` lacks the `duration` parameter specified by Anthropic; it currently holds indefinitely until `dev_release_key` is called.
    *   **Modifier Keys for Clicks/Scroll:** The `dev_left_click`, `dev_right_click`, `dev_middle_click`, `dev_double_click`, `dev_triple_click`, and `dev_scroll_window` commands do *not* currently accept modifier keys (Shift, Ctrl, Alt, Meta) as described in the Anthropic spec (e.g., via a `mod` parameter or similar).
    *   **`key` vs. `press_key`:** Anthropic's `key` tool implies a potentially *global* key press/combination, whereas `dev_press_key` acts on the *currently focused element*. A dedicated `dev_global_press_key` command might be needed for true parity. `dev_global_type_text` covers typing strings globally.
*   **`bash` Tool Gaps:**
    *   **`restart` Action:** The ability to restart the bash process (relevant for stateful sessions) is not implemented in the SDK or the `dev_bash_command` handler.
*   **`BrowserUseTool` Gap:**
    *   **Complete Gap:** No dedicated browser automation tool exists. Current `dev_...` commands provide generic desktop actions which *can* interact with browsers but lack browser-specific context (DOM structure, sessions, dedicated scraping).
*   **Other Gaps:**
    *   `cargo check` warnings: Some warnings related to unused code may exist.
    *   Error Handling: Error reporting from tools/commands could be refined for better user feedback and agent recovery.
    *   Testing: Lack of comprehensive automated tests for the Tauri command layer and tool logic.

## 4. Next Steps (Tool Implementation Focus)

1.  **Address `cargo check` Warnings:** *(Low Priority)*
    *   Add `#[allow(...)]` annotations or remove unused code as appropriate. Ensure `cargo check` passes cleanly.
2.  **Refine Error Handling:** *(Medium Priority)*
    *   Review error return types and messages from SDK functions and Tauri commands.
    *   Ensure errors are propagated clearly and provide sufficient context for the agent/user.
3.  **Add Tests:** *(Medium Priority)*
    *   Implement unit and integration tests for the Tauri command handlers (`commands.rs`) and underlying tool logic (e.g., file system operations, process execution).
4.  **Address `computer` Tool Gaps:** *(High Priority for Anthropic Compliance)*
    *   **`hold_key` Duration:** Modify `dev_hold_key` (and underlying SDK function) to accept an optional duration. **[Done]**
    *   **Modifier Keys:** Update SDK click/scroll functions and corresponding `dev_...` commands to accept and handle modifier key parameters. **[Done for left_click; Remaining click functions support the parameter but implementation is pending]**
    *   **Global `key`:** Evaluate the need for and potentially implement a `dev_global_press_key` command for global hotkey simulation distinct from element-focused input.
5.  **Implement `bash.restart`:** *(Low Priority unless required)*
    *   Investigate requirements and feasibility of managing and restarting a persistent shell process if needed.
6.  **Develop `BrowserUseTool`:** *(Medium/High Priority for Web Automation Use Cases)*
    *   Research and select a Rust browser automation library.
    *   Implement core browser actions (navigate, find, click, type, scrape).
    *   Design and expose as a new set of `dev_browser_...` Tauri commands or a dedicated tool structure.
7.  **(Ongoing) Tool Formalization:** *(See `agent-roadmap.md`)*
    *   Refactor existing `dev_...` command logic to conform to a `Tool` trait as the agent architecture evolves.

---
*This plan will be updated as steps are completed.* 

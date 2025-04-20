# Computer Use Implementation Status

This document outlines the current status of the computer use functionalities implemented in this project, comparing them against the Anthropic Computer Use specification. A direct equivalent specification from OpenAI was not found during the investigation; OpenAI's approach appears to rely more on general developer-defined tools rather than a predefined computer use toolset.

**Legend:**

*   `[x]` - Implemented
*   `[p]` - Partially Implemented (e.g., lacks coordinate support, specific options)
*   `[ ]` - Not Implemented
*   `(commands.rs)` - Primary Tauri command interface found in `src-tauri/src/commands.rs`
*   `(tools.rs)` - Tool definition and dispatch logic found in `src-tauri/src/tools.rs` (likely calling `computer-use-ai-sdk` or OS utils)

---

## I. Anthropic Computer Tool (`computer_20250124` / `computer_20241022`)

This tool handles direct GUI interaction (mouse, keyboard, screen).

### Keyboard Actions

*   `[x]` **key**: Press key/combination (`dev_press_key` in `commands.rs`, `press_key` tool defined in `tools.rs`)
*   `[x]` **hold_key**: Hold key for duration (`dev_hold_key` + `dev_release_key` in `commands.rs`, `hold_key` + `release_key` tools defined in `tools.rs` - *Note: Anthropic only lists `hold_key`*)
*   `[x]` **type**: Type text string (`dev_type_text` in `commands.rs`, `type_text` tool defined in `tools.rs`)

### Mouse Actions

*   `[x]` **cursor_position**: Get current coords (`cursor_position` tool defined in `tools.rs`)
*   `[x]` **mouse_move**: Move to (x, y) (`mouse_move` tool defined in `tools.rs`)
*   `[x]` **left_mouse_down**: Press left button (`left_mouse_down` tool defined in `tools.rs`)
*   `[x]` **left_mouse_up**: Release left button (`left_mouse_up` tool defined in `tools.rs`)
*   `[p]` **left_click**: Click left button
    *   `[x]` Click at (x, y) (`left_click` tool defined in `tools.rs`)
    *   `[x]` Click focused element (`dev_click_focused_element` in `commands.rs`, `click_focused_element` tool defined in `tools.rs`)
    *   `[x]` Click element by selector (`dev_click_element_by_selector` in `commands.rs`, `click_element_by_selector` tool defined in `tools.rs`)
    *   `[x]` Click with modifier keys held (Anthropic spec mention - Implemented via simulation in `call_tool` using `hold_key`/`release_key`)
*   `[x]` **left_click_drag**: Click and drag (`left_click_drag` tool defined in `tools.rs`)
*   `[x]` **right_click**: Click right button (at coords) (`right_click` tool defined in `tools.rs`)
*   `[x]` **middle_click**: Click middle button (at coords) (`middle_click` tool defined in `tools.rs`)
*   `[x]` **double_click**: Double-click left button (at coords) (`double_click` tool defined in `tools.rs`)
*   `[x]` **triple_click**: Triple-click left button (Anthropic `_20250124` tool)
*   `[x]` **scroll**: Scroll wheel
    *   `[x]` Scroll up/down/left/right by amount (`scroll_window` in `commands.rs`, `scroll` tool defined in `tools.rs`)
    *   `[x]` Scroll at specific (x, y) coordinates (Anthropic spec mention - Implemented via `scroll_at_position` tool in `tools.rs`)
    *   `[x]` Scroll with modifier keys held (Anthropic spec mention - Implemented via simulation in `call_tool` for `scroll_window` and `scroll_at_position`)

### Other Actions

*   `[x]` **wait**: Pause execution (`dev_wait` in `commands.rs`, `wait` tool defined in `tools.rs`)
*   `[x]` **screenshot**: Take screenshot (`capture_screenshot_command` in `commands.rs`, `capture_screenshot` tool defined in `tools.rs`)

---

## II. Anthropic Text Editor Tool (`text_editor_20250124` / `text_editor_20241022`)

This tool handles file viewing and manipulation.

*   `[x]` **view**: View file/directory (`view_file_or_dir` tool defined in `tools.rs`)
    *   `[x]` View entire file/directory
    *   `[x]` View specific line range (Tool schema in `tools.rs` has `start_line`, `end_line`. Implementation added in `call_tool`)
*   `[x]` **create**: Create file (`create_file` tool defined in `tools.rs`)
*   `[x]` **str_replace**: Replace string in file (`str_replace_editor` helper and `str_replace` tool defined in `tools.rs`)
*   `[x]` **insert**: Insert string at line (`insert_text_into_file` tool defined in `tools.rs`)
*   `[x]` **undo_edit**: Revert last edit (`undo_edit` tool defined in `tools.rs`)

---

## III. Anthropic Bash Tool (`bash_20250124` / `bash_20241022`)

This tool handles shell command execution.

*   `[x]` **command**: Execute bash command (`bash_command` tool defined in `tools.rs`)
*   `[p]` **restart**: Restart shell state (Not explicitly defined as a separate tool in `tools.rs`, though `bash_command` schema has `restart` parameter - Parameter acknowledged in `call_tool`, but no state reset needed due to current execution model).

---

## IV. Extra / Custom Implemented Functionality

These functions are implemented but do not directly map to the standard Anthropic computer use tools. They are exposed either as Tauri commands or internal tools.

*   `[x]` **List Running Applications**: (`list_apps` in `commands.rs`)
*   `[x]` **Get Focused Element Info**: (`dev_get_focused_element_info` in `commands.rs`, `get_focused_element_info` tool defined in `tools.rs`)
*   `[x]` **Capture Element Screenshot**: (`capture_element_screenshot_command` in `commands.rs`, `capture_element_screenshot` tool defined in `tools.rs`)
*   `[x]` **Open Application**: (`dev_open_application` in `commands.rs`, `open_application` tool defined in `tools.rs`)
*   `[x]` **Open URL**: (`dev_open_url` in `commands.rs`, `open_url` tool defined in `tools.rs`)
*   `[x]` **Global Type Text**: (`dev_global_type_text` in `commands.rs` - *Potentially overlaps with standard `type` but might bypass focus*)
*   `[x]` **Get Clipboard**: (`dev_get_clipboard` in `commands.rs`, `get_clipboard` tool defined in `tools.rs`)
*   `[x]` **Set Clipboard**: (`dev_set_clipboard` in `commands.rs`, `set_clipboard` tool defined in `tools.rs`)
*   `[x]` **Find Element by Selector**: (`dev_find_element_by_selector` in `commands.rs`, `find_element_by_selector` tool defined in `tools.rs`)
*   `[x]` **Click Element by Selector**: (`dev_click_element_by_selector` in `commands.rs`, `click_element_by_selector` tool defined in `tools.rs`)
*   `[x]` **Get Window List**: (`dev_get_window_list` in `commands.rs`, `get_window_list` tool defined in `tools.rs`)
*   `[x]` **Get Window Info**: (`dev_get_window_info` in `commands.rs`, `get_window_info` tool defined in `tools.rs`)
*   `[x]` **Focus Window**: (`dev_focus_window` in `commands.rs`, `focus_window` tool defined in `tools.rs`)
*   `[x]` **Get Selected Text**: (`dev_get_selected_text` in `commands.rs`, `get_selected_text` tool defined in `tools.rs`)
*   `[x]` **Release Key**: (`dev_release_key` in `commands.rs`, `release_key` tool defined in `tools.rs` - Companion to `hold_key`)
*   `[x]` **Check Server Status**: (`check_server_status` in `commands.rs` - Internal health check?)
*   `[x]` **TTS**: (`tts::invoke_tts` in `lib.rs` - Text-to-Speech)
*   `[x]` **Submit Query**: (`anthropic::submit_query` in `lib.rs` - Core AI interaction)

---

## V. Summary of Gaps vs. Anthropic Spec

*   **Computer Tool:**
    *   `[x]` `triple_click` action is missing.
    *   `[x]` `left_click` action is missing support for holding modifier keys. -> Implemented via simulation.
    *   `[x]` `scroll` action is missing support for specific (x, y) coordinates and holding modifier keys. -> Coordinates implemented (`scroll_at_position`), modifiers implemented via simulation.
*   **Bash Tool:**
    *   `[p]` `restart` action is not explicitly defined as a separate tool, though the parameter exists in the `bash_command` tool schema. -> Parameter acknowledged, no state reset needed.

---

## VI. Potential Additions / Stretch Goals

*   **Implement Missing Anthropic Actions:** Fill the gaps listed above (~~triple_click~~, ~~click/scroll modifiers~~ -> implemented, ~~scroll coordinates~~ -> implemented, ~~bash restart tool~~ -> parameter handled).
*   **OS-Specific Features:** Explore deeper integration with macOS accessibility APIs or platform features beyond the current scope (e.g., interacting with specific controls not covered by generic elements, reading screen content via VoiceOver API, window manipulation beyond focus/info).
*   **Multi-Monitor Support:** Explicitly handle coordinates and screenshots across multiple displays if not already covered by the underlying SDK.
*   **More Robust Element Selection:** Enhance selector capabilities (e.g., XPath-like queries, image-based searching if feasible).
*   **Process Management:** Tools to list, start, or stop processes.
*   **File System Operations:** More granular file operations beyond the Text Editor tool (e.g., copy, move, delete, check existence, get metadata).
*   **System Information:** Tools to retrieve system details (OS version, hardware specs, network status).
*   **OpenAI Compatibility:** If OpenAI releases a concrete computer use spec in the future, map existing tools and implement any necessary additions.
*   **Bash State Management:** If a future requirement involves a persistent shell session between `bash` tool calls, implement proper state management and make the `restart` parameter functional.

Plan:
Analyze computer_use_ai_sdk (Mac): I will examine the code within src-tauri/mcp-server-os-level/src/platforms/macos/ to identify the currently implemented computer use actions (like click, type, scroll, open app, open URL, screenshot, get focused element, etc.). I'll look for any obvious stubs or placeholders.
Review Anthropic Documentation: I will check the Anthropic Computer Use documentation you provided (@<https://docs.anthropic.com/en/docs/agents-and-tools/computer-use>) to get a definitive list of expected actions and their parameters.
Identify Gaps: I'll compare the implemented functions with the Anthropic list.
Implement/Complete Functions: I will proceed to implement or complete the functions file-by-file, starting with the macOS platform implementations in src-tauri/mcp-server-os-level/src/platforms/macos/.
Update Tauri Commands: Ensure corresponding Tauri commands exist in src-tauri/src/lib.rs to expose the SDK functionality to the frontend/AI agent.
Verify with cargo check: After each significant code change, I will run cargo check --manifest-path src-tauri/Cargo.toml.
Let's start by listing the contents of the macOS platform directory to see the relevant files.
Okay, the macOS platform code is primarily in engine.rs, interaction.rs, element.rs, attributes.rs, and actions.rs.
Now, I'll perform a web search using the Anthropic documentation URL to extract the list of expected computer use actions.
Based on the Anthropic documentation (computer_20250124 schema), the primary computer interaction tool (computer) should support the following actions:
Keyboard Actions:
key: Press a key or key-combination.
hold_key: Hold down a key/combination for a duration.
type: Type a string of text.
Mouse Actions:
cursor_position: Get the current cursor position.
mouse_move: Move the cursor to (x, y).
left_mouse_down: Press the left mouse button.
left_mouse_up: Release the left mouse button.
left_click: Click left button at (x, y) (optionally holding keys).
left_click_drag: Drag from start_coordinate to (x, y).
right_click: Click right button at (x, y).
middle_click: Click middle button at (x, y).
double_click: Double-click left button at (x, y).
triple_click: Triple-click left button at (x, y).
scroll: Scroll by amount in a direction at (x, y).
Other Actions:
wait: Wait for a duration.
screenshot: Take a screenshot.
The documentation also mentions separate tools for:
Text Editor (text_editor_...): view, create, str_replace, insert, undo_edit. (This seems like a higher-level abstraction potentially built on bash/computer tools or direct file I/O).
Bash (bash_...): command, restart. (This involves running shell commands).
Your current SDK (computer-use-ai-sdk) seems to have implementations for:
Screenshot: capture_screenshot_command (in src-tauri/src/lib.rs) calls macos_utils::capture_and_encode_screenshot(). Full screen capture exists.
Element Screenshot: capture_element_screenshot_command (in src-tauri/src/lib.rs) calls macos_utils::capture_element_screenshot(). Element-specific capture exists.
Get Focused Element: dev_get_focused_element_info (in src-tauri/src/lib.rs) uses get_focused_element_ns_workspace. Retrieving focused element info exists.
Click Focused Element: dev_click_focused_element (in src-tauri/src/lib.rs) likely uses accessibility APIs via the SDK's engine. Clicking based on focused element seems partially implemented.
Type Text: dev_type_text (in src-tauri/src/lib.rs) likely uses the SDK's engine. Typing text seems implemented.
Press Key: dev_press_key (in src-tauri/src/lib.rs) likely uses the SDK's engine. Pressing specific keys seems implemented.
Open Application: dev_open_application (in src-tauri/src/lib.rs) uses desktop.open_application(). Opening apps exists.
Open URL: dev_open_url (in src-tauri/src/lib.rs) uses desktop.open_url(). Opening URLs exists.
Scroll Window: dev_scroll_window (in src-tauri/src/lib.rs) seems implemented.
Analysis & Gaps (Focusing on computer tool actions for macOS):
Keyboard:
key: Implemented (dev_press_key -> engine.press_key).
hold_key: Missing. Needs implementation in mcp-server-os-level/src/platforms/macos/interaction.rs (or similar) and exposure via the engine trait and Tauri command.
type: Implemented (dev_type_text -> engine.type_text).
Mouse:
cursor_position: Missing. Needs implementation (likely using core_graphics::event::CGEvent::mouse_location()) and exposure.
mouse_move: Missing. Needs implementation (using CGEvent::mouse_event) and exposure.
left_mouse_down/up: Missing. Needs implementation (using CGEvent::mouse_event with appropriate types) and exposure.
left_click: Partially implemented via dev_click_focused_element. Needs a version accepting coordinates (engine.click_point?).
left_click_drag: Missing. Needs implementation (sequence of move, down, move, up events) and exposure.
right_click: Missing. Needs implementation (CGEvent::mouse_event with right button type) and exposure.
middle_click: Missing. Needs implementation (CGEvent::mouse_event with middle button type) and exposure.
double_click/triple_click: Missing. Needs implementation (CGEvent::mouse_event with click count parameter) and exposure.
scroll: Implemented (dev_scroll_window -> engine.scroll_window). Seems to target the focused window rather than a specific coordinate as per Anthropic spec. Might need refinement or an additional coordinate-based scroll.
Other:
wait: Missing. Could be implemented simply with std::thread::sleep, exposed via Tauri.
screenshot: Implemented (capture_screenshot_command).
Additional Tools:
Text Editor: Not implemented. This would require significant file I/O logic, potentially state management for undo, and careful path handling.
Bash: Not implemented. Would require spawning shell processes (std::process::Command) and managing their state/output.
Next Steps:
Let's start by implementing the missing basic computer tool actions for macOS within the computer-use-ai-sdk crate (mcp-server-os-level). We'll begin with the mouse actions, as they form a core part of GUI interaction.
I will start by adding the missing mouse actions (mouse_move, left_mouse_down, left_mouse_up, right_click, middle_click, double_click, triple_click, left_click_drag, cursor_position) to the MacOSEngine implementation in mcp-server-os-level/src/platforms/macos/engine.rs and the corresponding trait definitions in mcp-server-os-level/src/platforms/mod.rs. This will involve using core_graphics APIs.

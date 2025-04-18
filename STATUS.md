Based on the tauri::generate_handler! list in src-tauri/src/lib.rs, here are the exposed "dev" commands which seem to map to the computer use primitives:
capture_screenshot_command: Maps to takeScreenshot.
dev_get_focused_element_info: Partially maps to getAttributes (provides some info about the focused element).
capture_element_screenshot_command: Specific screenshot capability.
dev_click_focused_element: Maps to click (on the focused element).
dev_type_text: Maps to pressKeys (for typing text).
dev_press_key: Maps to pressKeys (for individual key presses/combinations).
dev_open_application: Maps to openApplication.
dev_open_url: Maps to goToUrl.
dev_scroll_window: Maps to scroll.
dev_global_type_text: Alternative text typing.
dev_get_clipboard: Maps to getClipboard.
dev_set_clipboard: Maps to setClipboard.
dev_hold_key, dev_release_key: Lower-level key actions, supporting pressKeys.
dev_wait: Utility command.
dev_find_element_by_selector: Supports finding elements for subsequent actions.
dev_click_element_by_selector: Maps to click (using a selector).
list_apps: Utility command (not a direct primitive, but related).
Comparison with Anthropic Primitives:
Based on the Anthropic documentation, here are some potentially missing or incomplete primitives:
setValue: While dev_type_text can input text, setValue often implies clearing the existing value first before typing. It's unclear if dev_type_text does this. An explicit setValue command might be beneficial.
goBack / goForward: These browser-specific navigation actions are missing. Implementing them reliably outside of browser automation would be complex.
extractText: While dev_get_focused_element_info provides some text/value, a dedicated command to extract all visible text from a specific element or region might be needed for broader use cases. This usually relies heavily on accessibility APIs.
extractHyperlinks: Similar to extractText, requires accessibility API integration to find and extract link elements and their URLs.
getAttributes: dev_get_focused_element_info provides some information, but a dedicated getAttributes might be expected to return a richer set of accessibility attributes for a given element (like role, description, state, etc.).
Incomplete Mac Functions:
Identifying functions that are stubbed or incomplete specifically for macOS within your codebase (src-tauri/) is difficult without seeing the full implementation details in commands.rs, utils.rs, and the underlying computer_use_ai_sdk. The dev_ commands in lib.rs mostly delegate to the desktop_instance, suggesting the core logic (including platform specifics) resides in the SDK.
The warnings from cargo check about unused functions/methods within the computer-use-ai-sdk might hint at areas that aren't fully utilized or are potentially incomplete, but they aren't definitive errors.
Summary:
The code restructuring is complete and passes checks.
Most core Anthropic computer use primitives seem to have corresponding dev_ commands.
Potentially missing/incomplete primitives include: setValue (explicitly), goBack/goForward, extractText (general), extractHyperlinks, and getAttributes (comprehensive).
Confirming incomplete Mac-specific implementations requires deeper inspection of the commands, utils, and SDK code.

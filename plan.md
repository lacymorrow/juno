# Project Plan: macOS Anthropic Computer Use Implementation

This document outlines the plan for implementing the necessary macOS functionalities to support the Anthropic Computer Use specification within the dotdot Tauri application.

## 1. Product Requirements Document (PRD)

### 1.1. Goal

To provide a complete and robust implementation of the Anthropic Computer Use toolset on the macOS platform, enabling an AI agent to interact with the user's desktop environment effectively and reliably via the `computer-use-ai-sdk`.

### 1.2. Scope

*   **In Scope:**
    *   Implement all macOS-specific functions required by the actions defined in the Anthropic Computer Use tool specification (e.g., `typeText`, `pressKey`, `holdKey`, `releaseKey`, `scroll`, `wait`, `findElement`, `getClipboard`, `setClipboard`, `screenshot`).
    *   Implement macOS support for the `text_editor` and `bash` tools defined by Anthropic.
    *   Ensure functions correctly utilize macOS Accessibility APIs (`AXUIElement`) and/or Core Graphics event simulation where appropriate.
    *   Integrate implemented functions into the `computer-use-ai-sdk`'s `Desktop` and `UIElement` abstractions.
    *   Define and expose corresponding tool definitions (`ToolDefinition`) in `Desktop::list_tools` for the Anthropic agent.
    *   Ensure basic error handling and reporting for each implemented function.
    *   Maintain compilation success via `cargo check` after each significant code modification in the Rust backend.
*   **Out of Scope:**
    *   Implementation for Windows or Linux platforms.
    *   Significant changes to the frontend UI (unless required for development/testing tools).
    *   Advanced error recovery scenarios beyond basic reporting.
    *   Performance optimization beyond ensuring reasonable responsiveness.

### 1.3. Core Functionality Requirements

*   **Identify Missing Functions:** Systematically identify all functions stubbed or missing in the `computer-use-ai-sdk/src/platforms/macos/` modules (`engine.rs`, `element.rs`, `interaction.rs`, etc.) based on the Anthropic spec.
*   **Implement Actions:**
    *   **`typeText` (Global):** Simulate typing text without targeting a specific element.
    *   **`pressKey` (Global/Element):** Simulate pressing keyboard keys and combinations (potentially update `dev_press_key` and add global version).
    *   **`holdKey` (Global):** Simulate holding a modifier key down.
    *   **`releaseKey` (Global):** Simulate releasing a modifier key.
    *   **`scroll` (Global/Element):** Implement reliable scrolling for windows/elements (refine `dev_scroll_window` and add global/element versions).
    *   **`wait`:** Implement a pause/delay mechanism.
    *   **`findElement`:** Ensure robust element finding via selectors (ongoing refinement).
    *   **`getClipboard`:** Read text content from the system clipboard.
    *   **`setClipboard`:** Write text content to the system clipboard.
    *   **`screenshot`:** Capture screenshots (already partially implemented, potentially enhance).
*   **Implement Tools:**
    *   **`text_editor`:** Implement functionality to read, write, and potentially edit files.
    *   **`bash`:** Implement functionality to execute shell commands.
*   **Tool Integration:** Add `ToolDefinition` entries for all implemented actions/tools in `Desktop::list_tools`.
*   **Error Handling:** Functions should return appropriate `AutomationError` variants on failure.

## 2. Implementation Plan & Log

This section tracks the steps taken and planned for the implementation.

*   **[Done] Initial Setup & Refactoring:**
    *   Identified missing macOS functions based on user request and Anthropic spec.
    *   Fixed compilation errors related to logging (`Desktop::log` removal, replaced with `tracing`).
    *   Fixed compilation errors related to type mismatches (`Locator::new`, `AutomationError`).
    *   Implemented `dev_scroll_window` Tauri command using `MacOSEngine::scroll_at_current_position`.
    *   Ensured `cargo check --manifest-path src-tauri/Cargo.toml` passes.
    *   *Summary:* Addressed initial compilation issues arising from refactoring the logging system and type signature changes. Implemented the basic scroll dev tool command.

*   **[Done] Implement Global `typeText`:**
    *   Added `type_text` method to `AccessibilityEngine` trait.
    *   Implemented `type_text` in `MacOSEngine` using Core Graphics keyboard event simulation (via `interaction::type_text_global`).
    *   Added `type_text` method to `Desktop` struct in `lib.rs`.
    *   Added `typeText` tool definition and handler to `Desktop::list_tools` and `Desktop::call_tool` in `lib.rs`.
    *   Ensured `cargo check` passes.
    *   *Summary:* Implemented the ability for the agent to type text globally without needing a specific target element, using macOS keyboard event simulation.

*   **[Done] Implement Clipboard Tools:**
    *   Identified existing `get_clipboard_contents` and `set_clipboard_contents` in `macos::interaction` using `clipboard-macos`.
    *   Added `get_clipboard_content` and `set_clipboard_content` methods to `AccessibilityEngine` trait (`platforms/mod.rs`).
    *   Implemented these methods in `MacOSEngine` (`macos/engine.rs`).
    *   Added corresponding methods to `Desktop` struct implementation in `lib.rs`.
    *   Added `getClipboard` and `setClipboard` tool definitions to `list_tools` in `lib.rs`.
    *   Added handlers for `getClipboard` and `setClipboard` in `call_tool` in `lib.rs`.
    *   Added missing `ToolNotFound` variant to `AutomationError` enum (`errors.rs`).
    *   Fixed module path issues related to `utils` vs `macos_utils`.
    *   Ensured `cargo check` passes.
    *   *Summary:* Added tools for getting and setting the system clipboard content.

*   **[Done] Implement `holdKey`/`releaseKey`:**
    *   Added `hold_key` and `release_key` functions to `macos::interaction` using `CGEvent`.
    *   Added `hold_key` and `release_key` methods to `AccessibilityEngine` trait.
    *   Implemented these methods in `MacOSEngine`, parsing modifier key names and using constants defined in `macos::constants` for key codes and modifier flags.
    *   Exposed `hold_key` and `release_key` via `Desktop` struct in `lib.rs`.
    *   Added `holdKey` and `releaseKey` tool definitions and handlers to `lib.rs`.
    *   Fixed issues with `CGEventFlags` constants usage by importing and using `MODIFIER_*` constants from `constants.rs`.
    *   Ensured `cargo check` passes.
    *   *Summary:* Added tools to simulate holding and releasing modifier keys (Shift, Command, Control, Option/Alt).

*   **[Done] Implement `wait`:**
    *   Added `wait` method to `AccessibilityEngine` trait.
    *   Implemented `wait` in `MacOSEngine` using `std::thread::sleep`.
    *   Exposed `wait` via `Desktop` struct in `lib.rs`.
    *   Added `wait` tool definition and handler to `lib.rs`.
    *   Ensured `cargo check` passes.
    *   *Summary:* Added a tool to pause execution for a specified duration in milliseconds.

*   **Next Steps:**
    *   Implement missing mouse actions (e.g., `cursor_position`, `mouse_move`, `left_mouse_down/up`, coordinate-based `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `left_click_drag`).
    *   Implement `text_editor` tool functionality.
    *   Implement `bash` tool functionality.
    *   Address `cargo check` warnings.
    *   Refine error handling and test implementations.

---
*This plan will be updated after each significant step.* 

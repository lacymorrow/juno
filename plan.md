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
    *   Implemented `type_text` in `MacOSEngine` using Core Graphics keyboard event simulation.
    *   Added `type_text` method to `Desktop` struct.
    *   Added `typeText` tool definition and handler to `Desktop::list_tools` and `Desktop::call_tool`.
    *   Ensured `cargo check` passes.
    *   *Summary:* Implemented the ability for the agent to type text globally without needing a specific target element, using macOS keyboard event simulation.

*   **Next Steps:**
    *   Implement `getClipboard` and `setClipboard` functionality in `macos::interaction` and expose via `Desktop`.
    *   Add `getClipboard` and `setClipboard` tool definitions.
    *   Implement `holdKey` and `releaseKey` functionality.
    *   Add `holdKey` and `releaseKey` tool definitions.
    *   Implement `wait` functionality.
    *   Add `wait` tool definition.
    *   Implement `text_editor` tool functionality.
    *   Implement `bash` tool functionality.
    *   Address `cargo check` warnings.
    *   Refine error handling and test implementations.

---
*This plan will be updated after each significant step.* 

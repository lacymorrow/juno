# Implementation Plan

run app with:
`bun run tauri dev`
## Phase 1: Core OS Control (macOS Focus)

### Task 1: Implement Basic macOS UI Automation Primitives

- Status: In Progress
- Summary: Basic AXUIElement wrappers and core actions (click, type, bounds, attributes) are implemented in `src-tauri/mcp-server-os-level/src/platforms/macos/`. Refactored core attribute and interaction logic from `element.rs` into `attributes.rs` and `interaction.rs` respectively.
- Next Steps: Continue refactoring `element.rs` to improve modularity and testability.

### Task 2: Enable Web Content Interaction

- Status: In Progress
- Summary: Current implementation fails to interact reliably with web content due to limitations of the Accessibility API (AXUIElement) within web views. The agent cannot "see" the DOM.
- Next Steps:
    1.  Implement screenshot functionality in Rust using `core_graphics`. (Status: **In Progress** - `capture_screenshot` function added to `macos/utils.rs`)
    2.  Expose screenshots via a Tauri command, returning base64 encoded image data.
    3.  Update agent tools/prompts to request and utilize screenshots for visual context, especially for web interactions.
    4.  Refine element finding logic to combine visual analysis (from screenshots) with accessibility data.
    5.  Set a standard browser User-Agent in `tauri.conf.json` to prevent potential website compatibility issues.

### Task 3: Implement Missing macOS Control Features
- Status: TODO
- Summary: Several standard UI automation features are missing.
- Next Steps (Prioritized):
    1.  Window Management (Minimize, Maximize, Close, Resize, Move)
    2.  Right Click
    3.  Hover
    4.  Drag and Drop
    5.  Menu Bar Interaction
    6.  Clipboard Access (Read/Write)
    7.  Advanced Scrolling (Pixel-level, Area-specific)
    8.  Dialog/Alert Handling
    9.  Implement `is_enabled` and `is_focused` checks.

## Phase 2: Cross-Platform Support (Windows, Linux)
- Status: Not Started

## Phase 3: Agent Integration & Refinement
- Status: Not Started

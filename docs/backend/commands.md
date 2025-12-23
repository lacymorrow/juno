# Backend Commands API

This document categorizes the Tauri commands exposed to the frontend.

## Core System
- `init_app`: Initializes application state.
- `get_system_info`: Returns OS version, architecture, and resource usage.
- `check_permissions`: Verifies accessibility/screen recording access.
- `open_settings`: Launches the native system settings panel.

## Agents & AI
- `submit_query`: Main entry point for user chat input.
- `cancel_agent`: Stops the current agent execution immediately.
- `get_agent_status`: Returns current thinking/tool-use state.
- `list_tools`: Returns available tools for the UI to display.

## Utilities
- `validate_api_key`: Checks provider API keys (Anthropic/OpenAI).
- `get_app_version`: Returns the value from `Cargo.toml`.
- `update_app`: Triggers the updater flow.

## Voice
- `start_dictation`: Initializes audio capture and transcription stream.
- `stop_dictation`: Finalizes the stream and returns full text.
- `set_input_device`: Changes the active microphone.

## Window Control
- `show_main_window`: Brings the main interface to front.
- `hide_main_window`: Hides the interface.
- `resize_window`: Programmatically resizes the main panel (used for dynamic UI expansion).
- `set_click_through`: Toggles mouse event transparency.

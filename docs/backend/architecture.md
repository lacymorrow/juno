# Backend Architecture

The backend of Juno AI is built in Rust using the Tauri framework. It serves as the "nervous system" of the application, handling low-level OS interactions, AI orchestration, and maintaining the source of truth for application state.

## Core System (`src-tauri/src/`)

### Entry Point (`main.rs`)
The application entry point initializes the Tauri builder, loads plugins, and sets up the primary event loop.
- **Builder Pattern**: Uses `tauri::Builder` to register plugins (`tauri-plugin-voice-transcription`, `tauri-plugin-nspanel`) and state.
- **Menu**: Sets up the system tray and application menu via `menu::create_app_menu`.
- **Run Loop**: Starts the application, preventing exit on window close (for macOS background apps).

### Global State (`state.rs`)
All shared application state is managed via `AppState`, which is wrapped in a thread-safe `Arc<Mutex<>>` or Tauri's managed state containers.
- **Components**:
  - `ProcessState`: Tracks if an agent is thinking or processing.
  - `Settings`: User configuration (API keys, hotkeys).
  - `ConversationHistory`: In-memory store of recent chat messages.

### Startup & Setup (`setup.rs`)
Handles initialization tasks that must occur *after* the app is running but *before* the user interacts.
- **Spotlight Panel**: Initializes the floating "spotlight" window (NSPanel on macOS).
- **Shortcuts**: Registers global hotkeys (e.g., `Cmd+Shift+Space`) using `MASShortcut` (via `tauri-nspanel`).
- **Permissions**: Checks for accessibility and screen recording access on launch.

## Event System
Juno relies heavily on an event-driven architecture to communicate between the Rust backend and the TypeScript frontend.

### Frontend -> Backend
- **Commands**: The frontend calls Rust functions using `invoke('command_name', payload)`.
- **Shortcuts**: Global hotkeys trigger internal Rust events which may then invoke frontend logic.

### Backend -> Frontend
- **Events**: The backend emits events via `app_handle.emit_all("event_name", payload)`.
- **Key Events**:
  - `BAR_STATE_UPDATE`: Syncs the visual state of the floating bar (Listening, Thinking, Idling).
  - `STREAMING_TEXT_STREAM`: Sends chunks of LLM tokens to the UI.
  - `AGENT_EVENT`: Notifies the UI of agent actions (Tool calls, Logic steps).

## Window Management (`window_management.rs`)
Manages the lifecycle and behavior of the application's windows.
- **Floating Bar**: A custom NSPanel that floats above other apps.
- **Settings Window**: A standard Tauri window for configuration.
- **Behavior**: Handles focus stealing, resizing, and positioning based on the active screen.

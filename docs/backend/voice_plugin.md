# Voice Utilities & Transcription Plugin

Juno implementation of voice features relies on a custom Tauri plugin: `tauri-plugin-voice-transcription`.

## Architecture

This plugin is designed for low-latency, privacy-focused local speech-to-text.

### Core Components (`lib.rs`)
- **Plugin Entry**: Registers the plugin with Tauri.
- **State Management**: Holds the `VoiceController` and `AlwaysListeningController`.

### Voice Controller (`controller.rs`)
The engine room of the transcription system.
- **Audio Capture**: Uses `cpal` to stream audio from the default input device.
- **Resampling**: `rubato` resamples input (e.g., 44.1kHz or 48kHz) to the 16kHz required by Whisper.
- **Transcription**: `whisper-rs` (Rust bindings for `whisper.cpp`) runs the inference.
- **Threading**: A dedicated audio thread ensures the UI and main logic are never blocked by audio processing.

### Model Optimization
To save memory (RAM/VRAM), the plugin uses a `SharedWhisperManager`.
- **Problem**: Loading the Whisper model (~1.5GB for medium) twice (once for dictation, once for wake-word) is wasteful.
- **Solution**: The `SharedWhisperManager` loads the model *once* and passes `Arc<WhisperContext>` to both controllers. They use a standard `Mutex` to coordinate access, ensuring only one performs inference at a time (e.g., dictation takes priority over wake-word).

## Modes

### 1. Dictation
- **Trigger**: User activates "Dictation Mode".
- **Behavior**: Continuous stream of partial results.
- **Event**: Emits `VOICE_TRANSCRIPTION_PARTIAL_RESULT` aggressively for real-time feedback.

### 2. Always Listening (Wake Word)
- **Trigger**: Background monitoring.
- **Behavior**: buffers audio in a ring buffer. Occasionally runs inference to detect specific trigger phrases (e.g., "Hey Juno").
- **Privacy**: Does not record to disk; buffer is strictly in-memory and overwritten constantly.

# Tauri Plugin Voice Transcription

A Tauri plugin that provides voice transcription and dictation capabilities using OpenAI's Whisper model through the `whisper-rs` crate.

## Features

- **Real-time voice dictation** with partial results
- **File transcription** support for WAV audio files
- **Cross-platform** audio recording using CPAL
- **Automatic audio resampling** to match Whisper requirements
- **Event-based API** for seamless frontend integration
- **Configurable model paths** and settings
- **TypeScript/JavaScript bindings** for easy use

## Installation

Add the plugin to your Tauri project:

```toml
[dependencies]
tauri-plugin-voice-transcription = "0.1.0"
```

## Usage

### Rust Setup

Register the plugin in your Tauri app:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_voice_transcription::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Configuration

Add plugin configuration to your `tauri.conf.json`:

```json
{
  "plugins": {
    "voice-transcription": {
      "modelPath": "models/ggml-base.en.bin",
      "sampleRate": 16000,
      "bufferDurationMs": 1500,
      "enablePartials": true,
      "language": "en",
      "debugMode": false
    }
  }
}
```

### Frontend Usage

```typescript
import { 
  startDictation, 
  stopDictation, 
  toggleDictation, 
  getDictationStatus,
  transcribeFile 
} from 'tauri-plugin-voice-transcription-api';
import { listen } from '@tauri-apps/api/event';

// Start dictation
await startDictation();

// Listen for partial results
const unlistenPartial = await listen('voice-transcription:partial-result', (event) => {
  console.log('Partial:', event.payload.text);
});

// Listen for final results
const unlistenFinal = await listen('voice-transcription:final-result', (event) => {
  console.log('Final:', event.payload.text);
});

// Stop dictation
await stopDictation();

// Transcribe a file
const transcription = await transcribeFile('/path/to/audio.wav');
```

## Events

The plugin emits the following events:

- `voice-transcription:dictation-started` - Emitted when dictation starts
- `voice-transcription:dictation-stopped` - Emitted when dictation stops
- `voice-transcription:partial-result` - Emitted with partial transcription results
- `voice-transcription:final-result` - Emitted with the final transcription

## Whisper Models

Download Whisper models from [whisper.cpp models](https://github.com/ggerganov/whisper.cpp/tree/master/models) or use the `download-models` feature:

```toml
tauri-plugin-voice-transcription = { version = "0.1.0", features = ["download-models"] }
```

Common models:
- `ggml-tiny.en.bin` - English only, ~39 MB
- `ggml-base.en.bin` - English only, ~74 MB
- `ggml-small.en.bin` - English only, ~150 MB
- `ggml-medium.en.bin` - English only, ~462 MB
- `ggml-large.bin` - Multilingual, ~1460 MB

## Requirements

- Rust 1.70+
- Tauri 2.0.0-beta+
- A valid Whisper model file
- Microphone permissions

## Platform Support

- ✅ Windows
- ✅ macOS
- ✅ Linux

## License

MIT OR Apache-2.0 

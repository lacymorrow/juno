# Juno Sound System Documentation

The Juno AI Computer Use Agent includes a comprehensive sound system that provides audio feedback for user interactions, agent operations, and voice controls.

## Overview

The sound system is built with a **centralized backend-driven architecture**:
- **Rust Backend**: Platform-specific audio playback with centralized control logic
- **TypeScript Frontend**: Minimal React hooks for UI-specific sounds only
- **Cross-Platform Support**: macOS (afplay), Windows (PowerShell), Linux (multiple players)
- **Platform-Specific Audio Formats**: 
  - **macOS**: CAF (Core Audio Format) for native support and reliability
  - **Other platforms**: OGG for cross-platform compatibility
- **Context-Aware Sound Selection**: Different sounds for different operation types
- **Duplicate Prevention**: Backend coordination prevents overlapping audio

## Available Sounds

### Hero Sounds (Celebrations & Achievements)
- `HeroSimpleCelebration01-03`: Simple celebration sounds for task completion
- `HeroDecorativeCelebration01-03`: More elaborate celebration sounds for major achievements

### Alerts & Notifications
- `AlertSimple`: Basic alert sound
- `AlertHighIntensity`: Urgent alert sound
- `NotificationSimple01-02`: Simple notification sounds
- `NotificationAmbient`: Gentle, ambient notification
- `NotificationDecorative01-02`: Rich notification sounds
- `NotificationHighIntensity`: Attention-grabbing notification
- `RingtoneMinimal`: Ringtone-style sound
- `AlarmGentle`: Gentle alarm sound

## Backend API (Rust)

### Tauri Commands

```rust
// Play a specific sound type
play_sound_by_type(sound_type: SoundType) -> Result<SoundPlayResult, String>

// Play a sound file by path
play_sound_file(file_path: String) -> Result<SoundPlayResult, String>

// Convenience functions
play_notification_sound() -> Result<SoundPlayResult, String>
play_success_sound() -> Result<SoundPlayResult, String>
play_error_sound() -> Result<SoundPlayResult, String>
play_alert_sound() -> Result<SoundPlayResult, String>

// Get available sounds
get_available_sounds() -> Result<Vec<SoundType>, String>
```

### Sound Types

```rust
pub enum SoundType {
    // Hero sounds
    HeroSimpleCelebration01,
    HeroSimpleCelebration02,
    HeroSimpleCelebration03,
    HeroDecorativeCelebration01,
    HeroDecorativeCelebration02,
    HeroDecorativeCelebration03,
    
    // Alerts and notifications
    AlertSimple,
    AlertHighIntensity,
    NotificationSimple01,
    NotificationSimple02,
    NotificationAmbient,
    NotificationDecorative01,
    NotificationDecorative02,
    NotificationHighIntensity,
    RingtoneMinimal,
    AlarmGentle,
}
```

### Platform Support

- **macOS**: Uses `afplay` command-line tool
- **Windows**: Uses PowerShell with `Media.SoundPlayer`
- **Linux**: Auto-detects available players (`paplay`, `aplay`, `mpg123`, `ffplay`)

## Frontend API (TypeScript)

### Basic Hook Usage

```typescript
import { useSound } from '../hooks/useSound';

const MyComponent = () => {
  const sound = useSound();
  
  const handleSuccess = async () => {
    const result = await sound.playSuccess();
    if (result.success) {
      console.log('Success sound played!');
    }
  };
  
  return (
    <button onClick={handleSuccess}>
      Play Success Sound
    </button>
  );
};
```

### Specialized Hooks

#### Agent Sounds
```typescript
import { useAgentSounds } from '../hooks/useSound';

const AgentComponent = () => {
  const agentSounds = useAgentSounds();
  
  // Play different sounds for agent states
  await agentSounds.playAgentStart();    // When agent starts processing
  await agentSounds.playAgentSuccess();  // When agent completes successfully
  await agentSounds.playAgentError();    // When agent encounters an error
  await agentSounds.playAgentAttention(); // When agent needs attention
};
```

#### Voice Sounds
```typescript
import { useVoiceSounds } from '../hooks/useSound';

const VoiceComponent = () => {
  const voiceSounds = useVoiceSounds();
  
  // Voice interaction sounds
  await voiceSounds.playVoiceStart();      // When voice input starts
  await voiceSounds.playVoiceEnd();        // When voice input ends
  await voiceSounds.playDictationStart();  // When dictation starts
  await voiceSounds.playDictationEnd();    // When dictation ends
};
```

### Direct Sound Type Usage

```typescript
import { useSound } from '../hooks/useSound';
import { SoundType } from '../types/sound';

const CustomSoundComponent = () => {
  const sound = useSound();
  
  const playCustomSound = async () => {
    const result = await sound.playSound(SoundType.NotificationAmbient);
    console.log('Sound result:', result);
  };
  
  const playCustomFile = async () => {
    const result = await sound.playSoundFile(
      'sounds/ogg/01 Hero Sounds/hero_simple-celebration-01.ogg'
    );
    console.log('File result:', result);
  };
};
```

## Integration Examples

### Agent Operations

```typescript
// In your agent processing code
const agentSounds = useAgentSounds();

const handleAgentQuery = async (query: string) => {
  // Play start sound
  await agentSounds.playAgentStart();
  
  try {
    const result = await processQuery(query);
    // Play success sound
    await agentSounds.playAgentSuccess();
    return result;
  } catch (error) {
    // Play error sound
    await agentSounds.playAgentError();
    throw error;
  }
};
```

### Voice Interaction

```typescript
// In your voice control code
const voiceSounds = useVoiceSounds();

const handleVoiceInput = async () => {
  // Start voice recording
  await voiceSounds.playVoiceStart();
  
  const transcript = await recordVoice();
  
  // End voice recording
  await voiceSounds.playVoiceEnd();
  
  return transcript;
};
```

### Dictation Mode

```typescript
// In your dictation feature
const voiceSounds = useVoiceSounds();

const handleDictation = async () => {
  // Start dictation
  await voiceSounds.playDictationStart();
  
  const text = await transcribeVoice();
  
  // Insert text at cursor
  await insertTextAtCursor(text);
  
  // End dictation
  await voiceSounds.playDictationEnd();
};
```

## File Structure

```
public/sounds/
├── ogg/
│   ├── 01 Hero Sounds/
│   │   ├── hero_simple-celebration-01.ogg
│   │   ├── hero_simple-celebration-02.ogg
│   │   └── ...
│   └── 02 Alerts and Notifications/
│       ├── alert_simple.ogg
│       ├── notification_simple-01.ogg
│       └── ...
└── caf/ (macOS-specific format)
    └── [same structure as ogg]

src/
├── types/
│   └── sound.ts              # TypeScript interfaces
├── hooks/
│   └── useSound.ts           # React hooks
└── components/
    └── SoundDemo.tsx         # Demo component

src-tauri/src/commands/
└── sound.rs                  # Rust implementation
```

## Configuration

The sound system automatically detects the platform and uses the appropriate audio player. No additional configuration is required.

### Sound File Format

- **Primary Format**: OGG Vorbis (cross-platform)
- **Alternative Format**: CAF (Core Audio Format for macOS)
- **Quality**: High-quality audio optimized for UI feedback

## Error Handling

The system gracefully handles errors:
- Missing audio files
- Unavailable audio players
- Platform-specific issues

All functions return a `SoundPlayResult` with success status and error messages.

## Performance

- **Lazy Loading**: Sounds are loaded on-demand
- **Non-Blocking**: Audio playback doesn't block the UI
- **Platform Optimized**: Uses native audio systems for best performance

## Testing

Use the `SoundDemo` component to test all available sounds:

```typescript
import { SoundDemo } from '../components/SoundDemo';

// Include in your app for testing
<SoundDemo />
```

This provides a comprehensive interface to test all sound types and functionality.

## Architecture Principles

### Backend-Driven Control ✅
- All primary sound logic is implemented in Rust backend
- Agent operations trigger sounds directly from `anthropic.rs`
- Application lifecycle sounds managed in `lib.rs`
- Prevents duplicate triggers and ensures coordination

### Context-Aware Sound Mapping
- `NotificationAmbient` - Gentle notifications and boot sounds
- `HeroDecorativeCelebration01` - Agent operation success
- `AlertHighIntensity` - Agent operation errors
- `AlarmGentle` - General alerts and warnings
- Voice-specific sounds - Dictation start/stop events

### Frontend Sound Usage ⚠️
- **Limited to UI-specific interactions only**
- Voice interaction feedback (start/stop recording)
- Error sounds for frontend-specific failures
- **Never duplicate backend sound triggers**

## Best Practices

1. **Backend First**: Implement sound logic in Rust backend when possible
2. **Context-Specific Sounds**: Use different sound types for different operations
3. **Avoid Duplication**: Never trigger the same sound from both frontend and backend
4. **Handle Errors**: Always check the `SoundPlayResult` for error handling
5. **Accessibility**: Provide visual feedback as alternatives to audio
6. **User Preferences**: Consider adding sound preferences/muting options

## Anti-Patterns to Avoid 🚫

```typescript
// DON'T: Frontend triggering sounds that backend also handles
const handleAgentResponse = (response) => {
  sound.playSuccess(); // ❌ Backend already handles this
  // Process response...
};

// DON'T: Multiple simultaneous sound triggers
sound.playNotification();
sound.playAlert(); // ❌ Creates overlapping audio

// DON'T: Same sound for different contexts
sound.playSuccess(); // For agent success
sound.playSuccess(); // For file save - should use different sound
```

## Future Enhancements

Potential future improvements:
- Volume control
- User-customizable sound sets
- Sound preferences/settings
- Additional audio formats
- 3D/spatial audio effects
- Sound queuing system 

# Wake Word Testing Tools - DevTools Panel ✅

## Overview

The DevTools panel now includes comprehensive wake word testing capabilities to debug and optimize Juno's always-listening mode. These tools provide real-time monitoring, configuration adjustment, and diagnostic capabilities.

## Features

### 📊 **Real-Time Status Monitoring**
- **Always Listening State**: Active/Inactive status with visual indicators
- **Live Sensitivity Display**: Current sensitivity value (0.1 - 2.0)
- **Wake Words List**: Currently configured wake words with management options
- **Audio Volume Monitoring**: Real-time RMS volume display
- **Event Log**: Timestamped activity feed showing all system events

### 🎛️ **Interactive Controls**

#### **Start/Stop Toggle**
- Single-click toggle for always listening mode
- Visual state indicators (green for active, red for inactive)
- Loading states with proper feedback

#### **Sensitivity Adjustment**
- Interactive range slider (0.1 - 2.0)
- Real-time updates without restart
- Visual markers for Low (0.1), Default (0.5), and High (2.0)
- Immediate effect on detection threshold

#### **Wake Word Management**
- **Add Wake Words**: Text input with instant addition
- **Remove Wake Words**: One-click deletion with confirmation
- **Live Updates**: Changes apply immediately to the detection system

### 🔍 **Advanced Debugging Tools**

#### **Debug Status Command**
- Comprehensive system status report
- Detailed plugin state information
- Audio system diagnostics
- Performance metrics and thresholds
- Console logging for detailed analysis

#### **Event Monitoring**
- Real-time event stream with timestamps
- Color-coded event types:
  - ✅ **Started/Stopped**: System state changes
  - 🎤 **Activated**: Wake word detection triggered
  - 📊 **Sensitivity**: Configuration changes
  - ➕/🗑️ **Wake Words**: List modifications
  - 🔍 **Debug**: Diagnostic information

### 📈 **Audio Analysis**
- **Volume Threshold Detection**: Visual feedback for activation levels
- **RMS Volume Display**: Current audio input levels
- **Detection Sensitivity**: Live adjustment and testing
- **Wake Word Recognition**: Real-time feedback on detection attempts

## Usage Guide

### **Getting Started**
1. Open Juno and navigate to DevTools
2. Locate the "Wake Word Testing" section at the top
3. Click "Start Always Listening" to begin monitoring

### **Testing Wake Words**
1. Ensure always listening is active (green status)
2. Adjust sensitivity if needed (start with 0.5)
3. Speak wake words clearly: "hey juno" or "computer"
4. Monitor the event log for detection feedback
5. Check audio volume levels if no detection occurs

### **Optimizing Detection**
1. **Low Detection Rate**: 
   - Increase sensitivity (0.8 - 1.2)
   - Speak louder or closer to microphone
   - Check audio input permissions
   
2. **False Positives**:
   - Decrease sensitivity (0.2 - 0.4)
   - Add more specific wake words
   - Check background noise levels

3. **No Detection**:
   - Use Debug Status to check system state
   - Verify microphone permissions
   - Check audio input device settings

### **Advanced Configuration**

#### **Custom Wake Words**
- Add domain-specific phrases
- Test with different voice tones
- Consider accent variations
- Use 2-3 syllable phrases for best results

#### **Sensitivity Tuning**
- **Quiet Environments**: 0.2 - 0.5
- **Normal Use**: 0.5 - 0.8  
- **Noisy Environments**: 0.8 - 1.5
- **High Sensitivity**: 1.5 - 2.0

## Technical Implementation

### **Frontend Components**
- **WakeWordTesting.tsx**: Main testing interface
- **Real-time Updates**: Event-driven state management
- **Loading States**: Proper feedback for all operations
- **Error Handling**: Comprehensive error reporting

### **Backend Integration**
- **Debug Command**: `debug_always_listening_status`
- **Plugin Communication**: Direct interface with voice transcription plugin
- **State Synchronization**: App state and plugin state alignment
- **Event Emission**: Real-time updates to frontend

### **Available Commands**
```typescript
// Start/Stop/Toggle
start_always_listening_mode()
stop_always_listening_mode()
toggle_always_listening_mode()

// Configuration
set_always_listening_sensitivity(sensitivity: number)
get_always_listening_sensitivity()
set_always_listening_wake_words(wakeWords: string[])
get_always_listening_wake_words()

// Status and Debugging
get_always_listening_status()
debug_always_listening_status()
```

## Event System

### **Event Types**
- `always-listening-event`: System state changes
- `volume-threshold-event`: Audio level monitoring
- `wake-word-detected`: Successful detection events
- `intent-detected`: User intent recognition

### **Event Payload Structure**
```typescript
interface AlwaysListeningEvent {
  type: 'started' | 'stopped' | 'activated' | 'deactivated' | 'wake_word_detected' | 'volume_threshold';
  payload: {
    sensitivity?: number;
    wakeWords?: string[];
    volume?: number;
    detectedWord?: string;
    timestamp: string;
  };
}
```

## Troubleshooting

### **Common Issues**

#### **Wake Words Not Triggering**
1. Check microphone permissions in System Settings
2. Verify audio input device is working
3. Use Debug Status to check plugin state
4. Increase sensitivity gradually
5. Test with different wake words

#### **High False Positive Rate**
1. Decrease sensitivity setting
2. Use more specific wake words
3. Check for background noise
4. Consider using noise cancellation

#### **System Performance Issues**
1. Check CPU usage during operation
2. Monitor memory consumption
3. Verify audio processing threads
4. Review debug logs for bottlenecks

### **Debug Information**
The Debug Status command provides:
- Plugin initialization state
- Audio system configuration
- Current detection parameters
- Recent detection attempts
- Performance metrics
- Error logs and warnings

## Best Practices

### **Wake Word Selection**
- Use 2-3 syllable phrases
- Avoid common words that appear in conversations
- Test with multiple voices and accents
- Consider household members' speech patterns

### **Sensitivity Configuration**
- Start with default (0.5) and adjust gradually
- Test in different acoustic environments
- Consider time-of-day variations (quiet vs. noisy)
- Monitor false positive/negative rates

### **Performance Optimization**
- Regular testing with Debug Status
- Monitor system resource usage
- Keep wake word list concise (2-5 words)
- Test after system updates or configuration changes

## Integration with Juno

The wake word testing tools integrate seamlessly with Juno's existing architecture:
- **Shared State Management**: Uses AppState for configuration persistence
- **Event-Driven Updates**: Real-time UI synchronization
- **Plugin Architecture**: Direct communication with voice transcription plugin
- **Error Handling**: Consistent error reporting and user feedback

This comprehensive testing suite ensures reliable wake word detection and provides developers with the tools needed to optimize the always-listening experience for users. 

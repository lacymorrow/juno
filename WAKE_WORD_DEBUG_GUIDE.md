# Wake Word Detection Debugging Guide 🔍

## **Critical Fixes Applied**

I've made important improvements to the wake word detection system:

### ✅ **Volume Threshold Lowered**
- **Changed**: `VOLUME_THRESHOLD` from `0.01` to `0.002` (5x more sensitive)
- **Why**: Previous threshold required shouting to activate
- **Impact**: Now activates with normal speaking voice

### ✅ **Audio Buffer Increased**
- **Changed**: `INTENT_DETECTION_BUFFER_MS` from `1500ms` to `3000ms`
- **Why**: Wake phrases like "hey juno" need more time to complete
- **Impact**: Better capture of complete wake word phrases

### ✅ **Enhanced Logging**
- **Added**: Detailed volume monitoring, transcription results, and wake word matching
- **Added**: Regular volume threshold reports every 5 seconds
- **Added**: Comprehensive transcription logging with success/failure details

---

## **Step-by-Step Debugging Process**

### **1. Open DevTools Panel**
1. Launch Juno
2. Navigate to DevTools (should be in the main interface)
3. Look for **"Wake Word Testing"** section at the top

### **2. Start Always Listening**
1. Click the **"Start Always Listening"** button (green play icon)
2. Check that status shows **"Active"** 
3. Verify sensitivity is set (default: 0.5)
4. Confirm wake words are configured: `["hey juno", "computer"]`

### **3. Check Debug Status**
1. Click **"Debug Status"** button
2. Review the comprehensive diagnostic information:
   - Plugin status and initialization
   - Current volume threshold calculations  
   - Audio device configuration
   - Whisper model status
   - Wake word configuration

### **4. Monitor Real-Time Activity**
Watch the **Event Log** in the DevTools panel for:

#### **Normal Monitoring Messages (Every 5 seconds):**
```
📊 Volume monitoring: 0.000234 < 0.001000 (threshold)
```

#### **Volume Threshold Exceeded:**
```
✅ Volume threshold exceeded: 0.002340 > 0.001000 (base: 0.002, sensitivity: 0.5)
```

#### **Wake Word Detection Attempts:**
```
🎤 Transcription result: 'hey juno can you help me' (length: 26)
✅ WAKE WORD DETECTED: 'hey juno' found in 'hey juno can you help me'
```

#### **Failed Detection:**
```
🎤 Transcription result: 'hello there' (length: 11)  
❌ No wake words detected in: 'hello there'
```

---

## **Common Issues & Solutions**

### **🔴 Issue: No Volume Detection**
**Symptoms:** No volume monitoring messages in event log
**Solutions:**
1. Check microphone permissions in System Settings
2. Verify audio input device is working in other apps
3. Try restarting the always listening mode
4. Check macOS Privacy & Security > Microphone permissions

### **🔴 Issue: Volume Too Low**
**Symptoms:** Seeing volume monitoring messages but all values are 0.000000
**Solutions:**
1. Speak closer to microphone
2. Increase microphone input volume in System Settings
3. Lower sensitivity in DevTools (try 0.3 or 0.2)
4. Check for microphone mute/hardware issues

### **🔴 Issue: Volume Exceeds Threshold But No Transcription**
**Symptoms:** Volume threshold exceeded messages but no transcription attempts
**Solutions:**
1. Check Whisper model is loaded (debug status)
2. Verify audio resampling is working
3. Look for transcription error messages
4. Try restarting the app to reload Whisper

### **🔴 Issue: Transcription Works But No Wake Words**
**Symptoms:** Getting transcription results but no wake word matches
**Solutions:**
1. Verify wake words are configured correctly
2. Try speaking more clearly
3. Test different wake words in DevTools
4. Check if transcription is picking up your words correctly

### **🔴 Issue: Intermittent Detection**
**Symptoms:** Wake words work sometimes but not consistently
**Solutions:**
1. Increase audio buffer size (try 4000ms)
2. Speak at consistent volume and pace
3. Minimize background noise
4. Ensure good microphone quality

---

## **Testing Commands**

### **Test Wake Words:**
- "hey juno"
- "computer"
- "hey juno can you help me"
- "computer take a screenshot" 

### **Adjust Settings:**
- **Sensitivity**: Start with 0.5, lower to 0.3 if too sensitive, raise to 0.8 if not sensitive enough
- **Wake Words**: Add custom wake words in DevTools panel
- **Test Volume**: Speak at different distances and volumes

---

## **Advanced Debugging**

### **Console Logs (Developer Tools)**
Open browser dev tools (F12) and check for:
- Always listening events being emitted
- Volume threshold calculations
- Transcription results and errors
- Plugin communication issues

### **System-Level Checks**
1. **Microphone Permissions**: System Settings > Privacy & Security > Microphone
2. **Audio Input**: Test microphone in other apps (Voice Memos, etc.)
3. **Background Apps**: Close other apps using microphone
4. **Audio Sample Rate**: Most microphones default to 44.1kHz (gets resampled to 16kHz)

### **Performance Monitoring**
- Check CPU usage during always listening
- Monitor memory usage over time
- Watch for audio dropouts or processing delays

---

## **Expected Behavior**

### **Successful Activation Sequence:**
1. ✅ Always listening starts
2. 📊 Regular volume monitoring (every 5s)
3. 🔊 Volume threshold exceeded
4. 🎤 Audio processing and transcription
5. ✅ Wake word detected
6. 🚀 Agent activation/response

### **Normal Operation:**
- Continuous background monitoring
- Periodic debug messages (every 5 seconds)
- Immediate response to volume changes
- Clear transcription results
- Reliable wake word matching

---

## **Getting Help**

If wake words still aren't working after following this guide:

1. **Share Debug Output**: Copy the debug status output from DevTools
2. **Share Event Log**: Copy recent entries from the wake word testing panel
3. **System Info**: Note your macOS version, microphone model, and audio setup
4. **Test Results**: What specific phrases you tried and what happened

The enhanced logging should now provide clear visibility into exactly what's happening in the wake word detection pipeline!

## Enhanced Debugging Tools Now Available

The wake word detection system now includes comprehensive debugging tools to help diagnose why wake words aren't triggering.

## Current Status

**Problem**: Always listening mode is running but wake words are not triggering activation.

**Symptoms**: 
- Empty transcription results in logs
- System shows as active but doesn't respond to wake words
- Volume levels seem to be detected but no transcription occurs

## Enhanced Debugging Features

### 1. Wake Word Testing Panel (DevTools)

Location: `src/components/devtools/WakeWordTesting.tsx`

**New Features Added**:
- **Transcription Debugging**: Real-time monitoring of transcription pipeline
- **Audio Level Monitoring**: Live audio volume level tracking
- **Whisper Model Testing**: Test the Whisper model with synthetic audio
- **Force Transcription Test**: Trigger immediate transcription of live audio
- **Recent Transcriptions Log**: See last 10 transcription attempts with details
- **Enhanced Event Monitoring**: Track all wake word detection events

### 2. Backend Debugging Commands

New commands added to diagnose transcription issues:

#### Available Debug Commands:
```bash
# Enable transcription debugging
invoke("set_transcription_debugging", { enabled: true })

# Enable audio level monitoring  
invoke("set_audio_level_monitoring", { enabled: true })

# Test Whisper model with synthetic audio
invoke("test_whisper_model")

# Force transcription test with live audio
invoke("force_transcription_test")
```

### 3. Enhanced Logging and Events

The system now emits detailed events for debugging:

- `always-listening-event` with transcription debug info
- Audio level monitoring events
- Transcription confidence scores
- Wake word matching results (exact and fuzzy)

## Testing the New Features

### 1. Access Enhanced DevTools

1. Open Juno application
2. Click the DevTools panel button (or press Cmd+D from tray)
3. Navigate to the "Wake Word Testing" section
4. You'll see the new debugging controls

### 2. Run Whisper Model Test

1. Click "Test Whisper Model" button
2. This creates synthetic audio (silence + beep) and tests if Whisper can transcribe it
3. Check the console for results - this tells us if the Whisper model is working

### 3. Enable Transcription Debugging

1. Toggle "Transcription Debugging" to ON
2. This will log every transcription attempt with:
   - Audio length
   - Volume level
   - Transcription result (or empty)
   - Confidence scores
   - Wake word matches

### 4. Monitor Audio Levels

1. Toggle "Audio Level Monitoring" to ON
2. Speak near your microphone
3. Watch the "Recent Events" log for volume level reports
4. This confirms audio is being captured

### 5. Force Transcription Test

1. Ensure always listening is active
2. Click "Force Transcription Test"
3. Speak immediately (the system will try to transcribe whatever audio it's currently capturing)
4. Check logs for transcription results

## Root Cause Analysis

Based on the logs showing empty transcription results, the issue is likely:

### Most Likely Causes:

1. **Whisper Model Issues**:
   - Model file corruption or incorrect format
   - Model not loading properly
   - Wrong model parameters

2. **Audio Processing Issues**:
   - Audio format conversion problems
   - Sample rate mismatch (should be 16kHz for Whisper)
   - Audio buffer too short/empty

3. **Whisper API Usage**:
   - Incorrect parameter settings
   - Thread configuration issues
   - Language settings

### Diagnostic Steps:

1. **Test Whisper Model**: Use the "Test Whisper Model" button to verify the model works with synthetic audio
2. **Check Audio Pipeline**: Enable audio level monitoring to confirm audio is being captured
3. **Monitor Transcription**: Enable transcription debugging to see what's happening during transcription attempts
4. **Force Test**: Use force transcription test to manually trigger transcription

## Key Files Modified

### Frontend:
- `src/components/devtools/WakeWordTesting.tsx` - Enhanced with new debugging UI
- `src/types/devtools.ts` - Added types for debugging interfaces

### Backend:
- `tauri-plugin-voice-transcription/src/always_listening.rs` - Added debugging methods
- `tauri-plugin-voice-transcription/src/commands.rs` - Added debug commands
- `src-tauri/src/commands/always_listening.rs` - Added wrapper commands
- `src-tauri/src/lib.rs` - Registered new commands

## Next Steps

1. **Run the Whisper Model Test** to verify if the model initialization is working
2. **Enable transcription debugging** to see detailed logs of what's happening
3. **Check the transcription pipeline** - if Whisper model test fails, the issue is with model loading
4. **If model test passes but live transcription fails**, the issue is with audio processing

The enhanced debugging tools should help pinpoint exactly where in the pipeline the issue occurs.

## Expected Behavior

When working correctly:
- Whisper model test should transcribe the synthetic beep (may be empty/silence, but shouldn't error)
- Audio level monitoring should show volume spikes when speaking
- Transcription debugging should show non-empty transcription results when speaking wake words
- Wake word detection should activate and emit activation events

## Troubleshooting Commands

```bash
# Check always listening status with detailed debug info
invoke("debug_always_listening_status")

# Test just the model loading
invoke("test_whisper_model") 

# Check if audio is being captured
invoke("set_audio_level_monitoring", { enabled: true })

# See transcription attempts in real-time
invoke("set_transcription_debugging", { enabled: true })
```

Use these tools to systematically debug the wake word detection pipeline and identify where the failure is occurring. 

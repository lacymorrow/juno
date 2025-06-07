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

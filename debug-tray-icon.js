// Quick debug script to check and fix tray icon state
// Run this in the browser console or create a button to execute it

async function debugTrayIconState() {
    const { invoke } = window.__TAURI__.tauri;
    
    try {
        // Check current always listening status
        const isAlwaysListening = await invoke('get_always_listening_status');
        console.log('🔍 Always Listening Mode:', isAlwaysListening ? 'ENABLED' : 'DISABLED');
        
        if (isAlwaysListening) {
            console.log('⚠️  Always Listening is enabled - this causes the green tray icon');
            console.log('💡 To disable and show white icon when idle, run: disableAlwaysListening()');
        }
        
        // Get debug info
        const debugInfo = await invoke('debug_always_listening_status');
        console.log('📊 Debug Info:', debugInfo);
        
    } catch (error) {
        console.error('Error checking state:', error);
    }
}

// Function to disable always listening mode
async function disableAlwaysListening() {
    const { invoke } = window.__TAURI__.tauri;
    
    try {
        await invoke('stop_always_listening_mode');
        console.log('✅ Always Listening mode disabled - tray icon should now show white when idle');
    } catch (error) {
        console.error('Error disabling always listening:', error);
    }
}

// Run the debug function
debugTrayIconState();
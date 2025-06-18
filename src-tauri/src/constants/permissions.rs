//! # Permission Constants
//!
//! Permission types, descriptions, and instructions.

// Permission types
pub mod types {
    pub const ACCESSIBILITY: &str = "accessibility";
    pub const SCREEN_RECORDING: &str = "screen_recording";
    pub const MICROPHONE: &str = "microphone";
    pub const INPUT_MONITORING: &str = "input_monitoring";
}



// Permission descriptions
pub mod descriptions {
    pub const ACCESSIBILITY_DESC: &str = "Juno requires accessibility permissions to automate desktop tasks and interact with applications on your behalf.";
    pub const MICROPHONE_DESC: &str = "Juno uses the microphone for voice transcription and voice commands.";
    pub const APPLE_EVENTS_DESC: &str = "Juno uses Apple Events to control and automate applications.";
    pub const SCREEN_RECORDING_DESC: &str = "Juno needs screen capture permissions to take screenshots and analyze the desktop for automation tasks.";
    pub const INPUT_MONITORING_DESC: &str = "Juno needs input monitoring permissions to register global keyboard shortcuts for voice control and automation features.";
}

// Permission instructions
pub mod instructions {
    pub const ACCESSIBILITY_INSTRUCTIONS: &str = "Go to System Preferences > Privacy & Security > Accessibility and add Juno";
    pub const SCREEN_RECORDING_INSTRUCTIONS: &str = "Go to System Preferences > Privacy & Security > Screen Recording and add Juno";
    pub const MICROPHONE_INSTRUCTIONS: &str = "Go to System Preferences > Privacy & Security > Microphone and add Juno";
    pub const INPUT_MONITORING_INSTRUCTIONS: &str = "Optional: Go to System Preferences > Privacy & Security > Input Monitoring and add Juno to enable global shortcuts";
}

// System preferences URLs
pub mod urls {
    pub const MICROPHONE_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
    pub const SCREEN_RECORDING_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
    pub const INPUT_MONITORING_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";
    pub const ACCESSIBILITY_PRIVACY_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
}



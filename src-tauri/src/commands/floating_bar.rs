use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Listener, State};

use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use tracing::{debug, error, warn, info};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::constants::{timeouts, events};
use crate::state::AppState;
use crate::settings::{manager::SettingsManager, FloatingBarSettings};

// Floating bar configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingBarConfig {
    pub show_voice_indicator: bool,
    pub enable_animations: bool,
    pub auto_hide: bool,
    pub auto_hide_delay: u32,
    pub opacity: f32,
}

impl Default for FloatingBarConfig {
    fn default() -> Self {
        Self {
            show_voice_indicator: true,
            enable_animations: true,
            auto_hide: false,
            auto_hide_delay: crate::constants::timeouts::UI_NOTIFICATION_DISPLAY_MS as u32,
            opacity: 0.95,
        }
    }
}

impl FloatingBarConfig {
    /// Load configuration from centralized settings or create default.
    /// Uses centralized SettingsManager instead of individual JSON store.
    /// Used by: Application startup and configuration management.
    pub async fn load_from_centralized_settings(app_handle: &AppHandle) -> Result<Self, String> {
        let settings_manager = SettingsManager::new(app_handle.clone())
            .map_err(|e| format!("Failed to create settings manager: {}", e))?;

        let load_result = settings_manager.get_floating_bar_settings().await;

        if let Ok(settings) = load_result {
            debug!("Loaded floating bar configuration from centralized settings");
            return Ok(convert_settings_to_config(&settings));
        }

        // Handle error case
        debug!("Failed to load floating bar settings, creating default");
        let default_config = Self::default();
        default_config.save_to_centralized_settings(app_handle).await?;
        Ok(default_config)
    }

    /// Save configuration to centralized settings.
    /// Uses centralized SettingsManager instead of individual JSON store.
    /// Used by: Settings UI and configuration updates.
    pub async fn save_to_centralized_settings(&self, app_handle: &AppHandle) -> Result<(), String> {
        let settings_manager = SettingsManager::new(app_handle.clone())
            .map_err(|e| format!("Failed to create settings manager: {}", e))?;

        let settings = convert_config_to_settings(self);
        settings_manager.set_floating_bar_settings(&settings).await
            .map_err(|e| format!("Failed to save floating bar settings: {}", e))?;

        debug!("Saved floating bar configuration to centralized settings");
        Ok(())
    }


}

/// Convert centralized FloatingBarSettings to FloatingBarConfig
/// Provides seamless integration between centralized settings and existing code.
fn convert_settings_to_config(settings: &FloatingBarSettings) -> FloatingBarConfig {
    FloatingBarConfig {
        show_voice_indicator: settings.show_voice_indicator,
        enable_animations: settings.enable_animations,
        auto_hide: settings.auto_hide,
        auto_hide_delay: settings.auto_hide_delay,
        opacity: settings.opacity,
    }
}

/// Convert FloatingBarConfig to centralized FloatingBarSettings
/// Provides seamless integration between existing code and centralized settings.
fn convert_config_to_settings(config: &FloatingBarConfig) -> FloatingBarSettings {
    FloatingBarSettings {
        show_voice_indicator: config.show_voice_indicator,
        enable_animations: config.enable_animations,
        auto_hide: config.auto_hide,
        auto_hide_delay: config.auto_hide_delay,
        opacity: config.opacity,
    }
}

// Bar states that match the frontend
#[derive(Debug, Clone, PartialEq)]
pub enum BarState {
    Default,
    Expanding,
    Input,
    Shrinking,
    Loading,
    Finishing,
    Success,
    Listening,
    Error,
    Transcribing,
    Speaking,
    Dictating,
    AlwaysListening,
    // New agent-specific states
    AgentListening,
    AgentThinking,
    AgentResponding,
    DictationReady,
    DictationActive,
    DictationProcessing,
}

impl BarState {
    fn as_str(&self) -> &'static str {
        match self {
            BarState::Default => "default",
            BarState::Expanding => "expanding",
            BarState::Input => "input",
            BarState::Shrinking => "shrinking",
            BarState::Loading => "loading",
            BarState::Finishing => "finishing",
            BarState::Success => "success",
            BarState::Listening => "listening",
            BarState::Error => "error",
            BarState::Transcribing => "transcribing",
            BarState::Speaking => "speaking",
            BarState::Dictating => "dictating",
            BarState::AlwaysListening => "always-listening",
            // New agent-specific states
            BarState::AgentListening => "agent_listening",
            BarState::AgentThinking => "agent_thinking",
            BarState::AgentResponding => "agent_responding",
            BarState::DictationReady => "dictation_ready",
            BarState::DictationActive => "dictation_active",
            BarState::DictationProcessing => "dictation_processing",
        }
    }
}

// Central floating bar state manager
pub struct FloatingBarManager {
    current_state: BarState,
    input_value: String,
    last_submitted_value: String,
    current_error: Option<String>,
    transcription_text: String,
    spoken_text: String,
    is_agent_working: bool,
    is_dictation_mode: bool,
    is_always_listening: bool,
    audio_level: f32,
    voice_mode: String, // "idle", "agent", "dictation"
    agent_state: Option<String>, // "Finished", "Failed", "Cancelled", "Offline"
    app_handle: AppHandle,
    current_transition_id: Option<String>,
}

impl FloatingBarManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            current_state: BarState::Default,
            input_value: String::new(),
            last_submitted_value: String::new(),
            current_error: None,
            transcription_text: String::new(),
            spoken_text: String::new(),
            is_agent_working: false,
            is_dictation_mode: false,
            is_always_listening: false,
            audio_level: 0.0,
            voice_mode: "idle".to_string(),
            agent_state: None,
            app_handle,
            current_transition_id: None,
        }
    }

    // Emit state update to frontend
    async fn emit_state_update(&self) {
        let state_data = serde_json::json!({
            "barState": self.current_state.as_str(),
            "inputValue": self.input_value,
            "lastSubmittedValue": self.last_submitted_value,
            "currentError": self.current_error,
            "transcriptionText": self.transcription_text,
            "spokenText": self.spoken_text,
            "isAgentWorking": self.is_agent_working,
            "isDictationMode": self.is_dictation_mode,
            "isAlwaysListening": self.is_always_listening,
            "audioLevel": self.audio_level,
            "voiceMode": self.voice_mode,
            "agentState": self.agent_state,
        });

        if let Err(e) = self.app_handle.emit("bar-state-update", state_data) {
            error!("Failed to emit bar-state-update: {}", e);
        }
    }

    // Set state and emit update
    async fn set_state(&mut self, new_state: BarState) {
        debug!("FloatingBarManager: State changing from {:?} to {:?}", self.current_state, new_state);
        self.current_state = new_state;
        self.emit_state_update().await;
    }

    // Handle user clicking on the bar
    pub async fn handle_click(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling bar click, current state: {:?}", self.current_state);

        // Only allow expansion from default state when agent is not working
        if self.current_state != BarState::Default || self.is_agent_working {
            return Ok(());
        }

        // Start expansion
        self.set_state(BarState::Expanding).await;

        // After animation, transition to input using safe spawning
        let app_handle = self.app_handle.clone();
        safe_spawn_async_task(move || async move {
            sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
            if let Some(manager) = get_bar_manager(&app_handle).await {
                let mut manager = manager.lock().await;
                manager.set_state(BarState::Input).await;
            }
        });

        Ok(())
    }

    // Handle window focus change
    pub async fn handle_focus_change(&mut self, is_focused: bool) -> Result<(), String> {
        debug!("FloatingBarManager: Handling focus change, focused: {}, current state: {:?}", is_focused, self.current_state);

        // Never change state if agent is working
        if self.should_remain_expanded_for_status() {
            debug!("FloatingBarManager: Agent is working, preserving state");
            return Ok(());
        }

        if is_focused {
            // Automatically expand to input state when window gains focus (like clicking)
            // Only allow expansion from default state when agent is not working
            if self.current_state == BarState::Default && !self.is_agent_working {
                debug!("FloatingBarManager: Window gained focus, expanding to input state");

                // FIXED: Consolidated state transition to avoid race conditions
                // First set expanding state, then schedule input state after animation
                self.set_state(BarState::Expanding).await;

                // Store transition target to prevent race conditions with other operations
                let transition_id = Uuid::new_v4().to_string();
                self.current_transition_id = Some(transition_id.clone());

                // After animation, transition to input (if no other transitions happened)
                let app_handle = self.app_handle.clone();
                safe_spawn_async_task(move || async move {
                    sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
                    if let Some(manager) = get_bar_manager(&app_handle).await {
                        let mut manager = manager.lock().await;

                        // Only proceed if this is still the active transition
                        if manager.current_transition_id.as_ref() == Some(&transition_id) {
                            manager.set_state(BarState::Input).await;
                            manager.current_transition_id = None;
                        }
                    }
                });
            } else {
                debug!("FloatingBarManager: Window gained focus, but not in default state or agent is working");
            }
        } else {
            // When window loses focus, shrink if input is empty and agent is idle
            if self.current_state == BarState::Input && self.input_value.trim().is_empty() {
                self.handle_input_blur().await?;
            }
        }

        Ok(())
    }

    // Handle input blur
    pub async fn handle_input_blur(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling input blur, current state: {:?}", self.current_state);

        // Only shrink if in input state and input is empty and agent is not working
        if self.current_state == BarState::Input && self.input_value.trim().is_empty() && !self.should_remain_expanded_for_status() {
            // FIXED: Consolidated state transition to avoid race conditions
            self.set_state(BarState::Shrinking).await;

            // Store transition target to prevent race conditions with other operations
            let transition_id = Uuid::new_v4().to_string();
            self.current_transition_id = Some(transition_id.clone());

            // After animation, return to default (if no other transitions happened)
            let app_handle = self.app_handle.clone();
            safe_spawn_async_task(move || async move {
                sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
                if let Some(manager) = get_bar_manager(&app_handle).await {
                    let mut manager = manager.lock().await;

                    // Only proceed if this is still the active transition
                    if manager.current_transition_id.as_ref() == Some(&transition_id) {
                        manager.input_value.clear();
                        manager.set_state(BarState::Default).await;
                        manager.current_transition_id = None;
                    }
                }
            });
        }

        Ok(())
    }

    // Handle input value change
    pub async fn handle_input_change(&mut self, new_value: String) -> Result<(), String> {
        debug!("FloatingBarManager: Input changed to: '{}'", new_value);
        self.input_value = new_value;
        self.emit_state_update().await;
        Ok(())
    }

    // Handle query submission
    pub async fn handle_submit(&mut self, query: String) -> Result<(), String> {
        debug!("FloatingBarManager: Handling query submission: '{}'", query);

        if query.trim().is_empty() {
            return Ok(());
        }

        self.last_submitted_value = query.clone();
        self.input_value.clear();
        self.current_error = None;
        self.agent_state = None; // Clear agent state for new task
        self.is_agent_working = true;

        // FIXED: Consolidated state transition to avoid race conditions
        // Show success state briefly, then transition directly to loading (stay expanded)
        self.set_state(BarState::Success).await;

        // Store transition target to prevent race conditions with other operations
        let transition_id = Uuid::new_v4().to_string();
        self.current_transition_id = Some(transition_id.clone());

        // Transition directly to loading without shrinking
        let app_handle = self.app_handle.clone();
        let query_for_agent = query.clone();
        safe_spawn_async_task(move || async move {
            sleep(Duration::from_millis(timeouts::UI_SLIDE_DELAY_MS)).await;
            if let Some(manager) = get_bar_manager(&app_handle).await {
                let mut manager = manager.lock().await;

                // Only proceed if this is still the active transition
                if manager.current_transition_id.as_ref() == Some(&transition_id) {
                    // Skip shrinking, go directly to loading to keep bar expanded
                    manager.set_state(BarState::Loading).await;
                    manager.current_transition_id = None;

                    // Trigger the AI agent using safe spawning
                    let app_handle_for_agent = app_handle.clone();
                    safe_spawn_async_task(move || async move {
                        let app_handle_clone = app_handle_for_agent.clone();
                        let state = app_handle_for_agent.state::<crate::state::AppState>();
                        if let Err(e) = crate::anthropic::submit_query(query_for_agent, state, app_handle_clone).await {
                            error!("Failed to submit query to AI agent: {}", e);
                            // Handle the error by updating the floating bar
                            if let Some(manager) = get_bar_manager(&app_handle).await {
                                let mut manager = manager.lock().await;
                                let _ = manager.handle_agent_completion("Failed", Some(e)).await;
                            }
                        }
                    });
                }
            }
        });

        Ok(())
    }

    // Handle agent completion
    pub async fn handle_agent_completion(&mut self, agent_state: &str, response_text: Option<String>) -> Result<(), String> {
        debug!("FloatingBarManager: Handling agent completion with state: {}", agent_state);

        self.is_agent_working = false;

        // Reset input values regardless of completion state
        self.input_value.clear();
        self.last_submitted_value.clear();
        self.transcription_text.clear();
        self.spoken_text.clear();

        // Store the agent state for frontend to use in determining success/failure messages
        self.agent_state = Some(agent_state.to_string());

        // FIXED: Consolidated state transitions to avoid race conditions
        // Store transition target to prevent race conditions with other operations
        let transition_id = Uuid::new_v4().to_string();
        self.current_transition_id = Some(transition_id.clone());

        match agent_state {
            "Finished" => {
                self.set_state(BarState::Finishing).await;

                // Schedule completion state transition without recursive manager access
                let app_handle = self.app_handle.clone();
                let transition_id_clone = transition_id.clone();
                safe_spawn_async_task(move || async move {
                    sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
                    // Emit event to complete transition instead of direct manager access
                    let _ = app_handle.emit("floating-bar-complete-transition", transition_id_clone);
                });
            }
            "Failed" | "Cancelled" | "Offline" => {
                self.current_error = Some(
                    if agent_state == "Cancelled" {
                        "Agent execution was cancelled".to_string()
                    } else if agent_state == "Offline" {
                        "Connection unavailable".to_string()
                    } else {
                        format!("Agent failed: {}", response_text.unwrap_or_default())
                    }
                );
                self.set_state(BarState::Error).await;

                // Schedule error state cleanup without recursive manager access
                let app_handle = self.app_handle.clone();
                let transition_id_clone = transition_id.clone();
                safe_spawn_async_task(move || async move {
                    sleep(Duration::from_millis(timeouts::UI_NOTIFICATION_DISPLAY_MS)).await;
                    // Emit event to clear error state instead of direct manager access
                    let _ = app_handle.emit("floating-bar-clear-error", transition_id_clone);
                });
            }
            _ => {
                self.set_state(BarState::Default).await;
                self.current_transition_id = None;
            }
        }

        Ok(())
    }

    // Handle dictation events
    pub async fn handle_dictation_started(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation started");
        self.input_value.clear();
        self.transcription_text.clear();

        // Set appropriate initial state based on dictation mode
        // If dictation mode is active, go directly to Dictating (orange)
        // Otherwise, go to Listening (blue) for agent mode
        if self.is_dictation_mode {
            self.voice_mode = "dictation".to_string();
            self.set_state(BarState::DictationActive).await;
        } else {
            self.voice_mode = "agent".to_string();
            self.is_agent_working = true;
            self.set_state(BarState::AgentListening).await;
        }

        Ok(())
    }

    pub async fn handle_dictation_partial(&mut self, partial_text: String) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation partial: '{}'", partial_text);
        self.transcription_text = partial_text;

        if self.current_state == BarState::AgentListening {
            self.set_state(if self.is_dictation_mode { BarState::DictationProcessing } else { BarState::Transcribing }).await;
        }

        self.emit_state_update().await;
        Ok(())
    }

    pub async fn handle_dictation_finished(&mut self, query: Option<String>) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation finished with query: {:?}", query);

        if let Some(query_text) = query {
            self.last_submitted_value = query_text.clone();
            self.input_value = query_text;

            if self.is_dictation_mode {
                // In dictation mode, return to default after brief processing
                self.voice_mode = "idle".to_string();
                self.set_state(BarState::Default).await;
            } else {
                // In agent mode, continue with agent processing
                self.voice_mode = "agent".to_string();
                self.is_agent_working = true;
                self.set_state(BarState::AgentThinking).await;
            }
        } else {
            // No query, return to default
            self.voice_mode = "idle".to_string();
            self.transcription_text.clear();
            self.set_state(BarState::Default).await;
        }

        Ok(())
    }

    // Handle TTS events
    pub async fn handle_tts_started(&mut self, text: String) -> Result<(), String> {
        debug!("FloatingBarManager: Handling TTS started with text: '{}'", text);
        self.spoken_text = text;
        self.set_state(BarState::Speaking).await;
        Ok(())
    }

    pub async fn handle_tts_finished(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling TTS finished");
        self.spoken_text.clear();
        self.set_state(BarState::Input).await;
        Ok(())
    }

    // Handle dictation mode changes
    pub async fn handle_dictation_mode_change(&mut self, is_active: bool) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation mode change: {}", is_active);
        self.is_dictation_mode = is_active;

        if is_active {
            self.voice_mode = "dictation".to_string();
            self.set_state(BarState::Dictating).await;
        } else {
            self.voice_mode = "idle".to_string();
            // When dictation mode becomes inactive, return to default state
            // This ensures the orange UI disappears when keys are released before threshold
            if self.current_state == BarState::Dictating {
                self.set_state(BarState::Default).await;
            }
        }

        Ok(())
    }

    // Handle always listening mode changes
    pub async fn handle_always_listening_change(&mut self, is_active: bool) -> Result<(), String> {
        debug!("FloatingBarManager: Handling always listening mode change: {}", is_active);
        self.is_always_listening = is_active;

        if is_active && self.current_state == BarState::Default {
            self.set_state(BarState::AlwaysListening).await;
        } else if !is_active && self.current_state == BarState::AlwaysListening {
            self.set_state(BarState::Default).await;
        }

        Ok(())
    }

    // Handle agent status changes
    pub async fn handle_agent_started(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling agent started");
        self.is_agent_working = true;
        self.voice_mode = "agent".to_string();
        self.set_state(BarState::AgentListening).await;
        Ok(())
    }

    pub async fn handle_agent_stopped(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling agent stopped");
        self.is_agent_working = false;
        self.voice_mode = "idle".to_string();
        // Return to default state unless we're in a specific state that should be preserved
        if matches!(self.current_state, BarState::AgentListening | BarState::AgentThinking | BarState::AgentResponding | BarState::Listening | BarState::Transcribing) {
            self.set_state(BarState::Default).await;
        }
        Ok(())
    }

    pub async fn handle_agent_cancelled(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling agent cancelled");
        self.is_agent_working = false;
        self.voice_mode = "idle".to_string();
        self.is_dictation_mode = false; // Also reset dictation mode
        // Clear any transcription text and return to default
        self.transcription_text.clear();
        self.input_value.clear(); // Clear any input value
        self.last_submitted_value.clear(); // Clear submitted value
        self.current_error = None; // Clear any errors
        self.set_state(BarState::Default).await;
        Ok(())
    }

    // Helper to check if bar should remain expanded
    fn should_remain_expanded_for_status(&self) -> bool {
        matches!(self.current_state,
            BarState::Loading | BarState::Finishing | BarState::Success |
            BarState::Speaking | BarState::Listening | BarState::Transcribing |
            BarState::Dictating | BarState::AlwaysListening | BarState::Error |
            BarState::AgentListening | BarState::AgentThinking | BarState::AgentResponding |
            BarState::DictationReady | BarState::DictationActive | BarState::DictationProcessing
        ) || self.is_agent_working
    }
}

// Static storage for the bar manager
static BAR_MANAGER: TokioMutex<Option<Arc<TokioMutex<FloatingBarManager>>>> = TokioMutex::const_new(None);

// Initialize the bar manager with event listeners
pub async fn initialize_bar_manager(app_handle: AppHandle) {
    let manager = Arc::new(TokioMutex::new(FloatingBarManager::new(app_handle.clone())));
    let mut global_manager = BAR_MANAGER.lock().await;
    *global_manager = Some(manager.clone());

    // Set up event listeners for agent status changes
    setup_agent_event_listeners(app_handle, manager).await;
}

// Import the safe async spawning utility
use crate::utils::async_runtime::safe_spawn_async_task;

// Set up event listeners for agent status changes
async fn setup_agent_event_listeners(app_handle: AppHandle, manager: Arc<TokioMutex<FloatingBarManager>>) {
    debug!("FloatingBarManager: Setting up agent event listeners");

    // Listen for delayed transitions with timeout protection
    let manager_clone = manager.clone();
    let app_handle_clone = app_handle.clone();
    app_handle.listen("floating-bar-delayed-transition", move |event| {
        let manager = manager_clone.clone();
        let app_handle = app_handle_clone.clone();

        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;

            // Parse timeout and transition ID from event payload
            let (timeout_ms, transition_id) = match serde_json::from_str::<serde_json::Value>(event.payload()) {
                Ok(payload) => {
                    let timeout = payload.get("timeout_ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(750) as u64;
                    let id = payload.get("transition_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (timeout, id)
                }
                Err(_) => (750, None), // Default values
            };

            // Only proceed if this is still the active transition
            if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                sleep(Duration::from_millis(timeout_ms)).await;

                // Double-check the transition ID after the delay
                if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                    manager.set_state(BarState::Input).await;
                }
            }
        });
    });

    // Listen for voice-transcription:final-result to handle dictation completion
    let manager_clone = manager.clone();
    app_handle.listen("voice-transcription:final-result", move |event| {
        let manager = manager_clone.clone();

        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;

            // Parse the final result to extract query text
            let payload_str = event.payload();
            let extracted_text = match serde_json::from_str::<serde_json::Value>(payload_str) {
                Ok(payload_json) => {
                    payload_json.get("text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                }
                Err(_) => None,
            };

            if let Err(e) = manager.handle_dictation_finished(extracted_text).await {
                error!("Failed to handle dictation finished: {}", e);
            }
        });
    });

    // Listen for error state cleanup
    let manager_clone = manager.clone();
    app_handle.listen("floating-bar-clear-error", move |event| {
        let manager = manager_clone.clone();

        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;

            // Parse transition ID from event payload
            let transition_id = if let Ok(payload) = serde_json::from_str::<String>(event.payload()) {
                Some(payload)
            } else {
                None
            };

            // Only proceed if this is still the active transition
            if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                manager.current_error = None;
                manager.set_state(BarState::Default).await;
                manager.current_transition_id = None;
            }
        });
    });

    // Listen for completion state transition
    let manager_clone = manager.clone();
    app_handle.listen("floating-bar-complete-transition", move |event| {
        let manager = manager_clone.clone();

        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;

            // Parse transition ID from event payload
            let transition_id = if let Ok(payload) = serde_json::from_str::<String>(event.payload()) {
                Some(payload)
            } else {
                None
            };

            // Only proceed if this is still the active transition
            if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                manager.set_state(BarState::Default).await;
                manager.current_transition_id = None;
            }
        });
    });

    debug!("FloatingBarManager: Agent event listeners set up successfully");
}

// Get the bar manager
async fn get_bar_manager(app_handle: &AppHandle) -> Option<Arc<TokioMutex<FloatingBarManager>>> {
    let global_manager = BAR_MANAGER.lock().await;
    if let Some(manager) = global_manager.as_ref() {
        Some(manager.clone())
    } else {
        warn!("Bar manager not initialized, initializing now");
        drop(global_manager);
        initialize_bar_manager(app_handle.clone()).await;
        let global_manager = BAR_MANAGER.lock().await;
        global_manager.as_ref().cloned()
    }
}

// Tauri commands for frontend to call

#[tauri::command]
pub async fn floating_bar_click(app: AppHandle) -> Result<(), String> {
    if let Some(manager) = get_bar_manager(&app).await {
        let mut manager = manager.lock().await;
        manager.handle_click().await
    } else {
        Err("Bar manager not available".to_string())
    }
}

#[tauri::command]
pub async fn floating_bar_focus_change(app: AppHandle, is_focused: bool) -> Result<(), String> {
    if let Some(manager) = get_bar_manager(&app).await {
        let mut manager = manager.lock().await;
        manager.handle_focus_change(is_focused).await
    } else {
        Err("Bar manager not available".to_string())
    }
}

#[tauri::command]
pub async fn floating_bar_input_blur(app: AppHandle) -> Result<(), String> {
    if let Some(manager) = get_bar_manager(&app).await {
        let mut manager = manager.lock().await;
        manager.handle_input_blur().await
    } else {
        Err("Bar manager not available".to_string())
    }
}

#[tauri::command]
pub async fn floating_bar_input_change(app: AppHandle, value: String) -> Result<(), String> {
    if let Some(manager) = get_bar_manager(&app).await {
        let mut manager = manager.lock().await;
        manager.handle_input_change(value).await
    } else {
        Err("Bar manager not available".to_string())
    }
}

#[tauri::command]
pub async fn floating_bar_submit(app: AppHandle, query: String) -> Result<(), String> {
    if let Some(manager) = get_bar_manager(&app).await {
        let mut manager = manager.lock().await;
        manager.handle_submit(query).await
    } else {
        Err("Bar manager not available".to_string())
    }
}

// Event handlers for backend events
pub async fn handle_backend_response(app_handle: &AppHandle, agent_state: &str, response_text: Option<String>) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_completion(agent_state, response_text).await {
            error!("Failed to handle agent completion: {}", e);
        }
    }
}

pub async fn handle_dictation_started(app_handle: &AppHandle) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_started().await {
            error!("Failed to handle dictation started: {}", e);
        }
    }
}

pub async fn handle_dictation_partial(app_handle: &AppHandle, partial_text: String) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_partial(partial_text).await {
            error!("Failed to handle dictation partial: {}", e);
        }
    }
}

pub async fn handle_dictation_finished(app_handle: &AppHandle, query: Option<String>) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_finished(query).await {
            error!("Failed to handle dictation finished: {}", e);
        }
    }
}

pub async fn handle_tts_started(app_handle: &AppHandle, text: String) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_tts_started(text).await {
            error!("Failed to handle TTS started: {}", e);
        }
    }
}

pub async fn handle_tts_finished(app_handle: &AppHandle) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_tts_finished().await {
            error!("Failed to handle TTS finished: {}", e);
        }
    }
}

pub async fn handle_dictation_mode_change(app_handle: &AppHandle, is_active: bool) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_mode_change(is_active).await {
            error!("Failed to handle dictation mode change: {}", e);
        }
    }
}

pub async fn handle_always_listening_change(app_handle: &AppHandle, is_active: bool) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_always_listening_change(is_active).await {
            error!("Failed to handle always listening mode change: {}", e);
        }
    }
}

// Agent event handlers for external use
pub async fn handle_agent_started(app_handle: &AppHandle) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_started().await {
            error!("Failed to handle agent started: {}", e);
        }
    }
}

pub async fn handle_agent_stopped(app_handle: &AppHandle) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_stopped().await {
            error!("Failed to handle agent stopped: {}", e);
        }
    }
}

pub async fn handle_agent_cancelled(app_handle: &AppHandle) {
    if let Some(manager) = get_bar_manager(app_handle).await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_cancelled().await {
            error!("Failed to handle agent cancelled: {}", e);
        }
    }
}

// Configuration commands

/// Get the current floating bar configuration using centralized settings
#[tauri::command]
pub async fn get_floating_bar_config(
    app_handle: AppHandle,
) -> Result<FloatingBarConfig, String> {
    info!("Getting floating bar configuration from centralized settings");

    match FloatingBarConfig::load_from_centralized_settings(&app_handle).await {
        Ok(config) => {
            debug!("Successfully loaded floating bar config from centralized settings: {:?}", config);
            Ok(config)
        },
        Err(e) => {
            warn!("Failed to load floating bar config from centralized settings, using defaults: {}", e);
            Ok(FloatingBarConfig::default())
        }
    }
}

/// Set the floating bar configuration using centralized settings
#[tauri::command]
pub async fn set_floating_bar_config(
    app_handle: AppHandle,
    config: FloatingBarConfig,
) -> Result<(), String> {
    info!("Setting floating bar configuration in centralized settings: {:?}", config);

    // Save to centralized settings (which will also emit settings events)
    config.save_to_centralized_settings(&app_handle).await?;

    // Emit event to notify frontend of config change (for backward compatibility)
    if let Err(e) = app_handle.emit("floating-bar-config-changed", &config) {
        warn!("Failed to emit config change event: {}", e);
    }

    info!("Floating bar configuration updated successfully in centralized settings");
    Ok(())
}

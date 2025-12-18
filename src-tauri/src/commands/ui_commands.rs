use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Listener};
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use tracing::{debug, error, warn, info};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::constants::{timeouts, events, ui};

use crate::settings::{manager::SettingsManager, FloatingBarSettings};
use crate::utils::async_runtime::safe_spawn_async_task;

// === CORE UI TYPES ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIElementConfig {
    pub element_type: String,
    pub visible: bool,
    pub position: Option<UIPosition>,
    pub size: Option<UISize>,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UISize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIInteractionEvent {
    pub element_id: String,
    pub interaction_type: String,
    pub data: Option<HashMap<String, serde_json::Value>>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIStateUpdate {
    pub element_id: String,
    pub state: HashMap<String, serde_json::Value>,
    pub timestamp: u64,
}

// === FLOATING BAR SPECIFIC TYPES ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BarState {
    Default,
    Expanding,
    Input,
    Shrinking,
    Submitting,
    Loading,
    Success,
    Error,
    Speaking,
    Listening,
    Transcribing,
    Dictating,
    DictationReady,
    AlwaysListening,
    Finishing,
    AgentResponding,
}

impl BarState {
    pub fn as_str(&self) -> &str {
        match self {
            BarState::Default => ui::bar_states::DEFAULT,
            BarState::Expanding => ui::bar_states::EXPANDING,
            BarState::Input => ui::bar_states::INPUT,
            BarState::Shrinking => ui::bar_states::SHRINKING,
            BarState::Submitting => ui::bar_states::SUBMITTING,
            BarState::Loading => ui::bar_states::LOADING,
            BarState::Success => ui::bar_states::SUCCESS,
            BarState::Error => ui::bar_states::ERROR,
            BarState::Speaking => ui::bar_states::SPEAKING,
            BarState::Listening => ui::bar_states::LISTENING,
            BarState::Transcribing => ui::bar_states::TRANSCRIBING,
            BarState::Dictating => ui::bar_states::DICTATING,
            BarState::DictationReady => ui::bar_states::DICTATION_READY,
            BarState::AlwaysListening => ui::bar_states::ALWAYS_LISTENING,
            BarState::Finishing => ui::bar_states::FINISHING,
            BarState::AgentResponding => ui::bar_states::AGENT_RESPONDING,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingBarConfig {
    pub show_voice_indicator: bool,
    pub enable_animations: bool,
    pub auto_hide: bool,
    pub auto_hide_delay: u32,
    pub opacity: f32,
    pub bar_appearance: String,
}

impl Default for FloatingBarConfig {
    fn default() -> Self {
        Self {
            show_voice_indicator: true,
            enable_animations: true,
            auto_hide: false,
            auto_hide_delay: timeouts::UI_NOTIFICATION_DISPLAY_MS as u32,
            opacity: 0.95,
            bar_appearance: ui::bar_appearances::FLOATING.to_string(),
        }
    }
}

// === CORE UI MANAGER ===

#[derive(Debug)]
pub struct UIManager {
    pub app_handle: AppHandle,
    pub elements: HashMap<String, UIElementConfig>,

    // Floating Bar State (integrated directly)
    pub bar_state: BarState,
    pub input_value: String,
    pub last_submitted_value: String,
    pub current_error: Option<String>,
    pub transcription_text: String,
    pub spoken_text: String,
    pub is_agent_working: bool,
    pub is_dictation_mode: bool,
    pub is_always_listening: bool,
    pub audio_level: f64,
    pub voice_mode: String,
    pub agent_state: Option<String>,
    pub current_transition_id: Option<String>,
    pub bar_config: FloatingBarConfig,
    
    // Deduplication fields
    pub last_submission_time: Option<Instant>,
    pub last_submission_query: Option<String>,
}

impl UIManager {
    pub async fn new(app_handle: AppHandle) -> Result<Self, String> {
        let bar_config = Self::load_bar_config(&app_handle).await?;

        Ok(Self {
            app_handle,
            elements: HashMap::new(),
            bar_state: BarState::Default,
            input_value: String::new(),
            last_submitted_value: String::new(),
            current_error: None,
            transcription_text: String::new(),
            spoken_text: String::new(),
            is_agent_working: false,
            is_dictation_mode: false,
            is_always_listening: false,
            audio_level: 0.0,
            voice_mode: ui::voice_modes::IDLE.to_string(),
            agent_state: None,
            current_transition_id: None,
            bar_config,
            last_submission_time: None,
            last_submission_query: None,
        })
    }

    // === CONFIGURATION MANAGEMENT ===

    async fn load_bar_config(app_handle: &AppHandle) -> Result<FloatingBarConfig, String> {
        let settings_manager = SettingsManager::new(app_handle.clone())
            .map_err(|e| format!("Failed to create settings manager: {}", e))?;

        match settings_manager.get_floating_bar_settings().await {
            Ok(settings) => Ok(Self::convert_settings_to_config(&settings)),
            Err(_) => {
                debug!("Failed to load floating bar settings, using defaults");
                Ok(FloatingBarConfig::default())
            }
        }
    }

    fn convert_settings_to_config(settings: &FloatingBarSettings) -> FloatingBarConfig {
        FloatingBarConfig {
            show_voice_indicator: settings.show_voice_indicator,
            enable_animations: settings.enable_animations,
            auto_hide: settings.auto_hide,
            auto_hide_delay: settings.auto_hide_delay,
            opacity: settings.opacity,
            bar_appearance: settings.bar_appearance.clone(),
        }
    }

    async fn save_bar_config(&self) -> Result<(), String> {
        let settings_manager = SettingsManager::new(self.app_handle.clone())
            .map_err(|e| format!("Failed to create settings manager: {}", e))?;

        let settings = FloatingBarSettings {
            show_voice_indicator: self.bar_config.show_voice_indicator,
            enable_animations: self.bar_config.enable_animations,
            auto_hide: self.bar_config.auto_hide,
            auto_hide_delay: self.bar_config.auto_hide_delay,
            opacity: self.bar_config.opacity,
            bar_appearance: self.bar_config.bar_appearance.clone(),
        };

        settings_manager.set_floating_bar_settings(&settings).await
            .map_err(|e| format!("Failed to save floating bar settings: {}", e))?;

        Ok(())
    }

    // === FLOATING BAR FUNCTIONALITY ===

    async fn emit_bar_state_update(&self) {
        let state_data = serde_json::json!({
            "barState": self.bar_state.as_str(),
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

        if let Err(e) = self.app_handle.emit(events::bar::STATE_UPDATE, state_data) {
            error!("Failed to emit bar-state-update: {}", e);
        }
    }

    async fn set_bar_state(&mut self, new_state: BarState) {
        debug!("UI Manager: Bar state changing from {:?} to {:?}", self.bar_state, new_state);
        self.bar_state = new_state;
        self.emit_bar_state_update().await;
    }
    
    /// Navigate the bar window to the appropriate route based on bar appearance
    async fn navigate_bar_window(&self) -> Result<(), String> {
        // Always use the floating-bar window (the only one that exists)
        let window_label = ui::window_labels::FLOATING_BAR;
        
        // Determine the route based on bar appearance
        let route = match self.bar_config.bar_appearance.as_str() {
            ui::bar_appearances::APP => "/app-bar",
            ui::bar_appearances::VOICE_AI => "/voice-bar", 
            ui::bar_appearances::DYNAMIC => "/dynamic-bar",
            _ => "/floating-bar",
        };
        
        // Navigate the window to the appropriate route
        if let Some(window) = self.app_handle.get_webview_window(window_label) {
            let current_url = window.url().map_err(|e| format!("Failed to get current URL: {}", e))?;
            
            // Build the new URL by taking the base and appending the route
            let base_url = current_url.as_str().split('#').next().unwrap_or(current_url.as_str());
            let base_url = base_url.split('/').take(3).collect::<Vec<_>>().join("/");
            let new_url = format!("{}{}", base_url, route);
            
            // Only navigate if we're not already on the right route
            if !current_url.as_str().ends_with(route) {
                window.navigate(new_url.parse().map_err(|e| format!("Failed to parse URL: {}", e))?)
                    .map_err(|e| format!("Failed to navigate window: {}", e))?;
                
                debug!("Navigated bar window to route: {}", route);
            }
        } else {
            debug!("Bar window not found for navigation");
        }
        
        Ok(())
    }

    pub async fn handle_bar_click(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling bar click, current state: {:?}", self.bar_state);

        if self.bar_state != BarState::Default || self.is_agent_working {
            return Ok(());
        }

        self.set_bar_state(BarState::Expanding).await;

        let app_handle = self.app_handle.clone();
        safe_spawn_async_task(move || async move {
            sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
            if let Some(manager) = get_ui_manager().await {
                let mut manager = manager.lock().await;
                manager.set_bar_state(BarState::Input).await;
            }
        });

        Ok(())
    }

    pub async fn handle_bar_focus_change(&mut self, is_focused: bool) -> Result<(), String> {
        debug!("UI Manager: Handling focus change, focused: {}, current state: {:?}", is_focused, self.bar_state);

        if self.should_remain_expanded_for_status() {
            debug!("UI Manager: Agent is working, preserving state");
            return Ok(());
        }

        if is_focused {
            if self.bar_state == BarState::Default && !self.is_agent_working {
                debug!("UI Manager: Window gained focus, expanding to input state");
                self.set_bar_state(BarState::Expanding).await;

                let transition_id = Uuid::new_v4().to_string();
                self.current_transition_id = Some(transition_id.clone());

                let app_handle = self.app_handle.clone();
                safe_spawn_async_task(move || async move {
                    sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
                    if let Some(manager) = get_ui_manager().await {
                        let mut manager = manager.lock().await;
                        if manager.current_transition_id.as_ref() == Some(&transition_id) {
                            manager.set_bar_state(BarState::Input).await;
                            manager.current_transition_id = None;
                        }
                    }
                });
            }
        } else {
            if self.bar_state == BarState::Input && self.input_value.trim().is_empty() {
                self.handle_bar_input_blur().await?;
            }
        }

        Ok(())
    }

    pub async fn handle_bar_input_blur(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling input blur, current state: {:?}", self.bar_state);

        if self.bar_state == BarState::Input && self.input_value.trim().is_empty() && !self.should_remain_expanded_for_status() {
            self.set_bar_state(BarState::Shrinking).await;

            let transition_id = Uuid::new_v4().to_string();
            self.current_transition_id = Some(transition_id.clone());

            let app_handle = self.app_handle.clone();
            safe_spawn_async_task(move || async move {
                sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
                if let Some(manager) = get_ui_manager().await {
                    let mut manager = manager.lock().await;
                    if manager.current_transition_id.as_ref() == Some(&transition_id) {
                        manager.input_value.clear();
                        manager.set_bar_state(BarState::Default).await;
                        manager.current_transition_id = None;
                    }
                }
            });
        }

        Ok(())
    }

    pub async fn handle_bar_input_change(&mut self, new_value: String) -> Result<(), String> {
        debug!("UI Manager: Input changed to: '{}'", new_value);
        self.input_value = new_value;
        self.emit_bar_state_update().await;
        Ok(())
    }

    pub async fn handle_bar_submit(&mut self, query: String) -> Result<(), String> {
        debug!("UI Manager: Handling submit with query: '{}'", query);

        if query.trim().is_empty() {
            return Ok(());
        }

        // Check for duplicate submission within 1 second
        let now = Instant::now();
        if let (Some(last_time), Some(last_query)) = (&self.last_submission_time, &self.last_submission_query) {
            if last_query == &query && now.duration_since(*last_time).as_millis() < 1000 {
                warn!("Duplicate submission detected within 1 second, ignoring: '{}'", query);
                return Ok(());
            }
        }

        // Update deduplication tracking
        self.last_submission_time = Some(now);
        self.last_submission_query = Some(query.clone());

        // Set immediate submitting state for UI feedback
        self.last_submitted_value = query.clone();
        self.current_error = None;
        self.agent_state = None;
        self.is_agent_working = true;
        self.voice_mode = ui::voice_modes::AGENT.to_string();

        self.set_bar_state(BarState::Submitting).await;

        // Emit unified agent query submission event
        let query_payload = serde_json::json!({ "query": query });
        if let Err(e) = self
            .app_handle
            .emit(events::agent::QUERY_READY, query_payload)
        {
            error!("Failed to emit agent query submission: {}", e);
            return Err(format!("Failed to submit query: {}", e));
        }

        Ok(())
    }

    pub async fn handle_backend_response(&mut self, response_text: Option<String>, agent_state: String) -> Result<(), String> {
        debug!("UI Manager: Handling backend response, agent_state: {}", agent_state);

        let transition_id = Uuid::new_v4().to_string();
        self.current_transition_id = Some(transition_id.clone());
        self.agent_state = Some(agent_state.clone());

        match agent_state.as_str() {
            ui::agent_status::FINISHED => {
                self.set_bar_state(BarState::Finishing).await;

                let app_handle = self.app_handle.clone();
                let transition_id_clone = transition_id.clone();
                safe_spawn_async_task(move || async move {
                    sleep(Duration::from_millis(timeouts::UI_FADE_DELAY_MS)).await;
                    let _ = app_handle.emit(events::bar::COMPLETE_TRANSITION, transition_id_clone);
                });
            }
            ui::agent_status::FAILED | ui::agent_status::CANCELLED | ui::agent_status::OFFLINE => {
                self.current_error = Some(
                    if agent_state == ui::agent_status::CANCELLED {
                        "Agent execution was cancelled".to_string()
                    } else if agent_state == ui::agent_status::OFFLINE {
                        "Connection unavailable".to_string()
                    } else {
                        format!("Agent failed: {}", response_text.unwrap_or_default())
                    }
                );
                self.set_bar_state(BarState::Error).await;

                let app_handle = self.app_handle.clone();
                let transition_id_clone = transition_id.clone();
                safe_spawn_async_task(move || async move {
                    sleep(Duration::from_millis(timeouts::UI_NOTIFICATION_DISPLAY_MS)).await;
                    let _ = app_handle.emit(events::bar::CLEAR_ERROR, transition_id_clone);
                });
            }
            _ => {
                self.set_bar_state(BarState::Default).await;
                self.current_transition_id = None;
            }
        }

        Ok(())
    }

    /// Visual-only submit handler to provide immediate UI feedback without emitting agent events
    pub async fn handle_submit_visual_only(&mut self, query: String) -> Result<(), String> {
        debug!("UI Manager: Handling visual-only submit with query: '{}'", query);

        if query.trim().is_empty() {
            return Ok(());
        }

        // Update state for immediate visual feedback (no QUERY_READY emission here)
        self.last_submitted_value = query;
        self.current_error = None;
        self.agent_state = None;
        self.is_agent_working = true; // Shows activity immediately
        self.voice_mode = ui::voice_modes::AGENT.to_string();

        // Transition to Submitting for quicker perceived responsiveness
        self.set_bar_state(BarState::Submitting).await;
        Ok(())
    }

    // === VOICE & DICTATION FUNCTIONALITY ===

    pub async fn handle_dictation_mode_change(&mut self, is_active: bool) -> Result<(), String> {
        debug!("UI Manager: Handling dictation mode change: {}", is_active);
        self.is_dictation_mode = is_active;

        if is_active {
            self.voice_mode = ui::voice_modes::DICTATION.to_string();
            self.set_bar_state(BarState::Dictating).await;
        } else {
            self.voice_mode = ui::voice_modes::IDLE.to_string();
            if !self.is_agent_working {
                self.set_bar_state(BarState::Default).await;
            }
        }

        Ok(())
    }

    pub async fn handle_always_listening_change(&mut self, is_active: bool) -> Result<(), String> {
        debug!("UI Manager: Handling always listening change: {}", is_active);
        self.is_always_listening = is_active;

        if is_active {
            self.voice_mode = ui::voice_modes::ALWAYS_LISTENING.to_string();
            self.set_bar_state(BarState::AlwaysListening).await;
        } else {
            self.voice_mode = ui::voice_modes::IDLE.to_string();
            if !self.is_agent_working && !self.is_dictation_mode {
                self.set_bar_state(BarState::Default).await;
            }
        }

        Ok(())
    }

    pub async fn handle_agent_started(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling agent started");
        self.is_agent_working = true;
        self.voice_mode = ui::voice_modes::AGENT.to_string();
        self.set_bar_state(BarState::Loading).await;
        Ok(())
    }

    pub async fn handle_agent_stopped(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling agent stopped");
        self.is_agent_working = false;
        self.voice_mode = ui::voice_modes::IDLE.to_string();
        if matches!(self.bar_state, BarState::Submitting | BarState::Loading | BarState::AgentResponding | BarState::Listening | BarState::Transcribing) {
            self.set_bar_state(BarState::Default).await;
        }
        Ok(())
    }

    pub async fn handle_agent_cancelled(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling agent cancelled");
        self.is_agent_working = false;
        self.voice_mode = ui::voice_modes::IDLE.to_string();
        self.is_dictation_mode = false;
        self.transcription_text.clear();
        self.input_value.clear();
        self.last_submitted_value.clear();
        self.current_error = None;
        self.set_bar_state(BarState::Default).await;
        Ok(())
    }

    // === TTS FUNCTIONALITY ===

    pub async fn handle_tts_started(&mut self, text: String) -> Result<(), String> {
        debug!("UI Manager: Handling TTS started with text: '{}'", text);
        self.spoken_text = text;
        self.voice_mode = ui::voice_modes::SPEAKING.to_string();
        self.set_bar_state(BarState::Speaking).await;
        Ok(())
    }

    pub async fn handle_tts_finished(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling TTS finished");
        self.spoken_text.clear();
        if !self.is_agent_working && !self.is_dictation_mode && !self.is_always_listening {
            self.voice_mode = ui::voice_modes::IDLE.to_string();
            self.set_bar_state(BarState::Default).await;
        }
        Ok(())
    }

    // === DICTATION FUNCTIONALITY ===

    pub async fn handle_dictation_started(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling dictation started");
        self.transcription_text.clear();
        self.voice_mode = ui::voice_modes::DICTATION.to_string();
        self.set_bar_state(BarState::Listening).await;
        Ok(())
    }

    pub async fn handle_dictation_partial(&mut self, partial_text: String) -> Result<(), String> {
        debug!("UI Manager: Handling dictation partial: '{}'", partial_text);
        self.transcription_text = partial_text;
        self.set_bar_state(BarState::Transcribing).await;
        Ok(())
    }

    pub async fn handle_dictation_finished(&mut self, query: Option<String>) -> Result<(), String> {
        debug!("UI Manager: Handling dictation finished with query: {:?}", query);

        if let Some(query_text) = query {
            if !query_text.trim().is_empty() {
                // Submit the dictated query
                self.handle_bar_submit(query_text).await?;
            } else {
                // Empty result, return to appropriate state
                if self.is_dictation_mode {
                    self.set_bar_state(BarState::Dictating).await;
                } else {
                    self.set_bar_state(BarState::Default).await;
                }
            }
        } else {
            // No result, return to appropriate state
            if self.is_dictation_mode {
                self.set_bar_state(BarState::Dictating).await;
            } else {
                self.set_bar_state(BarState::Default).await;
            }
        }

        self.transcription_text.clear();
        Ok(())
    }

    // === FLOATING PANEL FUNCTIONALITY ===

    pub async fn set_panel_click_through(&self, enabled: bool) -> Result<(), String> {
        info!("UI Manager: Setting panel click-through: {}", enabled);

        #[cfg(target_os = "macos")]
        {
            use cocoa::{appkit::NSWindow, base::{id as cocoa_id, BOOL, YES, NO}};
            use objc::{msg_send, sel, sel_impl};
            use dispatch::Queue;

            if let Some(window) = self.app_handle.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
                match window.ns_window() {
                    Ok(ns_window_ptr) => {
                        let ns_window = ns_window_ptr as cocoa_id;
                        if !ns_window.is_null() {
                            let ns_window_addr = ns_window as usize;
                            let ignore_events: BOOL = if enabled { YES } else { NO };

                            let result = std::panic::catch_unwind(|| {
                                Queue::main().exec_sync(|| {
                                    unsafe {
                                        let ns_window = ns_window_addr as cocoa_id;
                                        let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: ignore_events];
                                    }
                                });
                            });

                            match result {
                                Ok(_) => info!("UI Manager: Panel click-through set successfully"),
                                Err(_) => return Err("Failed to set panel click-through".to_string()),
                            }
                        }
                    }
                    Err(e) => return Err(format!("Failed to get NSWindow: {}", e)),
                }
            } else {
                return Err("Floating panel window not found".to_string());
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            return Err("Click-through behavior only supported on macOS".to_string());
        }

        Ok(())
    }

    pub async fn set_panel_level(&self, level: i32) -> Result<(), String> {
        info!("UI Manager: Setting panel window level: {}", level);

        #[cfg(target_os = "macos")]
        {
            use cocoa::base::{id as cocoa_id};
            use objc::{msg_send, sel, sel_impl};
            use dispatch::Queue;

            if let Some(window) = self.app_handle.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
                match window.ns_window() {
                    Ok(ns_window_ptr) => {
                        let ns_window = ns_window_ptr as cocoa_id;
                        if !ns_window.is_null() {
                            let ns_window_addr = ns_window as usize;
                            let safe_level = match level {
                                0 => 0, 1 => 1, 3 => 3, 5 => 5, 8 => 8, 24 => 24,
                                _ => 3, // Default to floating level
                            };

                            let result = std::panic::catch_unwind(|| {
                                Queue::main().exec_sync(|| {
                                    unsafe {
                                        let ns_window = ns_window_addr as cocoa_id;
                                        let _: () = msg_send![ns_window, setLevel: safe_level];
                                    }
                                });
                            });

                            match result {
                                Ok(_) => info!("UI Manager: Panel level set successfully"),
                                Err(_) => return Err("Failed to set panel level".to_string()),
                            }
                        }
                    }
                    Err(e) => return Err(format!("Failed to get NSWindow: {}", e)),
                }
            } else {
                return Err("Floating panel window not found".to_string());
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            return Err("Window level control only supported on macOS".to_string());
        }

        Ok(())
    }

    // === UTILITY FUNCTIONS ===

    fn should_remain_expanded_for_status(&self) -> bool {
        matches!(self.bar_state,
            BarState::Submitting | BarState::Loading | BarState::Finishing | BarState::Success |
            BarState::Speaking | BarState::Listening | BarState::Transcribing |
            BarState::Dictating | BarState::AlwaysListening | BarState::Error |
            BarState::AgentResponding | BarState::DictationReady
        ) || self.is_agent_working
    }

    // === ELEMENT MANAGEMENT ===

    pub async fn create_element(&mut self, element_id: String, config: UIElementConfig) -> Result<(), String> {
        debug!("UI Manager: Creating element: {}", element_id);
        self.elements.insert(element_id.clone(), config);

        let state_update = UIStateUpdate {
            element_id: element_id.clone(),
            state: self.get_element_state(&element_id),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis() as u64,
        };

        if let Err(e) = self.app_handle.emit(events::ui::ELEMENT_CREATED, &state_update) {
            error!("Failed to emit element created event: {}", e);
        }

        Ok(())
    }

    pub async fn update_element(&mut self, element_id: String, config: UIElementConfig) -> Result<(), String> {
        debug!("UI Manager: Updating element: {}", element_id);
        self.elements.insert(element_id.clone(), config);

        let state_update = UIStateUpdate {
            element_id: element_id.clone(),
            state: self.get_element_state(&element_id),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis() as u64,
        };

        if let Err(e) = self.app_handle.emit(events::ui::ELEMENT_UPDATED, &state_update) {
            error!("Failed to emit element updated event: {}", e);
        }

        Ok(())
    }

    pub async fn delete_element(&mut self, element_id: String) -> Result<(), String> {
        debug!("UI Manager: Deleting element: {}", element_id);
        self.elements.remove(&element_id);

        if let Err(e) = self.app_handle.emit(events::ui::ELEMENT_DELETED, &element_id) {
            error!("Failed to emit element deleted event: {}", e);
        }

        Ok(())
    }

    pub fn get_element_state(&self, element_id: &str) -> HashMap<String, serde_json::Value> {
        let mut state = HashMap::new();

        // Special handling for floating-bar
        if element_id == ui::element_ids::FLOATING_BAR {
            state.insert("barState".to_string(), serde_json::Value::String(self.bar_state.as_str().to_string()));
            state.insert("inputValue".to_string(), serde_json::Value::String(self.input_value.clone()));
            state.insert("lastSubmittedValue".to_string(), serde_json::Value::String(self.last_submitted_value.clone()));
            state.insert("currentError".to_string(), serde_json::to_value(&self.current_error).unwrap_or(serde_json::Value::Null));
            state.insert("transcriptionText".to_string(), serde_json::Value::String(self.transcription_text.clone()));
            state.insert("spokenText".to_string(), serde_json::Value::String(self.spoken_text.clone()));
            state.insert("isAgentWorking".to_string(), serde_json::Value::Bool(self.is_agent_working));
            state.insert("isDictationMode".to_string(), serde_json::Value::Bool(self.is_dictation_mode));
            state.insert("isAlwaysListening".to_string(), serde_json::Value::Bool(self.is_always_listening));
            state.insert("audioLevel".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(self.audio_level).unwrap_or(serde_json::Number::from(0))));
            state.insert("voiceMode".to_string(), serde_json::Value::String(self.voice_mode.clone()));
            state.insert("agentState".to_string(), serde_json::to_value(&self.agent_state).unwrap_or(serde_json::Value::Null));
        }

        // Add element-specific state from configuration
        if let Some(element_config) = self.elements.get(element_id) {
            state.insert("visible".to_string(), serde_json::Value::Bool(element_config.visible));
            state.insert("element_type".to_string(), serde_json::Value::String(element_config.element_type.clone()));

            if let Some(position) = &element_config.position {
                state.insert("position".to_string(), serde_json::to_value(position).unwrap_or(serde_json::Value::Null));
            }

            if let Some(size) = &element_config.size {
                state.insert("size".to_string(), serde_json::to_value(size).unwrap_or(serde_json::Value::Null));
            }

            // Add custom properties
            for (key, value) in &element_config.properties {
                state.insert(key.clone(), value.clone());
            }
        }

        state
    }
}

// === GLOBAL UI MANAGER ===

static UI_MANAGER: OnceLock<Arc<TokioMutex<UIManager>>> = OnceLock::new();

pub async fn initialize_ui_manager(app_handle: AppHandle) -> Result<(), String> {
    debug!("Initializing UI Manager");

    // Check if already initialized
    if UI_MANAGER.get().is_some() {
        warn!("UI Manager already initialized, skipping duplicate initialization");
        return Ok(());
    }

    let manager = UIManager::new(app_handle.clone()).await?;
    let manager_arc = Arc::new(TokioMutex::new(manager));

    // Store globally
    UI_MANAGER.set(manager_arc.clone()).map_err(|_| "Failed to set UI manager")?;

    // Set up event listeners
    setup_ui_event_listeners(app_handle, manager_arc).await;

    info!("UI Manager initialized successfully");
    Ok(())
}

pub async fn get_ui_manager() -> Option<Arc<TokioMutex<UIManager>>> {
    UI_MANAGER.get().cloned()
}

// === EVENT LISTENERS ===

async fn setup_ui_event_listeners(app_handle: AppHandle, manager: Arc<TokioMutex<UIManager>>) {
    debug!("UI Manager: Setting up event listeners");

    // Complete transition events
    let manager_clone = manager.clone();
        app_handle.listen(crate::constants::events::bar::COMPLETE_TRANSITION, move |event| {
        let manager = manager_clone.clone();
        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;
            let transition_id = if let Ok(payload) = serde_json::from_str::<String>(event.payload()) {
                Some(payload)
            } else {
                None
            };

            if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                manager.set_bar_state(BarState::Default).await;
                manager.current_transition_id = None;
            }
        });
    });

    // Clear error events
    let manager_clone = manager.clone();
        app_handle.listen(crate::constants::events::bar::CLEAR_ERROR, move |event| {
        let manager = manager_clone.clone();
        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;
            let transition_id = if let Ok(payload) = serde_json::from_str::<String>(event.payload()) {
                Some(payload)
            } else {
                None
            };

            if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                manager.current_error = None;
                manager.set_bar_state(BarState::Default).await;
                manager.current_transition_id = None;
            }
        });
    });

    // Agent stream start events
    let manager_clone = manager.clone();
        app_handle.listen(crate::constants::events::streaming::STREAM_START, move |_event| {
        let manager = manager_clone.clone();
        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;
            if manager.is_agent_working {
                manager.set_bar_state(BarState::AgentResponding).await;
            }
        });
    });

    debug!("UI Manager: Event listeners set up successfully");
}

// === TAURI COMMANDS ===

#[tauri::command]
pub async fn ui_create_element(
    element_id: String,
    config: UIElementConfig,
) -> Result<(), String> {
    debug!("Creating UI element: {} (type: {})", element_id, config.element_type);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        manager.create_element(element_id, config).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_update_element(
    element_id: String,
    config: UIElementConfig,
) -> Result<(), String> {
    debug!("Updating UI element: {} (type: {})", element_id, config.element_type);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        manager.update_element(element_id, config).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_delete_element(element_id: String) -> Result<(), String> {
    debug!("Deleting UI element: {}", element_id);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        manager.delete_element(element_id).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_get_element_state(element_id: String) -> Result<HashMap<String, serde_json::Value>, String> {
    debug!("Getting UI element state: {}", element_id);

    if let Some(manager) = get_ui_manager().await {
        let manager = manager.lock().await;
        Ok(manager.get_element_state(&element_id))
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_handle_interaction(
    element_id: String,
    interaction: UIInteractionEvent,
) -> Result<(), String> {
    debug!("Handling interaction for UI element: {} (type: {})", element_id, interaction.interaction_type);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;

        // Check if this is a bar component (floating-bar, app-bar, voice-ai-bar, dynamic-bar)
        if element_id == ui::element_ids::FLOATING_BAR || element_id == ui::element_ids::APP_BAR || element_id == ui::element_ids::VOICE_AI_BAR || element_id == ui::element_ids::DYNAMIC_BAR {
            match interaction.interaction_type.as_str() {
                ui::interaction_types::CLICK => manager.handle_bar_click().await,
                ui::interaction_types::SUBMIT => {
                    if let Some(data) = &interaction.data {
                        if let Some(value) = data.get("value").and_then(|v| v.as_str()) {
                            manager.handle_bar_submit(value.to_string()).await
                        } else {
                            Err("Submit interaction missing value".to_string())
                        }
                    } else {
                        Err("Submit interaction missing data".to_string())
                    }
                },
                ui::interaction_types::INPUT_CHANGE => {
                    if let Some(data) = &interaction.data {
                        if let Some(value) = data.get("value").and_then(|v| v.as_str()) {
                            manager.handle_bar_input_change(value.to_string()).await
                        } else {
                            Err("Input change interaction missing value".to_string())
                        }
                    } else {
                        Err("Input change interaction missing data".to_string())
                    }
                },
                ui::interaction_types::FOCUS => {
                    if let Some(data) = &interaction.data {
                        if let Some(is_focused) = data.get("isFocused").and_then(|v| v.as_bool()) {
                            manager.handle_bar_focus_change(is_focused).await
                        } else {
                            manager.handle_bar_focus_change(true).await
                        }
                    } else {
                        manager.handle_bar_focus_change(true).await
                    }
                },
                ui::interaction_types::BLUR => manager.handle_bar_input_blur().await,
                ui::interaction_types::INITIALIZE => {
                    // Handle initialization specially - just acknowledge receipt
                    debug!("Initialized bar component: {}", element_id);
                    Ok(())
                },
                ui::interaction_types::ESCAPE => {
                    // Handle escape key - delegate to stop coordinator for proper cancellation
                    debug!("Escape key pressed on bar component: {}", element_id);
                    let coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
                    coordinator.stop_all_operations(&manager.app_handle, "Escape key pressed via UI").await.map_err(|e| e.to_string()).map(|_| ())
                },
                ui::interaction_types::ENTER => {
                    // Handle enter key - submit current input if any
                    debug!("Enter key pressed on bar component: {}", element_id);
                    if manager.input_value.trim().is_empty() {
                        Ok(())
                    } else {
                        let input_value = manager.input_value.clone();
                        manager.handle_bar_submit(input_value).await
                    }
                },
                _ => {
                    warn!("Unknown interaction type for bar component: {}", interaction.interaction_type);
                    Ok(())
                }
            }
        } else if element_id == ui::element_ids::FLOATING_PANEL {
            match interaction.interaction_type.as_str() {
                ui::interaction_types::SET_CLICK_THROUGH => {
                    if let Some(data) = &interaction.data {
                        if let Some(enabled) = data.get("enabled").and_then(|v| v.as_bool()) {
                            manager.set_panel_click_through(enabled).await
                        } else {
                            Err("Set click through interaction missing enabled value".to_string())
                        }
                    } else {
                        Err("Set click through interaction missing data".to_string())
                    }
                },
                ui::interaction_types::SET_LEVEL => {
                    if let Some(data) = &interaction.data {
                        if let Some(level) = data.get("level").and_then(|v| v.as_i64()) {
                            manager.set_panel_level(level as i32).await
                        } else {
                            Err("Set level interaction missing level value".to_string())
                        }
                    } else {
                        Err("Set level interaction missing data".to_string())
                    }
                },
                _ => {
                    warn!("Unknown interaction type for floating panel: {}", interaction.interaction_type);
                    Ok(())
                }
            }
        } else {
            warn!("Interaction handling not implemented for element: {}", element_id);
            Ok(())
        }
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

// === FLOATING BAR CONFIGURATION COMMANDS ===

#[tauri::command]
pub async fn ui_get_bar_config() -> Result<FloatingBarConfig, String> {
    debug!("Getting floating bar configuration");

    if let Some(manager) = get_ui_manager().await {
        let manager = manager.lock().await;
        Ok(manager.bar_config.clone())
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_set_bar_config(config: FloatingBarConfig) -> Result<(), String> {
    debug!("Setting floating bar configuration: {:?}", config);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        let appearance_changed = manager.bar_config.bar_appearance != config.bar_appearance;
        
        manager.bar_config = config.clone();
        manager.save_bar_config().await?;

        // Navigate to the appropriate route if appearance changed
        if appearance_changed {
            if let Err(e) = manager.navigate_bar_window().await {
                warn!("Failed to navigate bar window: {}", e);
            }
        }

        // Emit event to notify frontend
        if let Err(e) = manager.app_handle.emit(events::bar::CONFIG_CHANGED, &config) {
            warn!("Failed to emit config change event: {}", e);
        }

        Ok(())
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

// === PANEL SPECIFIC COMMANDS ===

#[tauri::command]
pub async fn ui_set_panel_click_through(enabled: bool) -> Result<(), String> {
    debug!("Setting panel click-through: {}", enabled);

    if let Some(manager) = get_ui_manager().await {
        let manager = manager.lock().await;
        manager.set_panel_click_through(enabled).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_set_panel_level(level: i32) -> Result<(), String> {
    debug!("Setting panel level: {}", level);

    if let Some(manager) = get_ui_manager().await {
        let manager = manager.lock().await;
        manager.set_panel_level(level).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

// === EXTERNAL EVENT HANDLERS (for integration with other systems) ===

pub async fn handle_agent_started(app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_started().await {
            error!("Failed to handle agent started: {}", e);
        }
    }
}

pub async fn handle_agent_stopped(app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_stopped().await {
            error!("Failed to handle agent stopped: {}", e);
        }
    }
}

pub async fn handle_agent_cancelled(app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_cancelled().await {
            error!("Failed to handle agent cancelled: {}", e);
        }
    }
}

pub async fn handle_backend_response(app_handle: &AppHandle, response_text: Option<String>, agent_state: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_backend_response(response_text, agent_state).await {
            error!("Failed to handle backend response: {}", e);
        }
    }
}

pub async fn handle_dictation_mode_change(app_handle: &AppHandle, is_active: bool) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_mode_change(is_active).await {
            error!("Failed to handle dictation mode change: {}", e);
        }
    }
}

pub async fn handle_always_listening_change(app_handle: &AppHandle, is_active: bool) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_always_listening_change(is_active).await {
            error!("Failed to handle always listening change: {}", e);
        }
    }
}

pub async fn handle_query_submitted(app_handle: &AppHandle, query: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_bar_submit(query).await {
            error!("Failed to handle query submitted: {}", e);
        }
    }
}

pub async fn handle_tts_started(app_handle: &AppHandle, text: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_tts_started(text).await {
            error!("Failed to handle TTS started: {}", e);
        }
    }
}

pub async fn handle_tts_finished(app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_tts_finished().await {
            error!("Failed to handle TTS finished: {}", e);
        }
    }
}

#[tauri::command]
pub async fn notify_query_submitted(
    query: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        manager.handle_submit_visual_only(query).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

pub async fn handle_dictation_started(app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_started().await {
            error!("Failed to handle dictation started: {}", e);
        }
    }
}

pub async fn handle_dictation_partial(app_handle: &AppHandle, partial_text: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_partial(partial_text).await {
            error!("Failed to handle dictation partial: {}", e);
        }
    }
}

pub async fn handle_dictation_finished(app_handle: &AppHandle, query: Option<String>) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_finished(query).await {
            error!("Failed to handle dictation finished: {}", e);
        }
    }
}

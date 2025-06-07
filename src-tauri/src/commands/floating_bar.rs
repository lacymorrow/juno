use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use tracing::{debug, error, warn};

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
    app_handle: AppHandle,
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
            app_handle,
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

        // After animation, transition to input
        let app_handle = self.app_handle.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(300)).await;
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
            // When window gains focus, expand if in default state
            if self.current_state == BarState::Default {
                self.handle_click().await?;
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
            self.set_state(BarState::Shrinking).await;

            // After animation, return to default
            let app_handle = self.app_handle.clone();
            tokio::spawn(async move {
                sleep(Duration::from_millis(300)).await;
                if let Some(manager) = get_bar_manager(&app_handle).await {
                    let mut manager = manager.lock().await;
                    manager.input_value.clear();
                    manager.set_state(BarState::Default).await;
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
        self.is_agent_working = true;

        // Show success state briefly
        self.set_state(BarState::Success).await;

        // Transition through shrinking to loading
        let app_handle = self.app_handle.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(600)).await;
            if let Some(manager) = get_bar_manager(&app_handle).await {
                let mut manager = manager.lock().await;
                manager.set_state(BarState::Shrinking).await;

                tokio::spawn(async move {
                    sleep(Duration::from_millis(300)).await;
                    if let Some(manager) = get_bar_manager(&app_handle).await {
                        let mut manager = manager.lock().await;
                        manager.set_state(BarState::Loading).await;
                    }
                });
            }
        });

        Ok(())
    }

    // Handle agent completion
    pub async fn handle_agent_completion(&mut self, agent_state: &str, response_text: Option<String>) -> Result<(), String> {
        debug!("FloatingBarManager: Handling agent completion with state: {}", agent_state);

        self.is_agent_working = false;

        match agent_state {
            "Finished" => {
                self.set_state(BarState::Finishing).await;

                let app_handle = self.app_handle.clone();
                tokio::spawn(async move {
                    sleep(Duration::from_millis(300)).await;
                    if let Some(manager) = get_bar_manager(&app_handle).await {
                        let mut manager = manager.lock().await;
                        manager.set_state(BarState::Input).await;
                    }
                });
            }
            "Failed" | "Cancelled" => {
                self.current_error = Some(
                    if agent_state == "Cancelled" {
                        "Agent execution was cancelled".to_string()
                    } else {
                        format!("Agent failed: {}", response_text.unwrap_or_default())
                    }
                );
                self.set_state(BarState::Error).await;

                let app_handle = self.app_handle.clone();
                tokio::spawn(async move {
                    sleep(Duration::from_millis(3000)).await;
                    if let Some(manager) = get_bar_manager(&app_handle).await {
                        let mut manager = manager.lock().await;
                        manager.current_error = None;
                        manager.set_state(BarState::Input).await;
                    }
                });
            }
            _ => {
                self.set_state(BarState::Input).await;
            }
        }

        Ok(())
    }

    // Handle dictation events
    pub async fn handle_dictation_started(&mut self) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation started");
        self.input_value.clear();
        self.transcription_text.clear();
        self.set_state(BarState::Listening).await;
        Ok(())
    }

    pub async fn handle_dictation_partial(&mut self, partial_text: String) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation partial: '{}'", partial_text);
        self.transcription_text = partial_text;

        if self.current_state == BarState::Listening {
            self.set_state(if self.is_dictation_mode { BarState::Dictating } else { BarState::Transcribing }).await;
        }

        self.emit_state_update().await;
        Ok(())
    }

    pub async fn handle_dictation_finished(&mut self, query: Option<String>) -> Result<(), String> {
        debug!("FloatingBarManager: Handling dictation finished with query: {:?}", query);
        self.transcription_text.clear();

        if let Some(query) = query {
            if self.is_dictation_mode {
                // Dictation mode - show completion briefly
                self.set_state(BarState::Finishing).await;

                let app_handle = self.app_handle.clone();
                tokio::spawn(async move {
                    sleep(Duration::from_millis(500)).await;
                    if let Some(manager) = get_bar_manager(&app_handle).await {
                        let mut manager = manager.lock().await;
                        manager.set_state(BarState::Default).await;
                    }
                });
            } else {
                // Regular dictation - show in input field
                self.input_value = query;
                self.set_state(BarState::Input).await;
            }
        } else {
            // No query - return to default
            self.set_state(BarState::Shrinking).await;

            let app_handle = self.app_handle.clone();
            tokio::spawn(async move {
                sleep(Duration::from_millis(300)).await;
                if let Some(manager) = get_bar_manager(&app_handle).await {
                    let mut manager = manager.lock().await;
                    manager.set_state(BarState::Default).await;
                }
            });
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
            self.set_state(BarState::Dictating).await;
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

    // Helper to check if bar should remain expanded
    fn should_remain_expanded_for_status(&self) -> bool {
        matches!(self.current_state,
            BarState::Loading | BarState::Finishing | BarState::Success |
            BarState::Speaking | BarState::Listening | BarState::Transcribing |
            BarState::Dictating | BarState::AlwaysListening | BarState::Error
        ) || self.is_agent_working
    }
}

// Static storage for the bar manager
static BAR_MANAGER: TokioMutex<Option<Arc<TokioMutex<FloatingBarManager>>>> = TokioMutex::const_new(None);

// Initialize the bar manager
pub async fn initialize_bar_manager(app_handle: AppHandle) {
    let manager = Arc::new(TokioMutex::new(FloatingBarManager::new(app_handle)));
    let mut global_manager = BAR_MANAGER.lock().await;
    *global_manager = Some(manager);
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

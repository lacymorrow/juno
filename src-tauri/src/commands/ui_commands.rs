use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, warn};

// === UI Element Types ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UIElementType {
    Bar,
    Panel,
    Chat,
    Overlay,
    Modal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum UIState {
    Default,
    Expanding,
    Expanded,
    Input,
    Shrinking,
    Submitting,
    Loading,
    Finishing,
    Success,
    Listening,
    Error,
    Transcribing,
    Speaking,
    Dictating,
    AlwaysListening,
    AgentResponding,
    DictationReady,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VoiceMode {
    Idle,
    Agent,
    Dictation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Working,
    Responding,
    Finished,
    Failed,
    Cancelled,
    Offline,
}

// === Configuration Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIElementConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub element_type: UIElementType,
    pub show_voice_indicator: bool,
    pub enable_animations: bool,
    pub auto_hide: bool,
    pub auto_hide_delay: u32,
    pub opacity: f32,
    pub position: Option<UIPosition>,
    pub dimensions: Option<UIDimensions>,
    pub click_through: Option<bool>,
    pub always_on_top: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIDimensions {
    pub width: f64,
    pub height: f64,
}

// === State Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIStateData {
    pub element_id: String,
    pub element_type: UIElementType,
    pub ui_state: UIState,
    pub input_value: String,
    pub last_submitted_value: String,
    pub current_error: Option<String>,
    pub transcription_text: String,
    pub spoken_text: String,
    pub is_agent_working: bool,
    pub is_dictation_mode: bool,
    pub is_always_listening: bool,
    pub audio_level: f32,
    pub voice_mode: VoiceMode,
    pub agent_state: AgentStatus,
    pub timestamp: u64,
}

// === Interaction Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIInteractionEvent {
    #[serde(rename = "type")]
    pub interaction_type: String,
    pub element_id: String,
    pub data: Option<serde_json::Value>,
}

// === UI Element Manager ===

#[derive(Debug)]
pub struct UIElement {
    pub config: UIElementConfig,
    pub state: UIStateData,
}

impl UIElement {
    pub fn new(element_id: String, element_type: UIElementType) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            config: UIElementConfig {
                id: element_id.clone(),
                element_type: element_type.clone(),
                show_voice_indicator: true,
                enable_animations: true,
                auto_hide: false,
                auto_hide_delay: 3000,
                opacity: 0.95,
                position: None,
                dimensions: None,
                click_through: None,
                always_on_top: None,
            },
            state: UIStateData {
                element_id,
                element_type,
                ui_state: UIState::Default,
                input_value: String::new(),
                last_submitted_value: String::new(),
                current_error: None,
                transcription_text: String::new(),
                spoken_text: String::new(),
                is_agent_working: false,
                is_dictation_mode: false,
                is_always_listening: false,
                audio_level: 0.0,
                voice_mode: VoiceMode::Idle,
                agent_state: AgentStatus::Idle,
                timestamp: now,
            },
        }
    }

    pub fn update_state(&mut self, updates: serde_json::Value) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        self.state.timestamp = now;

        // Apply partial updates to state
        if let Some(ui_state) = updates.get("uiState").and_then(|v| v.as_str()) {
            self.state.ui_state = serde_json::from_str(&format!("\"{}\"", ui_state))
                .map_err(|e| format!("Invalid UI state: {}", e))?;
        }

        if let Some(input_value) = updates.get("inputValue").and_then(|v| v.as_str()) {
            self.state.input_value = input_value.to_string();
        }

        if let Some(current_error) = updates.get("currentError") {
            self.state.current_error = if current_error.is_null() {
                None
            } else {
                current_error.as_str().map(|s| s.to_string())
            };
        }

        if let Some(transcription_text) = updates.get("transcriptionText").and_then(|v| v.as_str()) {
            self.state.transcription_text = transcription_text.to_string();
        }

        if let Some(spoken_text) = updates.get("spokenText").and_then(|v| v.as_str()) {
            self.state.spoken_text = spoken_text.to_string();
        }

        if let Some(is_agent_working) = updates.get("isAgentWorking").and_then(|v| v.as_bool()) {
            self.state.is_agent_working = is_agent_working;
        }

        if let Some(is_dictation_mode) = updates.get("isDictationMode").and_then(|v| v.as_bool()) {
            self.state.is_dictation_mode = is_dictation_mode;
        }

        if let Some(is_always_listening) = updates.get("isAlwaysListening").and_then(|v| v.as_bool()) {
            self.state.is_always_listening = is_always_listening;
        }

        if let Some(audio_level) = updates.get("audioLevel").and_then(|v| v.as_f64()) {
            self.state.audio_level = audio_level as f32;
        }

        if let Some(voice_mode) = updates.get("voiceMode").and_then(|v| v.as_str()) {
            self.state.voice_mode = serde_json::from_str(&format!("\"{}\"", voice_mode))
                .map_err(|e| format!("Invalid voice mode: {}", e))?;
        }

        if let Some(agent_state) = updates.get("agentState").and_then(|v| v.as_str()) {
            self.state.agent_state = serde_json::from_str(&format!("\"{}\"", agent_state))
                .map_err(|e| format!("Invalid agent state: {}", e))?;
        }

        Ok(())
    }

    pub fn update_config(&mut self, updates: serde_json::Value) -> Result<(), String> {
        // Apply partial updates to config
        if let Some(show_voice_indicator) = updates.get("showVoiceIndicator").and_then(|v| v.as_bool()) {
            self.config.show_voice_indicator = show_voice_indicator;
        }

        if let Some(enable_animations) = updates.get("enableAnimations").and_then(|v| v.as_bool()) {
            self.config.enable_animations = enable_animations;
        }

        if let Some(auto_hide) = updates.get("autoHide").and_then(|v| v.as_bool()) {
            self.config.auto_hide = auto_hide;
        }

        if let Some(auto_hide_delay) = updates.get("autoHideDelay").and_then(|v| v.as_u64()) {
            self.config.auto_hide_delay = auto_hide_delay as u32;
        }

        if let Some(opacity) = updates.get("opacity").and_then(|v| v.as_f64()) {
            self.config.opacity = opacity as f32;
        }

        if let Some(position) = updates.get("position") {
            if let (Some(x), Some(y)) = (
                position.get("x").and_then(|v| v.as_f64()),
                position.get("y").and_then(|v| v.as_f64())
            ) {
                self.config.position = Some(UIPosition { x, y });
            }
        }

        if let Some(dimensions) = updates.get("dimensions") {
            if let (Some(width), Some(height)) = (
                dimensions.get("width").and_then(|v| v.as_f64()),
                dimensions.get("height").and_then(|v| v.as_f64())
            ) {
                self.config.dimensions = Some(UIDimensions { width, height });
            }
        }

        if let Some(click_through) = updates.get("clickThrough").and_then(|v| v.as_bool()) {
            self.config.click_through = Some(click_through);
        }

        if let Some(always_on_top) = updates.get("alwaysOnTop").and_then(|v| v.as_bool()) {
            self.config.always_on_top = Some(always_on_top);
        }

        Ok(())
    }
}

// === Global UI Manager ===

#[derive(Debug)]
pub struct UIManager {
    elements: HashMap<String, UIElement>,
    app_handle: AppHandle,
}

impl UIManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            elements: HashMap::new(),
            app_handle,
        }
    }

    pub fn get_or_create_element(&mut self, element_id: &str, element_type: UIElementType) -> &mut UIElement {
        self.elements.entry(element_id.to_string()).or_insert_with(|| {
            UIElement::new(element_id.to_string(), element_type)
        })
    }

    pub fn get_element(&self, element_id: &str) -> Option<&UIElement> {
        self.elements.get(element_id)
    }

    pub fn get_element_mut(&mut self, element_id: &str) -> Option<&mut UIElement> {
        self.elements.get_mut(element_id)
    }

    pub async fn emit_state_update(&self, element_id: &str) -> Result<(), String> {
        if let Some(element) = self.get_element(element_id) {
            self.app_handle
                .emit("ui-state-update", &element.state)
                .map_err(|e| format!("Failed to emit state update: {}", e))?;
        }
        Ok(())
    }

    pub async fn emit_config_update(&self, element_id: &str) -> Result<(), String> {
        if let Some(element) = self.get_element(element_id) {
            self.app_handle
                .emit("ui-config-update", &element.config)
                .map_err(|e| format!("Failed to emit config update: {}", e))?;
        }
        Ok(())
    }

    pub async fn bridge_to_floating_bar(&mut self, interaction: &UIInteractionEvent) -> Result<(), String> {
        // Bridge interactions to existing floating bar commands
        match interaction.interaction_type.as_str() {
            "click" => {
                // Call existing floating_bar_click command (takes only app handle)
                crate::commands::floating_bar::floating_bar_click(
                    self.app_handle.clone(),
                ).await?;
            },
            "submit" => {
                // Call existing floating_bar_submit command
                if let Some(data) = &interaction.data {
                    if let Some(value) = data.get("value").and_then(|v| v.as_str()) {
                        crate::commands::floating_bar::floating_bar_submit(
                            self.app_handle.clone(),
                            value.to_string(),
                        ).await?;
                    }
                }
            },
            "input_change" => {
                // Call existing floating_bar_input_change command
                if let Some(data) = &interaction.data {
                    if let Some(value) = data.get("value").and_then(|v| v.as_str()) {
                        crate::commands::floating_bar::floating_bar_input_change(
                            self.app_handle.clone(),
                            value.to_string(),
                        ).await?;
                    }
                }
            },
            "focus" => {
                // Call existing floating_bar_focus_change command
                crate::commands::floating_bar::floating_bar_focus_change(
                    self.app_handle.clone(),
                    true,
                ).await?;
            },
            "blur" => {
                // Call existing floating_bar_focus_change command
                crate::commands::floating_bar::floating_bar_focus_change(
                    self.app_handle.clone(),
                    false,
                ).await?;
            },
            _ => {
                warn!("Unknown interaction type for floating bar: {}", interaction.interaction_type);
            }
        }
        Ok(())
    }
}

// Global UI Manager (using static for now, could be improved with proper state management)
use std::sync::OnceLock;
static UI_MANAGER: OnceLock<Arc<TokioMutex<UIManager>>> = OnceLock::new();

pub async fn initialize_ui_manager(app_handle: AppHandle) {
    let manager = UIManager::new(app_handle);
    let _ = UI_MANAGER.set(Arc::new(TokioMutex::new(manager)));
    debug!("UI Manager initialized");
}

async fn get_ui_manager() -> Option<Arc<TokioMutex<UIManager>>> {
    UI_MANAGER.get().cloned()
}

// === Tauri Commands ===

#[tauri::command]
pub async fn ui_get_state(element_id: String) -> Result<UIStateData, String> {
    debug!("Getting state for UI element: {}", element_id);

    if let Some(manager) = get_ui_manager().await {
        let manager = manager.lock().await;
        if let Some(element) = manager.get_element(&element_id) {
            Ok(element.state.clone())
        } else {
            Err(format!("UI element '{}' not found", element_id))
        }
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_set_state(
    element_id: String,
    element_type: UIElementType,
    state_update: serde_json::Value,
) -> Result<(), String> {
    debug!("Setting state for UI element: {} (type: {:?})", element_id, element_type);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        let element = manager.get_or_create_element(&element_id, element_type);

        element.update_state(state_update)?;
        manager.emit_state_update(&element_id).await?;

        Ok(())
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_get_config(element_id: String) -> Result<UIElementConfig, String> {
    debug!("Getting config for UI element: {}", element_id);

    if let Some(manager) = get_ui_manager().await {
        let manager = manager.lock().await;
        if let Some(element) = manager.get_element(&element_id) {
            Ok(element.config.clone())
        } else {
            Err(format!("UI element '{}' not found", element_id))
        }
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_set_config(
    element_id: String,
    config: serde_json::Value,
) -> Result<(), String> {
    debug!("Setting config for UI element: {}", element_id);

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;

        // Extract element type from config
        let element_type = config.get("type")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())
            .unwrap_or(UIElementType::Bar);

        let element = manager.get_or_create_element(&element_id, element_type);
        element.update_config(config)?;
        manager.emit_config_update(&element_id).await?;

        Ok(())
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

        // For now, bridge interactions to existing floating bar system
        // TODO: Add support for other element types (panel, chat, etc.)
        match element_id.as_str() {
            "floating-bar" => {
                manager.bridge_to_floating_bar(&interaction).await
            },
            _ => {
                warn!("Interaction handling not implemented for element: {}", element_id);
                Ok(())
            }
        }
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

// Window management commands that delegate to existing systems
#[tauri::command]
pub async fn ui_resize_window(
    element_id: String,
    width: f64,
    height: f64,
) -> Result<(), String> {
    debug!("Resizing window for UI element: {} to {}x{}", element_id, width, height);

    // Delegate to existing window management
    match element_id.as_str() {
        "floating-bar" => {
            // Use existing window resizing logic for floating bar
            // This would need to be implemented in the window management system
            Ok(())
        },
        "floating-panel" => {
            // Use existing panel resizing logic
            Ok(())
        },
        _ => {
            warn!("Window resizing not implemented for element: {}", element_id);
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn ui_move_window(
    element_id: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    debug!("Moving window for UI element: {} to {},{}", element_id, x, y);

    // Similar delegation pattern for window movement
    Ok(())
}

#[tauri::command]
pub async fn ui_set_click_through(
    app: AppHandle,
    element_id: String,
    enabled: bool,
) -> Result<(), String> {
    debug!("Setting click-through for UI element: {} to {}", element_id, enabled);

    match element_id.as_str() {
        "floating-panel" => {
            crate::commands::floating_panel::set_floating_panel_click_through(app, enabled)
        },
        _ => {
            warn!("Click-through not implemented for element: {}", element_id);
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn ui_show_window(element_id: String) -> Result<(), String> {
    debug!("Showing window for UI element: {}", element_id);
    // Delegate to window management
    Ok(())
}

#[tauri::command]
pub async fn ui_hide_window(element_id: String) -> Result<(), String> {
    debug!("Hiding window for UI element: {}", element_id);
    // Delegate to window management
    Ok(())
}

#[tauri::command]
pub async fn ui_set_window_level(
    app: AppHandle,
    element_id: String,
    level: i32,
) -> Result<(), String> {
    debug!("Setting window level for UI element: {} to {}", element_id, level);

    match element_id.as_str() {
        "floating-panel" => {
            crate::commands::floating_panel::set_floating_panel_level(app, level)
        },
        _ => {
            warn!("Window level setting not implemented for element: {}", element_id);
            Ok(())
        }
    }
}

use crate::constants::{events, timeouts, ui};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictationStateChangeEvent {
    pub previous_state: String,
    pub new_state: String,
    pub timestamp: u64,
    pub reason: String,
    pub component: String,
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
    Stopping,
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
            BarState::Stopping => ui::bar_states::STOPPING,
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
    /// Show the glowing activity border (the flame wrap) around the bar.
    pub show_glow_border: bool,
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
            show_glow_border: crate::constants::settings::defaults::show_glow_border(),
        }
    }
}

// === AUDIO LEVEL FOLDING ===

/// Bound on `bar-state-update` traffic caused by audio level events (~20 Hz).
const AUDIO_LEVEL_MIN_EMIT_INTERVAL: Duration = Duration::from_millis(50);

/// Decides what a plugin `voice-transcription:audio-level` event does to the
/// bar's `audioLevel` (LAC-4080). Pure state machine (no `AppHandle`) so the
/// folding rules are testable:
/// - mic closed → every level folds to silence, emitted only while the bar
///   still shows something non-zero
/// - silence (0.0) bypasses the throttle: it is the reset edge that parks
///   every waveform at baseline
/// - non-zero levels emit at a bounded rate, never per sample
#[derive(Debug, Default)]
pub struct AudioLevelFolder {
    last_emit: Option<Instant>,
}

impl AudioLevelFolder {
    /// Returns the level to store and emit, or `None` to drop the event.
    fn fold(&mut self, raw: f64, mic_open: bool, current: f64, now: Instant) -> Option<f64> {
        let level = if mic_open { raw.clamp(0.0, 1.0) } else { 0.0 };
        if level == 0.0 {
            self.last_emit = None;
            return (current != 0.0).then_some(0.0);
        }
        if self
            .last_emit
            .is_some_and(|last| now.duration_since(last) < AUDIO_LEVEL_MIN_EMIT_INTERVAL)
        {
            return None;
        }
        self.last_emit = Some(now);
        Some(level)
    }
}

// === CORE UI MANAGER ===

/// Where one Escape press takes the bar, decided from the bar state alone.
///
/// Every state but a resting one goes to `Default`. `None` means the bar is
/// already at rest and the press changes nothing, so pressing again is
/// harmless. `AlwaysListening` and `DictationReady` are resting: they are
/// standing intents the coordinated stop re-arms, not activities.
pub(crate) fn escape_target(state: &BarState) -> Option<BarState> {
    match state {
        BarState::Default | BarState::DictationReady | BarState::AlwaysListening => None,
        _ => Some(BarState::Default),
    }
}

/// Whether the bar state alone says Escape has work to stop. Resting states
/// and a bare composer do not; every other state does, even before the run
/// behind it has registered anywhere else.
pub(crate) fn escape_stops_work(state: &BarState) -> bool {
    !matches!(
        state,
        BarState::Default
            | BarState::DictationReady
            | BarState::AlwaysListening
            | BarState::Expanding
            | BarState::Input
            | BarState::Shrinking
    )
}

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
    /// True while `transcription_text` is a live streaming partial (rendered
    /// dimmed/italic in the bar). Reset to solid on final. Display-only.
    pub transcription_provisional: bool,
    pub spoken_text: String,
    pub is_agent_working: bool,
    pub is_dictation_mode: bool,
    pub is_always_listening: bool,
    pub audio_level: f64,
    audio_level_folder: AudioLevelFolder,
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
            transcription_provisional: false,
            spoken_text: String::new(),
            is_agent_working: false,
            is_dictation_mode: false,
            is_always_listening: false,
            audio_level: 0.0,
            audio_level_folder: AudioLevelFolder::default(),
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
            show_glow_border: settings.show_glow_border,
        }
    }

    async fn save_bar_config(&self) -> Result<(), String> {
        let settings_manager = SettingsManager::new(self.app_handle.clone())
            .map_err(|e| format!("Failed to create settings manager: {}", e))?;

        // Preserve fields this config view does not own (read-modify-write):
        // follow_cursor_display is set only via the settings toggle.
        let follow_cursor_display = settings_manager
            .get_floating_bar_settings()
            .await
            .map(|s| s.follow_cursor_display)
            .unwrap_or_else(|_| crate::constants::settings::defaults::follow_cursor_display());
        let settings = FloatingBarSettings {
            show_voice_indicator: self.bar_config.show_voice_indicator,
            enable_animations: self.bar_config.enable_animations,
            auto_hide: self.bar_config.auto_hide,
            auto_hide_delay: self.bar_config.auto_hide_delay,
            opacity: self.bar_config.opacity,
            bar_appearance: self.bar_config.bar_appearance.clone(),
            follow_cursor_display,
            show_glow_border: self.bar_config.show_glow_border,
        };

        settings_manager
            .set_floating_bar_settings(&settings)
            .await
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
            "transcriptionProvisional": self.transcription_provisional,
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
        debug!(
            "UI Manager: Bar state changing from {:?} to {:?}",
            self.bar_state, new_state
        );
        self.bar_state = new_state;
        self.emit_bar_state_update().await;
    }

    /// Put the bar back to the idle bar after an Escape press.
    ///
    /// The stop itself (agent, TTS, dictation, monitors) has already run by
    /// the time this is called; this is only the bar's half. It clears what
    /// the composer was holding and cancels any pending transition, so a
    /// delayed `Expanding -> Input` timer cannot re-open the bar a moment
    /// after Escape closed it. Already resting: nothing changes, which is what
    /// makes a second and third press harmless.
    pub async fn escape_to_idle(&mut self) {
        let Some(next) = escape_target(&self.bar_state) else {
            return;
        };
        self.input_value.clear();
        self.current_error = None;
        self.current_transition_id = None;
        self.agent_state = None;
        self.set_bar_state(next).await;
    }

    pub async fn handle_bar_click(&mut self) -> Result<(), String> {
        debug!(
            "UI Manager: Handling bar click, current state: {:?}",
            self.bar_state
        );

        if self.bar_state != BarState::Default || self.is_agent_working {
            return Ok(());
        }

        self.set_bar_state(BarState::Expanding).await;

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
        debug!(
            "UI Manager: Handling focus change, focused: {}, current state: {:?}",
            is_focused, self.bar_state
        );

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
        } else if self.bar_state == BarState::Input && self.input_value.trim().is_empty() {
            self.handle_bar_input_blur().await?;
        }

        Ok(())
    }

    pub async fn handle_bar_input_blur(&mut self) -> Result<(), String> {
        debug!(
            "UI Manager: Handling input blur, current state: {:?}",
            self.bar_state
        );

        if self.bar_state == BarState::Input
            && self.input_value.trim().is_empty()
            && !self.should_remain_expanded_for_status()
        {
            self.set_bar_state(BarState::Shrinking).await;

            let transition_id = Uuid::new_v4().to_string();
            self.current_transition_id = Some(transition_id.clone());

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
        if let (Some(last_time), Some(last_query)) =
            (&self.last_submission_time, &self.last_submission_query)
        {
            if last_query == &query && now.duration_since(*last_time).as_millis() < 1000 {
                warn!(
                    "Duplicate submission detected within 1 second, ignoring: '{}'",
                    query
                );
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

    pub async fn handle_backend_response(
        &mut self,
        response_text: Option<String>,
        agent_state: String,
    ) -> Result<(), String> {
        debug!(
            "UI Manager: Handling backend response, agent_state: {}",
            agent_state
        );

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
                self.current_error = Some(if agent_state == ui::agent_status::CANCELLED {
                    "Agent execution was cancelled".to_string()
                } else if agent_state == ui::agent_status::OFFLINE {
                    "Connection unavailable".to_string()
                } else {
                    format!("Agent failed: {}", response_text.unwrap_or_default())
                });
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

    /// Mirror an accepted query in the bar without emitting agent events.
    ///
    /// Called for every query `submit_query` accepts, whatever its source, so
    /// the bar reacts identically to typed, voice, component, cloud, and
    /// scheduled submissions. Idempotent: a query the bar itself just submitted
    /// (`handle_bar_submit`) is already in this state and is left untouched.
    pub async fn handle_submit_visual_only(&mut self, query: String) -> Result<(), String> {
        debug!(
            "UI Manager: Handling visual-only submit with query: '{}'",
            query
        );

        if query.trim().is_empty() {
            return Ok(());
        }

        if self.bar_state == BarState::Submitting && self.last_submitted_value == query {
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

    /// True while the microphone is capturing for the bar: hold-to-talk
    /// listening, dictation, or an active always-listening session.
    fn is_mic_session_open(&self) -> bool {
        self.is_dictation_mode
            || self.is_always_listening
            || matches!(
                self.bar_state,
                BarState::Listening
                    | BarState::Transcribing
                    | BarState::Dictating
                    | BarState::DictationReady
                    | BarState::AlwaysListening
            )
    }

    /// Fold a plugin audio-level event into `audio_level` and push it to the
    /// bars via `bar-state-update`, at a bounded rate (LAC-4080).
    pub async fn handle_audio_level(&mut self, raw_level: f64) {
        let mic_open = self.is_mic_session_open();
        if let Some(level) =
            self.audio_level_folder
                .fold(raw_level, mic_open, self.audio_level, Instant::now())
        {
            self.audio_level = level;
            self.emit_bar_state_update().await;
        }
    }

    pub async fn handle_dictation_mode_change(&mut self, is_active: bool) -> Result<(), String> {
        debug!("UI Manager: Handling dictation mode change: {}", is_active);

        let previous_state = self.voice_mode.clone();
        self.is_dictation_mode = is_active;

        if is_active {
            self.voice_mode = ui::voice_modes::DICTATION.to_string();
            self.set_bar_state(BarState::Dictating).await;
        } else {
            self.voice_mode = ui::voice_modes::IDLE.to_string();
            // The mic is closed; the waveform returns to baseline even if the
            // plugin's final 0.0 level event never arrives.
            self.audio_level = 0.0;
            if !self.is_agent_working {
                self.set_bar_state(BarState::Default).await;
            }
        }

        // Emit state changed event for frontend synchronization
        let event = DictationStateChangeEvent {
            previous_state,
            new_state: self.voice_mode.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis() as u64,
            reason: if is_active {
                "shortcut_triggered".to_string()
            } else {
                "stopped".to_string()
            },
            component: "backend".to_string(),
        };

        if let Err(e) = self
            .app_handle
            .emit(events::dictation_state::CHANGED, &event)
        {
            error!("Failed to emit dictation-state-changed event: {}", e);
        }

        Ok(())
    }

    pub async fn handle_always_listening_change(&mut self, is_active: bool) -> Result<(), String> {
        debug!(
            "UI Manager: Handling always listening change: {}",
            is_active
        );
        self.is_always_listening = is_active;

        // Pay for level monitoring only while the session is open (LAC-4080).
        // The toggle reaches the plugin's always-listening audio thread; the
        // dictation recording thread emits levels for its own session
        // lifetime already. Spawned so the plugin lock is never taken while
        // the UI manager lock is held.
        let app_handle = self.app_handle.clone();
        safe_spawn_async_task(move || async move {
            if let Err(e) =
                crate::commands::always_listening::set_audio_level_monitoring(is_active, app_handle)
                    .await
            {
                warn!("Failed to toggle audio level monitoring: {}", e);
            }
        });

        if is_active {
            self.voice_mode = ui::voice_modes::ALWAYS_LISTENING.to_string();
            self.set_bar_state(BarState::AlwaysListening).await;
        } else {
            self.voice_mode = ui::voice_modes::IDLE.to_string();
            self.audio_level = 0.0;
            if !self.is_agent_working && !self.is_dictation_mode {
                self.set_bar_state(BarState::Default).await;
            }
        }

        Ok(())
    }

    pub async fn handle_agent_started(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling agent started");
        self.is_agent_working = true;
        // Clear the previous run's outcome so the label reads "working", not "Finished"
        self.agent_state = None;
        self.voice_mode = ui::voice_modes::AGENT.to_string();
        self.set_bar_state(BarState::Loading).await;
        Ok(())
    }

    pub async fn handle_agent_stopped(&mut self) -> Result<(), String> {
        debug!("UI Manager: Handling agent stopped");
        self.is_agent_working = false;
        self.voice_mode = ui::voice_modes::IDLE.to_string();
        if matches!(
            self.bar_state,
            BarState::Submitting
                | BarState::Loading
                | BarState::AgentResponding
                | BarState::Listening
                | BarState::Transcribing
        ) {
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
        self.audio_level = 0.0;
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
        self.transcription_provisional = false;
        // Fresh session: never open on the previous session's last level.
        self.audio_level = 0.0;
        self.voice_mode = ui::voice_modes::DICTATION.to_string();
        self.set_bar_state(BarState::Listening).await;
        Ok(())
    }

    pub async fn handle_dictation_partial(
        &mut self,
        partial_text: String,
        provisional: bool,
    ) -> Result<(), String> {
        debug!(
            "UI Manager: Handling dictation partial (provisional={}): '{}'",
            provisional, partial_text
        );
        self.transcription_text = partial_text;
        self.transcription_provisional = provisional;
        self.set_bar_state(BarState::Transcribing).await;
        Ok(())
    }

    pub async fn handle_dictation_finished(&mut self, query: Option<String>) -> Result<(), String> {
        debug!(
            "UI Manager: Handling dictation finished with query: {:?}",
            query
        );

        // Recording has stopped; the state update below carries a flat level.
        self.audio_level = 0.0;

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
        self.transcription_provisional = false;
        Ok(())
    }

    // === FLOATING PANEL FUNCTIONALITY ===

    pub async fn set_panel_click_through(&self, enabled: bool) -> Result<(), String> {
        info!("UI Manager: Setting panel click-through: {}", enabled);

        #[cfg(target_os = "macos")]
        {
            use cocoa::base::{id as cocoa_id, BOOL, NO, YES};
            use dispatch::Queue;
            use objc::{msg_send, sel, sel_impl};

            if let Some(window) = self
                .app_handle
                .get_webview_window(crate::constants::window_labels::FLOATING_PANEL)
            {
                match window.ns_window() {
                    Ok(ns_window_ptr) => {
                        let ns_window = ns_window_ptr as cocoa_id;
                        if !ns_window.is_null() {
                            let ns_window_addr = ns_window as usize;
                            let ignore_events: BOOL = if enabled { YES } else { NO };

                            // NOTE: this catches a Rust panic only. An
                            // Objective-C exception raised by the runtime (bad
                            // selector, deallocated window) is NOT a Rust panic
                            // and unwinds straight past this guard, so the null
                            // check above is what actually protects the call.
                            // Kept because the release profile unwinds again and
                            // a panic here should not kill the app.
                            let result = std::panic::catch_unwind(|| {
                                Queue::main().exec_sync(|| unsafe {
                                    let ns_window = ns_window_addr as cocoa_id;
                                    let _: BOOL =
                                        msg_send![ns_window, setIgnoresMouseEvents: ignore_events];
                                });
                            });

                            match result {
                                Ok(_) => info!("UI Manager: Panel click-through set successfully"),
                                Err(_) => {
                                    return Err("Failed to set panel click-through".to_string())
                                }
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

    /// Set the *floating panel's* window level.
    ///
    /// Not the floating bar: that window's level is owned by
    /// `crate::bar_stacking`, which decides it from the situation rather than
    /// taking a number from a caller. Nothing should grow a second way to set
    /// the bar's level here.
    pub async fn set_panel_level(&self, level: i32) -> Result<(), String> {
        info!("UI Manager: Setting panel window level: {}", level);

        #[cfg(target_os = "macos")]
        {
            use cocoa::base::id as cocoa_id;
            use dispatch::Queue;
            use objc::{msg_send, sel, sel_impl};

            if let Some(window) = self
                .app_handle
                .get_webview_window(crate::constants::window_labels::FLOATING_PANEL)
            {
                match window.ns_window() {
                    Ok(ns_window_ptr) => {
                        let ns_window = ns_window_ptr as cocoa_id;
                        if !ns_window.is_null() {
                            let ns_window_addr = ns_window as usize;
                            let safe_level = match level {
                                0 => 0,
                                1 => 1,
                                3 => 3,
                                5 => 5,
                                8 => 8,
                                24 => 24,
                                _ => 3, // Default to floating level
                            };

                            // NOTE: this catches a Rust panic only. An
                            // Objective-C exception raised by the runtime (bad
                            // selector, deallocated window) is NOT a Rust panic
                            // and unwinds straight past this guard, so the null
                            // check above is what actually protects the call.
                            // Kept because the release profile unwinds again and
                            // a panic here should not kill the app.
                            let result = std::panic::catch_unwind(|| {
                                Queue::main().exec_sync(|| unsafe {
                                    let ns_window = ns_window_addr as cocoa_id;
                                    let _: () = msg_send![ns_window, setLevel: safe_level];
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
        matches!(
            self.bar_state,
            BarState::Submitting
                | BarState::Loading
                | BarState::Finishing
                | BarState::Success
                | BarState::Speaking
                | BarState::Listening
                | BarState::Transcribing
                | BarState::Dictating
                | BarState::AlwaysListening
                | BarState::Error
                | BarState::AgentResponding
                | BarState::DictationReady
        ) || self.is_agent_working
    }

    // === ELEMENT MANAGEMENT ===

    pub async fn create_element(
        &mut self,
        element_id: String,
        config: UIElementConfig,
    ) -> Result<(), String> {
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

        if let Err(e) = self
            .app_handle
            .emit(events::ui::ELEMENT_CREATED, &state_update)
        {
            error!("Failed to emit element created event: {}", e);
        }

        Ok(())
    }

    pub async fn update_element(
        &mut self,
        element_id: String,
        config: UIElementConfig,
    ) -> Result<(), String> {
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

        if let Err(e) = self
            .app_handle
            .emit(events::ui::ELEMENT_UPDATED, &state_update)
        {
            error!("Failed to emit element updated event: {}", e);
        }

        Ok(())
    }

    pub async fn delete_element(&mut self, element_id: String) -> Result<(), String> {
        debug!("UI Manager: Deleting element: {}", element_id);
        self.elements.remove(&element_id);

        if let Err(e) = self
            .app_handle
            .emit(events::ui::ELEMENT_DELETED, &element_id)
        {
            error!("Failed to emit element deleted event: {}", e);
        }

        Ok(())
    }

    pub fn get_element_state(&self, element_id: &str) -> HashMap<String, serde_json::Value> {
        let mut state = HashMap::new();

        // Special handling for floating-bar
        if element_id == ui::element_ids::FLOATING_BAR {
            state.insert(
                "barState".to_string(),
                serde_json::Value::String(self.bar_state.as_str().to_string()),
            );
            state.insert(
                "inputValue".to_string(),
                serde_json::Value::String(self.input_value.clone()),
            );
            state.insert(
                "lastSubmittedValue".to_string(),
                serde_json::Value::String(self.last_submitted_value.clone()),
            );
            state.insert(
                "currentError".to_string(),
                serde_json::to_value(&self.current_error).unwrap_or(serde_json::Value::Null),
            );
            state.insert(
                "transcriptionText".to_string(),
                serde_json::Value::String(self.transcription_text.clone()),
            );
            state.insert(
                "transcriptionProvisional".to_string(),
                serde_json::Value::Bool(self.transcription_provisional),
            );
            state.insert(
                "spokenText".to_string(),
                serde_json::Value::String(self.spoken_text.clone()),
            );
            state.insert(
                "isAgentWorking".to_string(),
                serde_json::Value::Bool(self.is_agent_working),
            );
            state.insert(
                "isDictationMode".to_string(),
                serde_json::Value::Bool(self.is_dictation_mode),
            );
            state.insert(
                "isAlwaysListening".to_string(),
                serde_json::Value::Bool(self.is_always_listening),
            );
            state.insert(
                "audioLevel".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(self.audio_level)
                        .unwrap_or(serde_json::Number::from(0)),
                ),
            );
            state.insert(
                "voiceMode".to_string(),
                serde_json::Value::String(self.voice_mode.clone()),
            );
            state.insert(
                "agentState".to_string(),
                serde_json::to_value(&self.agent_state).unwrap_or(serde_json::Value::Null),
            );
        }

        // Add element-specific state from configuration
        if let Some(element_config) = self.elements.get(element_id) {
            state.insert(
                "visible".to_string(),
                serde_json::Value::Bool(element_config.visible),
            );
            state.insert(
                "element_type".to_string(),
                serde_json::Value::String(element_config.element_type.clone()),
            );

            if let Some(position) = &element_config.position {
                state.insert(
                    "position".to_string(),
                    serde_json::to_value(position).unwrap_or(serde_json::Value::Null),
                );
            }

            if let Some(size) = &element_config.size {
                state.insert(
                    "size".to_string(),
                    serde_json::to_value(size).unwrap_or(serde_json::Value::Null),
                );
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
    UI_MANAGER
        .set(manager_arc.clone())
        .map_err(|_| "Failed to set UI manager")?;

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
    app_handle.listen(
        crate::constants::events::bar::COMPLETE_TRANSITION,
        move |event| {
            let manager = manager_clone.clone();
            safe_spawn_async_task(move || async move {
                let mut manager = manager.lock().await;
                let transition_id = serde_json::from_str::<String>(event.payload()).ok();

                if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                    manager.set_bar_state(BarState::Default).await;
                    manager.current_transition_id = None;
                }
            });
        },
    );

    // Clear error events
    let manager_clone = manager.clone();
    app_handle.listen(crate::constants::events::bar::CLEAR_ERROR, move |event| {
        let manager = manager_clone.clone();
        safe_spawn_async_task(move || async move {
            let mut manager = manager.lock().await;
            let transition_id = serde_json::from_str::<String>(event.payload()).ok();

            if manager.current_transition_id.as_ref() == transition_id.as_ref() {
                manager.current_error = None;
                manager.set_bar_state(BarState::Default).await;
                manager.current_transition_id = None;
            }
        });
    });

    // Agent stream start events
    let manager_clone = manager.clone();
    app_handle.listen(
        crate::constants::events::streaming::STREAM_START,
        move |_event| {
            let manager = manager_clone.clone();
            safe_spawn_async_task(move || async move {
                let mut manager = manager.lock().await;
                if manager.is_agent_working {
                    manager.set_bar_state(BarState::AgentResponding).await;
                }
            });
        },
    );

    // Plugin audio level → bar `audioLevel` (LAC-4080). The backend owns the
    // fold so every bar appearance renders one level from `bar-state-update`
    // instead of each listening to the plugin event itself.
    let manager_clone = manager.clone();
    app_handle.listen(
        crate::constants::events::voice_transcription::AUDIO_LEVEL,
        move |event| {
            let Some(level) = serde_json::from_str::<serde_json::Value>(event.payload())
                .ok()
                .and_then(|payload| payload.get("level").and_then(|l| l.as_f64()))
            else {
                warn!("Ignoring audio-level event with unreadable payload");
                return;
            };
            let manager = manager_clone.clone();
            safe_spawn_async_task(move || async move {
                let mut manager = manager.lock().await;
                manager.handle_audio_level(level).await;
            });
        },
    );

    debug!("UI Manager: Event listeners set up successfully");
}

// === TAURI COMMANDS ===

#[tauri::command]
pub async fn ui_create_element(element_id: String, config: UIElementConfig) -> Result<(), String> {
    debug!(
        "Creating UI element: {} (type: {})",
        element_id, config.element_type
    );

    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        manager.create_element(element_id, config).await
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

#[tauri::command]
pub async fn ui_update_element(element_id: String, config: UIElementConfig) -> Result<(), String> {
    debug!(
        "Updating UI element: {} (type: {})",
        element_id, config.element_type
    );

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
pub async fn ui_get_element_state(
    element_id: String,
) -> Result<HashMap<String, serde_json::Value>, String> {
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
    debug!(
        "Handling interaction for UI element: {} (type: {})",
        element_id, interaction.interaction_type
    );

    if let Some(manager) = get_ui_manager().await {
        let is_bar = is_bar_element(&element_id);

        // Escape is handled before the manager is locked, never inside the
        // match below. The coordinated stop takes this same lock (it moves the
        // bar to `Stopped` and stops dictation through it), and a tokio mutex
        // is not re-entrant: running the stop while holding the guard hung the
        // first Escape forever, left the manager locked so every later bar
        // interaction hung behind it, and left the stop latch set so every
        // later Escape was skipped as "already in progress".
        if is_bar && interaction.interaction_type == ui::interaction_types::ESCAPE {
            debug!("Escape key pressed on bar component: {}", element_id);
            let app_handle = manager.lock().await.app_handle.clone();
            bar_escape_to_idle(&app_handle, "Escape key pressed via UI").await;
            return Ok(());
        }

        let mut manager = manager.lock().await;

        if is_bar {
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
                }
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
                }
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
                }
                ui::interaction_types::BLUR => manager.handle_bar_input_blur().await,
                ui::interaction_types::INITIALIZE => {
                    // Handle initialization specially - just acknowledge receipt
                    debug!("Initialized bar component: {}", element_id);
                    Ok(())
                }
                ui::interaction_types::ENTER => {
                    // Handle enter key - submit current input if any
                    debug!("Enter key pressed on bar component: {}", element_id);
                    if manager.input_value.trim().is_empty() {
                        Ok(())
                    } else {
                        let input_value = manager.input_value.clone();
                        manager.handle_bar_submit(input_value).await
                    }
                }
                _ => {
                    warn!(
                        "Unknown interaction type for bar component: {}",
                        interaction.interaction_type
                    );
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
                }
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
                }
                _ => {
                    warn!(
                        "Unknown interaction type for floating panel: {}",
                        interaction.interaction_type
                    );
                    Ok(())
                }
            }
        } else {
            warn!(
                "Interaction handling not implemented for element: {}",
                element_id
            );
            Ok(())
        }
    } else {
        Err("UI Manager not initialized".to_string())
    }
}

/// Every bar appearance reports interactions under one of these ids.
fn is_bar_element(element_id: &str) -> bool {
    element_id == ui::element_ids::FLOATING_BAR
        || element_id == ui::element_ids::APP_BAR
        || element_id == ui::element_ids::VOICE_AI_BAR
        || element_id == ui::element_ids::DYNAMIC_BAR
}

/// # One Escape, for every appearance
///
/// Stop whatever is running, then put the bar back to the idle bar. Both
/// routes a press arrives on end here: the bar's own keydown (reported as the
/// `escape` interaction) and the passive stop-key monitor. Neither holds the
/// UI manager lock while the stop runs, because the stop takes it.
///
/// The stop is skipped only for a bar that is just a composer (or already at
/// rest) with nothing running, so closing an empty line does not tear down
/// voice triggers and timers for nothing. Any working state runs the stop even
/// if the run has not registered yet (a query accepted a moment ago). Either
/// way the bar ends at rest, and a press on a bar already at rest changes
/// nothing.
pub async fn bar_escape_to_idle(app_handle: &AppHandle, reason: &str) {
    let state = match get_ui_manager().await {
        Some(manager) => manager.lock().await.bar_state.clone(),
        None => BarState::Default,
    };
    if escape_stops_work(&state)
        || crate::commands::escape_key_coordinator::something_to_stop(app_handle).await
    {
        let coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
        if let Err(e) = coordinator.stop_all_operations(app_handle, reason).await {
            error!("Escape: coordinated stop failed: {}", e);
        }
    }
    if let Some(manager) = get_ui_manager().await {
        manager.lock().await.escape_to_idle().await;
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
        manager.bar_config = config.clone();
        manager.save_bar_config().await?;

        // The bar window is not navigated on an appearance change. Every bar
        // route renders `BarHost`, which swaps the component when this event
        // arrives; a navigation only reloaded the page (a blank window, a cold
        // fetch of the orb and avatar chunks, and a second settings load whose
        // failure toast surfaced in the settings window).
        if let Err(e) = manager
            .app_handle
            .emit(events::bar::CONFIG_CHANGED, &config)
        {
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

pub async fn handle_agent_started(_app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_started().await {
            error!("Failed to handle agent started: {}", e);
        }
    }
}

pub async fn handle_agent_stopped(_app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_stopped().await {
            error!("Failed to handle agent stopped: {}", e);
        }
    }
}

pub async fn handle_agent_cancelled(_app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_agent_cancelled().await {
            error!("Failed to handle agent cancelled: {}", e);
        }
    }
}

/// Set the bar to the Stopping state immediately for instant visual feedback.
/// Called from the escape key handler before the stop coordinator begins cleanup.
pub async fn set_stopping_state() {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        manager.agent_state = Some("stopping".to_string());
        manager.current_error = None;
        manager.set_bar_state(BarState::Stopping).await;
    }
}

pub async fn handle_backend_response(
    _app_handle: &AppHandle,
    response_text: Option<String>,
    agent_state: String,
) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager
            .handle_backend_response(response_text, agent_state)
            .await
        {
            error!("Failed to handle backend response: {}", e);
        }
    }
}

pub async fn handle_dictation_mode_change(_app_handle: &AppHandle, is_active: bool) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_mode_change(is_active).await {
            error!("Failed to handle dictation mode change: {}", e);
        }
    }
}

pub async fn handle_always_listening_change(_app_handle: &AppHandle, is_active: bool) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_always_listening_change(is_active).await {
            error!("Failed to handle always listening change: {}", e);
        }
    }
}

pub async fn handle_query_submitted(_app_handle: &AppHandle, query: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_bar_submit(query).await {
            error!("Failed to handle query submitted: {}", e);
        }
    }
}

pub async fn handle_tts_started(_app_handle: &AppHandle, text: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_tts_started(text).await {
            error!("Failed to handle TTS started: {}", e);
        }
    }
}

pub async fn handle_tts_finished(_app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_tts_finished().await {
            error!("Failed to handle TTS finished: {}", e);
        }
    }
}

/// Mirror a query that `submit_query` has accepted in the floating bar.
pub async fn handle_query_accepted(_app_handle: &AppHandle, query: String) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_submit_visual_only(query).await {
            error!("Failed to mirror accepted query in bar: {}", e);
        }
    }
}

/// Dispatch a user query from any UI surface (chat input, example prompt,
/// agent-rendered component) through the unified submission pipeline.
///
/// The bar enters `Submitting` immediately, then the `agent-query-ready`
/// listener runs `submit_query`, which announces the user message to every
/// window and executes the agent. Resolves as soon as the query is accepted,
/// not when the run finishes. Without a UI manager the event is emitted
/// directly so the query is never dropped.
#[tauri::command]
pub async fn dispatch_query(
    query: String,
    // Pictures pasted into the composer, as base64 data URLs.
    images: Option<Vec<String>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // An image on its own is a perfectly good message: "what is this?".
    let has_images = images.as_ref().is_some_and(|i| !i.is_empty());
    if query.trim().is_empty() && !has_images {
        return Ok(());
    }

    // Straight to the agent when there are attachments. The bar-submit path
    // carries only a string, and silently dropping the picture someone just
    // pasted is worse than not offering to paste at all.
    if !has_images {
        if let Some(manager) = get_ui_manager().await {
            let mut manager = manager.lock().await;
            return manager.handle_bar_submit(query).await;
        }
    }
    app_handle
        .emit(
            events::agent::QUERY_READY,
            serde_json::json!({ "query": query, "images": images }),
        )
        .map_err(|e| format!("Failed to dispatch query: {}", e))
}

pub async fn handle_dictation_started(_app_handle: &AppHandle) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_started().await {
            error!("Failed to handle dictation started: {}", e);
        }
    }
}

pub async fn handle_dictation_partial(
    _app_handle: &AppHandle,
    partial_text: String,
    provisional: bool,
) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager
            .handle_dictation_partial(partial_text, provisional)
            .await
        {
            error!("Failed to handle dictation partial: {}", e);
        }
    }
}

pub async fn handle_dictation_finished(_app_handle: &AppHandle, query: Option<String>) {
    if let Some(manager) = get_ui_manager().await {
        let mut manager = manager.lock().await;
        if let Err(e) = manager.handle_dictation_finished(query).await {
            error!("Failed to handle dictation finished: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAC-4080: a level event while listening changes the emitted
    /// `audioLevel`; after the mic closes it is 0.
    #[test]
    fn level_event_moves_audio_level_and_zeroes_after_mic_close() {
        let mut folder = AudioLevelFolder::default();
        let t0 = Instant::now();

        // Mic open: the level passes through and would be emitted.
        assert_eq!(folder.fold(0.6, true, 0.0, t0), Some(0.6));

        // A second event inside the throttle window is dropped (bounded
        // rate, never per sample).
        assert_eq!(
            folder.fold(0.9, true, 0.6, t0 + Duration::from_millis(10)),
            None
        );

        // Past the window it passes again.
        assert_eq!(
            folder.fold(0.4, true, 0.6, t0 + Duration::from_millis(60)),
            Some(0.4)
        );

        // Mic closed: any level folds to silence, emitted exactly once...
        assert_eq!(
            folder.fold(0.8, false, 0.4, t0 + Duration::from_millis(120)),
            Some(0.0)
        );
        // ...and not re-emitted while the bar already sits at baseline.
        assert_eq!(
            folder.fold(0.8, false, 0.0, t0 + Duration::from_millis(180)),
            None
        );
    }

    /// Escape from any non-resting state lands on the idle bar; from a
    /// resting state it changes nothing, so repeated presses are harmless.
    #[test]
    fn escape_returns_every_state_to_idle_and_is_idempotent() {
        use BarState::*;
        let busy = [
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
            Finishing,
            AgentResponding,
            Stopping,
        ];
        for state in busy {
            let first = escape_target(&state);
            assert_eq!(first, Some(Default), "{:?} must return to idle", state);
            // The second press sees the state the first one produced.
            assert_eq!(escape_target(&first.unwrap()), None);
        }
        for resting in [Default, DictationReady, AlwaysListening] {
            assert_eq!(escape_target(&resting), None, "{:?} is at rest", resting);
        }
    }

    /// A composer is closed without a coordinated stop; anything working
    /// is stopped even if the run has not registered yet.
    #[test]
    fn escape_stops_work_only_when_the_bar_is_working() {
        use BarState::*;
        for quiet in [
            Default,
            DictationReady,
            AlwaysListening,
            Expanding,
            Input,
            Shrinking,
        ] {
            assert!(
                !escape_stops_work(&quiet),
                "{:?} has nothing to stop",
                quiet
            );
        }
        for busy in [
            Submitting,
            Loading,
            AgentResponding,
            Listening,
            Dictating,
            Transcribing,
            Speaking,
            Finishing,
            Stopping,
            Error,
            Success,
        ] {
            assert!(escape_stops_work(&busy), "{:?} must be stopped", busy);
        }
    }

    /// The deadlock that made Escape dead in the shipping app: the escape
    /// interaction ran the coordinated stop while holding the UI manager
    /// guard, and the stop takes that same guard. Pin that Escape is routed
    /// before the lock, and that the locked match never runs the stop.
    #[test]
    fn escape_never_runs_the_stop_under_the_manager_lock() {
        let src = include_str!("ui_commands.rs");
        let start = src
            .find("pub async fn ui_handle_interaction(")
            .expect("ui_handle_interaction exists");
        let end = start + src[start..].find("\n}\n").expect("function ends");
        let body = &src[start..end];
        let escape_at = body
            .find("bar_escape_to_idle(")
            .expect("Escape is routed to bar_escape_to_idle");
        let locked_at = body
            .find("let mut manager = manager.lock().await;")
            .expect("the interaction match takes the lock");
        assert!(
            escape_at < locked_at,
            "Escape must be handled before the manager guard is taken"
        );
        assert!(
            !body.contains("stop_all_operations"),
            "the coordinated stop must never run inside ui_handle_interaction"
        );
    }

    #[test]
    fn silence_reset_bypasses_the_throttle() {
        let mut folder = AudioLevelFolder::default();
        let t0 = Instant::now();
        assert_eq!(folder.fold(0.6, true, 0.0, t0), Some(0.6));
        // The plugin's end-of-recording 0.0 lands mid-window and still parks
        // the waveform immediately.
        assert_eq!(
            folder.fold(0.0, true, 0.6, t0 + Duration::from_millis(5)),
            Some(0.0)
        );
    }

    #[test]
    fn out_of_range_levels_are_clamped() {
        let mut folder = AudioLevelFolder::default();
        let t0 = Instant::now();
        assert_eq!(folder.fold(3.7, true, 0.0, t0), Some(1.0));
        assert_eq!(
            folder.fold(-0.5, true, 1.0, t0 + Duration::from_millis(60)),
            Some(0.0)
        );
    }
}
